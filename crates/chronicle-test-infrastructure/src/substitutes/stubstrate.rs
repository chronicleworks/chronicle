use futures::{stream::BoxStream, StreamExt};
use pallet_chronicle::{chronicle_core::OperationSubmission, ChronicleTransactionId, Event};

use super::mockchain::{new_test_ext, ChronicleModule, RuntimeEvent, RuntimeOrigin, System, Test};
use protocol_abstract::{
	BlockId, FromBlock, LedgerEvent, LedgerEventContext, LedgerReader, LedgerWriter, Position, Span,
};
use protocol_substrate::{PolkadotConfig, SubstrateStateReader, SubxtClientError};
use protocol_substrate_chronicle::{
	protocol::WriteConsistency, ChronicleEvent, ChronicleEventCodec, ChronicleTransaction,
};
use std::sync::{Arc, Mutex};
use subxt::metadata::{DecodeWithMetadata, EncodeWithMetadata};

#[derive(Clone)]
pub struct Stubstrate {
	rt: Arc<Mutex<sp_io::TestExternalities>>,
	tx: tokio::sync::broadcast::Sender<ChronicleEvent>,
	events: Arc<Mutex<Vec<ChronicleEvent>>>,
}

impl Default for Stubstrate {
	fn default() -> Self {
		Self::new()
	}
}

impl Stubstrate {
	pub fn new() -> Self {
		let (tx, _rx) = tokio::sync::broadcast::channel(100);
		Self { rt: Arc::new(Mutex::new(new_test_ext())), tx, events: Arc::new(Mutex::new(vec![])) }
	}

	#[tracing::instrument(skip(self))]
	pub fn readable_events(&self) -> Vec<ChronicleEvent> {
		self.events.lock().unwrap().clone()
	}

	pub fn stored_prov(&self) -> Vec<pallet_chronicle::ProvModel> {
		self.rt.lock().unwrap().execute_with(|| {
			pallet_chronicle::Provenance::<Test>::iter_values()
				.map(|k| k.try_into().unwrap())
				.collect()
		})
	}
}

#[async_trait::async_trait]
impl LedgerReader for Stubstrate {
	type Error = SubxtClientError;
	type Event = ChronicleEvent;
	type EventCodec = ChronicleEventCodec<PolkadotConfig>;

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
	type Submittable = OperationSubmission;
	type Transaction = ChronicleTransaction;

	// Minimally process the transaction offline to get a transaction id and submittable type
	async fn pre_submit(
		&self,
		tx: Self::Transaction,
	) -> Result<(Self::Submittable, ChronicleTransactionId), Self::Error> {
		Ok((
			OperationSubmission {
				correlation_id: tx.correlation_id.into_bytes(),
				identity: tx.identity,
				items: tx.operations,
			},
			tx.correlation_id.into(),
		))
	}

	// Submit is used to submit a transaction to the ledger
	async fn do_submit(
		&self,
		consistency: WriteConsistency,
		submittable: Self::Submittable,
	) -> Result<ChronicleTransactionId, (Self::Error, ChronicleTransactionId)> {
		let correlation_id = submittable.correlation_id;
		self.rt.lock().unwrap().execute_with(|| {
			System::set_block_number(1);
			ChronicleModule::apply(RuntimeOrigin::signed(1), submittable).unwrap();

			let ev = System::events().last().unwrap().event.clone();

			let opa_event = match ev {
				RuntimeEvent::ChronicleModule(event) => match event {
					Event::<Test>::Applied(diff, identity, correlation_id) =>
						Some(ChronicleEvent::Committed { diff, identity, correlation_id }),
					Event::<Test>::Contradiction(contradiction, identity, correlation_id) =>
						Some(ChronicleEvent::Contradicted {
							contradiction,
							identity,
							correlation_id,
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

		Ok(correlation_id.into())
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
		unimplemented!()
	}
}
