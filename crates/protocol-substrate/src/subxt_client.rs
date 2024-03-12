use std::{convert::Infallible, marker::PhantomData, net::SocketAddr, time::Duration};

use derivative::Derivative;
use futures::{
	stream::{self, BoxStream},
	FutureExt, StreamExt, TryFutureExt, TryStreamExt,
};

use pallet_chronicle::ChronicleTransactionId;
use subxt::{
	backend::BackendExt,
	config::ExtrinsicParams,
	error::MetadataError,
	ext::{
		codec::{Decode, Encode},
		sp_core::{twox_128, H256},
	},
	metadata::{
		types::{PalletMetadata, StorageEntryMetadata, StorageEntryType},
		DecodeWithMetadata, EncodeWithMetadata,
	},
	storage::{DynamicAddress, StorageAddress},
	tx::{Payload, SubmittableExtrinsic},
	utils::{AccountId32, MultiAddress, MultiSignature},
	Metadata, OnlineClient,
};

pub use subxt::Config;

use protocol_abstract::{
	BlockId, FromBlock, LedgerEvent, LedgerEventCodec, LedgerEventContext, LedgerReader,
	LedgerTransaction, LedgerWriter, Position, RetryLedger, WriteConsistency,
};

#[derive(Derivative)]
#[derivative(Clone(bound = ""))]
pub struct SubstrateClient<C: subxt::Config, EC: LedgerEventCodec, T: LedgerTransaction> {
	pub client: OnlineClient<C>,
	_p: PhantomData<(EC, T)>,
}

type ExtrinsicResult<C> =
	Result<(SubmittableExtrinsic<C, OnlineClient<C>>, [u8; 16]), subxt::Error>;

impl<C, EC, T> SubstrateClient<C, EC, T>
where
	C: subxt::Config<
		Hash = subxt::utils::H256,
		Address = MultiAddress<AccountId32, ()>,
		Signature = MultiSignature,
	>,
	<C::ExtrinsicParams as ExtrinsicParams<C>>::OtherParams: Default,
	T: LedgerTransaction + Send + Sync,
	<T as protocol_abstract::LedgerTransaction>::Payload: subxt::ext::scale_encode::EncodeAsFields,
	EC: LedgerEventCodec + Send + Sync,
{
	pub async fn connect(url: impl AsRef<str>) -> Result<Self, SubxtClientError> {
		Ok(Self { client: OnlineClient::from_url(url).await?, _p: Default::default() })
	}

	pub async fn connect_socket_addr(socket: SocketAddr) -> Result<Self, SubxtClientError> {
		tracing::info!("Connecting to Substrate client via SocketAddr: {:?}", socket);
		let client_result = OnlineClient::from_url(socket.to_string()).await;
		match client_result {
			Ok(client) => {
				tracing::info!("Successfully connected to Substrate client.");
				Ok(Self { client, _p: Default::default() })
			},
			Err(e) => {
				tracing::error!("Failed to connect to Substrate client: {:?}", e);
				Err(SubxtClientError::from(e))
			},
		}
	}

	pub fn retry(&self, duration: Duration) -> RetryLedger<Self>
	where
		Self: LedgerReader + Sized,
	{
		tracing::debug!(target: "substrate_client", "Creating a retryable ledger reader.");
		RetryLedger::new(self.clone(), duration)
	}

	// TODO: bring the pallet / call name in from trait

	#[tracing::instrument(level="trace" , skip(self, signer, correlation_id, operations), fields(correlation_id = %hex::encode(correlation_id), ret))]
	pub async fn create_extrinsic<S: subxt::tx::Signer<C> + Send>(
		&self,
		signer: &S,
		correlation_id: [u8; 16],
		operations: &T,
	) -> ExtrinsicResult<C> {
		let payload = Payload::new("Chronicle", "apply", operations.as_payload().await.unwrap());

		self.client
			.tx()
			.create_signed(&payload, signer, Default::default())
			.await
			.map(|extrinsic| (extrinsic, correlation_id))
	}

	pub async fn send_extrinsic(
		&self,
		consistency: WriteConsistency,
		extrinsic: (SubmittableExtrinsic<C, OnlineClient<C>>, [u8; 16]),
	) -> Result<ChronicleTransactionId, (subxt::Error, ChronicleTransactionId)> {
		extrinsic
			.0
			.submit_and_watch()
			.and_then(|progress| match consistency {
				WriteConsistency::Weak => futures::future::ok(()).boxed(),
				WriteConsistency::Strong => progress
					.wait_for_finalized_success()
					.and_then(|_| futures::future::ok(()))
					.boxed(),
			})
			.await
			.map(|_| extrinsic.1.into())
			.map_err(|e| (e, ChronicleTransactionId::from(extrinsic.1)))
	}
}

