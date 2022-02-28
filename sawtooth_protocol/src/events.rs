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
    ledger::{LedgerReader, Offset, SubscriptionError},
    prov::{ProcessorError, ProvModel},
};
use custom_error::custom_error;
use derivative::*;
use futures::{stream, Stream, StreamExt, TryFutureExt};

use k256::ecdsa::SigningKey;
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
use uuid::Uuid;

custom_error! {pub StateError
    Subscription                                    = "Invalid subscription",
    Runtime{source: JoinError}                      = "Failed to return from blocking operation",
    ZmqRx{source: ReceiveError}                     = "No response from validator",
    ZmqRxx{source: RecvTimeoutError}                = "No response from validator",
    ZmqTx{source: SendError}                        = "No response from validator",
    ProtobufEncode{source: EncodeError}             = "Protobuf encoding",
    ProtobufDecode{source: DecodeError}             = "Protobuf decoding",
    SubscribeError{msg: String}                     = "Subscription failed",
    RetryReceive{source: backoff::Error<sawtooth_sdk::messaging::stream::ReceiveError>} = "No response from validator",
    MissingBlockNum{}                               = "Missing block_num in block commit",
    MissingData{}                                   = "Missing block_num in block commit",
    UnparsableBlockNum {}                           = "Unparsable block_num in block commit",
    UnparsableEvent {source: serde_cbor::Error}     = "Unparsable event data",
    Processor { source: ProcessorError }            = "Json LD processing",
}

impl From<StateError> for SubscriptionError {
    fn from(e: StateError) -> SubscriptionError {
        SubscriptionError::Implementation {
            source: Box::new(e),
        }
    }
}

use crate::{
    messages::MessageBuilder,
    sawtooth::{
        client_events_subscribe_response::Status, ClientEventsSubscribeResponse, EventList,
    },
};

#[derive(Derivative)]
#[derivative(Debug, Clone)]
pub struct StateDelta {
    #[derivative(Debug = "ignore")]
    tx: ZmqMessageSender,
    rx: Arc<Mutex<MessageReceiver>>,
    builder: MessageBuilder,
}

impl StateDelta {
    #[instrument]
    pub fn new(address: &url::Url, signer: &SigningKey) -> Self {
        let builder = MessageBuilder::new(signer.to_owned(), "chronicle", "1.0");
        let (tx, rx) = ZmqMessageConnection::new(address.as_str()).create();
        info!(?address, "Subscribing to state updates");
        StateDelta {
            tx,
            rx: Arc::new(rx.into()),
            builder,
        }
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

    #[instrument]
    async fn get_state_from(
        &self,
        offset: &Offset,
    ) -> Result<impl futures::Stream<Item = Vec<(Offset, ProvModel, Uuid)>>, StateError> {
        let request = self.builder.make_subcription_request(offset);

        debug!(?request, "Subscription request");
        let mut buf = Vec::new();
        buf.reserve(request.encoded_len());
        request.encode(&mut buf)?;

        let fut = self.tx.send(
            Message_MessageType::CLIENT_EVENTS_SUBSCRIBE_REQUEST,
            &uuid::Uuid::new_v4().to_string(),
            &*buf,
        )?;

        let (_, response) = StateDelta::recv_from_messagefuture(fut).await.unwrap();

        let response = ClientEventsSubscribeResponse::decode(response?.get_content())?;

        debug!(?response, "Subscription response");

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
    ) -> impl futures::Stream<Item = Vec<(Offset, ProvModel, Uuid)>> {
        #[derive(Debug)]
        enum ParsedEvent {
            Block(String),
            State(ProvModel),
        }

        stream::unfold((rx, block), |(rx, block)| async move {
            loop {
                let events = StateDelta::recv_from_channel(rx.clone()).await;

                match events {
                    Err(StateError::ZmqRxx { .. }) => {
                        debug!("No events in time window");
                    }
                    Ok(Ok(events)) => {
                        let correlation = events.correlation_id.parse::<Uuid>().unwrap_or_default();
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
                                            serde_cbor::from_slice::<ProvModel>(&*event.data)
                                                .map_err(StateError::from)
                                                .map(|prov| Some(ParsedEvent::State(prov)))
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
                                            Ok(Some(ParsedEvent::State(next_prov))) => {
                                                prov.push((block.clone(), next_prov, correlation));
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

#[async_trait::async_trait(?Send)]
impl LedgerReader for StateDelta {
    #[instrument]
    async fn state_updates(
        &self,
        offset: Offset,
    ) -> Result<
        Pin<Box<dyn Stream<Item = (Offset, ProvModel, Uuid)> + Send>>,
        common::ledger::SubscriptionError,
    > {
        let self_clone = self.clone();

        let subscribe = backoff::future::retry(ExponentialBackoff::default(), || {
            self_clone.get_state_from(&offset).map_err(|e| {
                error!(?e, "Error subscribing");
                backoff::Error::transient(e)
            })
        });

        Ok(subscribe.await?.flat_map(stream::iter).boxed())
    }
}
