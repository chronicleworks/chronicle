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
use tracing::{debug, error, info, instrument, warn};

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
        client_events_subscribe_response::Status, ClientEventsSubscribeResponse, EventList,
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
    ) -> Result<MessageResult, StateError> {
        let response = tokio::task::spawn_blocking(move || {
            let response = fut.lock().unwrap().recv_timeout(Duration::from_secs(2));
            response
        })
        .await??;

        Ok(response)
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
        let request = self.builder.make_subscription_request(offset);

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

        Ok(Self::event_stream(self.rx.clone(), offset.clone()))
    }

    fn event_stream(
        rx: Arc<Mutex<MessageReceiver>>,
        block: Offset,
    ) -> impl futures::Stream<Item = Vec<Result<Commit, (ChronicleTransactionId, Contradiction)>>>
    {
        #[derive(Debug)]
        enum ParsedEvent {
            Block(String),
            State(Box<ProvModel>, ChronicleTransactionId),
            Contradiction(Contradiction, ChronicleTransactionId),
        }

        stream::unfold((rx, block), |(rx, block)| async move {
            loop {
                let events = StateDelta::recv_from_channel(rx.clone()).await;

                match events {
                    Err(StateError::ZmqRxx { .. }) => {}
                    Ok(Ok(events)) => {
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
                                                .map(|(transaction_id, (_span, res))| match res {
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
                                    (vec![], block),
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

                                return Some((events.0, (rx, events.1)));
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
        })
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
