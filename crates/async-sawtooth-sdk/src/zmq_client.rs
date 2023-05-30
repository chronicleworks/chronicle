use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use futures::{stream, stream::BoxStream, StreamExt};

use sawtooth_sdk::{
    messages::validator::Message_MessageType,
    messaging::{
        stream::{MessageConnection, MessageReceiver, MessageSender},
        zmq_stream::{ZmqMessageConnection, ZmqMessageSender},
    },
};

use prost::Message;

use tokio::{runtime::Handle, sync::oneshot::channel};
use tracing::{debug, error, instrument, trace};
use url::Url;

use crate::{error::SawtoothCommunicationError, messages::PingResponse};

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
    async fn send_and_recv_one<RX: protobuf::Message, TX: protobuf::Message + Clone>(
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

    fn close(&self) {}
}

#[derive(Clone)]
pub struct ReconnectingRequestResponseChannel<
    Inner: RequestResponseSawtoothChannel + Sized + Clone + Send + Sync,
>(Inner);

impl<Inner: RequestResponseSawtoothChannel + Sized + Clone + Send + Sync>
    ReconnectingRequestResponseChannel<Inner>
{
    pub fn new(inner: Inner) -> Self {
        Self(inner)
    }
}

#[async_trait::async_trait]
impl RequestResponseSawtoothChannel
    for ReconnectingRequestResponseChannel<ZmqRequestResponseSawtoothChannel>
{
    async fn send_and_recv_one<RX: protobuf::Message, TX: protobuf::Message + Clone>(
        &self,
        tx: TX,
        message_type: Message_MessageType,
        timeout: std::time::Duration,
    ) -> Result<RX, SawtoothCommunicationError> {
        loop {
            let res = self
                .0
                .send_and_recv_one(tx.clone(), message_type, timeout)
                .await;

            if res.is_ok() {
                return res;
            }

            if let Err(e) = res {
                debug!(zmq_send_error = ?e, "ZMQ send error, reconnecting");
            }

            self.0.reconnect();
        }
    }

    async fn recv_stream<RX: protobuf::Message>(
        self,
    ) -> Result<BoxStream<'static, RX>, SawtoothCommunicationError> {
        let chan = self.0.clone();
        loop {
            let res = chan.clone().recv_stream().await;

            if res.is_ok() {
                return res;
            }

            self.0.reconnect();
        }
    }

    fn close(&self) {
        self.0.close()
    }

    fn reconnect(&self) {
        self.0.reconnect()
    }
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

    pub fn retrying(self) -> ReconnectingRequestResponseChannel<Self> {
        ReconnectingRequestResponseChannel::new(self)
    }
}

#[async_trait::async_trait]
impl RequestResponseSawtoothChannel for ZmqRequestResponseSawtoothChannel {
    #[instrument(name = "receive_one", level = "trace", skip(self), ret(Debug))]
    async fn send_and_recv_one<RX: protobuf::Message, TX: protobuf::Message>(
        &self,
        tx: TX,
        message_type: Message_MessageType,
        timeout: std::time::Duration,
    ) -> Result<RX, SawtoothCommunicationError> {
        let correlation_id = uuid::Uuid::new_v4().to_string();
        let mut bytes = vec![];
        tx.write_to_vec(&mut bytes)?;

        let (tx, rx) = channel::<Result<_, SawtoothCommunicationError>>();

        let send_clone = self.tx.clone();
        Handle::current().spawn_blocking(move || {
            let future = send_clone
                .lock()
                .unwrap()
                .send(message_type, &correlation_id, &bytes);

            if let Err(e) = future {
                error!(send_message=%correlation_id, error=%e);
                tx.send(Err(SawtoothCommunicationError::from(e))).unwrap();
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
            loop {
                let channel = channel.clone();
                let response = tokio::task::spawn_blocking(move || {
                    let response = channel
                        .rx
                        .lock()
                        .unwrap()
                        .recv_timeout(Duration::from_secs(10));
                    debug!(have_response=?response);
                    (channel, response)
                })
                .await;
                match response {
                    Ok((channel, Ok(Ok(response))))
                        if response.message_type == Message_MessageType::PING_REQUEST =>
                    {
                        trace!(ping_request = ?response.correlation_id);
                        let ping_reply = PingResponse::default().encode_to_vec();
                        channel
                            .tx
                            .lock()
                            .unwrap()
                            .send(
                                Message_MessageType::PING_RESPONSE,
                                &response.correlation_id,
                                &ping_reply,
                            )
                            .map_err(|e| error!(send_ping_reply = ?e))
                            .ok();
                    }
                    Ok((channel, Ok(Ok(response)))) => {
                        let response = RX::parse_from_bytes(&response.content)
                            .map_err(SawtoothCommunicationError::from);
                        if let Err(e) = &response {
                            error!(decode_message= ?e);
                            break None;
                        } else {
                            break Some((response.unwrap(), channel));
                        }
                    }
                    Ok((self, Ok(Err(zmq)))) => {
                        error!(stream_error=%zmq);
                        break None;
                    }
                    Err(e) => {
                        error!(task_error=%e);
                        break None;
                    }
                    Ok((self, Err(e))) => {
                        trace!(poll_timeout = ?e);
                    }
                }
            }
        });

        Ok(stream.boxed())
    }

    fn reconnect(&self) {
        let (tx, rx) = ZmqMessageConnection::new(self.address.as_str()).create();
        *self.tx.lock().unwrap() = tx;
        *self.rx.lock().unwrap() = rx;
    }

    fn close(&self) {
        self.tx.lock().unwrap().close();
    }
}
