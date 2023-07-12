use async_sawtooth_sdk::zmq_client::{
    HighestBlockValidatorSelector, ZmqRequestResponseSawtoothChannel,
};
use clap::ArgMatches;
use cli::{load_key_from_match, Wait};
use common::import::{load_bytes_from_url, FromUrlError};
use futures::{channel::oneshot, join, Future, FutureExt, StreamExt};
use k256::{
    ecdsa::SigningKey,
    pkcs8::{EncodePrivateKey, LineEnding},
    SecretKey,
};
use opa_tp_protocol::{
    address::{FAMILY, VERSION},
    async_sawtooth_sdk::{
        error::SawtoothCommunicationError,
        ledger::{LedgerReader, LedgerWriter, TransactionId},
    },
    state::{key_address, policy_address, Keys, OpaOperationEvent},
    submission::SubmissionBuilder,
    transaction::OpaSubmitTransaction,
    OpaLedger,
};
use serde::Deserialize;
use serde_derive::Serialize;
use std::{fs::File, io::Write, net::ToSocketAddrs, path::PathBuf, str::from_utf8, time::Duration};
use thiserror::Error;
use url::Url;

use rand::rngs::StdRng;
use rand_core::SeedableRng;
use tokio::runtime::Handle;
use tracing::{debug, error, info, instrument, span, trace, Instrument, Level};
use user_error::UFE;
mod cli;

#[derive(Error, Debug)]
pub enum OpaCtlError {
    #[error("Operation cancelled {0}")]
    Cancelled(oneshot::Canceled),