#[derive(Debug, thiserror::Error)]
pub enum SubxtClientError {
	#[error("Subxt error: {0}")]
	SubxtError(
		#[from]
		#[source]
		subxt::Error,
	),

	#[error("Invalid block")]
	InvalidBlock,

	#[error("Codec: {0}")]
	Codec(
		#[from]
		#[source]
		subxt::ext::codec::Error,
	),

	#[error("Decode: {0}")]
	Decode(
		#[from]
		#[source]
		subxt::error::DecodeError,
	),

	#[error("Serde: {0}")]
	Serde(
		#[from]
		#[source]
		subxt::ext::scale_value::serde::SerializerError,
	),
}

impl From<Infallible> for SubxtClientError {
	fn from(_value: Infallible) -> Self {
		unreachable!()
	}
}

impl<C, H, EC, T> SubstrateClient<C, EC, T>
where
	C: subxt::Config<Hash = subxt::utils::H256, Header = H>,
	H: subxt::config::Header<Number = u32> + Send + Sync + Decode + Encode,
	EC: LedgerEventCodec<Error = SubxtClientError, Source = subxt::events::EventDetails<C>>
		+ Send
		+ Sync,
	T: LedgerTransaction + Send + Sync,
{
	// Return child blocks of from_block, limiting to num_blocks if not none
	async fn block_hashes_from(
		&self,
		from_block: C::Hash,
		num_blocks: Option<u32>,
	) -> Result<BoxStream<Result<C::Hash, SubxtClientError>>, SubxtClientError> {
		// Get the block at hash
		let block = self.client.blocks().at(from_block).await?;

		let from_block_num = block.number();

		let hashes = stream::unfold(
			(self.client.clone(), from_block_num),
			move |(client, block_num)| async move {
				if let Some(num_blocks) = num_blocks {
					if num_blocks == block_num {
						return None;
					}
				}

				let block_hash: Result<C::Hash, _> = client
					.backend()
					.call_decoding(
						"chain_getBlockHash",
						Some(&vec![block_num].encode()),
						subxt::utils::H256::zero(),
					)
					.await
					.map_err(SubxtClientError::from);

				Some((block_hash, (client, block_num + 1)))
			},
		);

		Ok(Box::pin(hashes))
	}

	// Return events from `number_of_blocks` blocks from the client, starting at `from_block`
	async fn events_for_block(
		&self,
		from_block: C::Hash,
	) -> Result<BoxStream<LedgerEventContext<<EC as LedgerEventCodec>::Sink>>, SubxtClientError> {
		let header = self.client.backend().block_header(from_block).await?;
		let block_num = match header {
			Some(header) => Ok(header.number()),
			None => {
				tracing::error!("Block header is None");
				Err(SubxtClientError::InvalidBlock)
			},
		}?;

		let events_for_block = match self.client.events().at(from_block).await {
			Ok(events) => Ok(events),
			Err(e) => {
				tracing::error!("Failed to get events for block: {}", e);
				Err(SubxtClientError::InvalidBlock)
			},
		}?;

		let events_for_block =
			stream::unfold(events_for_block.iter(), |mut events_for_block| async move {
				match events_for_block.next() {
					Some(Ok(event)) => match EC::maybe_deserialize(event).await {
						Ok(Some(event)) => Some((event, events_for_block)),
						_ => None,
					},
					Some(Err(e)) => {
						tracing::error!("Cannot fetch event {}", e);
						None
					},
					_ => None,
				}
			});

		let event_stream = events_for_block.map(move |(event, span)| {
			let correlation_id = event.correlation_id();
			(
				event,
				ChronicleTransactionId::from(correlation_id),
				BlockId::Block(from_block),
				Position::from(block_num),
				span,
			)
		});

		Ok(event_stream.boxed())
	}

	async fn stream_finalized_events(
		&self,
	) -> Result<BoxStream<LedgerEventContext<<EC as LedgerEventCodec>::Sink>>, SubxtClientError> {
		let blocks = self.client.blocks().subscribe_finalized().await?;

		let parsed_events = blocks
			.map_err(SubxtClientError::from)
			.and_then(|block| async move {
				let block_num = block.number();
				let block_hash = block.hash();

				let events = block.events().await.map_err(SubxtClientError::from);

				match events {
					Err(e) => Err(e),
					Ok(events) => {
						let events = events
							.iter()
							.filter_map(|event| {
								event
									.map_err(SubxtClientError::from)
									.and_then(|event| {
										futures::executor::block_on(EC::maybe_deserialize(event))
									})
									.transpose()
									.map(|event| {
										event.map(|(event, span)| {
											let correlation_id = event.correlation_id();
											(
												event,
												ChronicleTransactionId::from(correlation_id),
												BlockId::Block(block_hash),
												Position::from(block_num),
												span,
											)
										})
									})
							})
							.collect::<Vec<_>>();
						Ok(stream::iter(events))
					},
				}
			})
			.boxed();

		//Unfold and terminate stream on error
		let flattened_stream = stream::unfold(parsed_events, |mut parsed_events| async move {
			match parsed_events.next().await {
				Some(Ok(events)) => Some((events, parsed_events)),
				Some(Err(e)) => {
					tracing::error!("Subscription error {}", e);
					None
				},
				_ => None,
			}
		})
		.flatten()
		.boxed();

		// Terminate on parse error in flattened stream,
		let flattened_stream =
			stream::unfold(flattened_stream, |mut flattened_stream| async move {
				match flattened_stream.next().await {
					Some(Err(e)) => {
						tracing::error!("Event parse error {}", e);
						None
					},
					Some(Ok(event)) => Some((event, flattened_stream)),
					None => None,
				}
			})
			.boxed();

		Ok(flattened_stream)
	}

	async fn historical_events(
		&self,
		from_block: C::Hash,
		num_blocks: Option<u32>,
	) -> Result<BoxStream<LedgerEventContext<<EC as LedgerEventCodec>::Sink>>, SubxtClientError> {
		let from_block_clone = self;
		let block_hashes = from_block_clone.block_hashes_from(from_block, num_blocks).await?;

		let events = stream::unfold(
			(block_hashes, self),
			move |(mut block_hashes, self_clone)| async move {
				let next_block_hash = block_hashes.next().await;
				match next_block_hash {
					Some(Ok(block_hash)) => {
						let events = self_clone.events_for_block(block_hash).await;
						match events {
							Ok(events) => Some((events, (block_hashes, self_clone))),
							Err(e) => {
								tracing::error!("Subscription error {}", e);
								None
							},
						}
					},
					Some(Err(e)) => {
						tracing::error!("Subscription error {}", e);
						None
					},
					_ => None,
				}
			},
		)
		.flatten()
		.boxed();

		Ok(events)
	}
}

