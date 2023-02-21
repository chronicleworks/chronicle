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

#[async_trait::async_trait]
pub trait RequestResponseSawtoothChannel {
    // Send a message and wait for a response, decoding the response as a
    // protobuf message of type `TX`.
    async fn send_and_recv_one<RX: protobuf::Message, TX: protobuf::Message>(
        &self,
        tx: TX,
        message_type: Message_MessageType,
        timeout: std::time::Duration,
    ) -> Result<RX, SawtoothCommunicationError>;

    // Continue listening on this channel for messages, decoding them as a
    // protobuf message of type `RX`.
    async fn recv_stream<RX: protobuf::Message>(
        self,
    ) -> Result<BoxStream<'static, RX>, SawtoothCommunicationError>;

    fn reconnect(&self);
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
        let res = Handle::current().block_on(async move {
            let mut future = self
                .tx
                .lock()
                .unwrap()
                .send(message_type, &correlation_id, &bytes)?;

            debug!(send_message=%correlation_id);
            trace!(body=?tx);

            Ok(future.get_timeout(timeout)?)
        });

        res.and_then(|res| {
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
