use std::{
    collections::HashMap,
    net::SocketAddr,
    num::NonZeroUsize,
    pin::Pin,
    sync::Arc,
    time::{Duration, Instant},
};

use futures::{
    select,
    stream::{BoxStream, FuturesUnordered, SplitSink},
    FutureExt, SinkExt, StreamExt,
};
use k256::sha2::{Digest, Sha256};
use lru::LruCache;

use crate::messages::{message::MessageType, EventList, PingResponse};
use pinvec::PinVec;
use pow_of_2::PowOf2;

use prost::Message;

use tmq::{dealer::Dealer, Context, Multipart};
use tokio::sync::{
    broadcast,
    oneshot::{self},
};
use tracing::{debug, error, info, info_span, instrument, trace, warn, Instrument};

use uuid::Uuid;

use crate::error::SawtoothCommunicationError;

#[derive(Debug, Clone, Copy)]
// Control routing behaviour over multiple connections
pub enum SendRouting {
    // Ensure this message is routed to all validators - the message will not
    // send unless all validators are connected. This should be used for subscriptions
    All,
    // Send the message to a single connected validator, determined by the
    // channel - this should be used for transactions
    Selective,
}

/// A trait representing a communication channel for sending request and receiving
/// response messages to/from the Sawtooth network.
///
/// This trait defines methods for sending a single message and waiting for a
/// response, and for subscribing to a stream of messages.
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
    async fn send_and_recv_one<RX: prost::Message + Default, TX: prost::Message>(
        &self,
        tx: &TX,
        send_message_type: MessageType,
        expect_message_type: MessageType,
        timeout: std::time::Duration,
    ) -> Result<RX, SawtoothCommunicationError>;

    /// Send a message and wait for responses from all connected validators, decoding the responses as a
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
    /// The response messages, decoded as a protobuf message of type RX.
    ///
    /// # Errors
    ///
    /// Returns an error if a send or receive operation fails.
    async fn send_and_recv_all<RX: prost::Message + Default, TX: prost::Message>(
        &self,
        tx: &TX,
        send_message_type: MessageType,
        expect_message_type: MessageType,
        timeout: std::time::Duration,
    ) -> Result<Vec<RX>, SawtoothCommunicationError>;

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
    async fn recv_stream<RX: prost::Message + Default>(
        &self,
        message_type: MessageType,
    ) -> Result<BoxStream<'static, RX>, SawtoothCommunicationError>;
}

#[derive(Clone)]
pub struct RetryingRequestResponseChannel<
    Inner: RequestResponseSawtoothChannel + Sized + Clone + Send + Sync,
>(Inner);

impl<Inner: RequestResponseSawtoothChannel + Sized + Clone + Send + Sync>
    RetryingRequestResponseChannel<Inner>
{
    pub fn new(inner: Inner) -> Self {
        Self(inner)
    }
}

#[async_trait::async_trait]
impl RequestResponseSawtoothChannel
    for RetryingRequestResponseChannel<ZmqRequestResponseSawtoothChannel>
{
    async fn send_and_recv_one<RX: prost::Message + Default, TX: prost::Message>(
        &self,
        tx: &TX,
        send_message_type: MessageType,
        expect_message_type: MessageType,
        timeout: std::time::Duration,
    ) -> Result<RX, SawtoothCommunicationError> {
        loop {
            let res = self
                .0
                .send_and_recv_one(tx, send_message_type, expect_message_type, timeout)
                .await;

            if res.is_ok() {
                return res;
            }

            if let Err(e) = res {
                debug!(zmq_send_error = ?e);
            }
        }
    }

    async fn send_and_recv_all<RX: prost::Message + Default, TX: prost::Message>(
        &self,
        tx: &TX,
        send_message_type: MessageType,
        expect_message_type: MessageType,
        timeout: std::time::Duration,
    ) -> Result<Vec<RX>, SawtoothCommunicationError> {
        loop {
            let res = self
                .0
                .send_and_recv_all(tx, send_message_type, expect_message_type, timeout)
                .await;

            if res.is_ok() {
                return res;
            }

            if let Err(e) = res {
                debug!(zmq_send_error = ?e, "ZMQ send error, reconnecting");
            }
        }
    }

    async fn recv_stream<RX: prost::Message + Default>(
        &self,
        message_type: MessageType,
    ) -> Result<BoxStream<'static, RX>, SawtoothCommunicationError> {
        // This operation cannot fail for network reasons, only due to stopped
        // tasks, so just forward it
        self.0.recv_stream(message_type).await
    }
}

type MessageAndType = (crate::messages::message::MessageType, Vec<u8>);
#[derive(Debug)]
struct SendMessageToOne {
    message_type: MessageType,
    message: Vec<u8>,
    reply_channel: oneshot::Sender<MessageAndType>,
}
#[derive(Debug)]
struct SendMessageToAll {
    message_type: MessageType,
    message: Vec<u8>,
    reply_channel: oneshot::Sender<Vec<MessageAndType>>,
}

#[derive(Debug)]
enum SocketCommand {
    // Start sending a message to a single validator on the Sawtooth network
    SendMessageToOne(SendMessageToOne),

    // Start sending a message to all validators on the Sawtooth network
    SendMessageToAll(SendMessageToAll),

    //Respond to the ping request on connection index
    PingResponse(usize, String),

    SetBlockHeight(usize, u64),
    // Ask for a broadcast channel for unsolicited messages
    Stream(
        (
            oneshot::Sender<broadcast::Receiver<Arc<MessageAndType>>>,
            MessageType,
        ),
    ),

    Shutdown,
}