#[async_trait::async_trait]
impl<C, E, T> LedgerWriter for SubstrateClient<C, E, T>
where
	C: subxt::Config<
		Address = MultiAddress<AccountId32, ()>,
		AccountId = AccountId32,
		Hash = H256,
		Signature = MultiSignature,
	>,
	<C::ExtrinsicParams as ExtrinsicParams<C>>::OtherParams: Default + Send,
	E: LedgerEventCodec<Source = subxt::events::EventDetails<C>> + Send + Sync,
	T: LedgerTransaction + Send + Sync + subxt::tx::Signer<C>,
	<T as LedgerTransaction>::Payload: subxt::ext::scale_encode::EncodeAsFields,
{
	type Error = SubxtClientError;
	type Submittable = (SubmittableExtrinsic<C, OnlineClient<C>>, [u8; 16]);
	type Transaction = T;

	async fn pre_submit(
		&self,
		tx: Self::Transaction,
	) -> Result<(Self::Submittable, ChronicleTransactionId), Self::Error> {
		let correlation_id = tx.correlation_id();
		let (ext, id) = self.create_extrinsic(&tx, correlation_id, &tx).await?;

		Ok(((ext, id), id.into()))
	}

	async fn do_submit(
		&self,
		consistency: WriteConsistency,
		submittable: Self::Submittable,
	) -> Result<ChronicleTransactionId, (Self::Error, ChronicleTransactionId)> {
		tracing::info!(
			target: "substrate_client",
			correlation_id = ?submittable.1,
			"Submitting extrinsic with correlation ID."
		);

		self.send_extrinsic(consistency, submittable)
			.await
			.map_err(|(e, id)| (e.into(), id))
	}
}

