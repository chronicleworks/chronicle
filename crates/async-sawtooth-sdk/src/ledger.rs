use std::time::Duration;

use derivative::Derivative;
use futures::{
    future,
    stream::{self, BoxStream},
    StreamExt,
};

use k256::ecdsa::SigningKey;
use protobuf::Message;
use sawtooth_sdk::messages::{
    batch::Batch,
    block::BlockHeader,
    client_batch_submit::{
        ClientBatchSubmitRequest, ClientBatchSubmitResponse, ClientBatchSubmitResponse_Status,
    },
    client_block::{ClientBlockListResponse, ClientBlockListResponse_Status},
    client_event::{ClientEventsSubscribeResponse, ClientEventsSubscribeResponse_Status},
    client_state::{ClientStateGetResponse, ClientStateGetResponse_Status},
    events::EventList,
    transaction::Transaction,
    validator::Message_MessageType,
};
use serde::{Deserialize, Serialize};

use tracing::{debug, error, instrument};

use crate::{
    error::SawtoothCommunicationError, sawtooth::MessageBuilder,
    zmq_client::RequestResponseSawtoothChannel,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct BlockId(String);

impl std::fmt::Display for BlockId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
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
pub enum Offset {
    Genesis,
    Identity(u64),
}

impl From<u64> for Offset {
    fn from(x: u64) -> Self {
        Offset::Identity(x)
    }
}

impl PartialOrd for Offset {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (Offset::Genesis, Offset::Genesis) => Some(std::cmp::Ordering::Equal),
            (Offset::Genesis, Offset::Identity(_)) => Some(std::cmp::Ordering::Less),
            (Offset::Identity(_), Offset::Genesis) => Some(std::cmp::Ordering::Greater),
            (Offset::Identity(x), Offset::Identity(y)) => x.partial_cmp(y),
        }
    }
}

impl Offset {
    pub fn map<T, F>(&self, f: F) -> Option<T>
    where
        F: FnOnce(&u64) -> T,
    {
        if let Offset::Identity(x) = self {
            Some(f(x))
        } else {
            None
        }
    }

    pub fn distance(&self, other: &Self) -> u64 {
        match (self, other) {
            (Offset::Genesis, Offset::Genesis) => 0,
            (Offset::Genesis, Offset::Identity(x)) => *x,
            (Offset::Identity(x), Offset::Genesis) => *x,
            (Offset::Identity(x), Offset::Identity(y)) => x.saturating_sub(*y),
        }
    }
}

// An application specific ledger event with its corresponding transaction id,
// block height and trace span
pub type LedgerEventContext<Event> = (Event, TransactionId, Offset, u64);

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

    pub fn block_height(&self) -> Result<(Offset, BlockId), R::Error> {
        tokio::runtime::Handle::current().block_on(self.reader.block_height())
    }

    pub fn state_updates(
        &self,
        event_type: &str,
        from_offset: Option<Offset>,
        number_of_blocks: Option<u64>,
    ) -> Result<BoxStream<LedgerEventContext<R::Event>>, R::Error> {
        tokio::runtime::Handle::current().block_on(self.reader.state_updates(
            event_type,
            from_offset,
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
    async fn block_height(&self) -> Result<(Offset, BlockId), Self::Error>;
    /// Subscribe to state updates from this ledger, starting at `offset`, and
    /// ending the stream after `number_of_blocks` blocks have been processed.
    async fn state_updates(
        &self,
        // The application event type to subscribe to
        event_type: &str,
        // The offset to start from, or `None` to start from the current block
        from_offset: Option<Offset>,
        // The number of blocks to process before ending the stream
        number_of_blocks: Option<u64>,
    ) -> Result<BoxStream<LedgerEventContext<Self::Event>>, Self::Error>;
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
    async fn get_block_height(&self) -> Result<(u64, String), SawtoothCommunicationError> {
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
            Ok((header.block_num, header.previous_block_id))
        } else {
            Err(SawtoothCommunicationError::UnexpectedStatus {
                status: response.status as i32,
            })
        }
    }

    #[instrument(skip(self))]
    async fn get_events_from(
        &self,
        event_type: &str,
        offset: &Offset,
        offset_id: &Option<BlockId>,
    ) -> Result<BoxStream<LedgerEventContext<Event>>, SawtoothCommunicationError> {
        let subscription_request = self
            .builder
            .make_subscription_request(offset_id, vec![event_type.into()]);
        debug!(?subscription_request);
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

        self.event_stream(event_type, *offset).await
    }

    async fn event_stream(
        &self,
        event_type: &str,
        block: Offset,
    ) -> Result<BoxStream<LedgerEventContext<Event>>, SawtoothCommunicationError> {
        let event_type = event_type.to_owned();
        #[derive(Debug)]
        enum ParsedEvent<Event> {
            Block(u64),
            Operation(Event, TransactionId, u64),
        }

        let channel = self.channel.clone();

        let event_stream = channel.recv_stream::<EventList>().await?;
        let event_stream = event_stream.then(move |events| {
            let event_type = event_type.clone();

            async move {
                debug!(?events, "Received events");
                let mut updates = vec![];
                for event in events.events {
                    updates.push(match &*event.event_type {
                        "sawtooth/block-commit" => event
                            .attributes
                            .iter()
                            .find(|attr| attr.key == "block_num")
                            .ok_or(SawtoothCommunicationError::MissingBlockNum)
                            .and_then(|attr| attr.value.parse().map_err(Into::into))
                            .map(|attr| Some(ParsedEvent::Block(attr))),
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

                debug!(?updates, "Parsed events");

                // Fold the updates into a vector of operations and their block num
                updates
                    .into_iter()
                    .fold((vec![], block), |(mut operations, block), event| {
                        match event {
                            // Next block num
                            Ok(Some(ParsedEvent::Block(next))) => (operations, Offset::from(next)),
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
                    })
            }
        });

        let events_with_block = event_stream
            .flat_map(|events_for_block| {
                stream::iter(events_for_block.0)
                    .map(move |event| (event.0, event.1, events_for_block.1, event.2))
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

    async fn block_height(&self) -> Result<(Offset, BlockId), Self::Error> {
        let (block, id) = self.get_block_height().await?;
        Ok((Offset::from(block), BlockId(id)))
    }

    #[instrument(skip(self))]
    async fn state_updates(
        &self,
        event_type: &str,
        from_offset: Option<Offset>,
        number_of_blocks: Option<u64>,
    ) -> Result<BoxStream<LedgerEventContext<Event>>, Self::Error> {
        let self_clone = self.clone();

        let (from_offset, from_block_id) = match from_offset {
            None => (Offset::Genesis, None),
            Some(from_offset) => {
                let (_num, id) = self_clone.get_block_height().await?;
                (from_offset, Some(BlockId(id)))
            }
        };

        let subscribe = self
            .get_events_from(event_type, &from_offset, &from_block_id)
            .await?;

        Ok(subscribe
            .take_while(move |(_, _, offset, _)| {
                future::ready(if let Some(number_of_blocks) = number_of_blocks {
                    offset.distance(&from_offset) <= number_of_blocks
                } else {
                    true
                })
            })
            .boxed())
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