// Routes incoming messages to the appropriate reply or broadcast channels and
// handles ping requests.
pub struct Router {
    // Reply channels are used for request/response messages that are sent to a
    // single validator chosen by the ValidatorSelector
    reply_channels: HashMap<String, (oneshot::Sender<MessageAndType>, Instant)>,
    // Broadcast channels are used for request/response messages that are broadcast to all
    // connected validators, such as subscriptions. All must respond before the
    // aggregated responses are sent to consumers
    #[allow(clippy::type_complexity)]
    broadcast_reply_channels: HashMap<
        String,
        (
            usize,
            Vec<(usize, MessageAndType)>,
            oneshot::Sender<Vec<MessageAndType>>,
            Instant,
        ),
    >,
    // Drop channels signal that a consumer is no longer waiting for a response
    broadcast: HashMap<MessageType, broadcast::Sender<Arc<MessageAndType>>>,
    command_channel: tokio::sync::mpsc::Sender<SocketCommand>,
    lru: LruCache<[u8; 32], ()>,
}

impl Router {
    fn new(reply: tokio::sync::mpsc::Sender<SocketCommand>) -> Self {
        Self {
            reply_channels: HashMap::new(),
            broadcast_reply_channels: HashMap::new(),
            broadcast: HashMap::new(),
            command_channel: reply,
            lru: LruCache::new(NonZeroUsize::new(1000).unwrap()),
        }
    }

    fn gc_timed_out_channels(&mut self, older_than: Duration) {
        let now = Instant::now();
        let mut to_remove = vec![];
        for (k, (_, t)) in self.reply_channels.iter() {
            if (t.elapsed() + older_than) < now.elapsed() {
                to_remove.push(k.clone())
            }
        }

        for k in to_remove {
            debug!(gc_reply_channel=%k);
            self.reply_channels.remove(&k);
        }

        let mut to_remove = vec![];
        for (k, (_, _, _, t)) in self.broadcast_reply_channels.iter() {
            if (t.elapsed() + older_than) < now.elapsed() {
                to_remove.push(k.clone())
            }
        }

        for k in to_remove {
            debug!(gc_broadcast_reply_channel=%k);
            self.broadcast_reply_channels.remove(&k);
        }
    }

    #[instrument(level = "debug", skip(self), fields(message_type = %message.message_type, correlation_id = %message.tx_id))]
    async fn route(&mut self, connection: usize, message: crate::messages::Message) {
        let message_type = crate::messages::message::MessageType::from_i32(message.message_type);
        if message_type.is_none() {
            error!(
                message_type = message.message_type,
                "Received message with invalid message type"
            );

            return;
        }
        let message_type = message_type.unwrap();

        debug!(routing_message=?message_type,correlation_id=?message.tx_id);

        // Handle pings
        if message_type == MessageType::PingRequest {
            debug!(?message_type);
            self.command_channel
                .send(SocketCommand::PingResponse(connection, message.tx_id))
                .await
                .ok();

            return;
        }
        // Peek incoming messages for ClientEvents / sawtooth block commit events, so
        // we can determine write priority based on highest block num
        if message_type == MessageType::ClientEvents {
            debug!(peek_client_events_for_block=?message_type);
            let events = EventList::decode(&*message.content)
                .map_err(|e| error!(failed_to_decode_events = ?e))
                .ok();
            if let Some(events) = events {
                for event in events.events {
                    debug!(peek_client_events_for_block=?event.event_type);
                    if event.event_type == "sawtooth/block-commit" {
                        if let Some(block_num) = event
                            .attributes
                            .iter()
                            .find(|a| a.key == "block_num")
                            .and_then(|a| a.value.parse().ok())
                        {
                            debug!(connection=?connection,block_num=?block_num);
                            self.command_channel
                                .send(SocketCommand::SetBlockHeight(connection, block_num))
                                .await
                                .ok();
                        }
                    }
                }
            }

            // Broadcast it if not duplicate and we have a registered type
            debug!(possibly_broadcast_message=?message_type);
            let mut message_hash = Sha256::new();
            message_hash.update(&message.content);
            let message_hash = message_hash.finalize();
            if self.lru.get(&*message_hash).is_none() {
                debug!(broadcast_message=%hex::encode(message_hash));
                self.lru.push(message_hash.try_into().unwrap(), ());
                //We still need to broadcast this message
                if let Some(broadcast) = self.broadcast.get(&message_type) {
                    broadcast
                        .send(Arc::new((message_type, message.content)))
                        .map_err(|e| warn!(failed_to_broadcast = ?e, "Failed to broadcast"))
                        .ok();
                } else {
                    warn!(no_broadcast_channel = ?message_type);
                }
            } else {
                debug!(ignore_duplicate=%hex::encode(message_hash));
            }

            return;
        }
        // Check if this is a send_and_receive_all message
        else if let Some((required_responses, mut responses, chan, instant)) =
            self.broadcast_reply_channels.remove(&message.tx_id)
        {
            responses.push((connection, (message_type, message.content)));
            if responses.len() == required_responses {
                let mut responses = responses.drain(..).collect::<Vec<_>>();
                responses.sort_by_key(|(i, _)| *i);
                let responses = responses.into_iter().map(|(_, x)| x).collect::<Vec<_>>();
                chan.send(responses)
                    .map_err(|e| warn!(failed_to_send_reply = ?e, "Failed to send reply"))
                    .ok();
            } else {
                self.broadcast_reply_channels.insert(
                    message.tx_id,
                    (required_responses, responses, chan, instant),
                );
            }
        }
        // Otherwise this is send_and_receive_one
        else {
            match self.reply_channels.remove(&message.tx_id) {
                // We have a reply channel for this correlation id, so send our
                // message to it. This may fail if a client has timed out
                Some((expected, _)) => {
                    debug!(reply_to_correlation_id=?message.tx_id);
                    expected
                        .send((message_type, message.content))
                        .map_err(|e| warn!(failed_to_send_reply = ?e, "Failed to send reply"))
                        .ok();
                }
                // We have a response message with a correlation id, but nothing is
                // currently waiting for it
                None => {
                    warn!(unroutable_correlation_id=%message.tx_id)
                }
            }
        }

        self.gc_timed_out_channels(Duration::from_secs(30));
    }