#[async_trait::async_trait]
pub trait SubstrateStateReader {
	type Error: std::error::Error;
	/// Get the state entry at `address`
	async fn get_state_entry<K: EncodeWithMetadata + Send + Sync, V: DecodeWithMetadata>(
		&self,
		pallet_name: &str,
		entry_name: &str,
		address: K,
	) -> Result<Option<V>, Self::Error>;
}

pub(crate) fn validate_storage_address<Address: StorageAddress>(
	address: &Address,
	pallet: PalletMetadata<'_>,
) -> Result<(), subxt::Error> {
	if let Some(hash) = address.validation_hash() {
		validate_storage(pallet, address.entry_name(), hash)?;
	}
	Ok(())
}

/// Return details about the given storage entry.
fn lookup_entry_details<'a>(
	pallet_name: &str,
	entry_name: &str,
	metadata: &'a Metadata,
) -> Result<(PalletMetadata<'a>, &'a StorageEntryMetadata), subxt::Error> {
	let pallet_metadata = metadata.pallet_by_name_err(pallet_name)?;
	let storage_metadata = pallet_metadata
		.storage()
		.ok_or_else(|| MetadataError::StorageNotFoundInPallet(pallet_name.to_owned()))?;
	let storage_entry = storage_metadata
		.entry_by_name(entry_name)
		.ok_or_else(|| MetadataError::StorageEntryNotFound(entry_name.to_owned()))?;
	Ok((pallet_metadata, storage_entry))
}

/// Validate a storage entry against the metadata.
fn validate_storage(
	pallet: PalletMetadata<'_>,
	storage_name: &str,
	hash: [u8; 32],
) -> Result<(), subxt::Error> {
	let Some(expected_hash) = pallet.storage_hash(storage_name) else {
		return Err(MetadataError::IncompatibleCodegen.into());
	};
	if expected_hash != hash {
		return Err(MetadataError::IncompatibleCodegen.into());
	}
	Ok(())
}

/// Fetch the return type out of a [`StorageEntryType`].
fn return_type_from_storage_entry_type(entry: &StorageEntryType) -> u32 {
	match entry {
		StorageEntryType::Plain(ty) => *ty,
		StorageEntryType::Map { value_ty, .. } => *value_ty,
	}
}

/// Given some bytes, a pallet and storage name, decode the response.
fn decode_storage_with_metadata<T: DecodeWithMetadata>(
	bytes: &mut &[u8],
	metadata: &Metadata,
	storage_metadata: &StorageEntryMetadata,
) -> Result<T, subxt::Error> {
	let ty = storage_metadata.entry_type();
	let return_ty = return_type_from_storage_entry_type(ty);
	let val = T::decode_with_metadata(bytes, return_ty, metadata)?;
	Ok(val)
}

pub(crate) fn write_storage_address_root_bytes<Address: StorageAddress>(
	addr: &Address,
	out: &mut Vec<u8>,
) {
	out.extend(twox_128(addr.pallet_name().as_bytes()));
	out.extend(twox_128(addr.entry_name().as_bytes()));
}

