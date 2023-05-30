use std::{
    convert::Infallible,
    sync::{Arc, Mutex},
    time::Duration,
};

use derivative::Derivative;
use futures::{
    future::{self},
    stream::{self, BoxStream},
    StreamExt,
};

use k256::ecdsa::SigningKey;
use protobuf::Message;
use sawtooth_sdk::messages::{
    batch::Batch,
    block::{Block, BlockHeader},
    client_batch_submit::{
        ClientBatchSubmitRequest, ClientBatchSubmitResponse, ClientBatchSubmitResponse_Status,
    },
    client_block::{
        ClientBlockGetResponse, ClientBlockGetResponse_Status, ClientBlockListResponse,
        ClientBlockListResponse_Status,
    },
    client_event::{ClientEventsSubscribeResponse, ClientEventsSubscribeResponse_Status},
    client_state::{ClientStateGetResponse, ClientStateGetResponse_Status},
    events::EventList,
    transaction::Transaction,
    validator::Message_MessageType,
};
use serde::{Deserialize, Serialize};

use thiserror::Error;
use tracing::{debug, error, info, instrument, trace, warn};

use crate::{
    error::SawtoothCommunicationError, sawtooth::MessageBuilder,
    zmq_client::RequestResponseSawtoothChannel,
};

#[derive(Debug, Error)]
pub enum BlockIdError {
    #[error("Not hex")]
    Hex(#[from] hex::FromHexError),

    #[error("Not 32 bytes")]
    Size(#[from] std::array::TryFromSliceError),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlockId {
    Marker, //Block ids can be null, empty string etc
    Block([u8; 64]),
}

impl TryFrom<String> for BlockId {
    type Error = Infallible;

    /// Sawtooth uses a special marker value for the first block, which is not
    /// parsable as a hash - 8 hex zero bytes.
    ///
    /// # Examples
    ///
    /// Correct string to BlockId conversion:
    ///
    /// ```
    /// use std::convert::TryInto;
    /// use async_sawtooth_sdk::ledger::BlockId;
    ///
    /// let s = "abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234".to_string();
    /// let block_id: BlockId = s.try_into().unwrap();
    /// match block_id {
    ///     BlockId::Block(_) => println!("Correct conversion"),
    ///     _ => panic!("Incorrect conversion"),
    /// }
    /// ```
    ///
    /// Handling special marker case:
    ///
    /// ```
    /// use std::convert::TryInto;
    /// use async_sawtooth_sdk::ledger::BlockId;
    ///
    /// let s = "00000000000000000000".to_string();
    /// let block_id: BlockId = s.try_into().unwrap();
    /// match block_id {
    ///     BlockId::Marker => println!("Correct conversion to Marker"),
    ///     _ => panic!("Incorrect conversion, expected Marker"),
    /// }
    /// ```
    #[instrument(level = "trace", ret(Debug))]
    fn try_from(s: String) -> Result<BlockId, Infallible> {
        let res = hex::decode(&s).map_err(BlockIdError::from).and_then(|x| {
            x.as_slice()
                .try_into()
                .map_err(BlockIdError::from)
                .map(BlockId::Block)
        });
        if let Err(e) = res {
            warn!(try_parse_block_id=%s,parse_block_error=?e);
            Ok(BlockId::Marker)
        } else {
            Ok(res.unwrap())
        }
    }
}

impl std::fmt::Display for BlockId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlockId::Marker => write!(f, "Marker"),
            BlockId::Block(bytes) => write!(f, "{}", hex::encode(bytes)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TransactionId(String);

impl TransactionId {
    pub fn new(tx_id: String) -> Self {
        Self(tx_id)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for TransactionId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl std::fmt::Display for TransactionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Position(u64);

impl From<u64> for Position {
    fn from(height: u64) -> Self {
        Position(height)
    }
}

impl PartialOrd for Position {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        let (Position(x), Position(y)) = (self, other);
        x.partial_cmp(y)
    }
}

impl Position {
    pub fn new(height: u64) -> Self {
        Position(height)
    }

    pub fn map<T, F>(&self, f: F) -> T
    where
        F: FnOnce(&u64) -> T,
    {
        f(&self.0)
    }

    pub fn distance(&self, other: &Self) -> u64 {
        let (Position(x), Position(y)) = (self, other);
        x.saturating_sub(*y)
    }
}

impl std::fmt::Display for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Position(x) => f.write_str(&format!("{}", x)),
        }
    }
}

// An application specific ledger event with its corresponding transaction id,
// block height and trace span
pub type LedgerEventContext<Event> = (Event, TransactionId, BlockId, Position, u64);

#[async_trait::async_trait]
pub trait LedgerEvent {
    async fn deserialize(buf: &[u8]) -> Result<(Self, u64), SawtoothCommunicationError>
    where
        Self: Sized;
}

#[async_trait::async_trait]
impl LedgerEvent for () {
    async fn deserialize(_buf: &[u8]) -> Result<(Self, u64), SawtoothCommunicationError> {
        Ok(((), 0))
    }
}

#[async_trait::async_trait]
pub trait LedgerTransaction {
    fn signer(&self) -> &SigningKey;
    fn addresses(&self) -> Vec<String>;
    async fn as_sawtooth_tx(
        &self,
        message_builder: &MessageBuilder,
    ) -> (Transaction, TransactionId);
}

#[async_trait::async_trait]
pub trait LedgerWriter {
    type Error: std::error::Error;
    type Transaction: LedgerTransaction;