    // Create a new broadcast channel for the given message type if needed and
    // return a subscription to it
    fn subscribe(&mut self, message_type: MessageType) -> broadcast::Receiver<Arc<MessageAndType>> {
        self.broadcast
            .entry(message_type)
            .or_insert_with(|| {
                let (tx, _) = broadcast::channel(100);
                tx
            })
            .subscribe()
    }

    fn expect_reply(&mut self, chan: oneshot::Sender<MessageAndType>) -> Uuid {
        let correlation_id = Uuid::new_v4();
        self.reply_channels
            .insert(correlation_id.to_string(), (chan, Instant::now()));
        correlation_id
    }
    fn expect_n_replies(
        &mut self,
        num_validators: usize,
        chan: oneshot::Sender<Vec<MessageAndType>>,
    ) -> Uuid {
        let correlation_id = Uuid::new_v4();
        self.broadcast_reply_channels.insert(
            correlation_id.to_string(),
            (num_validators, vec![], chan, Instant::now()),
        );
        correlation_id
    }
}

pub struct ValidatorConnectionStatus {
    pub address: SocketAddr,
    pub connected: bool,
    pub last_block_index: Option<u64>,
}

impl ValidatorConnectionStatus {
    fn new(address: &SocketAddr) -> Self {
        Self {
            address: address.to_owned(),
            connected: false,
            last_block_index: None,
        }
    }
}
pub trait ValidatorSelector {
    fn select(&self, validator_status: &[ValidatorConnectionStatus]) -> Option<usize>;
}

pub struct HighestBlockValidatorSelector;

// Return the validator with the highest block index, or None if no validators are connected
impl ValidatorSelector for HighestBlockValidatorSelector {
    fn select(&self, validator_status: &[ValidatorConnectionStatus]) -> Option<usize> {
        validator_status
            .iter()
            .enumerate()
            .rev()
            .filter_map(|(i, x)| if x.connected { Some((i, x)) } else { None })
            .max_by(|(_, y), (_, x)| {
                y.last_block_index
                    .unwrap_or_default()
                    .cmp(&x.last_block_index.unwrap_or_default())
            })
            .map(|(i, _)| i)
    }
}

pin_project_lite::pin_project! {
pub struct ValidatorSinks<SELECTOR: ValidatorSelector> {
    validator_selector: SELECTOR,
    addresses: Vec<SocketAddr>,
    statuses: Vec<ValidatorConnectionStatus>,
    #[pin]
    sinks: PinVec<SplitSink<Dealer, Multipart>>
}
}

impl<SELECTOR: ValidatorSelector> ValidatorSinks<SELECTOR> {
    fn new(
        addresses: &[SocketAddr],
        sinks: Vec<SplitSink<Dealer, Multipart>>,
        validator_selector: SELECTOR,
    ) -> Self {
        let mut pinned_sinks = PinVec::new(PowOf2::<usize>::from_exp(3));
        for sink in sinks.into_iter() {
            pinned_sinks.push(sink);
        }
        Self {
            addresses: addresses.to_owned(),
            sinks: pinned_sinks,
            statuses: addresses
                .iter()
                .map(ValidatorConnectionStatus::new)
                .collect(),
            validator_selector,
        }
    }

    #[instrument(skip(self))]
    fn set_block_height(&mut self, connection_index: usize, block: u64) {
        if let Some(status) = self.statuses.get_mut(connection_index) {
            status.last_block_index = Some(block);
        }
    }

    fn connected(&mut self, connection_index: usize) {
        if let Some(status) = self.statuses.get_mut(connection_index) {
            status.connected = true;
        }
    }

    fn disconnected(&mut self, connection_index: usize) {
        if let Some(status) = self.statuses.get_mut(connection_index) {
            status.connected = false;
        }
    }

    // If we have at least one connected validator, use the selection strategy to pick one
    async fn send_to_selected<MSG: Into<Multipart>>(
        self: Pin<&mut Self>,
        buf: MSG,
    ) -> Result<(), SawtoothCommunicationError> {
        if self
            .statuses
            .iter()
            .filter(|status| status.connected)
            .count()
            == 0
        {
            return Err(SawtoothCommunicationError::NoConnectedValidators);
        }

        let mut write_sink_index = self.validator_selector.select(&self.statuses);

        if write_sink_index.is_none() {
            warn!("No validator returned by selection strategy, falling back to first validator");
            write_sink_index = Some(0);
        }

        let mut write_sink_index = write_sink_index.unwrap();

        if write_sink_index > self.sinks.len() {
            warn!("Invalid validator index selected, falling back to first validator");
            write_sink_index = 0;
        }

        if !self.statuses[write_sink_index].connected {
            return Err(SawtoothCommunicationError::SendingToDisconnectedValidator);
        }

        self.send_to(write_sink_index, buf.into()).await
    }

    pub async fn send_to_all<MSG: Into<Multipart>, F: Fn() -> MSG>(
        self: Pin<&mut Self>,
        buf: F,
    ) -> Result<(), SawtoothCommunicationError> {
        if self
            .statuses
            .iter()
            .filter(|status| status.connected)
            .count()
            != self.statuses.len()
        {
            return Err(SawtoothCommunicationError::SendingToDisconnectedValidator);
        }
        let sink_count = self.sinks.len();

        // Send to all in sequence
        let pin_sinks = self.project().sinks.get_mut();
        for write_sink_index in 0..sink_count {
            let mut write_sink = pin_sinks.get_mut(write_sink_index).unwrap();
            write_sink.send(buf().into()).await?;
        }

        Ok(())
    }

    async fn send_to(
        self: Pin<&mut Self>,
        write_sink_index: usize,
        buf: Multipart,
    ) -> Result<(), SawtoothCommunicationError> {
        debug!(use_validator_connection = %self.addresses[write_sink_index], "Sending message to validator");
        let pin_self = self.project();
        let mut write_sink = pin_self.sinks.get_mut().get_mut(write_sink_index).unwrap();

        Ok(write_sink.send(buf).await?)
    }
}

