#![cfg_attr(not(feature = "std"), no_std)]

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
use parity_scale_codec::MaxEncodedLen;
use scale_info::TypeInfo;
pub use weights::*;

#[derive(parity_scale_codec::Encode, parity_scale_codec::Decode, TypeInfo, Debug)]
pub struct StorageAddress(String);

impl From<&LedgerAddress> for StorageAddress {
    fn from(value: &LedgerAddress) -> Self {
        StorageAddress(format!("{}", value))
    }
}

impl MaxEncodedLen for StorageAddress {
    fn max_encoded_len() -> usize {
        2048usize
    }
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::{OptionQuery, *};
    use frame_system::pallet_prelude::*;

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
    pub type Provenance<T> = StorageMap<_, Twox128, super::StorageAddress, common::prov::ProvModel>;

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
        /// Error names should be descriptive.
        NoneValue,
        /// Errors should have helpful documentation associated with them.
        StorageOverflow,
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

            let deps = ops
                .iter()
                .flat_map(|tx| tx.dependencies())
                .collect::<std::collections::HashSet<_>>();

            let addresses_to_load = deps.iter().map(StorageAddress::from).collect::<Vec<_>>();

            let input_models: Vec<common::prov::ProvModel> = addresses_to_load
                .iter()
                .filter_map(Provenance::<T>::get)
                .collect();

            let prov_before_application = common::prov::ProvModel::apply(input_models);

            // Update storage.

            // Emit an event.
            Self::deposit_event(Event::Applied(common::prov::ProvModel::default()));
            // Return a successful DispatchResultWithPostInfo
            Ok(())
        }
    }
}
