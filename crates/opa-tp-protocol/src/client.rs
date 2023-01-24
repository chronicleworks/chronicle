use protobuf::ProtobufError;
use sawtooth_sdk::messages::validator::Message_MessageType;
use sawtooth_sdk::messaging::stream::{MessageConnection, MessageSender, ReceiveError, SendError};
use sawtooth_sdk::messaging::{
    stream::MessageReceiver,
    zmq_stream::{ZmqMessageConnection, ZmqMessageSender},
};
use thiserror::Error;
use tokio::runtime::Handle;
use tracing::{debug, instrument, trace};
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
}

#[async_trait::async_trait(?Send)]
pub trait RequestResponseSawtoothChannel {
    fn message_type() -> Message_MessageType;
    async fn receive_one<RX: protobuf::Message, TX: protobuf::Message>(
        &self,
        tx: &TX,
    ) -> Result<RX, SawtoothCommunicationError>;
}

pub struct ZmqRequestResponseSawtoothChannel {
    tx: ZmqMessageSender,
    _rx: MessageReceiver,
}

impl ZmqRequestResponseSawtoothChannel {
    pub fn new(address: &Url) -> Self {
        let (tx, _rx) = ZmqMessageConnection::new(address.as_str()).create();
        ZmqRequestResponseSawtoothChannel { tx, _rx }
    }
}

#[async_trait::async_trait(?Send)]
impl RequestResponseSawtoothChannel for ZmqRequestResponseSawtoothChannel {
    fn message_type() -> Message_MessageType {
        Message_MessageType::CLIENT_BATCH_SUBMIT_REQUEST
    }

    #[instrument(name = "receive_one", level = "info", skip(self), ret(Debug))]
    async fn receive_one<RX: protobuf::Message, TX: protobuf::Message>(
        &self,
        tx: &TX,
    ) -> Result<RX, SawtoothCommunicationError> {
        let correlation_id = uuid::Uuid::new_v4().to_string();
        let mut bytes = vec![];
        tx.write_to_vec(&mut bytes)?;
        let res = Handle::current().block_on(async move {
            let mut future = self
                .tx
                .send(Self::message_type(), &correlation_id, &bytes)?;

            debug!(send_message=%correlation_id);
            trace!(body=?tx);

            Ok(future.get_timeout(std::time::Duration::from_secs(10))?)
        });

        res.and_then(|res| {
            RX::parse_from_bytes(&res.content).map_err(SawtoothCommunicationError::from)
        })
    }
}
