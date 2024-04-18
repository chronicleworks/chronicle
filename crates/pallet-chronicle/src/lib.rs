#![cfg_attr(not(feature = "std"), no_std)]

/// Re-export types required for runtime
pub use common::prov::*;

use common::ledger::ChronicleAddress;
/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://docs.substrate.io/reference/frame-pallets/>
pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

//#[cfg(feature = "runtime-benchmarks")]
//mod benchmarking;
pub mod weights;

pub mod chronicle_core {
	pub use common::{ledger::*, prov::*};
}
pub use weights::*;

// A configuration type for opa settings, serializable to JSON etc
#[derive(frame_support::Serialize, frame_support::Deserialize)]
pub struct OpaConfiguration {
	pub policy_name: scale_info::prelude::string::String,
	pub entrypoint: scale_info::prelude::string::String,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use common::ledger::OperationSubmission;
	use frame_support::{pallet_prelude::*, traits::BuildGenesisConfig};
	use frame_system::pallet_prelude::*;
	use sp_std::{collections::btree_set::BTreeSet, vec::Vec};

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		/// Type representing the weight of this pallet
		type WeightInfo: WeightInfo;

		type OperationSubmission: Parameter + Into<OperationSubmission> + parity_scale_codec::Codec;
	}

	/// Genesis configuration, whether or not we need to enforce OPA policies
	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub opa_settings: Option<OpaConfiguration>,
		pub _phantom: PhantomData<T>,
	}

	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self { opa_settings: None, _phantom: PhantomData }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			tracing::info!("Chronicle: Building genesis configuration.");
			if let Some(ref settings) = self.opa_settings {
				OpaSettings::<T>::put(Some(common::opa::OpaSettings {
					policy_address: common::opa::PolicyAddress::from(sp_core_hashing::blake2_128(
						settings.policy_name.as_bytes(),
					)),
					policy_name: settings.policy_name.clone(),
					entrypoint: settings.entrypoint.clone(),
				}));
				tracing::debug!("Chronicle: OPA settings are set.");
			} else {
				OpaSettings::<T>::put(None::<common::opa::OpaSettings>);
			}
		}
	}

	#[pallet::storage]
	#[pallet::getter(fn prov)]
	pub type Provenance<T> = StorageMap<_, Twox128, ChronicleAddress, common::prov::ProvModel>;

	#[pallet::storage]
	#[pallet::getter(fn get_opa_settings)]
	pub type OpaSettings<T> = StorageValue<_, Option<common::opa::OpaSettings>>;

	// Pallets use events to inform users when important changes are made.
	// https://docs.substrate.io/main-docs/build/events-errors/
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		Applied(common::prov::ProvModel, common::identity::SignedIdentity, [u8; 16]),
		Contradiction(common::prov::Contradiction, common::identity::SignedIdentity, [u8; 16]),
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
				_ => unreachable!(),
			}
		}
	}

	// Dispatchable functions allows users to interact with the pallet and invoke state changes.
	// These functions materialize as "extrinsics", which are often compared to transactions.
	// Dispatchable functions must be annotated with a weight and must return a DispatchResult.
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		// Apply a vector of chronicle operations, yielding an event that indicates state change or
		// contradiction
		#[pallet::call_index(0)]
		#[pallet::weight({
			let weight = T::WeightInfo::operation_weight(&operations.items);
			let dispatch_class = DispatchClass::Normal;
			let pays_fee = Pays::No;
			(weight, dispatch_class, pays_fee)
		})]
		pub fn apply(origin: OriginFor<T>, operations: OperationSubmission) -> DispatchResult {
			// Check that the extrinsic was signed and get the signer.
			// This function will return an error if the extrinsic is not signed.
			// https://docs.substrate.io/main-docs/build/origins/
			let _who = ensure_signed(origin)?;

			// Get operations and load tßheir dependencies
			let deps = operations
				.items
				.iter()
				.flat_map(|tx| tx.dependencies())
				.collect::<BTreeSet<_>>();

			let initial_input_models: Vec<_> = deps
				.into_iter()
				.map(|addr| (addr.clone(), Provenance::<T>::get(&addr)))
				.collect();

			let mut state: common::ledger::OperationState = common::ledger::OperationState::new();

			state.update_state(initial_input_models.into_iter());

			let mut model = common::prov::ProvModel::default();

			for op in operations.items.iter() {
				let res = op.process(model, state.input());
				match res {
					// A contradiction raises an event, not an error and shortcuts processing -
					// contradiction attempts are useful provenance and should not be a purely
					// operational concern
					Err(common::prov::ProcessorError::Contradiction(source)) => {
						tracing::info!(contradiction = %source);

						Self::deposit_event(Event::<T>::Contradiction(
							source,
							(*operations.identity).clone(),
							operations.correlation_id,
						));

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
			Self::deposit_event(Event::Applied(
				delta,
				(*operations.identity).clone(),
				operations.correlation_id,
			));
			// Return a successful DispatchResultWithPostInfo
			Ok(())
		}
	}
}
