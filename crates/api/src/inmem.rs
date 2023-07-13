use async_stl_client::{
    error::SawtoothCommunicationError,
    ledger::SawtoothLedger,
    zmq_client::{HighestBlockValidatorSelector, ZmqRequestResponseSawtoothChannel},
};
use chronicle_protocol::address::{FAMILY, VERSION};
use protobuf::{self, Message, ProtobufEnum};

use chronicle_protocol::{messages::ChronicleSubmitTransaction, protocol::ChronicleOperationEvent};
use chronicle_sawtooth_tp::tp::ChronicleTransactionHandler;
use futures::{select, FutureExt, SinkExt, StreamExt};
use sawtooth_sdk::{
    messages::{
        block::{Block, BlockHeader},
        client_batch_submit::{
            ClientBatchSubmitRequest, ClientBatchSubmitResponse, ClientBatchSubmitResponse_Status,
        },
        client_block::{
            ClientBlockGetByNumRequest, ClientBlockGetResponse, ClientBlockGetResponse_Status,
            ClientBlockListResponse, ClientBlockListResponse_Status,
        },
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
    net::{Ipv4Addr, SocketAddr},
    sync::{Arc, Mutex},
    thread::{self},
};
use tmq::{router, Context, Multipart};
use tokio::runtime;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tracing::{debug, error, info, instrument};
use uuid::Uuid;

type TestTxEvents = Vec<(String, Vec<(String, String)>, Vec<u8>)>;

pub trait SimulatedSawtoothBehavior {
    fn handle_request(
        &self,
        message_type: Message_MessageType,
        request: Vec<u8>,
    ) -> Result<(Message_MessageType, Vec<u8>), SawtoothCommunicationError>;
}

pub type InMemLedger = SawtoothLedger<
    ZmqRequestResponseSawtoothChannel,
    ChronicleOperationEvent,
    ChronicleSubmitTransaction,
>;

pub struct SimulatedTransactionContext {
    pub state: RefCell<BTreeMap<String, Vec<u8>>>,
    pub events: RefCell<TestTxEvents>,
    tx: tokio::sync::mpsc::UnboundedSender<Option<(Message_MessageType, Vec<u8>)>>,
}

impl SimulatedTransactionContext {
    pub fn new(
        tx: tokio::sync::mpsc::UnboundedSender<Option<(Message_MessageType, Vec<u8>)>>,
    ) -> Self {
        Self {
            state: RefCell::new(BTreeMap::new()),
            events: RefCell::new(vec![]),
            tx,
        }
    }

    pub fn new_with_state(
        tx: tokio::sync::mpsc::UnboundedSender<Option<(Message_MessageType, Vec<u8>)>>,
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

        self.tx
            .send(Some((Message_MessageType::CLIENT_EVENTS, stl_event)))
            .unwrap();

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

#[derive(Clone)]
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
    ) -> Result<(Message_MessageType, Vec<u8>), SawtoothCommunicationError> {
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
                Ok((Message_MessageType::CLIENT_BATCH_SUBMIT_RESPONSE, buf))
            }
            Message_MessageType::CLIENT_BLOCK_GET_BY_NUM_REQUEST => {
                let req = ClientBlockGetByNumRequest::parse_from_bytes(&request).unwrap();
                debug!(get_block=?req);
                let mut response = ClientBlockGetResponse::new();
                let block_header = BlockHeader {
                    block_num: req.get_block_num(),
                    previous_block_id: hex::encode([0; 32]),
                    ..Default::default()
                };
                let block_header_bytes = block_header.write_to_bytes().unwrap();
                response.set_block(Block {
                    header: block_header_bytes,
                    ..Default::default()
                });
                response.set_status(ClientBlockGetResponse_Status::OK);
                let mut buf = vec![];
                response.write_to_vec(&mut buf).unwrap();
                Ok((Message_MessageType::CLIENT_BLOCK_GET_RESPONSE, buf))
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
                Ok((Message_MessageType::CLIENT_BLOCK_LIST_RESPONSE, buf))
            }
            // We can just return Ok here, no need to fake routing
            Message_MessageType::CLIENT_EVENTS_SUBSCRIBE_REQUEST => {
                let mut response = ClientEventsSubscribeResponse::new();
                response.set_status(ClientEventsSubscribeResponse_Status::OK);
                let mut buf = vec![];
                response.write_to_vec(&mut buf).unwrap();
                Ok((Message_MessageType::CLIENT_EVENTS_SUBSCRIBE_RESPONSE, buf))
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
                Ok((Message_MessageType::CLIENT_STATE_GET_RESPONSE, buf))
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
    pub fn new_with_state(
        state: BTreeMap<String, Vec<u8>>,
    ) -> Result<Self, SawtoothCommunicationError> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

        let context = Arc::new(Mutex::new(SimulatedTransactionContext::new_with_state(
            tx, state,
        )));

        let (policy, entrypoint) = ("allow_transactions", "allow_transactions.allowed_users");

        let handler = Arc::new(ChronicleTransactionHandler::new(policy, entrypoint).unwrap());

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
                              let request: async_stl_client::messages::Message =
                                  async_stl_client::prost::Message::decode(&*multipart[1].to_vec()).map_err(|e| error!(%e)).unwrap();

                              debug!(request = ?request);

                              let response = behavior_clone
                                  .handle_request(
                                      Message_MessageType::from_i32(request.message_type).unwrap(),
                                      request.content,
                                  )
                                  .unwrap();


                              let message_wrapper = async_stl_client::messages::Message {
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
                              let message_wrapper = async_stl_client::messages::Message {
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

        Ok(Self {
            ledger: InMemLedger::new(
                ZmqRequestResponseSawtoothChannel::new(
                    &format!("test_{}", Uuid::new_v4()),
                    &[connect_addr],
                    HighestBlockValidatorSelector,
                )?,
                FAMILY,
                VERSION,
            ),
            context,
        })
    }

    pub fn new() -> Result<Self, SawtoothCommunicationError> {
        Self::new_with_state(BTreeMap::new())
    }

    pub fn readable_state(&self) -> Vec<(String, Value)> {
        self.context.lock().unwrap().readable_state()
    }
}
