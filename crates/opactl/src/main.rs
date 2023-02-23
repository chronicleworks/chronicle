use std::{
    fs::File,
    io::{Read, Write},
    path::PathBuf,
    str::from_utf8,
};

use clap::ArgMatches;
use cli::{load_key_from_match, Wait};
use futures::{channel::oneshot, Future, StreamExt};
use k256::{
    ecdsa::SigningKey,
    pkcs8::{EncodePrivateKey, LineEnding},
    SecretKey,
};
use opa_tp_protocol::{
    ledger::{LedgerReader, LedgerWriter, OpaEvent, OpaLedger, OpaSubmitTransaction},
    state::{key_address, policy_address, Keys, OpaOperationEvent},
    submission::{OpaTransactionId, SubmissionBuilder},
    zmq_client::{SawtoothCommunicationError, ZmqRequestResponseSawtoothChannel},
};
use serde::Deserialize;
use serde_derive::Serialize;
use thiserror::Error;

use rand::rngs::StdRng;
use rand_core::SeedableRng;
use tokio::runtime::Handle;
use tracing::{debug, error, instrument, span, Level};
use url::Url;
use user_error::UFE;
mod cli;

#[derive(Error, Debug)]
pub enum OpaCtlError {
    #[error("Communication error: {0}")]
    Communication(#[from] SawtoothCommunicationError),
    #[error("IO error: {0}")]
    IO(#[from] std::io::Error),
    #[error("Pkcs8 error: {0}")]
    Pkcs8(#[from] k256::pkcs8::Error),
    #[error("Utf8 error: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("Json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Transaction not found after wait {0}")]
    TransactionNotFound(OpaTransactionId),
    #[error("Transaction failed {0}")]
    TransactionFailed(String),
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
    R: LedgerReader<OpaEvent, Error = SawtoothCommunicationError> + Send + Sync + Clone + 'static,
>(
    reader: R,
    goal_tx_id: OpaTransactionId,
    max_steps: u64,
) -> impl Future<Output = Waited> {
    let span = span!(Level::DEBUG, "wait_for_opa_transaction");
    let _entered = span.enter();

    // Set up a oneshot channel to notify the returned task
    let (notify_tx, notify_rx) = oneshot::channel::<Waited>();

    Handle::current().spawn(async move {
        // We can immediately return if we are not waiting

        let goal_clone = goal_tx_id.clone();

        let mut stream = reader.state_updates(None, Some(max_steps)).await.unwrap();

        while let Some((op, tx, _, _)) = stream.next().await {
            if tx == goal_clone {
                if let OpaOperationEvent::Error(_) = op {
                    notify_tx
                        .send(Waited::WaitedAndOperationFailed(op))
                        .unwrap();
                    break;
                }
                notify_tx.send(Waited::WaitedAndFound(op)).unwrap();
                break;
            }
        }
    });

    async move { notify_rx.await.unwrap() }
}

#[instrument(skip(reader, writer, matches))]
async fn handle_wait<
    R: LedgerReader<OpaEvent, Error = SawtoothCommunicationError> + Clone + Send + Sync + 'static,
    W: LedgerWriter<OpaSubmitTransaction, Error = SawtoothCommunicationError>,
>(
    matches: &ArgMatches,
    reader: R,
    writer: W,
    submission: OpaSubmitTransaction,
    transactor_key: &SigningKey,
) -> Result<Waited, OpaCtlError> {
    let wait = Wait::from_matches(matches);
    let (tx_id, tx) = writer.pre_submit(&submission).await?;

    match wait {
        Wait::NoWait => {
            writer.submit(tx, transactor_key).await?;

            Ok(Waited::NoWait)
        }
        Wait::NumberOfBlocks(blocks) => {
            let waiter = ambient_transactions(reader, tx_id.clone(), blocks).await;
            writer.submit(tx, transactor_key).await?;

            match waiter.await {
                Waited::WaitedAndDidNotFind => Err(OpaCtlError::TransactionNotFound(tx_id)),
                Waited::WaitedAndOperationFailed(OpaOperationEvent::Error(e)) => {
                    Err(OpaCtlError::TransactionFailed(e))
                }
                x => Ok(x),
            }
        }
    }
}

async fn dispatch_args<
    W: LedgerWriter<OpaSubmitTransaction, Error = SawtoothCommunicationError>,
    R: LedgerReader<OpaEvent, Error = SawtoothCommunicationError> + Send + Sync + Clone + 'static,
>(
    matches: ArgMatches,
    writer: W,
    reader: R,
) -> Result<Waited, OpaCtlError> {
    let span = span!(Level::DEBUG, "dispatch_args");
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
        Some(("generate", _matches)) => {
            let key = SecretKey::random(StdRng::from_entropy());
            let key = key.to_pkcs8_pem(LineEnding::CRLF).unwrap();

            if let Some(path) = matches.get_one::<String>("output") {
                let mut file = File::create(path).unwrap();
                file.write_all(key.as_bytes()).unwrap();
            } else {
                println!("{}", *key);
            }

            Ok(Waited::NoWait)
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
            let rotate_key =
                SubmissionBuilder::register_key(id, &new_key.verifying_key(), &current_root_key)
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
            let policy: &PathBuf = matches.get_one("policy").unwrap();
            let mut policy_file = File::open(policy)?;
            let mut policy = vec![];
            policy_file.read_to_end(&mut policy)?;
            let id = matches.get_one::<String>("id").unwrap();

            let bootstrap = SubmissionBuilder::set_policy(id, policy, root_key).build(span_id);
            Ok(handle_wait(
                matches,
                reader,
                writer,
                OpaSubmitTransaction::bootstrap_root(bootstrap, &transactor_key),
                &transactor_key,
            )
            .await?)
        }
        Some(("get-key", matches)) => {
            let key = reader
                .get_state_entry(&key_address(matches.get_one::<String>("id").unwrap()))
                .await?;

            debug!(loaded_key = ?from_utf8(&key));

            let key: Keys = serde_json::from_slice(&key)?;

            let key = key.current.key;

            if let Some(path) = matches.get_one::<String>("output") {
                let mut file = File::create(path).unwrap();
                file.write_all(key.as_bytes()).unwrap();
            } else {
                println!("{key}");
            }

            Ok(Waited::NoWait)
        }
        Some(("get-policy", _matches)) => {
            let policy = reader
                .get_state_entry(&policy_address(matches.get_one::<String>("id").unwrap()))
                .await?;

            let path = matches.get_one::<String>("output").unwrap();
            let mut file = File::create(path)?;
            file.write_all(&policy)?;

            Ok(Waited::NoWait)
        }
        _ => unreachable!(),
    }
}

#[tokio::main]
async fn main() {
    chronicle_telemetry::telemetry(None, chronicle_telemetry::ConsoleLogging::Pretty);
    let args = cli::cli().get_matches();
    let address: &Url = args.get_one("sawtooth-address").unwrap();
    let client = ZmqRequestResponseSawtoothChannel::new(address);
    let reader = OpaLedger::new(client.clone());
    let writer = reader.clone();

    dispatch_args(args, writer, reader)
        .await
        .map_err(|opactl| {
            error!(?opactl);
            opactl.into_ufe().print();
            std::process::exit(1);
        })
        .map(|waited| {
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
mod test {
    use clap::ArgMatches;
    use futures::{Stream, StreamExt};
    use sawtooth_sdk::messages::client_state::{
        ClientStateGetRequest, ClientStateGetResponse, ClientStateGetResponse_Status,
    };

    use k256::{
        pkcs8::{EncodePrivateKey, LineEnding},
        SecretKey,
    };
    use opa_tp::{abstract_tp::TP, tp::OpaTransactionHandler};

    use opa_tp_protocol::ledger::OpaLedger;

    use opa_tp_protocol::zmq_client::{RequestResponseSawtoothChannel, SawtoothCommunicationError};
    use protobuf::Message;
    use rand::rngs::StdRng;
    use rand_core::SeedableRng;
    use sawtooth_sdk::{
        messages::{
            block::{Block, BlockHeader},
            client_batch_submit::{
                ClientBatchSubmitRequest, ClientBatchSubmitResponse,
                ClientBatchSubmitResponse_Status,
            },
            client_block::{ClientBlockListResponse, ClientBlockListResponse_Status},
            client_event::{ClientEventsSubscribeResponse, ClientEventsSubscribeResponse_Status},
            processor::TpProcessRequest,
            transaction::TransactionHeader,
            validator::Message_MessageType,
        },
        processor::handler::{ContextError, TransactionContext},
    };
    use serde_json::{
        Value, {self},
    };

    use std::{
        cell::RefCell,
        collections::BTreeMap,
        io::Write,
        pin::Pin,
        sync::{Arc, Mutex},
    };
    use tempfile::NamedTempFile;
    use tokio_stream::wrappers::UnboundedReceiverStream;
    use tracing::{debug, instrument};

    use crate::{cli, dispatch_args};

    type TestTxEvents = Vec<(String, Vec<(String, String)>, Vec<u8>)>;

    pub trait SimulatedSawtoothBehavior {
        fn handle_request(
            &self,
            message_type: Message_MessageType,
            request: Vec<u8>,
        ) -> Result<Vec<u8>, SawtoothCommunicationError>;
    }

    type ChannelHolder = Arc<Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<Option<Vec<u8>>>>>>;
    // A simulation of zmq transport + routing, using a function that takes a
    // request buffer and returns a response buffer.
    #[derive(Clone)]
    pub struct SimulatedSubmissionChannel {
        behavior: Arc<Box<dyn SimulatedSawtoothBehavior + Send + Sync>>,
        rx: ChannelHolder,
    }

    impl SimulatedSubmissionChannel {
        pub fn new(
            behavior: Box<dyn SimulatedSawtoothBehavior + Send + Sync + 'static>,
            rx: tokio::sync::mpsc::UnboundedReceiver<Option<Vec<u8>>>,
        ) -> Self {
            Self {
                behavior: Arc::new(behavior),
                rx: Arc::new(Some(rx).into()),
            }
        }
    }

    #[async_trait::async_trait]
    impl RequestResponseSawtoothChannel for SimulatedSubmissionChannel {
        #[instrument(skip(self) ret(Debug))]
        async fn send_and_recv_one<RX: protobuf::Message, TX: protobuf::Message>(
            &self,
            tx: TX,
            message_type: Message_MessageType,
            _timeout: std::time::Duration,
        ) -> Result<RX, SawtoothCommunicationError> {
            let mut in_buf = vec![];
            tx.write_to_vec(&mut in_buf).unwrap();
            let out_buf = self.behavior.handle_request(message_type, in_buf)?;
            Ok(RX::parse_from_bytes(&out_buf).unwrap())
        }

        #[instrument(skip(self))]
        async fn recv_stream<RX: protobuf::Message>(
            self,
        ) -> Result<Pin<Box<dyn Stream<Item = RX> + Send>>, SawtoothCommunicationError> {
            Ok(
                UnboundedReceiverStream::new(self.rx.lock().unwrap().take().unwrap())
                    .map(|rx| RX::parse_from_bytes(&rx.unwrap()).unwrap())
                    .boxed(),
            )
        }

        fn reconnect(&self) {}
    }

    pub struct TestTransactionContext {
        pub state: RefCell<BTreeMap<String, Vec<u8>>>,
        pub events: RefCell<TestTxEvents>,
        tx: tokio::sync::mpsc::UnboundedSender<Option<Vec<u8>>>,
    }

    impl TestTransactionContext {
        pub fn new(tx: tokio::sync::mpsc::UnboundedSender<Option<Vec<u8>>>) -> Self {
            Self {
                state: RefCell::new(BTreeMap::new()),
                events: RefCell::new(vec![]),
                tx,
            }
        }

        pub fn new_with_state(
            tx: tokio::sync::mpsc::UnboundedSender<Option<Vec<u8>>>,
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
    }

    impl TransactionContext for TestTransactionContext {
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
            let stl_event = sawtooth_sdk::messages::events::Event {
                event_type: event_type.clone(),
                attributes: attributes
                    .iter()
                    .map(|(k, v)| sawtooth_sdk::messages::events::Event_Attribute {
                        key: k.clone(),
                        value: v.clone(),
                        ..Default::default()
                    })
                    .collect(),
                data: data.to_vec(),
                ..Default::default()
            };
            let list = sawtooth_sdk::messages::events::EventList {
                events: vec![stl_event].into(),
                ..Default::default()
            };
            let stl_event = list.write_to_bytes().unwrap();

            self.tx.send(Some(stl_event)).unwrap();

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

    struct WellBehavedBehavior {
        handler: Arc<OpaTransactionHandler>,
        context: Arc<Mutex<TestTransactionContext>>,
    }

    impl SimulatedSawtoothBehavior for WellBehavedBehavior {
        #[instrument(skip(self, request))]
        fn handle_request(
            &self,
            message_type: Message_MessageType,
            request: Vec<u8>,
        ) -> Result<Vec<u8>, SawtoothCommunicationError> {
            match message_type {
                // Batch submit request, decode and apply the transactions
                // in the batch
                Message_MessageType::CLIENT_BATCH_SUBMIT_REQUEST => {
                    let mut req = ClientBatchSubmitRequest::parse_from_bytes(&request).unwrap();
                    let batch = req.take_batches().into_iter().next().unwrap();

                    debug!(received_batch = ?batch, transactions = ?batch.transactions);

                    // Convert transaction into TpProcessRequest
                    for tx in batch.transactions {
                        let req = TpProcessRequest {
                            payload: tx.get_payload().to_vec(),
                            header: Some(
                                TransactionHeader::parse_from_bytes(tx.get_header()).unwrap(),
                            )
                            .into(),
                            signature: tx.get_header_signature().to_string(),
                            ..Default::default()
                        };

                        self.handler
                            .as_ref()
                            .apply(&req, &mut *self.context.lock().unwrap())
                            .unwrap();
                    }
                    let mut response = ClientBatchSubmitResponse::new();
                    response.set_status(ClientBatchSubmitResponse_Status::OK);
                    let mut buf = vec![];
                    response.write_to_vec(&mut buf).unwrap();
                    Ok(buf)
                }
                // Always respond with a block height of one
                Message_MessageType::CLIENT_BLOCK_LIST_REQUEST => {
                    let mut response = ClientBlockListResponse::new();
                    let block_header = BlockHeader {
                        block_num: 1,
                        ..Default::default()
                    };
                    let block_header_bytes = block_header.write_to_bytes().unwrap();
                    response.set_blocks(
                        vec![Block {
                            header: block_header_bytes,
                            ..Default::default()
                        }]
                        .into(),
                    );
                    response.set_status(ClientBlockListResponse_Status::OK);
                    let mut buf = vec![];
                    response.write_to_vec(&mut buf).unwrap();
                    Ok(buf)
                }
                // We can just return Ok here, no need to fake routing
                Message_MessageType::CLIENT_EVENTS_SUBSCRIBE_REQUEST => {
                    let mut response = ClientEventsSubscribeResponse::new();
                    response.set_status(ClientEventsSubscribeResponse_Status::OK);
                    let mut buf = vec![];
                    response.write_to_vec(&mut buf).unwrap();
                    Ok(buf)
                }
                Message_MessageType::CLIENT_STATE_GET_REQUEST => {
                    let mut request = ClientStateGetRequest::parse_from_bytes(&request).unwrap();
                    let address = request.take_address();

                    let state = self
                        .context
                        .lock()
                        .unwrap()
                        .get_state_entries(&[address])
                        .unwrap();

                    let mut response = ClientStateGetResponse {
                        status: ClientStateGetResponse_Status::OK,
                        ..Default::default()
                    };

                    if state.is_empty() {
                        response.set_status(ClientStateGetResponse_Status::NO_RESOURCE);
                    } else {
                        response.set_value(state[0].1.clone());
                    }

                    let mut buf = vec![];
                    response.write_to_vec(&mut buf).unwrap();
                    Ok(buf)
                }
                _ => panic!("Unexpected message type {} received", message_type as i32),
            }
        }
    }

    struct EmbeddedOpaTp {
        pub ledger: OpaLedger<SimulatedSubmissionChannel>,
        context: Arc<Mutex<TestTransactionContext>>,
    }

    impl EmbeddedOpaTp {
        pub fn readable_state(&self) -> Vec<(String, Value)> {
            self.context.lock().unwrap().readable_state()
        }
    }

    fn embed_opa_tp() -> EmbeddedOpaTp {
        chronicle_telemetry::telemetry(None, chronicle_telemetry::ConsoleLogging::Pretty);

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let context = Arc::new(Mutex::new(TestTransactionContext::new(tx)));

        let handler = Arc::new(OpaTransactionHandler::new());

        let behavior = WellBehavedBehavior {
            handler,
            context: context.clone(),
        };

        EmbeddedOpaTp {
            ledger: OpaLedger::new(SimulatedSubmissionChannel::new(Box::new(behavior), rx)),
            context,
        }
    }

    fn reuse_opa_tp_state(tp: EmbeddedOpaTp) -> EmbeddedOpaTp {
        chronicle_telemetry::telemetry(None, chronicle_telemetry::ConsoleLogging::Pretty);

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let context = Arc::new(Mutex::new(TestTransactionContext::new_with_state(
            tx,
            tp.context.lock().unwrap().state.borrow().clone(),
        )));

        let handler = Arc::new(OpaTransactionHandler::new());

        let behavior = WellBehavedBehavior {
            handler,
            context: context.clone(),
        };

        EmbeddedOpaTp {
            ledger: OpaLedger::new(SimulatedSubmissionChannel::new(Box::new(behavior), rx)),
            context,
        }
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
              date: "[date]"
              key: "[pem]"
            expired: ~
            id: root
        "###);

        let opa_tp = reuse_opa_tp_state(opa_tp);

        let out_keyfile = NamedTempFile::new().unwrap();

        let matches = get_opactl_cmd(
            format!("opactl get-key --output {}", out_keyfile.path().display(),).as_str(),
        );

        insta::assert_yaml_snapshot!(
        dispatch_args(matches, opa_tp.ledger.clone(), opa_tp.ledger.clone())
            .await
            .unwrap(), @r###"
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
            .unwrap(), {
            ".**.date" => "[date]",
            ".**.key" => "[pem]",
        } ,@r###"
        ---
        WaitedAndFound:
          KeyUpdate:
            id: root
            current:
              key: "[pem]"
              date: "[date]"
            expired:
              key: "[pem]"
              date: "[date]"
        "###);

        insta::assert_yaml_snapshot!(opa_tp.readable_state(),{
            ".**.date" => "[date]",
            ".**.key" => "[pem]",
        }, @r###"
        ---
        - - 7ed19313e8ece6c4f5551b9bd1090797ad25c6d85f7b523b2214d4fe448372279aa95c
          - current:
              date: "[date]"
              key: "[pem]"
            expired:
              date: "[date]"
              key: "[pem]"
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
            .unwrap(), {
            ".**.date" => "[date]",
            ".**.key" => "[pem]",
        },@r###"
        ---
        WaitedAndFound:
          KeyUpdate:
            id: test
            current:
              key: "[pem]"
              date: "[date]"
            expired: ~
        "###);

        insta::assert_yaml_snapshot!(opa_tp.readable_state(), {
            ".**.date" => "[date]",
            ".**.key" => "[pem]",
        }, @r###"
        ---
        - - 7ed19313e8ece6c4f5551b9bd1090797ad25c6d85f7b523b2214d4fe448372279aa95c
          - current:
              date: "[date]"
              key: "[pem]"
            expired: ~
            id: root
        - - 7ed19336d8b5677c39a7b872910f948944dd84ba014846c81fcd53fe1fd5289b9dfd1c
          - current:
              date: "[date]"
              key: "[pem]"
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
            .unwrap(), {
            ".**.date" => "[date]",
            ".**.key" => "[pem]",
        } ,@r###"
        ---
        WaitedAndFound:
          KeyUpdate:
            id: root
            current:
              key: "[pem]"
              date: "[date]"
            expired:
              key: "[pem]"
              date: "[date]"
        "###);

        insta::assert_yaml_snapshot!(opa_tp.readable_state(), {
            ".**.date" => "[date]",
            ".**.key" => "[pem]",
        } ,@r###"
        ---
        - - 7ed19313e8ece6c4f5551b9bd1090797ad25c6d85f7b523b2214d4fe448372279aa95c
          - current:
              date: "[date]"
              key: "[pem]"
            expired:
              date: "[date]"
              key: "[pem]"
            id: root
        - - 7ed19336d8b5677c39a7b872910f948944dd84ba014846c81fcd53fe1fd5289b9dfd1c
          - current:
              date: "[date]"
              key: "[pem]"
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
        .unwrap(), {
          ".**.date" => "[date]"
        }, @r###"
        ---
        WaitedAndFound:
          PolicyUpdate:
            id: test
            date: "[date]"
            policy_address: 7ed1931c262a4be700b69974438a35ae56a07ce96778b276c8a061dc254d9862c7ecff
        "###);

        insta::assert_yaml_snapshot!(opa_tp.readable_state(), {
          ".**.date" => "[date]",
          ".**.key" => "[pem]"
        } ,@r###"
        ---
        - - 7ed19313e8ece6c4f5551b9bd1090797ad25c6d85f7b523b2214d4fe448372279aa95c
          - current:
              date: "[date]"
              key: "[pem]"
            expired: ~
            id: root
        - - 7ed1931c262a4be700b69974438a35ae56a07ce96778b276c8a061dc254d9862c7ecff
          - - 0
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
            .unwrap(), {
              ".**.date" => "[date]"
            }, @r###"
        ---
        WaitedAndFound:
          PolicyUpdate:
            id: test
            date: "[date]"
            policy_address: 7ed1931c262a4be700b69974438a35ae56a07ce96778b276c8a061dc254d9862c7ecff
        "### );

        insta::assert_yaml_snapshot!(opa_tp.readable_state(), {
          ".**.date" => "[date]",
          ".**.key" => "[pem]"
        } ,@r###"
        ---
        - - 7ed19313e8ece6c4f5551b9bd1090797ad25c6d85f7b523b2214d4fe448372279aa95c
          - current:
              date: "[date]"
              key: "[pem]"
            expired: ~
            id: root
        - - 7ed1931c262a4be700b69974438a35ae56a07ce96778b276c8a061dc254d9862c7ecff
          - - 0
            - 1
        "###);
    }
}
