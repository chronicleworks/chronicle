use std::{
    pin::Pin,
    sync::{
        mpsc::{Receiver, RecvTimeoutError},
        Arc, Mutex,
    },
    time::Duration,
};

use backoff::ExponentialBackoff;

use common::{
    ledger::{Commit, CommitResult, LedgerReader, Offset, SubscriptionError},
    protocol::{deserialize_event, ProtocolError},
    prov::{
        ChronicleTransactionId, ChronicleTransactionIdError, Contradiction, ProcessorError,
        ProvModel,
    },
};
use custom_error::custom_error;
use derivative::*;
use futures::{stream, Stream, StreamExt, TryFutureExt};

use common::k256::ecdsa::SigningKey;
use hex::FromHexError;
use prost::{DecodeError, EncodeError, Message};
use sawtooth_sdk::{
    messages::validator::Message_MessageType,
    messaging::{
        stream::{
            MessageConnection, MessageFuture, MessageReceiver, MessageResult, MessageSender,
            ReceiveError, SendError,
        },
        zmq_stream::{ZmqMessageConnection, ZmqMessageSender},
    },
};
use tokio::task::JoinError;
use tracing::{debug, error, info, instrument, trace, warn};

custom_error! {pub StateError
    Subscription                                    = "Invalid subscription",
    Runtime{source: JoinError}                      = "Failed to return from blocking operation {source}",
    ZmqRx{source: ReceiveError}                     = "No response from validator {source}",
    ZmqRxx{source: RecvTimeoutError}                = "No response from validator {source}",
    ZmqTx{source: SendError}                        = "No response from validator {source}",
    ProtobufEncode{source: EncodeError}             = "Protobuf encoding {source}",
    ProtobufDecode{source: DecodeError}             = "Protobuf decoding {source}",
    SubscribeError{msg: String}                     = "Subscription failed {msg}",
    RetryReceive{source: backoff::Error<sawtooth_sdk::messaging::stream::ReceiveError>} = "No response from validator {source}",
    MissingBlockNum{}                               = "Missing block_num in block commit",
    MissingTransactionId{}                          = "Missing transaction_id in prov-update",
    InvalidTransactionId{source: common::k256::ecdsa::Error}
                                                    = "Invalid transaction id {source}",
    MissingData{}                                   = "Missing block_num in block commit",
    UnparsableBlockNum {}                           = "Unparsable block_num in block commit",
    UnparsableEvent {source: serde_cbor::Error}     = "Unparsable event data {source}",
    Processor { source: ProcessorError }            = "Json LD processing {source}",
    Hex{ source: FromHexError }                     = "Hex decode {source}",
    Signature {source: ChronicleTransactionIdError }= "Signature parse {source}",
    Protocol {source: ProtocolError}                = "Event protocol {source}"
}

impl From<StateError> for SubscriptionError {
    fn from(e: StateError) -> SubscriptionError {
        SubscriptionError::Implementation {
            source: Box::new(e),
        }
    }
}

use crate::{
    address::{FAMILY, VERSION},
    messages::MessageBuilder,
    sawtooth::{
        client_block_get_response, client_events_subscribe_response::Status, BlockHeader,
        ClientBlockGetResponse, ClientEventsSubscribeResponse, EventList, PingResponse,
    },
};

#[derive(Derivative)]
#[derivative(Debug, Clone)]
pub struct StateDelta {
    address: url::Url,
    #[derivative(Debug = "ignore")]
    tx: Arc<Mutex<ZmqMessageSender>>,
    rx: Arc<Mutex<MessageReceiver>>,
    builder: MessageBuilder,
}

impl StateDelta {
    #[instrument]
    pub fn new(address: &url::Url, signer: &SigningKey) -> Self {
        let builder = MessageBuilder::new(signer.to_owned(), FAMILY, VERSION);
        let (tx, rx) = ZmqMessageConnection::new(address.as_str()).create();
        info!(?address, "Subscribing to state updates");
        StateDelta {
            address: address.clone(),
            tx: Arc::new(tx.into()),
            rx: Arc::new(rx.into()),
            builder,
        }
    }

    fn reconnect(&self) {
        let (tx, rx) = ZmqMessageConnection::new(self.address.as_str()).create();
        *self.tx.lock().unwrap() = tx;
        *self.rx.lock().unwrap() = rx;
    }