pub(crate) fn storage_address_bytes<Address: StorageAddress>(
	addr: &Address,
	metadata: &Metadata,
) -> Result<Vec<u8>, subxt::Error> {
	let mut bytes = Vec::new();
	write_storage_address_root_bytes(addr, &mut bytes);
	addr.append_entry_bytes(metadata, &mut bytes)?;
	Ok(bytes)
}

#[async_trait::async_trait]
impl<C, EC, T> SubstrateStateReader for SubstrateClient<C, EC, T>
where
	C: subxt::Config,
	EC: LedgerEventCodec + Send + Sync,
	T: protocol_abstract::LedgerTransaction + Send + Sync,
{
	type Error = SubxtClientError;

	async fn get_state_entry<K: EncodeWithMetadata + Send + Sync, V: DecodeWithMetadata>(
		&self,
		pallet_name: &str,
		entry_name: &str,
		address: K,
	) -> Result<Option<V>, Self::Error> {
		let metadata = self.client.metadata();
		let (pallet, entry) = lookup_entry_details(pallet_name, entry_name, &metadata)?;

		let address = DynamicAddress::new(pallet_name, entry_name, vec![address]);

		// Metadata validation checks whether the static address given
		// is likely to actually correspond to a real storage entry or not.
		// if not, it means static codegen doesn't line up with runtime
		// metadata.
		validate_storage_address(&address, pallet)?;

		// Look up the return type ID to enable DecodeWithMetadata:
		let lookup_bytes = storage_address_bytes(&address, &metadata)?;
		if let Some(data) = self.client.storage().at_latest().await?.fetch_raw(lookup_bytes).await?
		{
			let val = decode_storage_with_metadata::<V>(&mut &*data, &metadata, entry)?;
			Ok(Some(val))
		} else {
			Ok(None)
		}
	}
}

#[async_trait::async_trait]
impl<C, H, EC, T> LedgerReader for SubstrateClient<C, EC, T>
where
	C: subxt::Config<Hash = subxt::utils::H256, Header = H>,
	H: subxt::config::Header<Number = u32> + Decode + Encode + Send + Sync,
	EC: LedgerEventCodec<Error = SubxtClientError, Source = subxt::events::EventDetails<C>>
		+ Send
		+ Sync,
	T: LedgerTransaction + Send + Sync,
{
	type Error = SubxtClientError;
	type Event = <EC as LedgerEventCodec>::Sink;
	type EventCodec = EC;

	// Get the block height of the ledger, and the id of the highest block
	async fn block_height(&self) -> Result<(Position, BlockId), Self::Error> {
		let block = self.client.blocks().at_latest().await?;

		Ok((Position::from(block.number()), BlockId::from(block.hash())))
	}

	/// Subscribe to state updates from this ledger, starting at `offset`, and
	/// ending the stream after `number_of_blocks` blocks have been processed.
	async fn state_updates(
		&self,
		// The block to start from
		from_block: FromBlock,
		// The number of blocks to process before ending the stream
		number_of_blocks: Option<u32>,
	) -> Result<BoxStream<LedgerEventContext<Self::Event>>, Self::Error> {
		// If fromblock is not head, then load in historical blocks and yield up to number_of_blocks
		// events
		let historical = match from_block {
			FromBlock::Head => stream::empty().boxed(),
			FromBlock::First => self
				.historical_events(self.client.backend().genesis_hash().await?, number_of_blocks)
				.await?
				.boxed(),
			FromBlock::BlockId(BlockId::Block(hash)) =>
				self.historical_events(hash, number_of_blocks).await?.boxed(),
			FromBlock::BlockId(BlockId::Unknown) => self
				.historical_events(self.client.backend().genesis_hash().await?, number_of_blocks)
				.await?
				.boxed(),
		};

		let all = historical.chain(self.stream_finalized_events().await?);

		//TODO: only take number_of_blocks worth of events before closing the stream

		Ok(all.boxed())
	}
}