// Zmq request response channel implementation.
// This is a cloneable handle to an zmq request response channel.
#[derive(Clone)]
pub struct ZmqRequestResponseSawtoothChannel {
    inner: Arc<ZmqRequestResponseSawtoothChannelInternal>,
}

impl ZmqRequestResponseSawtoothChannel {
    pub fn new<SELECTOR: ValidatorSelector + Send + 'static>(
        id_prefix: &str,
        addresses: &[SocketAddr],
        selector: SELECTOR,
    ) -> Result<Self, SawtoothCommunicationError> {
        Ok(Self {
            inner: Arc::new(ZmqRequestResponseSawtoothChannelInternal::new(
                id_prefix, addresses, selector,
            )?),
        })
    }

    pub fn retrying(self) -> RetryingRequestResponseChannel<Self> {
        RetryingRequestResponseChannel::new(self)
    }
}

#[async_trait::async_trait]
impl RequestResponseSawtoothChannel for ZmqRequestResponseSawtoothChannel {
    async fn send_and_recv_one<RX: prost::Message + Default, TX: prost::Message>(
        &self,
        tx: &TX,
        send_message_type: MessageType,
        expect_message_type: MessageType,
        timeout: std::time::Duration,
    ) -> Result<RX, SawtoothCommunicationError> {
        self.inner
            .send_and_recv_one(tx, send_message_type, expect_message_type, timeout)
            .await
    }

    async fn send_and_recv_all<RX: prost::Message + Default, TX: prost::Message>(
        &self,
        tx: &TX,
        send_message_type: MessageType,
        expect_message_type: MessageType,
        timeout: std::time::Duration,
    ) -> Result<Vec<RX>, SawtoothCommunicationError> {
        self.inner
            .send_and_recv_all(tx, send_message_type, expect_message_type, timeout)
            .await
    }

    async fn recv_stream<RX: prost::Message + Default>(
        &self,
        message_type: MessageType,
    ) -> Result<BoxStream<'static, RX>, SawtoothCommunicationError> {
        self.inner.recv_stream(message_type).await
    }
}

// Internal implementation of the ZmqRequestResponseSawtoothChannel,
// deliberately not clone so that we can send disconnection on drop
pub struct ZmqRequestResponseSawtoothChannelInternal {
    tx: tokio::sync::mpsc::Sender<SocketCommand>,
}

