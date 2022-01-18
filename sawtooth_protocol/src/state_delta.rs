use std::pin::Pin;
use std::time::Duration;

use common::ledger::{LedgerReader, Offset, SubscriptionError};
use common::prov::ProvModel;
use custom_error::custom_error;
use derivative::*;
use futures::{stream, FutureExt, Stream, StreamExt};

use futures::future::join_all;
use k256::ecdsa::SigningKey;
use prost::{DecodeError, EncodeError, Message};
use sawtooth_sdk::messages::validator::Message_MessageType;
use sawtooth_sdk::messaging::stream::{
    MessageConnection, MessageFuture, MessageResult, ReceiveError, SendError,
};
use sawtooth_sdk::messaging::zmq_stream::ZmqMessageConnection;
use sawtooth_sdk::messaging::{
    stream::MessageReceiver, stream::MessageSender, zmq_stream::ZmqMessageSender,
};
use tokio::task::JoinError;
use tracing::{debug, error, instrument};

custom_error! {pub StateError
    Subscription                                    = "Invalid subscription",
    Runtime{source: JoinError}                      = "Failed to return from blocking operation",
    ZmqRx{source: ReceiveError}                     = "No response from validator",
    ZmqTx{source: SendError}                        = "No response from validator",
    ProtobufEncode{source: EncodeError}                   = "Protobuf encoding",
    ProtobufDecode{source: DecodeError}                   = "Protobuf decoding",
    SubscribeError{msg: String} = "Subscription failed",
}

impl From<StateError> for SubscriptionError {
    fn from(e: StateError) -> SubscriptionError {
        SubscriptionError::Implementation {
            source: Box::new(e),
        }
    }
}

use crate::messages::MessageBuilder;
use crate::sawtooth::client_events_subscribe_response::Status;
use crate::sawtooth::{ClientEventsSubscribeResponse, EventList};

#[derive(Derivative)]
#[derivative(Debug)]
pub struct StateDelta {
    #[derivative(Debug = "ignore")]
    tx: ZmqMessageSender,
    rx: MessageReceiver,
    builder: MessageBuilder,
}

impl StateDelta {
    pub fn new(address: &url::Url, signer: &SigningKey) -> Self {
        let builder = MessageBuilder::new(signer.to_owned(), "chronicle", "1.0");
        let (tx, rx) = ZmqMessageConnection::new(address.as_str()).create();
        StateDelta { tx, rx, builder }
    }

    async fn recv_from(
        mut fut: MessageFuture,
    ) -> Result<(MessageFuture, MessageResult), StateError> {
        let (fut, response) = tokio::task::spawn_blocking(move || {
            let response = fut.get_timeout(Duration::from_millis(500));

            (fut, response)
        })
        .await?;

        Ok((fut, Ok(response?)))
    }

    #[instrument]
    async fn get_state_from(
        &self,
        offset: Offset,
    ) -> Result<impl futures::Stream<Item = Vec<(Offset, ProvModel)>>, StateError> {
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

        let (fut, response) = StateDelta::recv_from(fut).await?;

        let response = ClientEventsSubscribeResponse::decode(response?.get_content())?;

        debug!(?request, "Subscription response");

        if response.status() != Status::Ok {
            return Err(StateError::SubscribeError {
                msg: response.response_message,
            });
        }

        let stream = stream::unfold(fut, |fut| async move {
            let mut futs = vec![fut];

            loop {
                let (fut, events) = StateDelta::recv_from(futs.pop().unwrap()).await.unwrap();

                futs.push(fut);

                match events {
                    Err(ReceiveError::TimeoutError) => {
                        debug!("No events in time window");
                    }
                    Ok(events) => match EventList::decode(events.get_content()) {
                        Ok(events) => {
                            let prov =
                                join_all(events.events.into_iter().map(|event| async move {
                                    let prov = ProvModel::default();
                                    prov.apply_json_ld_bytes(&*event.data)
                                        .await
                                        .map(|prov| (Offset::Genesis, prov))
                                }))
                                .await
                                .into_iter()
                                .collect::<Result<Vec<_>, _>>();

                            match prov {
                                Ok(prov) => return Some((prov, futs.pop().unwrap())),
                                Err(e) => {
                                    error!(?e, "Decoding state");
                                }
                            }
                        }
                        Err(e) => {
                            error!(?e, "Decoding protobuf");
                        }
                    },
                    Err(e) => {
                        // Non recoverable channel error, end stream
                        error!(?e, "Zmq recv error");
                        return None;
                    }
                }
            }
        });

        Ok(stream)
    }
}

#[async_trait::async_trait(?Send)]
impl LedgerReader for StateDelta {
    async fn state_updates(
        &self,
        offset: Offset,
    ) -> Result<
        Pin<Box<dyn Stream<Item = (Offset, ProvModel)> + Send>>,
        common::ledger::SubscriptionError,
    > {
        Ok(self
            .get_state_from(offset)
            .await?
            .flat_map(|x| stream::iter(x))
            .boxed())
    }
}
