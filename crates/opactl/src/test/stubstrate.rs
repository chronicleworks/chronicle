use std::sync::{Arc, Mutex};

use frame_support::StoragePrefixedMap;
use futures::{stream::BoxStream, StreamExt};
use subxt::metadata::{DecodeWithMetadata, EncodeWithMetadata};

use common::opa::{codec::OpaSubmissionV1, Keys, PolicyMeta};
use pallet_opa::{ChronicleTransactionId, Event};
use protocol_abstract::{
	BlockId, FromBlock, LedgerEvent, LedgerEventContext, LedgerReader, LedgerTransaction,
	LedgerWriter, Position, Span, WriteConsistency,
};
use protocol_substrate::{PolkadotConfig, SubstrateStateReader, SubxtClientError};
use protocol_substrate_opa::{transaction::OpaTransaction, OpaEvent, OpaEventCodec};

use crate::test::mockchain::System;

use super::mockchain::{new_test_ext, OpaModule, RuntimeEvent, RuntimeOrigin, Test};

#[derive(Clone)]
pub struct Stubstrate {
	rt: Arc<Mutex<sp_io::TestExternalities>>,
	tx: tokio::sync::broadcast::Sender<OpaEvent>,
	events: Arc<Mutex<Vec<OpaEvent>>>,
}

impl Stubstrate {
	pub fn new() -> Self {
		let (tx, rx) = tokio::sync::broadcast::channel(100);
		Self { rt: Arc::new(Mutex::new(new_test_ext())), tx, events: Arc::new(Mutex::new(vec![])) }
	}

	#[tracing::instrument(skip(self))]
	pub fn readable_events(&self) -> Vec<OpaEvent> {
		self.events.lock().unwrap().clone()
	}

	pub fn stored_keys(&self) -> Vec<Keys> {
		self.rt.lock().unwrap().execute_with(|| {
			pallet_opa::KeyStore::<Test>::iter_values()
				.map(|k| k.try_into().unwrap())
				.collect()
		})
	}

	pub fn stored_policy(&self) -> Vec<PolicyMeta> {
		self.rt.lock().unwrap().execute_with(|| {
			pallet_opa::PolicyMetaStore::<Test>::iter_values()
				.map(|k| k.try_into().unwrap())
				.collect()
		})
	}
}

#[async_trait::async_trait]
impl LedgerReader for Stubstrate {
	type Error = SubxtClientError;
	type Event = OpaEvent;
	type EventCodec = OpaEventCodec<PolkadotConfig>;

	async fn block_height(&self) -> Result<(Position, BlockId), Self::Error> {
		unimplemented!();
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
		tracing::debug!("Starting state updates stream from block {:?}", from_block);
		let rx = self.tx.subscribe();
		let stream = tokio_stream::wrappers::BroadcastStream::new(rx)
			.map(|event| {
				let event = event.unwrap();
				let correlation_id = event.correlation_id().into();
				(event, correlation_id, BlockId::Unknown, Position::from(0), Span::NotTraced)
			})
			.boxed();
		Ok(stream)
	}
}

#[async_trait::async_trait]
impl LedgerWriter for Stubstrate {
	type Error = SubxtClientError;
	type Submittable = OpaTransaction;
	type Transaction = OpaTransaction;

	// Minimally process the transaction offline to get a transaction id and submittable type
	async fn pre_submit(
		&self,
		tx: Self::Transaction,
	) -> Result<(Self::Submittable, ChronicleTransactionId), Self::Error> {
		let id = tx.correlation_id().into();
		Ok((tx, id))
	}

	// Submit is used to submit a transaction to the ledger
	async fn do_submit(
		&self,
		_consistency: WriteConsistency,
		submittable: Self::Submittable,
	) -> Result<ChronicleTransactionId, (Self::Error, ChronicleTransactionId)> {
		self.rt.lock().unwrap().execute_with(|| {
			System::set_block_number(1);
			OpaModule::apply(
				RuntimeOrigin::signed(1),
				OpaSubmissionV1::from(submittable.submission().clone()),
			)
			.unwrap();

			let ev = System::events().last().unwrap().event.clone();

			let opa_event = match ev {
				RuntimeEvent::OpaModule(event) => match event {
					Event::<Test>::PolicyUpdate(meta, id) => Some(OpaEvent::PolicyUpdate {
						policy: meta.try_into().unwrap(),
						correlation_id: id,
					}),
					Event::<Test>::KeyUpdate(keys, id) => Some(OpaEvent::KeyUpdate {
						keys: keys.try_into().unwrap(),
						correlation_id: id,
					}),
					_ => None,
				},
				_ => None,
			};

			if let Some(event) = opa_event {
				self.events.lock().unwrap().push(event.clone());
				self.tx.send(event).unwrap();
			} else {
				tracing::warn!("Received an event that is not an OpaEvent");
			}
		});

		Ok(submittable.correlation_id().into())
	}
}

#[async_trait::async_trait]
impl SubstrateStateReader for Stubstrate {
	type Error = SubxtClientError;

	async fn get_state_entry<K: EncodeWithMetadata + Send + Sync, V: DecodeWithMetadata>(
		&self,
		pallet_name: &str,
		entry_name: &str,
		address: K,
	) -> Result<Option<V>, Self::Error> {
		tracing::info!(
			"Attempting to retrieve state entry for pallet: {}, entry: {}",
			pallet_name,
			entry_name
		);
		unimplemented!()
	}
}