impl ZmqRequestResponseSawtoothChannelInternal {
    pub fn new<SELECTOR: ValidatorSelector + Send + 'static>(
        id_prefix: &str,
        addresses: &[SocketAddr],
        selector: SELECTOR,
    ) -> Result<Self, SawtoothCommunicationError> {
        let (command_tx, mut command_rx) = tokio::sync::mpsc::channel(100);
        let return_command_tx = command_tx.clone();

        let identity = format!("{}/{}", id_prefix, Uuid::new_v4());

        let context = Context::new();

        // Start dealer sockets, and split them into sinks and streams as
        // our read / write pattern is not just request / reply
        let (validator_sinks, mut dealer_streams, mut monitors) = addresses
            .iter()
            .enumerate()
            .map(|(i, addr)| {
                let monitor_addr = format!("inproc://monitor-validator-{}-{}", id_prefix, i);
                let (sink, stream) = tmq::dealer(&context)
                    .set_identity(identity.as_bytes())
                    .monitor(
                        &monitor_addr,
                        zmq::SocketEvent::CONNECTED as i32 | zmq::SocketEvent::DISCONNECTED as i32,
                    )
                    .connect(&format!("tcp://{}", addr))?
                    .split();

                debug!(start_client = %addr);

                let monitor = tmq::pair(&context).connect(&monitor_addr)?;

                debug!(start_monitor_validator_pair = %monitor_addr);

                // Make sure we add the index to any received messages on
                // monitor or connection, as this is the simplest way to
                // determine which connection a message has come from
                Ok::<_, tmq::TmqError>((
                    sink,
                    stream.map(move |x| (i, x)),
                    monitor.map(move |x| (i, x)),
                ))
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .fold(
                (vec![], vec![], vec![]),
                |(mut sinks, mut streams, mut monitors), (sink, stream, monitor)| {
                    sinks.push(sink);
                    streams.push(stream);
                    monitors.push(monitor);
                    (sinks, streams, monitors)
                },
            );

        let addresses = addresses.to_vec();

        let span = info_span!("zmq_client_connection", addresses=?addresses);

        tokio::task::spawn(async move {
            let mut validator_sinks = ValidatorSinks::new(&addresses, validator_sinks, selector);
            debug!("connectivity task init");

            let mut router = Router::new(command_tx);

            debug!(init_zmq_client_task=?addresses);

            loop {
                let mut command_rx = command_rx.recv().into_stream().boxed().fuse();
                let mut dealer_rx = FuturesUnordered::new();
                for dealer in dealer_streams.iter_mut() {
                    dealer_rx.push(dealer.next());
                }
                let mut monitor_rx = FuturesUnordered::new();
                for monitor in monitors.iter_mut() {
                    monitor_rx.push(monitor.next());
                }
                select! {
                  dealer_rx = dealer_rx.select_next_some() => {
                    match dealer_rx {
                      Some((index, Ok(mut msg))) => {
                        debug!(incoming_message = index, ?msg);
                        if msg.is_empty() {
                          warn!(empty_multipart = ?msg);
                          continue;
                        }
                        let msg = msg.pop_front()
                          .ok_or_else(|| SawtoothCommunicationError::MalformedMessage)
                          .and_then(|x: tmq::Message| crate::messages::Message::decode(&*x).map_err(SawtoothCommunicationError::from));

                        if let Ok(msg) = msg {
                          router.route(index,msg).await;
                        } else {
                          warn!(invalid_message = ?msg);
                        }
                      },
                      Some((index, Err(e))) => {
                        error!(connection_index = index, ?e);
                      }
                      _ =>  {}
                    }
                  },
                  // Monitor event handling, unwrap is excusable here as these
                  // are according to the zmq specification
                  monitor = monitor_rx.select_next_some() => {
                    match monitor {
                          Some((index, Ok(mut msg))) => {
                              let event_part = msg.pop_front().unwrap();
                              let event_bytes: &[u8] = &event_part;

                              let event = u16::from_ne_bytes([event_bytes[0], event_bytes[1]]);
                              let address_part = msg.pop_front().unwrap();
                              let address = String::from_utf8(address_part.to_vec()).unwrap();

                              debug!(monitor_event = ?event, address = ?address);

                              match zmq::SocketEvent::from_raw(event) {
                                zmq::SocketEvent::DISCONNECTED => {
                                  info!(monitor_event_disconnected=%index);
                                  validator_sinks.disconnected(index);
                                },
                                zmq::SocketEvent::CONNECTED => {
                                  info!(monitor_event_connected=%index);
                                  validator_sinks.connected(index);
                                }
                                x => {
                                  warn!(invalid_zmq_event = ?(x as i32));
                                }
                              }
                          }
                          Some((index, Err(e))) => {
                            error!(connection_index = index, ?e);
                          }
                          None => {}
                    }
                  },
                  // Handle socket commands
                  command_rx = command_rx.select_next_some() => {
                    debug!(socket_command=?command_rx);
                    match command_rx {
                      Some(SocketCommand::SendMessageToOne(send_message)) => {
                        let correlation_id = router.expect_reply(send_message.reply_channel);
                        let validator_message = crate::messages::Message {
                          message_type: send_message.message_type as i32,
                          content: send_message.message,
                          tx_id: correlation_id.to_string(),
                        };

                        trace!(send=?validator_message);
                        let validator_message = validator_message.encode_to_vec();

                        let mut msg = tmq::Multipart::default();
                        msg.push_back(tmq::Message::from(&validator_message));

                        Pin::new(&mut validator_sinks).send_to_selected(msg).await.map_err(|e| error!(failed_to_send_message = ?e, "Failed to send message")).ok();
                      },
                      Some(SocketCommand::SendMessageToAll(send_message)) => {
                        let correlation_id = router.expect_n_replies(addresses.len(), send_message.reply_channel);
                        let validator_message = crate::messages::Message {
                          message_type: send_message.message_type as i32,
                          content: send_message.message,
                          tx_id: correlation_id.to_string(),
                        };

                        trace!(send=?validator_message);
                        let validator_message = validator_message.encode_to_vec();

                        Pin::new(&mut validator_sinks).send_to_all(move || {
                          let mut msg = tmq::Multipart::default();
                          msg.push_back(tmq::Message::from(&validator_message));
                          msg
                        }).await.map_err(|e| error!(failed_to_send_message = ?e, "Failed to send message")).ok();
                      },
                      Some(SocketCommand::Stream((send_stream, message_type))) => {
                        send_stream.send(router.subscribe(message_type))
                          .map_err(|e| warn!(failed_to_send_stream = ?e, "Failed to send broadcast stream"))
                          .ok();
                      },
                      Some(SocketCommand::SetBlockHeight(index,block_num)) => {
                        validator_sinks.set_block_height(index, block_num);
                      }
                      Some(SocketCommand::PingResponse(index,correlation_id)) => {
                        debug!(ping_respond=%index);

                        let validator_message = crate::messages::Message {
                          message_type: MessageType::PingResponse as _,
                          content: PingResponse{}.encode_to_vec(),
                          tx_id: correlation_id.to_string(),
                        };

                        let mut msg = tmq::Multipart::default();
                        msg.push_back(tmq::Message::from(&validator_message.encode_to_vec()));

                        Pin::new(&mut validator_sinks).send_to(index, msg).await.map_err(|e| error!(failed_to_send_message = ?e, "Failed to send message")).ok();
                      }
                      Some(SocketCommand::Shutdown) => {
                        info!("Shutting down");
                        break;
                      },

                      None => {}
                    }
                  }
                  complete => {
                      info!("exit zmq client");
                  }
                }
            }
        }.instrument(span));

        Ok(Self {
            tx: return_command_tx,
        })
    }

    fn close(&self) {
        self.tx.send(SocketCommand::Shutdown).now_or_never();
    }
}

impl Drop for ZmqRequestResponseSawtoothChannelInternal {
    fn drop(&mut self) {
        self.close()
    }
}

#[async_trait::async_trait]
impl RequestResponseSawtoothChannel for ZmqRequestResponseSawtoothChannelInternal {
    #[instrument(
        name = "send_and_receive_one",
        level = "trace",
        skip(self, tx),
        ret(Debug)
    )]
    async fn send_and_recv_one<RX: prost::Message + Default, TX: prost::Message>(
        &self,
        tx: &TX,
        send_message_type: MessageType,
        expect_message_type: MessageType,
        timeout: std::time::Duration,
    ) -> Result<RX, SawtoothCommunicationError> {
        // Create a oneshot to receive our reply, send it along with the message
        // and then use a tokio timeout to wait for the reply within the deadline
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();

        let buf = tx.encode_to_vec();

        self.tx
            .send(SocketCommand::SendMessageToOne(SendMessageToOne {
                message_type: send_message_type,
                message: buf,
                reply_channel: reply_tx,
            }))
            .await
            .map_err(|_| SawtoothCommunicationError::SendSocketCommand)?;

        let (received_message_type, recv_message) =
            tokio::time::timeout(timeout, reply_rx).await??;

        if expect_message_type as i32 != received_message_type as i32 {
            return Err(SawtoothCommunicationError::InvalidMessageType {
                expected: received_message_type,
                got: expect_message_type,
            });
        }

        Ok(RX::decode(&*recv_message)?)
    }

    #[instrument(
        name = "send_and_receive_all",
        level = "trace",
        skip(self, tx),
        ret(Debug)
    )]
    async fn send_and_recv_all<RX: prost::Message + Default, TX: prost::Message>(
        &self,
        tx: &TX,
        send_message_type: MessageType,
        expect_message_type: MessageType,
        timeout: std::time::Duration,
    ) -> Result<Vec<RX>, SawtoothCommunicationError> {
        // Create a oneshot to receive our reply, send it along with the message
        // and then use a tokio timeout to wait for the reply within the deadline
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();

        let buf = tx.encode_to_vec();

        self.tx
            .send(SocketCommand::SendMessageToAll(SendMessageToAll {
                message_type: send_message_type,
                message: buf,
                reply_channel: reply_tx,
            }))
            .await
            .map_err(|_| SawtoothCommunicationError::SendSocketCommand)?;

        let received = tokio::time::timeout(timeout, reply_rx).await??;

        for (received_message_type, _) in received.iter() {
            if expect_message_type as i32 != *received_message_type as i32 {
                return Err(SawtoothCommunicationError::InvalidMessageType {
                    expected: *received_message_type,
                    got: expect_message_type,
                });
            }
        }

        Ok(received
            .into_iter()
            .map(|(_, x)| RX::decode(&*x))
            .collect::<Result<Vec<_>, _>>()?)
    }

    async fn recv_stream<RX: prost::Message + Default>(
        &self,
        message_type: MessageType,
    ) -> Result<BoxStream<'static, RX>, SawtoothCommunicationError> {
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.tx
            .send(SocketCommand::Stream((tx, message_type)))
            .await
            .map_err(|_| SawtoothCommunicationError::SendSocketCommand)?;

        // Convert lag into stream termination
        Ok(tokio_stream::wrappers::BroadcastStream::new(rx.await?)
            .filter_map(|msg| async move {
                match msg {
                    Err(e) => {
                        error!(error = ?e, "Error receiving message");
                        None
                    }
                    Ok(message) => RX::decode(&*message.1)
                        .map_err(|e| error!(error = ?e, "Error parsing message"))
                        .ok(),
                }
            })
            .boxed())
    }
}