    async fn recv_from_messagefuture(
        mut fut: MessageFuture,
    ) -> Result<(MessageFuture, MessageResult), StateError> {
        let (fut, response) = tokio::task::spawn_blocking(move || {
            let response = fut.get_timeout(Duration::from_secs(30));
            info!(?response, "Subscription response");
            (fut, response)
        })
        .await?;

        Ok((fut, Ok(response?)))
    }

    async fn recv_from_channel(
        fut: Arc<Mutex<Receiver<MessageResult>>>,
        ping_respond: Arc<Mutex<ZmqMessageSender>>,
    ) -> Result<MessageResult, StateError> {
        let response = tokio::task::spawn_blocking(move || {
            let response = fut.lock().unwrap().recv_timeout(Duration::from_secs(2));
            response
        })
        .await??;

        if let Ok(message) = response.as_ref() {
            if message.message_type == Message_MessageType::PING_REQUEST {
                debug!(ping_response = message.correlation_id);
                let buf = PingResponse::default().encode_to_vec();
                ping_respond.lock().unwrap().send(
                    Message_MessageType::PING_RESPONSE,
                    &message.correlation_id,
                    &buf,
                )?;
            }
        }

        Ok(response)
    }

    #[instrument]
    async fn resolve_genesis_block(&self, offset: &Offset) -> Result<Offset, StateError> {
        if let Offset::Identity(_) = offset {
            return Ok(offset.clone());
        }

        let block = {
            let buf = MessageBuilder::get_head_block_id_request().encode_to_vec();
            loop {
                debug!("Resolving genesis block");
                let mut fut = self.tx.lock().unwrap().send(
                    Message_MessageType::CLIENT_BLOCK_GET_BY_NUM_REQUEST,
                    &uuid::Uuid::new_v4().to_string(),
                    &buf,
                )?;

                let response = fut.get_timeout(Duration::from_secs(2));
                if let Ok(response) = response {
                    let message: ClientBlockGetResponse =
                        ClientBlockGetResponse::decode(&*response.content)?;
                    trace!(block_by_num_response = ?message);
                    match (
                        client_block_get_response::Status::from_i32(message.status),
                        message.block,
                    ) {
                        (Some(client_block_get_response::Status::Ok), Some(block)) => break block,
                        (e, _) => {
                            error!(head_block_status = ?e)
                        }
                    };
                }

                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        };

        Ok(Offset::Identity(
            BlockHeader::decode(block.header.as_slice())?.previous_block_id,
        ))
    }

    /// Subscribe to state delta events and then set up the event stream
    #[instrument]
    async fn get_state_from(
        &self,
        offset: &Offset,
    ) -> Result<
        impl futures::Stream<Item = Vec<Result<Commit, (ChronicleTransactionId, Contradiction)>>>,
        StateError,
    > {
        info!(read_ledger_state_from_block_id = ?offset);
        let offset = self.resolve_genesis_block(offset).await?;
        let request = self.builder.make_subscription_request(&offset);

        debug!(?request, "Subscription request");
        let mut buf = Vec::new();
        buf.reserve(request.encoded_len());
        request.encode(&mut buf)?;

        let response = {
            loop {
                let fut = self.tx.lock().unwrap().send(
                    Message_MessageType::CLIENT_EVENTS_SUBSCRIBE_REQUEST,
                    &uuid::Uuid::new_v4().to_string(),
                    &buf,
                )?;
                match StateDelta::recv_from_messagefuture(fut).await {
                    Ok((_, response)) => {
                        break ClientEventsSubscribeResponse::decode(response?.get_content())?;
                    }
                    Err(e) => {
                        warn!(?e, "Subscription error");
                    }
                }
            }
        };

        if response.status() != Status::Ok {
            return Err(StateError::SubscribeError {
                msg: format!(
                    "status {:?} - '{}'",
                    response.status, response.response_message
                ),
            });
        }

        Ok(Self::event_stream(
            self.rx.clone(),
            offset.clone(),
            self.tx.clone(),
        ))
    }

    fn event_stream(
        rx: Arc<Mutex<MessageReceiver>>,
        block: Offset,
        tx: Arc<Mutex<ZmqMessageSender>>,
    ) -> impl futures::Stream<Item = Vec<Result<Commit, (ChronicleTransactionId, Contradiction)>>>
    {
        #[derive(Debug, Clone)]
        enum ParsedEvent {
            Block(String),
            State(Box<ProvModel>, ChronicleTransactionId),
            Contradiction(Contradiction, ChronicleTransactionId),
        }

        stream::unfold(
            ((rx, tx), block),
            |((rx, ping_respond), block)| async move {
                let last_block = &mut block.clone();
                loop {
                    let events =
                        StateDelta::recv_from_channel(rx.clone(), ping_respond.clone()).await;

                    match events {
                        Err(StateError::ZmqRxx { source }) => {
                            trace!(zmq_poll_no_items = ?source);
                        }
                        Ok(Ok(events)) => {
                            trace!(?events, "Received events");
                            match EventList::decode(events.get_content()) {
                                Ok(events) => {
                                    debug!(?events, "Received events");
                                    let mut updates = vec![];
                                    for event in events.events {
                                        updates.push(match &*event.event_type {
                                            "sawtooth/block-commit" => event
                                                .attributes
                                                .iter()
                                                .find(|attr| attr.key == "block_id")
                                                .ok_or(StateError::MissingBlockNum {})
                                                .map(|attr| {
                                                    Some(ParsedEvent::Block(attr.value.clone()))
                                                }),
                                            "chronicle/prov-update" => {
                                                let transaction_id = event
                                                    .attributes
                                                    .iter()
                                                    .find(|attr| attr.key == "transaction_id")
                                                    .ok_or(StateError::MissingTransactionId {})
                                                    .map(|attr| {
                                                        ChronicleTransactionId::from(&*attr.value)
                                                    });

                                                let event = deserialize_event(&event.data)
                                                    .await
                                                    .map_err(StateError::from);

                                                transaction_id
                                                    .map_err(StateError::from)
                                                    .and_then(|transaction_id| {
                                                        event.map(|event| (transaction_id, event))
                                                    })
                                                    .map(|(transaction_id, (_span, res))| match res
                                                    {
                                                        Err(contradiction) => {
                                                            Some(ParsedEvent::Contradiction(
                                                                contradiction,
                                                                transaction_id,
                                                            ))
                                                        }
                                                        Ok(delta) => Some(ParsedEvent::State(
                                                            Box::new(delta),
                                                            transaction_id,
                                                        )),
                                                    })
                                            }
                                            _ => Ok(None),
                                        });
                                    }

                                    debug!(?updates, "Parsed events");

                                    let events = updates.into_iter().fold(
                                        (vec![], last_block.clone()),
                                        |(mut prov, block), event| {
                                            match event {
                                                // Next block num
                                                Ok(Some(ParsedEvent::Block(next))) => {
                                                    (prov, Offset::from(&*next))
                                                }
                                                Ok(Some(ParsedEvent::State(
                                                    next_prov,
                                                    transaction_id,
                                                ))) => {
                                                    prov.push(Ok(Commit::new(
                                                        transaction_id,
                                                        block.clone(),
                                                        next_prov,
                                                    )));
                                                    (prov, block)
                                                }
                                                Ok(Some(ParsedEvent::Contradiction(
                                                    contradiction,
                                                    transaction_id,
                                                ))) => {
                                                    prov.push(Err((transaction_id, contradiction)));
                                                    (prov, block)
                                                }
                                                Err(e) => {
                                                    error!(?e, "Parsing state update");
                                                    (prov, block)
                                                }
                                                _ => (prov, block),
                                            }
                                        },
                                    );

                                    *last_block = events.1.clone();
                                    debug!(?last_block);
                                    return Some((events.0, ((rx, ping_respond), events.1)));
                                }
                                Err(e) => {
                                    error!(?e, "Decoding protobuf");
                                }
                            }
                        }
                        Ok(Err(e)) => {
                            // recoverable error
                            warn!(?e, "Zmq recv error");
                            return None;
                        }
                        Err(e) => {
                            // Non recoverable channel error, end stream
                            error!(?e, "Zmq recv error");
                            return None;
                        }
                    }
                }
            },
        )
    }
}

#[async_trait::async_trait]
impl LedgerReader for StateDelta {
    #[instrument]
    async fn state_updates(
        self,
        offset: Offset,
    ) -> Result<Pin<Box<dyn Stream<Item = CommitResult> + Send>>, SubscriptionError> {
        let self_clone = self.clone();
        let subscribe = backoff::future::retry(ExponentialBackoff::default(), || {
            self_clone.get_state_from(&offset).map_err(|e| {
                error!(?e, "Error subscribing");
                self_clone.reconnect();
                backoff::Error::transient(e)
            })
        });

        Ok(subscribe.await?.fuse().flat_map(stream::iter).boxed())
    }
}
