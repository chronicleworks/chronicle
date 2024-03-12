use std::str::FromStr;

use async_trait::async_trait;
use futures::stream::BoxStream;
use pallet_chronicle::ChronicleTransactionId;
use std::time::Duration;
use subxt::ext::sp_core::H256;
use thiserror::Error;
use tokio::time::sleep;
use tracing::{instrument, warn};

#[derive(Debug, Error)]
pub enum BlockIdError {
	#[error("Parse {0}")]
	Parse(
		#[from]
		#[source]
		anyhow::Error,
	),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlockId {
	Unknown,     //Block ids can be null, empty string etc
	Block(H256), //ToDo - trait
}

impl From<H256> for BlockId {
	fn from(hash: H256) -> Self {
		BlockId::Block(hash)
	}
}

impl TryFrom<&str> for BlockId {
	type Error = BlockIdError;

	#[instrument(level = "trace", skip(s), err)]
	fn try_from(s: &str) -> Result<Self, Self::Error> {
		let hash = H256::from_str(s).map_err(|e| BlockIdError::Parse(anyhow::Error::new(e)))?;
		Ok(BlockId::Block(hash))
	}
}

impl std::fmt::Display for BlockId {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			BlockId::Unknown => f.write_str("Unknown"),
			BlockId::Block(hash) => f.write_str(&format!("{:?}", hash)),
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Position(u32);

impl From<u32> for Position {
	fn from(height: u32) -> Self {
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
	pub fn new(height: u32) -> Self {
		Position(height)
	}

	pub fn map<T, F>(&self, f: F) -> T
	where
		F: FnOnce(&u32) -> T,
	{
		f(&self.0)
	}

	pub fn distance(&self, other: &Self) -> u32 {
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

// Type that can contain a distributed tracing span for transaction processors
// that support it
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Span {
	Span(u64),
	NotTraced,
}

// An application specific ledger event with its corresponding transaction id,
// block height and trace span
pub type LedgerEventContext<Event> = (Event, ChronicleTransactionId, BlockId, Position, Span);

#[async_trait::async_trait]
pub trait LedgerEvent {
	fn correlation_id(&self) -> [u8; 16];
}

#[async_trait::async_trait]
pub trait LedgerEventCodec {
	type Source;
	type Sink: LedgerEvent + Send + Sync;
	type Error: std::error::Error;
	// Attempt to deserialize an event, where there may be none present in the source
	async fn maybe_deserialize(
		source: Self::Source,
	) -> Result<Option<(Self::Sink, Span)>, Self::Error>
	where
		Self: Sized;
}

pub trait MessageBuilder {}

#[async_trait::async_trait]
pub trait LedgerTransaction {
	type Error: std::error::Error + Send + Sync + 'static;
	type Payload: Sized + Send + Sync;
	async fn as_payload(&self) -> Result<Self::Payload, Self::Error>;
	fn correlation_id(&self) -> [u8; 16];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteConsistency {
	Weak,
	Strong,
}

#[async_trait::async_trait]
pub trait LedgerWriter {
	type Error: std::error::Error;
	type Transaction: LedgerTransaction;
	type Submittable: Sized;

	// Minimally process the transaction offline to get a transaction id and submittable type
	async fn pre_submit(
		&self,
		tx: Self::Transaction,
	) -> Result<(Self::Submittable, ChronicleTransactionId), Self::Error>;

	// Submit is used to submit a transaction to the ledger
	async fn do_submit(
		&self,
		consistency: WriteConsistency,
		submittable: Self::Submittable,
	) -> Result<ChronicleTransactionId, (Self::Error, ChronicleTransactionId)>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FromBlock {
	// Do not attempt to catch up, start from the current head
	Head,
	// Discover the first useful block and start from there
	First,
	// Start from the given block
	BlockId(BlockId),
}

#[async_trait::async_trait]
pub trait LedgerReader {
	type Event: LedgerEvent;
	type EventCodec: LedgerEventCodec;
	type Error: std::error::Error;
	// Get the block height of the ledger, and the id of the highest block
	async fn block_height(&self) -> Result<(Position, BlockId), Self::Error>;
	/// Subscribe to state updates from this ledger, starting at `offset`, and
	/// ending the stream after `number_of_blocks` blocks have been processed.
	async fn state_updates(
		&self,
		// The block to start from
		from_block: FromBlock,
		// The number of blocks to process before ending the stream
		number_of_blocks: Option<u32>,
	) -> Result<BoxStream<LedgerEventContext<Self::Event>>, Self::Error>;
}

pub fn retryable_ledger<L: LedgerReader>(ledger: L, retry_delay: Duration) -> RetryLedger<L> {
	RetryLedger::new(ledger, retry_delay)
}

#[derive(Clone)]
pub struct RetryLedger<L: LedgerReader> {
	inner: L,
	retry_delay: Duration,
}

impl<L: LedgerReader> RetryLedger<L> {
	pub fn new(inner: L, retry_delay: Duration) -> Self {
		Self { inner, retry_delay }
	}
}

#[async_trait::async_trait]
impl<L> LedgerWriter for RetryLedger<L>
where
	L: LedgerReader + LedgerWriter + Send + Sync,
	<L as LedgerWriter>::Error: Send + Sync + 'static,
	<L as LedgerWriter>::Transaction: Send + Sync + 'static,
	L::Submittable: Send + Sync + 'static + Clone,
{
	type Error = <L as LedgerWriter>::Error;
	type Submittable = L::Submittable;
	type Transaction = L::Transaction;

	async fn pre_submit(
		&self,
		tx: Self::Transaction,
	) -> Result<(Self::Submittable, ChronicleTransactionId), Self::Error> {
		tracing::debug!(target: "ledger_writer", "Pre-submitting transaction");
		let pre_submit_result = self.inner.pre_submit(tx).await;
		match pre_submit_result {
			Ok(result) => Ok(result),
			Err(e) => {
				tracing::error!(error = %e, "Failed to pre-submit transaction");
				Err(e)
			},
		}
	}

	async fn do_submit(
		&self,
		consistency: WriteConsistency,
		submittable: Self::Submittable,
	) -> Result<ChronicleTransactionId, (Self::Error, ChronicleTransactionId)> {
		let mut attempts = 0;
		loop {
			match self.inner.do_submit(consistency, submittable.clone()).await {
				Ok(result) => {
					tracing::info!(target: "ledger_writer", "Successfully submitted transaction");
					return Ok(result);
				},
				Err(e) => {
					attempts += 1;
					tracing::warn!(error = %e.0, attempts, "Failed to submit transaction, retrying after delay");
					tokio::time::sleep(self.retry_delay).await;
				},
			}
		}
	}
}

#[async_trait]
impl<L: LedgerReader + Send + Sync> LedgerReader for RetryLedger<L>
where
	<L as LedgerReader>::Error: Send + Sync,
{
	type Error = L::Error;
	type Event = L::Event;
	type EventCodec = L::EventCodec;

	async fn block_height(&self) -> Result<(Position, BlockId), Self::Error> {
		let mut attempts = 0;
		loop {
			match self.inner.block_height().await {
				Ok(result) => {
					tracing::info!(target: "ledger_reader", "Successfully retrieved block height");
					return Ok(result);
				},
				Err(e) => {
					attempts += 1;
					tracing::warn!(error = %e, attempts, "Failed to get block height, retrying after delay");
					tokio::time::sleep(self.retry_delay).await;
				},
			}
		}
	}

	async fn state_updates(
		&self,
		from_block: FromBlock,
		number_of_blocks: Option<u32>,
	) -> Result<BoxStream<LedgerEventContext<Self::Event>>, Self::Error> {
		loop {
			match self.inner.state_updates(from_block, number_of_blocks).await {
				Ok(stream) => return Ok(stream),
				Err(e) => {
					warn!(error = %e, "Failed to subscribe to state updates, retrying after delay");
					sleep(self.retry_delay).await;
				},
			}
		}
	}
}