#[cfg(test)]
mod test {
    use std::{
        collections::VecDeque,
        net::{Ipv4Addr, SocketAddr},
        sync::{Arc, Mutex},
        time::Duration,
    };

    use futures::{FutureExt, SinkExt, StreamExt};
    use prost::Message;
    use tmq::{router, Multipart};
    use tokio::select;
    use tokio_stream::wrappers::ReceiverStream;
    use tracing::{debug, error, info_span, Instrument};
    use zmq::Context;

    use crate::{
        messages::{
            message::MessageType,
            ClientBlockGetByNumRequest, ClientBlockGetResponse, EventList, PingRequest, {self},
        },
        zmq_client::{
            HighestBlockValidatorSelector, RequestResponseSawtoothChannel,
            ZmqRequestResponseSawtoothChannel,
        },
    };

    use super::{ValidatorConnectionStatus, ValidatorSelector};

    #[test]
    pub fn highest_block_validator_selector() {
        let validator = HighestBlockValidatorSelector;

        //No connection - no selection
        assert_eq!(
            validator.select(&[ValidatorConnectionStatus {
                address: "127.0.0.1:1".parse().unwrap(),
                connected: false,
                last_block_index: None,
            }]),
            None,
        );

        //Single connection - return it
        assert_eq!(
            validator.select(&[ValidatorConnectionStatus {
                address: "127.0.0.1:1".parse().unwrap(),
                connected: true,
                last_block_index: None,
            }]),
            Some(0),
        );

        //Multiple connection with no other discriminator - return first
        assert_eq!(
            validator.select(&[
                ValidatorConnectionStatus {
                    address: "127.0.0.1:1".parse().unwrap(),
                    connected: true,
                    last_block_index: None,
                },
                ValidatorConnectionStatus {
                    address: "127.0.0.1:2".parse().unwrap(),
                    connected: true,
                    last_block_index: None,
                }
            ]),
            Some(0),
        );

        //Multiple connection with no other discriminator - return first
        assert_eq!(
            validator.select(&[
                ValidatorConnectionStatus {
                    address: "127.0.0.1:1".parse().unwrap(),
                    connected: true,
                    last_block_index: Some(0),
                },
                ValidatorConnectionStatus {
                    address: "127.0.0.1:2".parse().unwrap(),
                    connected: true,
                    last_block_index: Some(0),
                }
            ]),
            Some(0),
        );

        //Multiple connection with a higher block - return connection with
        //highest block
        assert_eq!(
            validator.select(&[
                ValidatorConnectionStatus {
                    address: "127.0.0.1:1".parse().unwrap(),
                    connected: true,
                    last_block_index: Some(0),
                },
                ValidatorConnectionStatus {
                    address: "127.0.0.1:1".parse().unwrap(),
                    connected: true,
                    last_block_index: Some(2),
                }
            ]),
            Some(1),
        );
    }

    struct TestRouter {
        tx: tokio::sync::mpsc::Sender<(MessageType, Vec<u8>)>,
        #[allow(clippy::type_complexity)]
        requests: Arc<Mutex<Vec<(MessageType, Vec<u8>)>>>,
    }

