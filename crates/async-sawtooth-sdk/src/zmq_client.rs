use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use futures::{stream, stream::BoxStream, StreamExt};
use protobuf::ProtobufError;

use sawtooth_sdk::{
    messages::validator::Message_MessageType,
    messaging::{
        stream::{MessageConnection, MessageReceiver, MessageSender, ReceiveError, SendError},
        zmq_stream::{ZmqMessageConnection, ZmqMessageSender},
    },
};
use thiserror::Error;
use tokio::runtime::Handle;
use tracing::{debug, error, instrument, trace};
use url::Url;

#[derive(Error, Debug)]
pub enum SawtoothCommunicationError {
    #[error("ZMQ error {0}")]
    ZMQ(#[from] zmq::Error),

    #[error("Send error {0}")]
    Send(#[from] SendError),

    #[error("Receive error {0}")]
    Receive(#[from] ReceiveError),

    #[error("Protobuf error {0}")]
    Protobuf(#[from] ProtobufError),
    #[error("Protobuf decode error {0}")]
    ProtobufProst(#[from] prost::DecodeError),
    #[error("Unexpected Status {status:?}")]
    UnexpectedStatus { status: i32 },
    #[error("No transaction id for event")]
    MissingTransactionId,
    #[error("Cannot determine block number for event")]
    MissingBlockNum,
    #[error("Unexpected message structure")]
    MalformedMessage,
    #[error("Json {0}")]
    Json(#[from] serde_json::Error),
    #[error("Subscribe error {code}")]
    SubscribeError { code: i32 },
    #[error("Block number is not number {source}")]
    BlockNumNotNumber {
        #[from]
        source: std::num::ParseIntError,
    },
    #[error("No blocks returned when searching for current block")]
    NoBlocksReturned,
}

/// A trait representing a communication channel for sending request and receiving
/// response messages to/from the Sawtooth network.
///
/// This trait defines methods for sending a single message and waiting for a response,
/// listening on the channel for response messages, and reconnecting to the Sawtooth
/// network if the connection is lost or interrupted.
///
/// A trait representing a communication channel for sending request and receiving
/// response messages to/from the Sawtooth network.
///
/// This trait defines methods for sending a single message and waiting for a response,
/// listening on the channel for response messages, and reconnecting to the Sawtooth
/// network if the connection is lost or interrupted.
///
#[async_trait::async_trait]
pub trait RequestResponseSawtoothChannel {
    /// Send a message and wait for a response, decoding the response as a
    /// protobuf message of type RX.
    ///
    /// # Arguments
    ///
    /// * tx - The message to send, encoded as a protobuf message of type TX.
    /// * message_type - The message type to send.
    /// * timeout - The maximum amount of time to wait for a response.
    ///
    /// # Returns
    ///
    /// The response message, decoded as a protobuf message of type RX.
    ///
    /// # Errors
    ///
    /// Returns an error if the send or receive operation fails.
    async fn send_and_recv_one<RX: protobuf::Message, TX: protobuf::Message>(
        &self,
        tx: TX,
        message_type: Message_MessageType,
        timeout: std::time::Duration,
    ) -> Result<RX, SawtoothCommunicationError>;

    /// Listens on this channel for messages, decoding them as a
    /// protobuf message of type `RX`. Terminates when the channel is closed.
    ///
    /// # Returns
    ///
    /// A stream of response messages, decoded as protobuf messages of type `RX`.
    ///
    /// # Errors
    ///
    /// Returns an error if the receive operation fails.
    /// Listens on this channel for messages, decoding them as a
    /// protobuf message of type `RX`. Terminates when the channel is closed.
    ///
    /// # Returns
    ///
    /// A stream of response messages, decoded as protobuf messages of type `RX`.
    ///
    /// # Errors
    ///
    /// Returns an error if the receive operation fails.
    async fn recv_stream<RX: protobuf::Message>(
        self,
    ) -> Result<BoxStream<'static, RX>, SawtoothCommunicationError>;

    fn reconnect(&self) {}
}

#[derive(Clone)]
pub struct ZmqRequestResponseSawtoothChannel {
    address: Url,
    tx: Arc<Mutex<ZmqMessageSender>>,
    rx: Arc<Mutex<MessageReceiver>>,
}

impl ZmqRequestResponseSawtoothChannel {
    pub fn new(address: &Url) -> Self {
        let (tx, rx) = ZmqMessageConnection::new(address.as_str()).create();
        ZmqRequestResponseSawtoothChannel {
            address: address.clone(),
            tx: Arc::new(tx.into()),
            rx: Arc::new(rx.into()),
        }
    }
}

#[async_trait::async_trait]
impl RequestResponseSawtoothChannel for ZmqRequestResponseSawtoothChannel {
    #[instrument(name = "receive_one", level = "debug", skip(self), ret(Debug))]
    async fn send_and_recv_one<RX: protobuf::Message, TX: protobuf::Message>(
        &self,
        tx: TX,
        message_type: Message_MessageType,
        timeout: std::time::Duration,
    ) -> Result<RX, SawtoothCommunicationError> {
        let correlation_id = uuid::Uuid::new_v4().to_string();
        let mut bytes = vec![];
        tx.write_to_vec(&mut bytes)?;

        let (tx, rx) = tokio::sync::oneshot::channel::<Result<_, SawtoothCommunicationError>>();

        let send_clone = self.tx.clone();
        Handle::current().spawn_blocking(move || {
            let future = send_clone
                .lock()
                .unwrap()
                .send(message_type, &correlation_id, &bytes);

            if let Err(e) = &future {
                error!(send_message=%correlation_id, error=%e);
                //tx.send(Err(SawtoothCommunicationError::from(e.clone())));
                return;
            }
            debug!(send_message=%correlation_id);
            trace!(body=?tx);

            tx.send(Ok(future.unwrap().get_timeout(timeout))).unwrap();
        });

        rx.await
            .unwrap()?
            .map_err(SawtoothCommunicationError::from)
            .and_then(|res| {
                RX::parse_from_bytes(&res.content).map_err(SawtoothCommunicationError::from)
            })
    }

    async fn recv_stream<RX: protobuf::Message>(
        self,
    ) -> Result<BoxStream<'static, RX>, SawtoothCommunicationError> {
        let channel = self;
        let stream = stream::unfold(channel, move |channel| async move {
            let response = tokio::task::spawn_blocking(move || {
                let response = channel
                    .rx
                    .lock()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(30));
                debug!(?response);
                (channel, response)
            })
            .await;
            match response {
                Ok((channel, Ok(Ok(response)))) => {
                    let response = RX::parse_from_bytes(&response.content)
                        .map_err(SawtoothCommunicationError::from);
                    if let Err(e) = &response {
                        error!(decode_message= ?e);
                        None
                    } else {
                        Some((response.unwrap(), channel))
                    }
                }
                Ok((self, Ok(Err(zmq)))) => {
                    error!(stream_error=%zmq);
                    None
                }
                Err(e) => {
                    error!(task_error=%e);
                    None
                }
                Ok((self, Err(_e))) => None,
            }
        });

        Ok(stream.boxed())
    }

    fn reconnect(&self) {
        let (tx, rx) = ZmqMessageConnection::new(self.address.as_str()).create();
        *self.tx.lock().unwrap() = tx;
        *self.rx.lock().unwrap() = rx;
    }
}
