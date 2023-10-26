#![cfg_attr(not(feature = "std"), no_std)]

/// Re-export types required for runtime
pub use common::prov::*;

use common::ledger::LedgerAddress;
/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://docs.substrate.io/reference/frame-pallets/>
pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
pub mod weights;
pub use common::prov::*;
pub use weights::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use sp_std::collections::btree_set::BTreeSet;
	use sp_std::vec::Vec;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		/// Type representing the weight of this pallet
		type WeightInfo: WeightInfo;

		type OperationList: Parameter
			+ Into<Vec<common::prov::operations::ChronicleOperation>>
			+ From<Vec<common::prov::operations::ChronicleOperation>>
			+ parity_scale_codec::Codec;
	}
	// The pallet's runtime storage items.
	// https://docs.substrate.io/main-docs/build/runtime-storage/
	#[pallet::storage]
	#[pallet::getter(fn prov)]
	// Learn more about declaring storage items:
	// https://docs.substrate.io/main-docs/build/runtime-storage/#declaring-storage-items
	pub type Provenance<T> = StorageMap<_, Twox128, LedgerAddress, common::prov::ProvModel>;

	// Pallets use events to inform users when important changes are made.
	// https://docs.substrate.io/main-docs/build/events-errors/
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		Applied(common::prov::ProvModel),
		Contradiction(common::prov::Contradiction),
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		Address,
		Contradiction,
		Compaction,
		Expansion,
		Identity,
		IRef,
		NotAChronicleIri,
		MissingId,
		MissingProperty,
		NotANode,
		NotAnObject,
		OpaExecutor,
		SerdeJson,
		SubmissionFormat,
		Time,
		Tokio,
		Utf8,
	}

	impl<T> From<common::prov::ProcessorError> for Error<T> {
		fn from(error: common::prov::ProcessorError) -> Self {
			match error {
				common::prov::ProcessorError::Address => Error::Address,
				common::prov::ProcessorError::Contradiction { .. } => Error::Contradiction,
				common::prov::ProcessorError::Expansion { .. } => Error::Expansion,
				common::prov::ProcessorError::Identity(_) => Error::Identity,
				common::prov::ProcessorError::NotAChronicleIri { .. } => Error::NotAChronicleIri,
				common::prov::ProcessorError::MissingId { .. } => Error::MissingId,
				common::prov::ProcessorError::MissingProperty { .. } => Error::MissingProperty,
				common::prov::ProcessorError::NotANode(_) => Error::NotANode,
				common::prov::ProcessorError::NotAnObject => Error::NotAnObject,
				common::prov::ProcessorError::OpaExecutor(_) => Error::OpaExecutor,
				common::prov::ProcessorError::SerdeJson(_) => Error::SerdeJson,
				common::prov::ProcessorError::SubmissionFormat(_) => Error::SubmissionFormat,
				common::prov::ProcessorError::Time(_) => Error::Time,
				common::prov::ProcessorError::Tokio => Error::Tokio,
				common::prov::ProcessorError::Utf8(_) => Error::Utf8,
				_ => unreachable!(), //TODO: NOT THIS
			}
		}
	}

	// Dispatchable functions allows users to interact with the pallet and invoke state changes.
	// These functions materialize as "extrinsics", which are often compared to transactions.
	// Dispatchable functions must be annotated with a weight and must return a DispatchResult.
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// An example dispatchable that takes a singles value as a parameter, writes the value to
		/// storage and emits an event. This function must be dispatched by a signed extrinsic.
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::apply())]
		pub fn apply(origin: OriginFor<T>, operations: T::OperationList) -> DispatchResult {
			// Check that the extrinsic was signed and get the signer.
			// This function will return an error if the extrinsic is not signed.
			// https://docs.substrate.io/main-docs/build/origins/
			let who = ensure_signed(origin)?;

			// Get operations and load their dependencies
			let ops: Vec<common::prov::operations::ChronicleOperation> = operations.into();

			let deps = ops.iter().flat_map(|tx| tx.dependencies()).collect::<BTreeSet<_>>();

			let initial_input_models: Vec<_> = deps
				.into_iter()
				.map(|addr| (addr.clone(), Provenance::<T>::get(&addr)))
				.collect();

			let mut state: common::ledger::OperationState = common::ledger::OperationState::new();

			state.update_state(initial_input_models.into_iter());

			let mut model = common::prov::ProvModel::default();

			for op in ops {
				let res = op.process(model, state.input());
				match res {
					// A contradiction raises an event, not an error and shortcuts processing - contradiction attempts are useful provenance
					// and should not be a purely operational concern
					Err(common::prov::ProcessorError::Contradiction(source)) => {
						tracing::info!(contradiction = %source);

						Self::deposit_event(Event::<T>::Contradiction(source));

						return Ok(());
					},
					// Severe errors should be logged
					Err(e) => {
						tracing::error!(chronicle_prov_failure = %e);

						return Err(Error::<T>::from(e).into());
					},
					Ok((tx_output, updated_model)) => {
						state.update_state_from_output(tx_output.into_iter());
						model = updated_model;
					},
				}
			}

			// Compute delta
			let dirty = state.dirty().collect::<Vec<_>>();

			tracing::trace!(dirty = ?dirty);

			let mut delta = common::prov::ProvModel::default();
			for common::ledger::StateOutput { address, data } in dirty {
				delta.combine(&data);

				// Update storage.
				Provenance::<T>::set(&address, Some(data));
			}

			// Emit an event.
			Self::deposit_event(Event::Applied(delta));
			// Return a successful DispatchResultWithPostInfo
			Ok(())
		}
	}
}