    #[error("Communication error: {0}")]
    Communication(#[from] SawtoothCommunicationError),

    #[error("IO error: {0}")]
    IO(#[from] std::io::Error),

    #[error("Json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Pkcs8 error")]
    Pkcs8,

    #[error("Transaction failed: {0}")]
    TransactionFailed(String),

    #[error("Transaction not found after wait: {0}")]
    TransactionNotFound(TransactionId),

    #[error("Error loading from URL: {0}")]
    Url(#[from] FromUrlError),

    #[error("Utf8 error: {0}")]
    Utf8(#[from] std::str::Utf8Error),
}

impl UFE for OpaCtlError {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Waited {
    NoWait,
    WaitedAndFound(OpaOperationEvent),
    WaitedAndOperationFailed(OpaOperationEvent),
    WaitedAndDidNotFind,
}

/// Collect incoming transaction ids before running submission, as there is the
/// potential to miss transactions if we do not collect them 'before' submission
async fn ambient_transactions<
    R: LedgerReader<Event = OpaOperationEvent, Error = SawtoothCommunicationError>
        + Send
        + Sync
        + Clone
        + 'static,
>(
    reader: R,
    goal_tx_id: TransactionId,
    max_steps: u64,
) -> impl Future<Output = Result<Waited, oneshot::Canceled>> {
    let span = span!(Level::DEBUG, "wait_for_opa_transaction");

    // Set up a oneshot channel to notify the returned task
    let (notify_tx, notify_rx) = oneshot::channel::<Waited>();

    // And a oneshot channel to ensure we are receiving events from the chain
    // before we return
    let (receiving_events_tx, receiving_events_rx) = oneshot::channel::<()>();

    Handle::current().spawn(async move {
        // We can immediately return if we are not waiting
        debug!(waiting_for=?goal_tx_id, max_steps=?max_steps);
        let goal_clone = goal_tx_id.clone();

        let mut stream = loop {
            let stream = reader
                .state_updates(
                    "opa/operation",
                    async_sawtooth_sdk::ledger::FromBlock::Head,
                    Some(max_steps),
                )
                .await;

            if let Ok(stream) = stream {
                break stream;
            }
            if let Err(e) = stream {
                error!(subscribe_to_events=?e);
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        };

        receiving_events_tx.send(()).ok();

        loop {
            futures::select! {
              next_block = stream.next().fuse() => {
                if let Some((op,tx, block_id, position,_)) = next_block {
                info!(goal_tx_found=tx==goal_clone,tx=?tx, goal=%goal_clone, op=?op, block_id=%block_id, position=?position);
                if tx == goal_clone {
                    if let OpaOperationEvent::Error(_) = op {
                        notify_tx
                            .send(Waited::WaitedAndOperationFailed(op))
                            .map_err(|e| error!(e=?e))
                            .ok();
                        return;
                    }
                    notify_tx
                        .send(Waited::WaitedAndFound(op))
                        .map_err(|e| error!(e=?e))
                        .ok();
                    return;
                  }
                }
              },
              complete => {
                debug!("Streams completed");
                break;
              }
            }
        }
    }.instrument(span));

    // Wait for the task to start receiving events
    trace!("awaiting incoming event");
    let _ = receiving_events_rx.await;
    trace!("event successfully received from the chain");

    notify_rx
}

#[instrument(skip(reader, writer, matches, submission))]
async fn handle_wait<
    R: LedgerReader<Event = OpaOperationEvent, Error = SawtoothCommunicationError>
        + Clone
        + Send
        + Sync
        + 'static,
    W: LedgerWriter<Transaction = OpaSubmitTransaction, Error = SawtoothCommunicationError>,
>(
    matches: &ArgMatches,
    reader: R,
    writer: W,
    submission: OpaSubmitTransaction,
    transactor_key: &SigningKey,
) -> Result<(Waited, R), OpaCtlError> {
    let wait = Wait::from_matches(matches);
    let (tx_id, tx) = writer.pre_submit(&submission).await?;
    match wait {
        Wait::NoWait => {
            debug!(submitting_tx=%tx_id);
            writer.submit(tx, transactor_key).await?;

            Ok((Waited::NoWait, reader))
        }
        Wait::NumberOfBlocks(blocks) => {
            debug!(submitting_tx=%tx_id, waiting_blocks=%blocks);
            let waiter = ambient_transactions(reader.clone(), tx_id.clone(), blocks).await;
            let writer = writer.submit(tx, transactor_key);

            match join!(writer, waiter) {
                (Err(e), _) => Err(e.into()),
                (_, Ok(Waited::WaitedAndDidNotFind)) => {
                    Err(OpaCtlError::TransactionNotFound(tx_id))
                }
                (_, Ok(Waited::WaitedAndOperationFailed(OpaOperationEvent::Error(e)))) => {
                    Err(OpaCtlError::TransactionFailed(e))
                }
                (_, Ok(x)) => Ok((x, reader)),
                (_, Err(e)) => Err(OpaCtlError::Cancelled(e)),
            }
        }
    }
}

async fn dispatch_args<
    W: LedgerWriter<Transaction = OpaSubmitTransaction, Error = SawtoothCommunicationError>,
    R: LedgerReader<Event = OpaOperationEvent, Error = SawtoothCommunicationError>
        + Send
        + Sync
        + Clone
        + 'static,
>(
    matches: ArgMatches,
    writer: W,
    reader: R,
) -> Result<(Waited, R), OpaCtlError> {
    let span = span!(Level::TRACE, "dispatch_args");
    let _entered = span.enter();
    let span_id = span.id().map(|x| x.into_u64()).unwrap_or(u64::MAX);
    match matches.subcommand() {
        Some(("bootstrap", matches)) => {
            let root_key: SigningKey = load_key_from_match("root-key", matches).into();
            let transactor_key: SigningKey = load_key_from_match("transactor-key", matches).into();
            let bootstrap =
                SubmissionBuilder::bootstrap_root(root_key.verifying_key()).build(span_id);
            Ok(handle_wait(
                matches,
                reader,
                writer,
                OpaSubmitTransaction::bootstrap_root(bootstrap, &transactor_key),
                &transactor_key,
            )
            .await?)
        }
        Some(("generate", matches)) => {
            let key = SecretKey::random(StdRng::from_entropy());
            let key = key
                .to_pkcs8_pem(LineEnding::CRLF)
                .map_err(|_| OpaCtlError::Pkcs8)?;

            if let Some(path) = matches.get_one::<PathBuf>("output") {
                let mut file = File::create(path)?;
                file.write_all(key.as_bytes())?;
            } else {
                print!("{}", *key);
            }

            Ok((Waited::NoWait, reader))
        }
        Some(("rotate-root", matches)) => {
            let current_root_key: SigningKey =
                load_key_from_match("current-root-key", matches).into();
            let new_root_key: SigningKey = load_key_from_match("new-root-key", matches).into();
            let transactor_key: SigningKey = load_key_from_match("transactor-key", matches).into();
            let rotate_key = SubmissionBuilder::rotate_key(
                "root",
                &current_root_key,
                &new_root_key,
                &current_root_key,
            )
            .build(span_id);
            Ok(handle_wait(
                matches,
                reader,
                writer,
                OpaSubmitTransaction::rotate_root(rotate_key, &transactor_key),
                &transactor_key,
            )
            .await?)
        }
        Some(("register-key", matches)) => {
            let current_root_key: SigningKey = load_key_from_match("root-key", matches).into();
            let new_key: SigningKey = load_key_from_match("new-key", matches).into();
            let id = matches.get_one::<String>("id").unwrap();
            let transactor_key: SigningKey = load_key_from_match("transactor-key", matches).into();
            let overwrite_existing = matches.get_flag("overwrite");
            let register_key = SubmissionBuilder::register_key(
                id,
                &new_key.verifying_key(),
                &current_root_key,
                overwrite_existing,
            )
            .build(span_id);
            Ok(handle_wait(
                matches,
                reader,
                writer,
                OpaSubmitTransaction::register_key(
                    id,
                    register_key,
                    &transactor_key,
                    overwrite_existing,
                ),
                &transactor_key,
            )
            .await?)
        }
        Some(("rotate-key", matches)) => {
            let current_root_key: SigningKey = load_key_from_match("root-key", matches).into();
            let current_key: SigningKey = load_key_from_match("current-key", matches).into();
            let id = matches.get_one::<String>("id").unwrap();
            let new_key: SigningKey = load_key_from_match("new-key", matches).into();
            let transactor_key: SigningKey = load_key_from_match("transactor-key", matches).into();
            let rotate_key =
                SubmissionBuilder::rotate_key("root", &current_key, &new_key, &current_root_key)
                    .build(span_id);
            Ok(handle_wait(
                matches,
                reader,
                writer,
                OpaSubmitTransaction::rotate_key(id, rotate_key, &transactor_key),
                &transactor_key,
            )
            .await?)
        }
        Some(("set-policy", matches)) => {
            let root_key: SigningKey = load_key_from_match("root-key", matches).into();
            let transactor_key: SigningKey = load_key_from_match("transactor-key", matches).into();
            let policy: &String = matches.get_one("policy").unwrap();

            let policy = load_bytes_from_url(policy).await?;

            let id = matches.get_one::<String>("id").unwrap();

            let bootstrap = SubmissionBuilder::set_policy(id, policy, root_key).build(span_id);
            Ok(handle_wait(
                matches,
                reader,
                writer,
                OpaSubmitTransaction::set_policy(id, bootstrap, &transactor_key),
                &transactor_key,
            )
            .await?)
        }
        Some(("get-key", matches)) => {
            let key: Vec<u8> = reader
                .get_state_entry(&key_address(matches.get_one::<String>("id").unwrap()))
                .await?;

            debug!(loaded_key = ?from_utf8(&key));

            let key: Keys = serde_json::from_slice(&key)?;

            let key = key.current.key;

            if let Some(path) = matches.get_one::<String>("output") {
                let mut file = File::create(path)?;
                file.write_all(key.as_bytes())?;
            } else {
                print!("{key}");
            }

            Ok((Waited::NoWait, reader))
        }
        Some(("get-policy", matches)) => {
            let policy: Result<Vec<u8>, _> = reader
                .get_state_entry(&policy_address(matches.get_one::<String>("id").unwrap()))
                .await;

            if let Err(SawtoothCommunicationError::ResourceNotFound) = policy {
                print!("No policy found");
                return Ok((Waited::NoWait, reader));
            }

            let policy = policy?;

            if let Some(path) = matches.get_one::<String>("output") {
                let mut file = File::create(path)?;
                file.write_all(&policy)?;
            }

            Ok((Waited::NoWait, reader))
        }
        _ => Ok((Waited::NoWait, reader)),
    }
}

#[tokio::main]
async fn main() {
    chronicle_telemetry::telemetry(None, chronicle_telemetry::ConsoleLogging::Pretty);
    let args = cli::cli().get_matches();
    let address: &Url = args.get_one("sawtooth-address").unwrap();
    let client = ZmqRequestResponseSawtoothChannel::new(
        &format!("opactl-{}", uuid::Uuid::new_v4()),
        &[format!(
            "{}:{}",
            address.host().expect("host").to_owned(),
            address.port().unwrap_or(4004)
        )
        .to_socket_addrs()
        .unwrap()
        .next()
        .unwrap()],
        HighestBlockValidatorSelector,
    )
    .unwrap()
    .retrying();

    let reader = OpaLedger::new(client, FAMILY, VERSION);
    let writer = reader.clone();

    dispatch_args(args, writer, reader)
        .await
        .map_err(|opactl| {
            error!(?opactl);
            opactl.into_ufe().print();
            std::process::exit(1);
        })
        .map(|(waited, _reader)| {
            if let Waited::WaitedAndFound(op) = waited {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::to_value(op).unwrap()).unwrap()
                );
            }
        })
        .ok();
}

// Use as much of the opa-tp as possible, by using a simulated `RequestResponseSawtoothChannel`
#[cfg(test)]
pub mod test {
    use async_sawtooth_sdk::{
        error::SawtoothCommunicationError,
        ledger::SawtoothLedger,
        messages::{
            message::MessageType, BlockHeader, ClientBatchSubmitResponse,
            ClientBlockGetByNumRequest, ClientBlockGetResponse, ClientBlockListResponse,
            ClientEventsSubscribeResponse, ClientStateGetRequest, ClientStateGetResponse,
        },
        zmq_client::{HighestBlockValidatorSelector, ZmqRequestResponseSawtoothChannel},
    };
    use clap::ArgMatches;
    use futures::{select, FutureExt, SinkExt, StreamExt};
    use opa_tp_protocol::{
        address::{FAMILY, VERSION},
        messages::OpaEvent,
        state::OpaOperationEvent,
        transaction::OpaSubmitTransaction,
    };

    use k256::{
        pkcs8::{EncodePrivateKey, LineEnding},
        SecretKey,
    };
    use opa_tp::{abstract_tp::TP, tp::OpaTransactionHandler};

    use prost::Message;
    use rand::rngs::StdRng;
    use rand_core::SeedableRng;
    use sawtooth_sdk::{
        messages::client_batch_submit::ClientBatchSubmitRequest,
        processor::handler::{ContextError, TransactionContext},
    };
    use serde_json::{self, Value};
    use tmq::{router, Context, Multipart};
    use tokio::runtime;
    use uuid::Uuid;

    use std::{
        cell::RefCell,
        collections::BTreeMap,
        io::Write,
        net::{Ipv4Addr, SocketAddr},
        sync::{Arc, Mutex},
        thread,
    };
    use tempfile::NamedTempFile;
    use tokio_stream::wrappers::UnboundedReceiverStream;
    use tracing::{debug, error, info, instrument};

    use crate::{cli, dispatch_args};

    type TestTxEvents = Vec<(String, Vec<(String, String)>, Vec<u8>)>;

    pub trait SimulatedSawtoothBehavior {
        fn handle_request(
            &self,
            message_type: MessageType,
            request: Vec<u8>,
        ) -> Result<(MessageType, Vec<u8>), SawtoothCommunicationError>;
    }

    pub type OpaLedger =
        SawtoothLedger<ZmqRequestResponseSawtoothChannel, OpaOperationEvent, OpaSubmitTransaction>;

    type PrintableEvent = Vec<(String, Vec<(String, String)>, Value)>;

    #[derive(Clone)]
    pub struct TestTransactionContext {
        pub state: RefCell<BTreeMap<String, Vec<u8>>>,
        pub events: RefCell<TestTxEvents>,
        tx: tokio::sync::mpsc::UnboundedSender<Option<(MessageType, Vec<u8>)>>,
    }

    impl TestTransactionContext {
        pub fn new(tx: tokio::sync::mpsc::UnboundedSender<Option<(MessageType, Vec<u8>)>>) -> Self {
            Self {
                state: RefCell::new(BTreeMap::new()),
                events: RefCell::new(vec![]),
                tx,
            }
        }

        pub fn new_with_state(
            tx: tokio::sync::mpsc::UnboundedSender<Option<(MessageType, Vec<u8>)>>,
            state: BTreeMap<String, Vec<u8>>,
        ) -> Self {
            Self {
                state: state.into(),
                events: RefCell::new(vec![]),
                tx,
            }
        }

        pub fn readable_state(&self) -> Vec<(String, Value)> {
            // Deal with the fact that policies are raw bytes, but meta data and
            // keys are json
            self.state
                .borrow()
                .iter()
                .map(|(k, v)| {
                    let as_string = String::from_utf8(v.clone()).unwrap();
                    if serde_json::from_str::<Value>(&as_string).is_ok() {
                        (k.clone(), serde_json::from_str(&as_string).unwrap())
                    } else {
                        (k.clone(), serde_json::to_value(v.clone()).unwrap())
                    }
                })
                .collect()
        }

        pub fn readable_events(&self) -> PrintableEvent {
            self.events
                .borrow()
                .iter()
                .map(|(k, attr, data)| {
                    (
                        k.clone(),
                        attr.clone(),
                        match &<OpaEvent as prost::Message>::decode(&**data)
                            .unwrap()
                            .payload
                            .unwrap()
                        {
                            opa_tp_protocol::messages::opa_event::Payload::Operation(operation) => {
                                serde_json::from_str(operation).unwrap()
                            }
                            opa_tp_protocol::messages::opa_event::Payload::Error(error) => {
                                serde_json::from_str(error).unwrap()
                            }
                        },
                    )
                })
                .collect()
        }
    }

    impl sawtooth_sdk::processor::handler::TransactionContext for TestTransactionContext {
        fn add_receipt_data(
            self: &TestTransactionContext,
            _data: &[u8],
        ) -> Result<(), ContextError> {
            unimplemented!()
        }

        #[instrument(skip(self))]
        fn add_event(
            self: &TestTransactionContext,
            event_type: String,
            attributes: Vec<(String, String)>,
            data: &[u8],
        ) -> Result<(), ContextError> {
            let stl_event = async_sawtooth_sdk::messages::Event {
                event_type: event_type.clone(),
                attributes: attributes
                    .iter()
                    .map(|(k, v)| async_sawtooth_sdk::messages::event::Attribute {
                        key: k.clone(),
                        value: v.clone(),
                    })
                    .collect(),
                data: data.to_vec(),
            };
            let list = async_sawtooth_sdk::messages::EventList {
                events: vec![stl_event],
            };
            let stl_event: Vec<u8> = list.encode_to_vec();

            self.tx
                .send(Some((MessageType::ClientEvents, stl_event)))
                .unwrap();

            self.events
                .borrow_mut()
                .push((event_type, attributes, data.to_vec()));

            Ok(())
        }

        fn delete_state_entries(
            self: &TestTransactionContext,
            _addresses: &[std::string::String],
        ) -> Result<Vec<String>, ContextError> {
            unimplemented!()
        }

        fn get_state_entries(
            &self,
            addresses: &[String],
        ) -> Result<Vec<(String, Vec<u8>)>, ContextError> {
            Ok(self
                .state
                .borrow()
                .iter()
                .filter(|(k, _)| addresses.contains(k))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect())
        }

        fn set_state_entries(
            self: &TestTransactionContext,
            entries: Vec<(String, Vec<u8>)>,
        ) -> std::result::Result<(), sawtooth_sdk::processor::handler::ContextError> {
            for entry in entries {
                self.state.borrow_mut().insert(entry.0, entry.1);
            }

            Ok(())
        }
    }

    fn apply_transactions(
        handler: &OpaTransactionHandler,
        context: &mut TestTransactionContext,
        transactions: &[sawtooth_sdk::messages::transaction::Transaction],
    ) {
        for tx in transactions {
            let req = sawtooth_sdk::messages::processor::TpProcessRequest {
                payload: tx.get_payload().to_vec(),
                header: Some(protobuf::Message::parse_from_bytes(tx.get_header()).unwrap()).into(),
                signature: tx.get_header_signature().to_string(),
                ..Default::default()
            };
            handler.apply(&req, context).unwrap();
        }
    }

    fn get_sorted_transactions(
        batch: &mut sawtooth_sdk::messages::batch::Batch,
    ) -> Vec<sawtooth_sdk::messages::transaction::Transaction> {
        let mut transactions = batch.transactions.clone();
        transactions.sort_by_key(|tx| tx.header_signature.clone());
        transactions.to_vec()
    }

    fn process_transactions(
        transactions: &[sawtooth_sdk::messages::transaction::Transaction],
        context: &mut TestTransactionContext,
        handler: &OpaTransactionHandler,
    ) -> (Vec<(String, Value)>, PrintableEvent) {
        apply_transactions(handler, context, transactions);
        (context.readable_state(), context.readable_events())
    }

    fn test_determinism(
        transactions: &[sawtooth_sdk::messages::transaction::Transaction],
        context: &TestTransactionContext,
        number_of_determinism_checking_cycles: usize,
    ) -> Vec<(Vec<(String, Value)>, PrintableEvent)> {
        let handler = OpaTransactionHandler::new();

        let contexts = (0..number_of_determinism_checking_cycles)
            .map(|_| {
                let mut context = context.clone();
                process_transactions(transactions, &mut context, &handler)
            })
            .collect::<Vec<_>>();

        // Check if the contexts are the same after running apply
        assert!(
            contexts.iter().all(|context| contexts[0] == *context),
            "All contexts must be the same after running apply. Contexts: {:?}",
            contexts,
        );

        contexts
    }

    fn assert_output_determinism(
        expected_contexts: &[(Vec<(String, Value)>, PrintableEvent)],
        readable_state_and_events: &(Vec<(String, Value)>, PrintableEvent),
    ) {
        // Check if the updated context is the same as the determinism check results
        assert!(
            expected_contexts
                .iter()
                .all(|context| readable_state_and_events == context),
            "Updated context must be the same as previously run tests"
        );
    }

    #[derive(Clone)]
    struct WellBehavedBehavior {
        handler: Arc<OpaTransactionHandler>,
        context: Arc<Mutex<TestTransactionContext>>,
    }

    impl WellBehavedBehavior {
        /// Submits a batch of transactions to the validator and performs determinism checks.
        fn submit_batch(&self, request: &[u8]) -> Result<Vec<u8>, SawtoothCommunicationError> {
            // Parse the request into a `ClientBatchSubmitRequest` object and extract the first batch.
            let req: ClientBatchSubmitRequest =
                protobuf::Message::parse_from_bytes(request).unwrap();
            let mut batch = req.batches.into_iter().next().unwrap();

            // Log some debug information about the batch and sort its transactions.
            debug!(received_batch = ?batch, transactions = ?batch.transactions);
            let transactions = get_sorted_transactions(&mut batch);

            // Get the current state and events before applying the transactions.
            let preprocessing_state_and_events = {
                let context = self.context.lock().unwrap();
                (context.readable_state(), context.readable_events())
            };

            // Perform determinism checking and get the expected contexts
            let number_of_determinism_checking_cycles = 5;
            let context = { TestTransactionContext::clone(&self.context.lock().unwrap()) };
            let expected_contexts = test_determinism(
                transactions.as_slice(),
                &context,
                number_of_determinism_checking_cycles,
            );

            // Update the context and perform an output determinism check.
            let mut context = self.context.lock().unwrap();
            apply_transactions(&self.handler, &mut context, transactions.as_slice());
            let updated_readable_state_and_events =
                (context.readable_state(), context.readable_events());
            assert_ne!(
                preprocessing_state_and_events, updated_readable_state_and_events,
                "Context must be updated after running apply"
            );
            assert_output_determinism(&expected_contexts, &updated_readable_state_and_events);

            // Create a response with an "OK" status and write it to a byte vector.
            let mut response = ClientBatchSubmitResponse::default();
            response
                .set_status(async_sawtooth_sdk::messages::client_batch_submit_response::Status::Ok);
            Ok(response.encode_to_vec())
        }
    }

    impl SimulatedSawtoothBehavior for WellBehavedBehavior {
        #[instrument(skip(self, request))]
        fn handle_request(
            &self,
            message_type: MessageType,
            request: Vec<u8>,
        ) -> Result<(MessageType, Vec<u8>), SawtoothCommunicationError> {
            match message_type {
                // Batch submit request, decode and apply the transactions
                // in the batch
                MessageType::ClientBatchSubmitRequest => {
                    let buf = self.submit_batch(&request)?;
                    Ok((MessageType::ClientBatchSubmitResponse, buf))
                }
                // Always respond with a block height of one
                MessageType::ClientBlockListRequest => {
                    let mut response = ClientBlockListResponse::default();
                    let block_header = BlockHeader {
                        block_num: 1,
                        ..Default::default()
                    };
                    let block_header_bytes = block_header.encode_to_vec();
                    response.blocks = vec![async_sawtooth_sdk::messages::Block {
                        header: block_header_bytes,
                        ..Default::default()
                    }];
                    response.set_status(
                        async_sawtooth_sdk::messages::client_block_list_response::Status::Ok,
                    );
                    Ok((
                        MessageType::ClientBlockListResponse,
                        response.encode_to_vec(),
                    ))
                }
                // We can just return Ok here, no need to fake routing
                MessageType::ClientEventsSubscribeRequest => {
                    let mut response = ClientEventsSubscribeResponse::default();
                    response.set_status(
                        async_sawtooth_sdk::messages::client_events_subscribe_response::Status::Ok,
                    );
                    Ok((
                        MessageType::ClientEventsSubscribeResponse,
                        response.encode_to_vec(),
                    ))
                }
                MessageType::ClientStateGetRequest => {
                    let request = ClientStateGetRequest::decode(&*request).unwrap();
                    let address = request.address;

                    let state = self
                        .context
                        .lock()
                        .unwrap()
                        .get_state_entries(&[address])
                        .unwrap();

                    let mut response = ClientStateGetResponse {
                        status: async_sawtooth_sdk::messages::client_state_get_response::Status::Ok
                            as i32,
                        ..Default::default()
                    };

                    if state.is_empty() {
                        response.set_status(async_sawtooth_sdk::messages::client_state_get_response::Status::NoResource);
                    } else {
                        response.value = state[0].1.clone();
                    }

                    let buf = response.encode_to_vec();
                    Ok((MessageType::ClientStateGetResponse, buf))
                }
                MessageType::ClientBlockGetByNumRequest => {
                    let req = ClientBlockGetByNumRequest::decode(&*request).unwrap();
                    let mut response = ClientBlockGetResponse::default();
                    let block_header = BlockHeader {
                        block_num: req.block_num,
                        previous_block_id: hex::encode([0; 32]),
                        ..Default::default()
                    };
                    let block_header_bytes = block_header.encode_to_vec();
                    response.block = Some(async_sawtooth_sdk::messages::Block {
                        header: block_header_bytes,
                        ..Default::default()
                    });

                    response.set_status(
                        async_sawtooth_sdk::messages::client_block_get_response::Status::Ok,
                    );
                    let buf = response.encode_to_vec();
                    Ok((MessageType::ClientBlockListResponse, buf))
                }
                _ => panic!("Unexpected message type {} received", message_type as i32),
            }
        }
    }

    struct EmbeddedOpaTp {
        pub ledger: OpaLedger,
        context: Arc<Mutex<TestTransactionContext>>,
    }

    impl EmbeddedOpaTp {
        pub fn new_with_state(
            state: BTreeMap<String, Vec<u8>>,
        ) -> Result<Self, SawtoothCommunicationError> {
            let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

            let context = Arc::new(Mutex::new(TestTransactionContext::new_with_state(
                tx, state,
            )));

            let handler = Arc::new(OpaTransactionHandler::new());

            let behavior = WellBehavedBehavior {
                handler,
                context: context.clone(),
            };

            let listen_port = portpicker::pick_unused_port().expect("No ports free");
            let listen_addr = SocketAddr::new(Ipv4Addr::new(127, 0, 0, 1).into(), listen_port);
            let connect_addr = SocketAddr::new(Ipv4Addr::new(127, 0, 0, 1).into(), listen_port);

            let behavior_clone = behavior;
            thread::spawn(move || {
                let rt = runtime::Builder::new_current_thread()
                    .enable_io()
                    .enable_time()
                    .build()
                    .unwrap();
                let mut rx = UnboundedReceiverStream::new(rx);
                let local = tokio::task::LocalSet::new();

                let task = local.run_until(async move {
                tokio::task::spawn_local(async move {
                    let (mut router_tx, mut router_rx) = router(&Context::new())
                        .bind(&format!("tcp://{}", listen_addr))
                        .unwrap()
                        .split();

                    debug!(listen_addr = ?listen_addr, "Embedded TP listening");
                    let mut last_address = vec![];
                    loop {
                        select! {
                            message = router_rx.next().fuse() => {
                              if message.is_none() {
                                break;
                              }

                              let multipart = message.unwrap().unwrap();

                              debug!(request = ?multipart);
                              last_address =  multipart[0].to_vec();
                              let request: async_sawtooth_sdk::messages::Message =
                                  async_sawtooth_sdk::prost::Message::decode(&*multipart[1].to_vec()).map_err(|e| error!(%e)).unwrap();

                              let response = behavior_clone
                                  .handle_request(
                                      MessageType::from_i32(request.message_type).unwrap(),
                                      request.content,
                                  )
                                  .unwrap();


                              let message_wrapper = async_sawtooth_sdk::messages::Message {
                                message_type: response.0 as i32,
                                tx_id: request.tx_id,
                                content: response.1
                              };

                              let mut multipart = Multipart::default();
                              multipart.push_back(last_address.clone().into());
                              multipart.push_back(tmq::Message::from(prost::Message::encode_to_vec(&message_wrapper)));

                              debug!(response = ?multipart);
                              router_tx.send(multipart).await.ok();
                            },
                            unsolicited_message = rx.next().fuse() => {
                              if unsolicited_message.is_none() {
                                break;
                              }
                              tracing::trace!(unsolicited_message=?unsolicited_message);

                              let unsolicited_message = unsolicited_message.unwrap().unwrap();
                              debug!(unsolicited_message = ?unsolicited_message);
                              let message_wrapper = async_sawtooth_sdk::messages::Message {
                                message_type: unsolicited_message.0 as i32,
                                tx_id: "".to_string(),
                                content: unsolicited_message.1
                              };
                              let mut multipart = Multipart::default();
                              multipart.push_back(last_address.clone().into());

                              multipart.push_back(tmq::Message::from(prost::Message::encode_to_vec(&message_wrapper)));
                              router_tx.send(multipart).await.ok();
                            },
                            complete => {
                              info!("close embedded router");
                            }
                        }
                    }
                })
                .await
            });
                rt.block_on(task).ok();
            });

            let channel = ZmqRequestResponseSawtoothChannel::new(
                &format!("test_{}", Uuid::new_v4()),
                &[connect_addr],
                HighestBlockValidatorSelector,
            )?;

            Ok(Self {
                ledger: OpaLedger::new(channel, FAMILY, VERSION),
                context,
            })
        }

        pub fn readable_state(&self) -> Vec<(String, Value)> {
            self.context.lock().unwrap().readable_state()
        }

        pub fn new() -> Self {
            EmbeddedOpaTp::new_with_state(BTreeMap::new()).unwrap()
        }
    }

    fn embed_opa_tp() -> EmbeddedOpaTp {
        chronicle_telemetry::telemetry(None, chronicle_telemetry::ConsoleLogging::Pretty);
        EmbeddedOpaTp::new()
    }

    fn reuse_opa_tp_state(tp: EmbeddedOpaTp) -> EmbeddedOpaTp {
        chronicle_telemetry::telemetry(None, chronicle_telemetry::ConsoleLogging::Pretty);
        EmbeddedOpaTp::new_with_state(tp.context.lock().unwrap().state.borrow().clone()).unwrap()
    }

    fn get_opactl_cmd(command_line: &str) -> ArgMatches {
        let cli = cli::cli();
        cli.get_matches_from(command_line.split_whitespace())
    }

    fn key_from_seed(seed: u8) -> String {
        let secret: SecretKey = SecretKey::random(StdRng::from_seed([seed; 32]));
        secret.to_pkcs8_pem(LineEnding::CRLF).unwrap().to_string()
    }

    async fn bootstrap_root_state() -> (String, EmbeddedOpaTp) {
        let root_key = key_from_seed(0);

        let mut keyfile = NamedTempFile::new().unwrap();
        keyfile.write_all(root_key.as_bytes()).unwrap();

        let matches = get_opactl_cmd(
            format!("opactl bootstrap --root-key {}", keyfile.path().display()).as_str(),
        );

        let opa_tp = embed_opa_tp();

        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
        dispatch_args(matches, opa_tp.ledger.clone(), opa_tp.ledger.clone())
            .await
            .unwrap();

        (root_key, reuse_opa_tp_state(opa_tp))
    }

    #[tokio::test]
    async fn bootstrap_root_and_get_key() {
        let (_root_key, opa_tp) = bootstrap_root_state().await;
        //Generate a key pem and set env vars
        insta::assert_yaml_snapshot!(opa_tp.readable_state(), {
            ".**.date" => "[date]",
            ".**.key" => "[pem]",
        } ,@r###"
        ---
        - - 7ed19313e8ece6c4f5551b9bd1090797ad25c6d85f7b523b2214d4fe448372279aa95c
          - current:
              key: "[pem]"
              version: 0
            expired: ~
            id: root
        "###);

        let opa_tp = reuse_opa_tp_state(opa_tp);

        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

        let out_keyfile = NamedTempFile::new().unwrap();

        let matches = get_opactl_cmd(
            format!("opactl get-key --output {}", out_keyfile.path().display(),).as_str(),
        );

        insta::assert_yaml_snapshot!(
        dispatch_args(matches, opa_tp.ledger.clone(), opa_tp.ledger.clone())
            .await
            .unwrap().0, @r###"
        ---
        NoWait
        "###);
    }

    #[tokio::test]
    async fn rotate_root() {
        let (root_key, opa_tp) = bootstrap_root_state().await;

        let mut old_keyfile = NamedTempFile::new().unwrap();
        old_keyfile.write_all(root_key.as_bytes()).unwrap();

        let new_root_key = key_from_seed(1);

        let mut new_keyfile = NamedTempFile::new().unwrap();
        new_keyfile.write_all(new_root_key.as_bytes()).unwrap();

        let matches = get_opactl_cmd(
            format!(
                "opactl rotate-root --current-root-key {} --new-root-key {}",
                old_keyfile.path().display(),
                new_keyfile.path().display()
            )
            .as_str(),
        );

        insta::assert_yaml_snapshot!(
        dispatch_args(matches, opa_tp.ledger.clone(), opa_tp.ledger.clone())
            .await
            .unwrap().0, {
            ".**.date" => "[date]",
            ".**.key" => "[pem]",
        } ,@r###"
        ---
        WaitedAndFound:
          KeyUpdate:
            id: root
            current:
              key: "[pem]"
              version: 1
            expired:
              key: "[pem]"
              version: 0
        "###);

        insta::assert_yaml_snapshot!(opa_tp.readable_state(),{
            ".**.date" => "[date]",
            ".**.key" => "[pem]",
        }, @r###"
        ---
        - - 7ed19313e8ece6c4f5551b9bd1090797ad25c6d85f7b523b2214d4fe448372279aa95c
          - current:
              key: "[pem]"
              version: 1
            expired:
              key: "[pem]"
              version: 0
            id: root
        "###);
    }

    #[tokio::test]
    async fn register_and_rotate_key() {
        let (root_key, opa_tp) = bootstrap_root_state().await;

        let mut root_keyfile = NamedTempFile::new().unwrap();
        root_keyfile.write_all(root_key.as_bytes()).unwrap();

        let new_user_key = key_from_seed(0);
        let mut new_keyfile = NamedTempFile::new().unwrap();
        new_keyfile.write_all(new_user_key.as_bytes()).unwrap();

        let matches = get_opactl_cmd(
            format!(
                "opactl register-key --root-key {} --new-key {} --id test",
                root_keyfile.path().display(),
                new_keyfile.path().display()
            )
            .as_str(),
        );

        insta::assert_yaml_snapshot!(
        dispatch_args(matches, opa_tp.ledger.clone(), opa_tp.ledger.clone())
            .await
            .unwrap().0, {
            ".**.date" => "[date]",
            ".**.key" => "[pem]",
        },@r###"
        ---
        WaitedAndFound:
          KeyUpdate:
            id: test
            current:
              key: "[pem]"
              version: 0
            expired: ~
        "###);

        insta::assert_yaml_snapshot!(opa_tp.readable_state(), {
            ".**.date" => "[date]",
            ".**.key" => "[pem]",
        }, @r###"
        ---
        - - 7ed19313e8ece6c4f5551b9bd1090797ad25c6d85f7b523b2214d4fe448372279aa95c
          - current:
              key: "[pem]"
              version: 0
            expired: ~
            id: root
        - - 7ed19336d8b5677c39a7b872910f948944dd84ba014846c81fcd53fe1fd5289b9dfd1c
          - current:
              key: "[pem]"
              version: 0
            expired: ~
            id: test
        "###);

        let rotate_user_key = key_from_seed(0);
        let mut rotate_keyfile = NamedTempFile::new().unwrap();
        rotate_keyfile
            .write_all(rotate_user_key.as_bytes())
            .unwrap();

        let matches = get_opactl_cmd(
            format!(
                "opactl rotate-key --root-key {} --current-key {} --new-key {} --id test",
                root_keyfile.path().display(),
                new_keyfile.path().display(),
                rotate_keyfile.path().display(),
            )
            .as_str(),
        );

        let opa_tp = reuse_opa_tp_state(opa_tp);

        insta::assert_yaml_snapshot!(
        dispatch_args(matches, opa_tp.ledger.clone(), opa_tp.ledger.clone())
            .await
            .unwrap().0, {
            ".**.date" => "[date]",
            ".**.key" => "[pem]",
        } ,@r###"
        ---
        WaitedAndFound:
          KeyUpdate:
            id: root
            current:
              key: "[pem]"
              version: 1
            expired:
              key: "[pem]"
              version: 0
        "###);

        insta::assert_yaml_snapshot!(opa_tp.readable_state(), {
            ".**.date" => "[date]",
            ".**.key" => "[pem]",
        } ,@r###"
        ---
        - - 7ed19313e8ece6c4f5551b9bd1090797ad25c6d85f7b523b2214d4fe448372279aa95c
          - current:
              key: "[pem]"
              version: 1
            expired:
              key: "[pem]"
              version: 0
            id: root
        - - 7ed19336d8b5677c39a7b872910f948944dd84ba014846c81fcd53fe1fd5289b9dfd1c
          - current:
              key: "[pem]"
              version: 0
            expired: ~
            id: test
        "###);
    }

    #[tokio::test]
    async fn set_and_update_policy() {
        let (root_key, opa_tp) = bootstrap_root_state().await;

        let mut root_keyfile = NamedTempFile::new().unwrap();
        root_keyfile.write_all(root_key.as_bytes()).unwrap();

        let mut policy = NamedTempFile::new().unwrap();
        policy.write_all(&[0]).unwrap();

        let matches = get_opactl_cmd(
            format!(
                "opactl set-policy --root-key {} --id test  --policy {}",
                root_keyfile.path().display(),
                policy.path().display()
            )
            .as_str(),
        );

        insta::assert_yaml_snapshot!(dispatch_args(
            matches,
            opa_tp.ledger.clone(),
            opa_tp.ledger.clone()
        )
        .await
        .unwrap().0, {
          ".**.date" => "[date]"
        }, @r###"
        ---
        WaitedAndFound:
          PolicyUpdate:
            id: test
            hash: 6e340b9cffb37a989ca544e6bb780a2c78901d3fb33738768511a30617afa01d
            policy_address: 7ed1931c262a4be700b69974438a35ae56a07ce96778b276c8a061dc254d9862c7ecff
        "###);

        insta::assert_yaml_snapshot!(opa_tp.readable_state(), {
          ".**.date" => "[date]",
          ".**.key" => "[pem]"
        } ,@r###"
        ---
        - - 7ed19313e8ece6c4f5551b9bd1090797ad25c6d85f7b523b2214d4fe448372279aa95c
          - current:
              key: "[pem]"
              version: 0
            expired: ~
            id: root
        - - 7ed1931c262a4be700b69974438a35ae56a07ce96778b276c8a061dc254d9862c7ecff
          - - 0
        - - 7ed1932b35db049f40833c5c2eaa47e070ce2648c478469a4cdf44ff7a37dd5468208e
          - hash: 6e340b9cffb37a989ca544e6bb780a2c78901d3fb33738768511a30617afa01d
            id: test
            policy_address: 7ed1931c262a4be700b69974438a35ae56a07ce96778b276c8a061dc254d9862c7ecff
        "###);

        policy.write_all(&[1]).unwrap();

        let matches = get_opactl_cmd(
            format!(
                "opactl set-policy --root-key {} --id test  --policy {}",
                root_keyfile.path().display(),
                policy.path().display()
            )
            .as_str(),
        );

        let opa_tp = reuse_opa_tp_state(opa_tp);

        insta::assert_yaml_snapshot!(dispatch_args(matches, opa_tp.ledger.clone(), opa_tp.ledger.clone())
            .await
            .unwrap().0, {
              ".**.date" => "[date]"
            }, @r###"
        ---
        WaitedAndFound:
          PolicyUpdate:
            id: test
            hash: b413f47d13ee2fe6c845b2ee141af81de858df4ec549a58b7970bb96645bc8d2
            policy_address: 7ed1931c262a4be700b69974438a35ae56a07ce96778b276c8a061dc254d9862c7ecff
        "### );

        insta::assert_yaml_snapshot!(opa_tp.readable_state(), {
          ".**.date" => "[date]",
          ".**.key" => "[pem]"
        } ,@r###"
        ---
        - - 7ed19313e8ece6c4f5551b9bd1090797ad25c6d85f7b523b2214d4fe448372279aa95c
          - current:
              key: "[pem]"
              version: 0
            expired: ~
            id: root
        - - 7ed1931c262a4be700b69974438a35ae56a07ce96778b276c8a061dc254d9862c7ecff
          - - 0
            - 1
        - - 7ed1932b35db049f40833c5c2eaa47e070ce2648c478469a4cdf44ff7a37dd5468208e
          - hash: b413f47d13ee2fe6c845b2ee141af81de858df4ec549a58b7970bb96645bc8d2
            id: test
            policy_address: 7ed1931c262a4be700b69974438a35ae56a07ce96778b276c8a061dc254d9862c7ecff
        "###);
    }
}