    // Pre-submit is used to get the transaction id before submitting the transaction
    async fn pre_submit(
        &self,
        tx: &Self::Transaction,
    ) -> Result<(TransactionId, Transaction), Self::Error>;

    // Submit is used to submit a transaction to the ledger
    async fn submit(&self, tx: Transaction, signer: &SigningKey) -> Result<(), Self::Error>;

    fn message_builder(&self) -> &MessageBuilder;
}

pub struct BlockingLedgerReader<R>
where
    R: LedgerReader + Send,
{
    reader: R,
}

#[derive(Debug, Clone, Copy)]
pub enum FromBlock {
    // Do not attempt to catch up, start from the current head
    Head,
    // Discover the first useful block and start from there
    First,
    // Start from the given block
    BlockId(BlockId),
}

impl<R> BlockingLedgerReader<R>
where
    R: LedgerReader + Send,
{
    pub fn new(reader: R) -> Self {
        Self { reader }
    }

    pub fn get_state_entry(&self, address: &str) -> Result<Vec<u8>, R::Error> {
        tokio::runtime::Handle::current().block_on(self.reader.get_state_entry(address))
    }

    pub fn block_height(&self) -> Result<(Position, BlockId), R::Error> {
        tokio::runtime::Handle::current().block_on(self.reader.block_height())
    }

    pub fn state_updates(
        &self,
        event_type: &str,
        from_block: FromBlock,
        number_of_blocks: Option<u64>,
    ) -> Result<BoxStream<LedgerEventContext<R::Event>>, R::Error> {
        tokio::runtime::Handle::current().block_on(self.reader.state_updates(
            event_type,
            from_block,
            number_of_blocks,
        ))
    }
}

pub struct BlockingLedgerWriter<W>
where
    W: LedgerWriter + Send,
{
    writer: W,
}

impl<W> BlockingLedgerWriter<W>
where
    W: LedgerWriter + Send,
{
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    pub fn pre_submit(
        &self,
        tx: &W::Transaction,
    ) -> Result<(TransactionId, Transaction), W::Error> {
        tokio::runtime::Handle::current().block_on(self.writer.pre_submit(tx))
    }

    pub fn submit(&self, tx: Transaction, signer: &SigningKey) -> Result<(), W::Error> {
        tokio::runtime::Handle::current().block_on(self.writer.submit(tx, signer))
    }

    pub fn do_submit(
        &self,
        tx: &W::Transaction,
        signer: &SigningKey,
    ) -> Result<TransactionId, (Option<TransactionId>, W::Error)> {
        let (tx_id, tx) = self.pre_submit(tx).map_err(|e| (None, e))?;
        self.submit(tx, signer)
            .map_err(|e| (Some(tx_id.clone()), e))?;
        Ok(tx_id)
    }
}

#[async_trait::async_trait]
pub trait LedgerReader {
    type Event: LedgerEvent;
    type Error: std::error::Error;
    /// Get the state entry at `address`
    async fn get_state_entry(&self, address: &str) -> Result<Vec<u8>, Self::Error>;
    // Get the block height of the ledger, and the id of the highest block
    async fn block_height(&self) -> Result<(Position, BlockId), Self::Error>;
    /// Subscribe to state updates from this ledger, starting at `offset`, and
    /// ending the stream after `number_of_blocks` blocks have been processed.
    async fn state_updates(
        &self,
        // The application event type to subscribe to
        event_type: &str,
        // The block to start from
        from_block: FromBlock,
        // The number of blocks to process before ending the stream
        number_of_blocks: Option<u64>,
    ) -> Result<BoxStream<LedgerEventContext<Self::Event>>, Self::Error>;

