use async_sawtooth_sdk::{
    error::SawtoothCommunicationError, ledger::SawtoothLedger, protobuf, protobuf::Message,
    zmq_client::RequestResponseSawtoothChannel,
};
use chronicle_protocol::address::{FAMILY, VERSION};

use chronicle_protocol::{messages::ChronicleSubmitTransaction, protocol::ChronicleOperationEvent};
use chronicle_sawtooth_tp::tp::ChronicleTransactionHandler;
use futures::{Stream, StreamExt};
use sawtooth_sdk::{
    messages::{
        block::{Block, BlockHeader},
        client_batch_submit::{
            ClientBatchSubmitRequest, ClientBatchSubmitResponse, ClientBatchSubmitResponse_Status,
        },
        client_block::{ClientBlockListResponse, ClientBlockListResponse_Status},
        client_event::{ClientEventsSubscribeResponse, ClientEventsSubscribeResponse_Status},
        client_state::{
            ClientStateGetRequest, ClientStateGetResponse, ClientStateGetResponse_Status,
        },
        processor::TpProcessRequest,
        transaction::TransactionHeader,
        validator::Message_MessageType,
    },
    processor::handler::{ContextError, TransactionContext, TransactionHandler},
};
use serde_json::Value;
use std::{
    cell::RefCell,
    collections::BTreeMap,
    pin::Pin,
    sync::{Arc, Mutex},
};
use tokio_stream::wrappers::UnboundedReceiverStream;
use tracing::{debug, instrument};

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
    #[instrument(skip(self), ret(Debug))]
    async fn send_and_recv_one<RX: protobuf::Message, TX: protobuf::Message>(
        &self,
        tx: TX,
        message_type: Message_MessageType,
        _timeout: std::time::Duration,
    ) -> Result<RX, SawtoothCommunicationError> {
        let mut in_buf = vec![];
        tx.write_to_vec(&mut in_buf).unwrap();
        let out_buf: Vec<u8> = self.behavior.handle_request(message_type, in_buf)?;
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
}

pub type InMemLedger =
    SawtoothLedger<SimulatedSubmissionChannel, ChronicleOperationEvent, ChronicleSubmitTransaction>;

pub struct SimulatedTransactionContext {
    pub state: RefCell<BTreeMap<String, Vec<u8>>>,
    pub events: RefCell<TestTxEvents>,
    tx: tokio::sync::mpsc::UnboundedSender<Option<Vec<u8>>>,
}

impl SimulatedTransactionContext {
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

impl TransactionContext for SimulatedTransactionContext {
    fn add_receipt_data(
        self: &SimulatedTransactionContext,
        _data: &[u8],
    ) -> Result<(), ContextError> {
        unimplemented!()
    }

    #[instrument(skip(self))]
    fn add_event(
        self: &SimulatedTransactionContext,
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
        self: &SimulatedTransactionContext,
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
        self: &SimulatedTransactionContext,
        entries: Vec<(String, Vec<u8>)>,
    ) -> std::result::Result<(), sawtooth_sdk::processor::handler::ContextError> {
        for entry in entries {
            self.state.borrow_mut().insert(entry.0, entry.1);
        }

        Ok(())
    }
}

pub struct WellBehavedBehavior {
    handler: Arc<ChronicleTransactionHandler>,
    context: Arc<Mutex<SimulatedTransactionContext>>,
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
                        header: Some(TransactionHeader::parse_from_bytes(tx.get_header()).unwrap())
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

pub struct EmbeddedChronicleTp {
    pub ledger: InMemLedger,
    context: Arc<Mutex<SimulatedTransactionContext>>,
}

impl EmbeddedChronicleTp {
    pub fn new() -> Self {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let context = Arc::new(Mutex::new(SimulatedTransactionContext::new(tx)));

        let (policy, entrypoint) = ("allow_transactions", "allow_transactions.allowed_users");

        let handler = Arc::new(ChronicleTransactionHandler::new(policy, entrypoint).unwrap());

        let behavior = WellBehavedBehavior {
            handler,
            context: context.clone(),
        };

        Self {
            ledger: InMemLedger::new(
                SimulatedSubmissionChannel::new(Box::new(behavior), rx),
                FAMILY,
                VERSION,
            ),
            context,
        }
    }

    pub fn readable_state(&self) -> Vec<(String, Value)> {
        self.context.lock().unwrap().readable_state()
    }
}

impl Default for EmbeddedChronicleTp {
    fn default() -> Self {
        Self::new()
    }
}