    impl TestRouter {
        pub fn new(responses: Vec<(MessageType, Vec<u8>)>) -> (SocketAddr, Self) {
            chronicle_telemetry::telemetry(None, chronicle_telemetry::ConsoleLogging::Pretty);
            let mut responses: VecDeque<_> = responses.into_iter().collect();
            let listen_port = portpicker::pick_unused_port().expect("No ports free");
            let listen_addr = SocketAddr::new(Ipv4Addr::new(127, 0, 0, 1).into(), listen_port);
            let connect_addr = SocketAddr::new(Ipv4Addr::new(127, 0, 0, 1).into(), listen_port);
            let (tx, rx) = tokio::sync::mpsc::channel::<(MessageType, Vec<u8>)>(1);
            let requests = Arc::new(Mutex::new(vec![]));
            let requests_clone = requests.clone();

            let span = info_span!("test_router", address=%listen_addr);
            tokio::task::spawn(async move {
                let mut unsolicited_rx = ReceiverStream::new(rx);
                let context = Context::new();
                let monitor_addr = format!("inproc://monitor-test-{}", listen_port);
                let (mut router_tx, mut router_rx) = router(&context)
                    .monitor(&monitor_addr, zmq::SocketEvent::CONNECTED as i32)
                    .bind(&format!("tcp://{}", listen_addr))
                    .map_err(|e| error!("{}", e))
                    .unwrap()
                    .split();

                debug!(listen_addr = ?listen_addr, "Test router listening");
                let mut last_client_address = vec![];

                loop {
                    select! {
                        // Connection events, use this to set the last client address
                        // Simulated unsolicited messages from the validator
                        unsolicited_message = unsolicited_rx.next().fuse() => {
                          if unsolicited_message.is_none() {
                            continue;
                          }
                          let unsolicited_message = unsolicited_message.unwrap();
                          debug!(unsolicited_message = ?unsolicited_message);
                          let message_wrapper =  messages::Message {
                            message_type: unsolicited_message.0 as i32,
                            tx_id: "".to_string(),
                            content: unsolicited_message.1
                          };
                          let mut multipart = Multipart::default();
                          multipart.push_back(last_client_address.clone().into());
                          multipart.push_back(tmq::Message::from(prost::Message::encode_to_vec(&message_wrapper)));
                          router_tx.send(multipart).await.map_err(|e| error!("{}",e)).ok();

                        },
                        // Incoming messages from the dealer
                        message = router_rx.next().fuse() => {
                          if message.is_none() {
                            break;
                          }

                          let multipart = message.unwrap().unwrap();
                          debug!(multipart = ?multipart);
                          last_client_address = multipart[0].to_vec();
                          let request = messages::Message::decode(&*multipart[1].to_vec()).map_err(|e| error!(%e)).unwrap();
                          debug!(request = ?request);
                          if MessageType::from_i32(request.message_type).unwrap() != MessageType::PingResponse {
                            requests_clone.lock().unwrap().push((MessageType::from_i32(request.message_type).unwrap(),request.content.clone()));
                            let mut multipart = Multipart::default();
                            multipart.push_back(last_client_address.clone().into());

                            let (message_type,message_body) = responses.pop_front().unwrap();
                            let response_message = messages::Message {
                              tx_id: request.tx_id,
                              message_type: message_type as i32,
                              content: message_body,
                            }.encode_to_vec();

                            multipart.push_back(tmq::Message::from(&response_message));

                            debug!(response = ?multipart);
                            router_tx.send(multipart).await.ok();
                          }

                        },
                    }
                }
            }.instrument(span));

            let router = TestRouter { tx, requests };

            (connect_addr, router)
        }
    }

    #[tokio::test]
    pub async fn send_and_receive_one() {
        let (addr, _stub) = TestRouter::new(vec![(
            MessageType::ClientBlockGetResponse,
            ClientBlockGetResponse {
                status: 0,
                block: Some(messages::Block::default()),
            }
            .encode_to_vec(),
        )]);
        let connection =
            ZmqRequestResponseSawtoothChannel::new("test", &[addr], HighestBlockValidatorSelector)
                .unwrap();

        tokio::time::sleep(Duration::from_millis(1000)).await;

        let res: ClientBlockGetResponse = connection
            .send_and_recv_one(
                &ClientBlockGetByNumRequest { block_num: 1 },
                MessageType::ClientBlockGetByNumRequest,
                MessageType::ClientBlockGetResponse,
                Duration::from_secs(1),
            )
            .await
            .unwrap();

        assert_eq!(res.status, 0);
        assert!(res.block.is_some());
    }

    #[tokio::test]
    pub async fn recv_stream_unique() {
        let (addr, stub) = TestRouter::new(vec![(
            MessageType::ClientBlockGetResponse,
            ClientBlockGetResponse {
                status: 0,
                block: None,
            }
            .encode_to_vec(),
        )]);
        let connection =
            ZmqRequestResponseSawtoothChannel::new("test", &[addr], HighestBlockValidatorSelector)
                .unwrap();

        tokio::time::sleep(Duration::from_millis(1000)).await;

        let _res: ClientBlockGetResponse = connection
            .send_and_recv_one(
                &ClientBlockGetByNumRequest { block_num: 1 },
                MessageType::ClientBlockGetByNumRequest,
                MessageType::ClientBlockGetResponse,
                Duration::from_secs(1),
            )
            .await
            .unwrap();

        let stream = connection
            .recv_stream::<EventList>(MessageType::ClientEvents)
            .await
            .unwrap();

        tokio::task::spawn(async move {
            for i in 0..10 {
                stub.tx
                    .send((
                        MessageType::ClientEvents,
                        EventList {
                            events: vec![crate::messages::Event {
                                event_type: "test/test".into(),
                                attributes: vec![crate::messages::event::Attribute {
                                    key: "attr".into(),
                                    value: format!("{}", i),
                                }],
                                data: vec![],
                            }],
                        }
                        .encode_to_vec(),
                    ))
                    .await
                    .unwrap();
            }
        });

        let res: Vec<_> = stream.take(10).collect::<Vec<EventList>>().await;
        // We should have received 10 async messages
        assert_eq!(res.len(), 10);
    }