    async fn reconnect(&self) {}
}

#[derive(Derivative)]
#[derivative(Debug, Clone)]
pub struct SawtoothLedger<
    Channel: RequestResponseSawtoothChannel + Clone + Send + Sync,
    LedgerEvent,
    Transaction,
> where
    LedgerEvent: std::fmt::Debug,
{
    builder: MessageBuilder,
    channel: Channel,
    last_seen_block: Arc<Mutex<Option<(BlockId, Position)>>>,
    _e: std::marker::PhantomData<LedgerEvent>,
    _t: std::marker::PhantomData<Transaction>,
}

impl<
        Channel: RequestResponseSawtoothChannel + Clone + Send + Sync,
        Event: LedgerEvent + Send + Sync + std::fmt::Debug,
        Transaction: LedgerTransaction + Send + Sync,
    > SawtoothLedger<Channel, Event, Transaction>
{
    pub fn new(channel: Channel, family: &str, version: &str) -> Self {
        let builder = MessageBuilder::new(family, version);
        SawtoothLedger {
            builder,
            channel,
            last_seen_block: Default::default(),
            _e: std::marker::PhantomData::default(),
            _t: std::marker::PhantomData::default(),
        }
    }

    #[instrument(skip(self), level = "trace")]
    async fn submit_batch(&self, batch: Batch) -> Result<(), SawtoothCommunicationError> {
        let request = ClientBatchSubmitRequest {
            batches: vec![batch].into(),
            ..Default::default()
        };
        let batch_response: ClientBatchSubmitResponse = self
            .channel
            .send_and_recv_one(
                request,
                sawtooth_sdk::messages::validator::Message_MessageType::CLIENT_BATCH_SUBMIT_REQUEST,
                std::time::Duration::from_secs(10),
            )
            .await?;
        if batch_response.status == ClientBatchSubmitResponse_Status::OK {
            Ok(())
        } else {
            Err(SawtoothCommunicationError::UnexpectedStatus {
                status: batch_response.status as i32,
            })
        }
    }

    #[instrument(skip(self))]
    async fn get_block_height(&self) -> Result<(Position, BlockId), SawtoothCommunicationError> {
        let request = self.builder.make_block_height_request();
        let response: ClientBlockListResponse = self
            .channel
            .send_and_recv_one(
                request,
                sawtooth_sdk::messages::validator::Message_MessageType::CLIENT_BLOCK_LIST_REQUEST,
                std::time::Duration::from_secs(10),
            )
            .await?;
        if response.status == ClientBlockListResponse_Status::OK {
            let block = response
                .get_blocks()
                .first()
                .ok_or(SawtoothCommunicationError::NoBlocksReturned)?;

            let header = BlockHeader::parse_from_bytes(&block.header)?;
            Ok((
                header.block_num.into(),
                block.header_signature.clone().try_into()?,
            ))
        } else {
            Err(SawtoothCommunicationError::UnexpectedStatus {
                status: response.status as i32,
            })
        }
    }

    #[instrument(level = "info", skip(self))]
    // Get the first usable block id from sawtooth
    async fn get_first_block(&self) -> Result<BlockId, SawtoothCommunicationError> {
        let block: Block = loop {
            let req = self.builder.get_first_block_id_request();

            let response: Result<ClientBlockGetResponse, _> = self
                .channel
                .send_and_recv_one(
                    req,
                    Message_MessageType::CLIENT_BLOCK_GET_BY_NUM_REQUEST,
                    Duration::from_secs(10),
                )
                .await;

            if let Ok(response) = response {
                trace!(block_by_num_response = ?response);
                match (response.status, response.block.into_option()) {
                    (ClientBlockGetResponse_Status::OK, Some(block)) => break block,
                    (e, _) => {
                        error!(head_block_status = ?e)
                    }
                };
            }

            tokio::time::sleep(Duration::from_secs(2)).await;
        };

        Ok(BlockId::try_from(block.header_signature)?)
    }

    #[instrument(skip(self))]
    async fn get_events_from(
        &self,
        event_type: &str,
        from_block: BlockId,
    ) -> Result<BoxStream<LedgerEventContext<Event>>, SawtoothCommunicationError> {
        let subscription_request = self
            .builder
            .make_subscription_request(&from_block, vec![event_type.into()]);
        debug!(subscription_request = ?subscription_request);
        let sub = self
            .channel
            .send_and_recv_one::<ClientEventsSubscribeResponse, _>(
                subscription_request,
                Message_MessageType::CLIENT_EVENTS_SUBSCRIBE_REQUEST,
                Duration::from_secs(10),
            )
            .await?;

        if sub.status != ClientEventsSubscribeResponse_Status::OK {
            return Err(SawtoothCommunicationError::SubscribeError {
                code: sub.status as i32,
            });
        }

        self.event_stream(event_type, from_block).await
    }

    async fn event_stream(
        &self,
        event_type: &str,
        from_block: BlockId,
    ) -> Result<BoxStream<LedgerEventContext<Event>>, SawtoothCommunicationError> {
        let event_type = event_type.to_owned();
        #[derive(Debug)]
        enum ParsedEvent<Event> {
            Block(BlockId, Position),
            Operation(Event, TransactionId, u64),
        }

        let channel = self.channel.clone();
        let from_block = from_block;
        let event_stream = channel.recv_stream::<EventList>().await?;
        let event_stream = event_stream.then(move |events| {
            let event_type = event_type.clone();
            async move {
                debug!(received_events = ?events);
                let mut updates = vec![];
                for event in events.events {
                    updates.push(match &*event.event_type {
                        "sawtooth/block-commit" => {
                            let position: Result<u64, _> = event
                                .attributes
                                .iter()
                                .find(|attr| attr.key == "block_num")
                                .ok_or(SawtoothCommunicationError::MissingBlockNum)
                                .and_then(|attr| attr.value.parse::<u64>().map_err(Into::into));

                            let block_id: Result<BlockId, _> = event
                                .attributes
                                .iter()
                                .find(|attr| attr.key == "block_id")
                                .ok_or(SawtoothCommunicationError::MissingBlockId)
                                .and_then(|attr| {
                                    BlockId::try_from(attr.value.clone()).map_err(Into::into)
                                });

                            position.and_then(|id| {
                                block_id.map(|block| Some(ParsedEvent::Block(block, id.into())))
                            })
                        }
                        x if *x == *event_type => {
                            let transaction_id = event
                                .attributes
                                .iter()
                                .find(|attr| attr.key == "transaction_id")
                                .ok_or(SawtoothCommunicationError::MissingTransactionId)
                                .map(|attr| TransactionId::from(&*attr.value));

                            let event = Event::deserialize(&event.data).await;

                            transaction_id
                                .map_err(SawtoothCommunicationError::from)
                                .and_then(|transaction_id| {
                                    event.map(|event| {
                                        let (operation, span) = event;
                                        Some(ParsedEvent::Operation(
                                            operation,
                                            transaction_id,
                                            span,
                                        ))
                                    })
                                })
                        }
                        _ => Ok(None),
                    });
                }

                debug!(parsed_events = ?updates);

                // Fold the updates into a vector of operations and their block num
                let updates: (Vec<_>, _) = updates.into_iter().fold(
                    (vec![], *self.last_seen_block.lock().unwrap()),
                    |(mut operations, block), event| {
                        trace!(combining_event= ?event);
                        match event {
                            // Next block num
                            Ok(Some(ParsedEvent::Block(id, num))) => {
                                debug!(last_seen_block = %id, position= %num);
                                (operations, Some((id, num)))
                            }
                            Ok(Some(ParsedEvent::Operation(
                                next_operation,
                                transaction_id,
                                span,
                            ))) => {
                                operations.push((next_operation, transaction_id, span));
                                (operations, block)
                            }
                            Err(e) => {
                                error!(?e, "Parsing state update");
                                (operations, block)
                            }
                            _ => (operations, block),
                        }
                    },
                );

                updates
            }
        });

        let events_with_block = event_stream
            .flat_map(move |events_for_block| {
                let last_block = (
                    events_for_block.1.map(|x| x.0).unwrap_or(from_block),
                    events_for_block.1.map(|x| x.1).unwrap_or(Position::from(0)),
                );
                //position

                *self.last_seen_block.lock().unwrap() = Some(last_block);
                info!(last_seen_block = %last_block.0, position= %last_block.1);

                stream::iter(events_for_block.0).map(move |event| {
                    let ev = (
                        event.0, //Event
                        event.1, //Transaction Id
                        last_block.0,
                        last_block.1,
                        event.2, //span
                    );
                    debug!(yield_event=?ev);

                    ev
                })
            })
            .boxed();

        Ok(events_with_block)
    }
}

#[async_trait::async_trait]
impl<
        Channel: RequestResponseSawtoothChannel + Clone + Send + Sync,
        Event: LedgerEvent + Send + Sync + std::fmt::Debug,
        Transaction: LedgerTransaction + Send + Sync,
    > LedgerReader for SawtoothLedger<Channel, Event, Transaction>
{
    type Error = SawtoothCommunicationError;
    type Event = Event;

    async fn get_state_entry(&self, address: &str) -> Result<Vec<u8>, Self::Error> {
        let request = self.builder.make_state_request(address);

        let response = self
            .channel
            .send_and_recv_one::<ClientStateGetResponse, _>(
                request,
                Message_MessageType::CLIENT_STATE_GET_REQUEST,
                Duration::from_secs(10),
            )
            .await?;

        if response.status == ClientStateGetResponse_Status::OK {
            Ok(response.value)
        } else {
            Err(SawtoothCommunicationError::UnexpectedStatus {
                status: response.status as i32,
            })
        }
    }

    async fn block_height(&self) -> Result<(Position, BlockId), Self::Error> {
        let (block, id) = self.get_block_height().await?;
        Ok((block, id))
    }

    #[instrument(skip(self), level = "debug")]
    async fn state_updates(
        &self,
        event_type: &str,
        from_block_id: FromBlock,
        number_of_blocks: Option<u64>,
    ) -> Result<BoxStream<LedgerEventContext<Event>>, Self::Error> {
        let (from_block_id, from_position) = {
            match &from_block_id {
                FromBlock::First => {
                    let first_block = self.get_first_block().await?;
                    debug!(first_block = %first_block);
                    (first_block, None)
                }
                FromBlock::Head => {
                    let (block_num, block_id) = self.get_block_height().await?;
                    debug!(block_height = %block_num, block_id = %block_id);
                    (block_id, Some(block_num))
                }
                FromBlock::BlockId(block_id) => (*block_id, None),
            }
        };

        debug!(?from_position, ?from_block_id);

        let subscribe = self.get_events_from(event_type, from_block_id).await?;

        Ok(subscribe
            .take_while(move |(_, _, _offset, position, _span)| {
                future::ready(match (number_of_blocks, from_position) {
                    (Some(number_of_blocks), Some(from_position)) => {
                        let distance = from_position.distance(position);
                        let remaining_blocks_before_timeout = number_of_blocks - distance;
                        debug!(remaining_blocks_before_timeout);

                        remaining_blocks_before_timeout > 0
                    }
                    _ => true,
                })
            })
            .boxed())
    }

    async fn reconnect(&self) {
        debug!("Reconnect ZMQ channel");
        self.channel.close();
        self.channel.reconnect();
    }
}

#[async_trait::async_trait]
impl<
        Channel: RequestResponseSawtoothChannel + Clone + Send + Sync,
        Event: LedgerEvent + Send + Sync + std::fmt::Debug,
        Transaction: LedgerTransaction + Send + Sync,
    > LedgerWriter for SawtoothLedger<Channel, Event, Transaction>
{
    type Error = SawtoothCommunicationError;
    type Transaction = Transaction;

    fn message_builder(&self) -> &MessageBuilder {
        &self.builder
    }

    async fn pre_submit(
        &self,
        tx: &Self::Transaction,
    ) -> Result<
        (
            TransactionId,
            sawtooth_sdk::messages::transaction::Transaction,
        ),
        Self::Error,
    > {
        let (sawtooth_tx, id) = tx.as_sawtooth_tx(self.message_builder()).await;
        Ok((id, sawtooth_tx))
    }

    #[instrument(skip(self), level = "trace", ret(Debug))]
    async fn submit(
        &self,
        tx: sawtooth_sdk::messages::transaction::Transaction,
        signer: &SigningKey,
    ) -> Result<(), Self::Error> {
        let batch = self.message_builder().wrap_tx_as_sawtooth_batch(tx, signer);
        self.submit_batch(batch).await?;
        Ok(())
    }
}