    #[tokio::test]
    pub async fn recv_stream_de_duplicates() {
        let (addr, stub) = TestRouter::new(vec![(
            MessageType::ClientBlockGetResponse,
            ClientBlockGetResponse {
                status: 0,
                block: None,
            }
            .encode_to_vec(),
        )]);
        let connection =
            ZmqRequestResponseSawtoothChannel::new("test", &[addr], HighestBlockValidatorSelector)
                .unwrap();

        tokio::time::sleep(Duration::from_millis(1000)).await;

        let _res: ClientBlockGetResponse = connection
            .send_and_recv_one(
                &ClientBlockGetByNumRequest { block_num: 1 },
                MessageType::ClientBlockGetByNumRequest,
                MessageType::ClientBlockGetResponse,
                Duration::from_secs(1),
            )
            .await
            .unwrap();

        let res = connection
            .recv_stream::<EventList>(MessageType::ClientEvents)
            .await
            .unwrap();

        // We only care about the bytewise interpretation of messages in this
        // test, this is not a viable ClientBlockGetResponse but will be enough
        // to pass de-duplication logic
        tokio::task::spawn(async move {
            stub.tx
                .send((MessageType::PingRequest, PingRequest {}.encode_to_vec()))
                .await
                .unwrap();
            for i in 0..10 {
                stub.tx
                    .send((
                        MessageType::ClientEvents,
                        EventList {
                            events: vec![crate::messages::Event {
                                event_type: "test/test".into(),
                                attributes: vec![crate::messages::event::Attribute {
                                    key: "attr".into(),
                                    value: format!("{}", i % 2),
                                }],
                                data: vec![],
                            }],
                        }
                        .encode_to_vec(),
                    ))
                    .await
                    .unwrap();
            }
        });

        let res: Vec<_> = res.take(2).collect::<Vec<EventList>>().await;
        // We should have received 2 async messages
        assert_eq!(res.len(), 2);
    }

    #[tokio::test]
    pub async fn send_defaults_to_first_connection_when_multiple_but_block_height_determines_active_write_connection(
    ) {
        let (addr1, stub1) = TestRouter::new(vec![
            (
                MessageType::ClientBlockGetResponse,
                ClientBlockGetResponse {
                    status: 0,
                    block: Some(messages::Block::default()),
                }
                .encode_to_vec(),
            ),
            (
                MessageType::ClientBlockGetResponse,
                ClientBlockGetResponse {
                    status: 0,
                    block: Some(messages::Block::default()),
                }
                .encode_to_vec(),
            ),
        ]);

        let (addr2, stub2) = TestRouter::new(vec![
            (
                MessageType::ClientBlockGetResponse,
                ClientBlockGetResponse {
                    status: 0,
                    block: Some(messages::Block::default()),
                }
                .encode_to_vec(),
            ),
            (
                MessageType::ClientBlockGetResponse,
                ClientBlockGetResponse {
                    status: 0,
                    block: Some(messages::Block::default()),
                }
                .encode_to_vec(),
            ),
        ]);

        let connection = ZmqRequestResponseSawtoothChannel::new(
            "test",
            &[addr1, addr2],
            HighestBlockValidatorSelector,
        )
        .unwrap();

        tokio::time::sleep(Duration::from_millis(1000)).await;

        // Send an initial message that substitutes as a subscription request
        let res: Vec<ClientBlockGetResponse> = connection
            .send_and_recv_all(
                &ClientBlockGetByNumRequest { block_num: 1 },
                MessageType::ClientBlockGetByNumRequest,
                MessageType::ClientBlockGetResponse,
                Duration::from_secs(1),
            )
            .await
            .unwrap();

        let _stream = connection
            .recv_stream::<crate::messages::EventList>(MessageType::ClientEvents)
            .await
            .unwrap();

        // Set up initial block heights
        stub1
            .tx
            .send((
                MessageType::ClientEvents,
                EventList {
                    events: vec![crate::messages::Event {
                        event_type: "sawtooth/block-commit".into(),
                        attributes: vec![crate::messages::event::Attribute {
                            key: "block_num".into(),
                            value: "0".into(),
                        }],
                        data: vec![],
                    }],
                }
                .encode_to_vec(),
            ))
            .await
            .unwrap();

        stub2
            .tx
            .send((
                MessageType::ClientEvents,
                EventList {
                    events: vec![crate::messages::Event {
                        event_type: "sawtooth/block-commit".into(),
                        attributes: vec![crate::messages::event::Attribute {
                            key: "block_num".into(),
                            value: "0".into(),
                        }],
                        data: vec![],
                    }],
                }
                .encode_to_vec(),
            ))
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(1000)).await;
        assert_eq!(res.len(), 2);

        // Requests should have been routed to both stubs
        assert_eq!(stub2.requests.lock().unwrap().len(), 1);
        assert_eq!(stub1.requests.lock().unwrap().len(), 1);

        // Set block height on both connections, but higher on the second
        stub2
            .tx
            .send((
                MessageType::ClientEvents,
                EventList {
                    events: vec![crate::messages::Event {
                        event_type: "sawtooth/block-commit".into(),
                        attributes: vec![crate::messages::event::Attribute {
                            key: "block_num".into(),
                            value: "2".into(),
                        }],
                        data: vec![],
                    }],
                }
                .encode_to_vec(),
            ))
            .await
            .unwrap();

        stub1
            .tx
            .send((
                MessageType::ClientEvents,
                EventList {
                    events: vec![crate::messages::Event {
                        event_type: "sawtooth/block-commit".into(),
                        attributes: vec![crate::messages::event::Attribute {
                            key: "block_num".into(),
                            value: "1".into(),
                        }],
                        data: vec![],
                    }],
                }
                .encode_to_vec(),
            ))
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(1000)).await;

        let res: ClientBlockGetResponse = connection
            .send_and_recv_one(
                &ClientBlockGetByNumRequest { block_num: 1 },
                MessageType::ClientBlockGetByNumRequest,
                MessageType::ClientBlockGetResponse,
                Duration::from_secs(1),
            )
            .await
            .unwrap();

        assert_eq!(res.status, 0);
        assert!(res.block.is_some());

        // The second stub should have received the send_and_receive_one
        assert_eq!(stub2.requests.lock().unwrap().len(), 2);
        assert_eq!(stub1.requests.lock().unwrap().len(), 1);
    }
}
