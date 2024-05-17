#![cfg_attr(not(feature = "std"), no_std)]

use core::convert::Infallible;

/// Re-export types required for runtime
pub use common::prov::*;
use common::{
    k256::ecdsa::{Signature, VerifyingKey},
    opa::{
        codec::{NewPublicKeyV1, OpaSubmissionV1, PayloadV1, SignedOperationV1},
        BootstrapRoot, KeyAddress, KeyRegistration, Keys, OpaSubmission, Operation, Payload,
        PolicyAddress, PolicyMeta, PolicyMetaAddress, RegisterKey, RotateKey, SetPolicy,
        SignedOperation, SignedOperationPayload,
    },
};

use scale_info::prelude::format;

pub fn policy_address(id: impl AsRef<str>) -> PolicyAddress {
    blake2_128(format!("opa:policy:binary:{}", id.as_ref()).as_bytes()).into()
}

pub fn policy_meta_address(id: impl AsRef<str>) -> PolicyMetaAddress {
    blake2_128(format!("opa:policy:meta:{}", id.as_ref()).as_bytes()).into()
}

pub fn key_address(id: impl AsRef<str>) -> KeyAddress {
    blake2_128(format!("opa:keys:{}", id.as_ref()).as_bytes()).into()
}

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://docs.substrate.io/reference/frame-pallets/>
pub use pallet::*;

//#[cfg(feature = "runtime-benchmarks")]
//mod benchmarking;
pub mod weights;

pub mod opa_core {
    pub use common::{ledger::*, opa::*};
}

use parity_scale_codec::Encode;
use sp_core_hashing::blake2_128;
use tracing::{error, instrument};
pub use weights::*;

#[derive(Debug)]
enum OpaError {
    OperationSignatureVerification,
    InvalidSigningKey,
    InvalidOperation,
}

impl From<Infallible> for OpaError {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}

// Verifies the submission.
// Keys == None indicates that the opa tp is not bootstrapped, so the bootstrap
// operation can be performed, otherwise this will be an error
// If the system has been bootstrapped, then the current key must match the signing
// key of the operation
#[instrument(skip(submission, root_keys), ret(Debug))]
fn verify_signed_operation<T: Config>(
    submission: &OpaSubmissionV1,
    root_keys: &Option<Keys>,
) -> Result<(), OpaError> {
    use k256::ecdsa::signature::Verifier;
    match &submission.payload {
        PayloadV1::BootstrapRoot(_) => Ok(()),
        PayloadV1::SignedOperation(SignedOperationV1 { payload, verifying_key, signature }) => {
            if root_keys.is_none() {
                error!("No registered root keys for signature verification");
                return Err(OpaError::OperationSignatureVerification);
            }
            let payload_bytes = payload.encode();
            let signature: Signature = k256::ecdsa::signature::Signature::from_bytes(signature)
                .map_err(|e| {
                    error!(signature = ?signature, signature_load_error = ?e);
                    OpaError::OperationSignatureVerification
                })?;
            let signing_key = <VerifyingKey as k256::pkcs8::DecodePublicKey>::from_public_key_pem(
                verifying_key.as_str(),
            )
                .map_err(|e| {
                    error!(verifying_key = ?verifying_key, key_load_error = ?e);
                    OpaError::OperationSignatureVerification
                })?;
            if let Err(e) = signing_key.verify(&payload_bytes, &signature) {
                error!(signature = ?signature, verify_error = ?e);
                return Err(OpaError::OperationSignatureVerification);
            }

            if *verifying_key == root_keys.as_ref().unwrap().current.key {
                Ok(())
            } else {
                error!(verifying_key = ?verifying_key, current_key = ?root_keys.as_ref().unwrap().current.key, "Invalid signing key");
                Err(OpaError::InvalidSigningKey)
            }
        }
    }
}

// Either apply our bootstrap operation or our signed operation
#[instrument(skip(payload), ret(Debug))]
fn apply_signed_operation<T: Config>(
    correlation_id: ChronicleTransactionId,
    payload: Payload,
) -> Result<(), OpaError> {
    use scale_info::prelude::string::ToString;
    match payload {
        Payload::BootstrapRoot(BootstrapRoot { public_key }) => {
            let existing_key = pallet::KeyStore::<T>::try_get(key_address("root"));

            if existing_key.is_ok() {
                error!("OPA TP has already been bootstrapped");
                return Err(OpaError::InvalidOperation);
            }

            let keys = Keys {
                id: "root".to_string(),
                current: KeyRegistration { key: public_key, version: 0 },
                expired: None,
            };

            pallet::KeyStore::<T>::set(key_address("root"), Some(keys.clone().into()));

            pallet::Pallet::<T>::deposit_event(pallet::Event::<T>::KeyUpdate(
                keys.into(),
                correlation_id,
            ));

            Ok(())
        }
        Payload::SignedOperation(SignedOperation {
                                     payload: SignedOperationPayload { operation },
                                     verifying_key: _,
                                     signature: _,
                                 }) => apply_signed_operation_payload::<T>(correlation_id, operation),
    }
}

#[instrument(skip(payload), ret(Debug))]
fn apply_signed_operation_payload<T: Config>(
    correlation_id: ChronicleTransactionId,
    payload: Operation,
) -> Result<(), OpaError> {
    match payload {
        Operation::RegisterKey(RegisterKey { public_key, id, overwrite_existing }) => {
            if id == "root" {
                error!("Cannot register a key with the id 'root'");
                return Err(OpaError::InvalidOperation);
            }

            let existing_key = pallet::KeyStore::<T>::try_get(key_address(&id));

            if existing_key.is_ok() {
                if overwrite_existing {
                    tracing::debug!("Registration replaces existing key");
                } else {
                    error!("Key already registered");
                    return Err(OpaError::InvalidOperation);
                }
            }

            let keys = Keys {
                id,
                current: KeyRegistration { key: public_key, version: 0 },
                expired: None,
            };

            pallet::KeyStore::<T>::set(key_address(&keys.id), Some(keys.clone().into()));

            pallet::Pallet::<T>::deposit_event(pallet::Event::<T>::KeyUpdate(
                keys.into(),
                correlation_id,
            ));

            Ok(())
        }
        Operation::RotateKey(RotateKey {
                                 payload,
                                 previous_signing_key,
                                 previous_signature,
                                 new_signing_key,
                                 new_signature,
                             }) => {
            // Get current key registration from state
            let existing_key = pallet::KeyStore::<T>::try_get(key_address(&payload.id));

            if existing_key.is_err() {
                error!("No key to rotate");
                return Err(OpaError::InvalidOperation);
            }

            let existing_key = existing_key.unwrap();

            if previous_signing_key != existing_key.current.key {
                error!("Key does not match current key");
                return Err(OpaError::InvalidOperation);
            }

            let payload_id = payload.id.clone();
            let payload_bytes: NewPublicKeyV1 = payload.into();
            // Verify the previous key and signature
            let payload_bytes = payload_bytes.encode();
            let previous_signature = Signature::try_from(&*previous_signature)
                .map_err(|_| OpaError::OperationSignatureVerification)?;
            let previous_key = <VerifyingKey as k256::pkcs8::DecodePublicKey>::from_public_key_pem(
                previous_signing_key.as_str(),
            )
                .map_err(|_| OpaError::OperationSignatureVerification)?;

            k256::ecdsa::signature::Verifier::verify(
                &previous_key,
                &payload_bytes,
                &previous_signature,
            )
                .map_err(|_| OpaError::OperationSignatureVerification)?;

            //Verify the new key and signature
            let new_signature = Signature::try_from(&*new_signature)
                .map_err(|_| OpaError::OperationSignatureVerification)?;
            let new_key = <VerifyingKey as k256::pkcs8::DecodePublicKey>::from_public_key_pem(
                new_signing_key.as_str(),
            )
                .map_err(|_| OpaError::OperationSignatureVerification)?;

            k256::ecdsa::signature::Verifier::verify(&new_key, &payload_bytes, &new_signature)
                .map_err(|_| OpaError::OperationSignatureVerification)?;

            //Store new keys
            let keys = Keys {
                id: payload_id,
                current: KeyRegistration {
                    key: new_signing_key,
                    version: existing_key.current.version + 1,
                },
                expired: Some(KeyRegistration {
                    key: previous_signing_key,
                    version: existing_key.current.version,
                }),
            };

            pallet::KeyStore::<T>::set(key_address(&keys.id), Some(keys.clone().into()));

            pallet::Pallet::<T>::deposit_event(pallet::Event::<T>::KeyUpdate(
                keys.into(),
                correlation_id,
            ));

            Ok(())
        }
        Operation::SetPolicy(SetPolicy { policy, id }) => {
            let hash = sp_core_hashing::blake2_128(policy.as_bytes());

            let meta = PolicyMeta {
                id: id.clone(),
                hash: hash.into(),
                policy_address: policy_address(&*id),
            };

            pallet::PolicyMetaStore::<T>::set(policy_meta_address(&*id), Some(meta.clone().into()));

            pallet::PolicyStore::<T>::set(policy_address(&*id), Some(policy.into()));

            pallet::Pallet::<T>::deposit_event(pallet::Event::<T>::PolicyUpdate(
                meta.into(),
                correlation_id,
            ));

            Ok(())
        }
    }
}

fn root_keys_from_state<T: Config>() -> Result<Option<Keys>, OpaError> {
    let existing_key = pallet::KeyStore::<T>::try_get(key_address("root"));

    if let Ok(existing_key) = existing_key {
        Ok(Some(existing_key.try_into()?))
    } else {
        Ok(None)
    }
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
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

        type OpaSubmission: Parameter
        + Into<common::opa::codec::OpaSubmissionV1>
        + parity_scale_codec::Codec;
    }

    // The pallet's runtime storage items.
    // https://docs.substrate.io/main-docs/build/runtime-storage/
    #[pallet::storage]
    #[pallet::getter(fn get_policy)]
    // Learn more about declaring storage items:
    // https://docs.substrate.io/main-docs/build/runtime-storage/#declaring-storage-items
    pub type PolicyStore<T> =
    StorageMap<_, Twox128, common::opa::PolicyAddress, common::opa::codec::PolicyV1>;
    #[pallet::storage]
    #[pallet::getter(fn get_policy_meta)]
    pub type PolicyMetaStore<T> =
    StorageMap<_, Twox128, common::opa::PolicyMetaAddress, common::opa::codec::PolicyMetaV1>;
    #[pallet::storage]
    #[pallet::getter(fn get_key)]
    pub type KeyStore<T> =
    StorageMap<_, Twox128, common::opa::KeyAddress, common::opa::codec::KeysV1>;

    // Pallets use events to inform users when important changes are made.
    // https://docs.substrate.io/main-docs/build/events-errors/
    #[pallet::event]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {
        PolicyUpdate(common::opa::codec::PolicyMetaV1, ChronicleTransactionId),
        KeyUpdate(common::opa::codec::KeysV1, ChronicleTransactionId),
    }

    // Errors inform users that something went wrong.
    #[pallet::error]
    pub enum Error<T> {
        OperationSignatureVerification,
        InvalidSigningKey,
        JsonSerialize,
        InvalidOperation,
    }

    impl<T> From<OpaError> for Error<T> {
        fn from(error: OpaError) -> Self {
            match error {
                OpaError::OperationSignatureVerification => Error::OperationSignatureVerification,
                OpaError::InvalidSigningKey => Error::InvalidSigningKey,
                OpaError::InvalidOperation => Error::InvalidOperation,
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
        #[pallet::weight(T::WeightInfo::apply())]
        pub fn apply(origin: OriginFor<T>, submission: T::OpaSubmission) -> DispatchResult {
            // Check that the extrinsic was signed and get the signer.
            // This function will return an error if the extrinsic is not signed.
            // https://docs.substrate.io/main-docs/build/origins/
            let _who = ensure_signed(origin)?;

            // We need to validate the submission's own internal signatures at the codec level
            let submission: OpaSubmissionV1 = submission.into();

            super::verify_signed_operation::<T>(
                &submission,
                &super::root_keys_from_state::<T>().map_err(Error::<T>::from)?,
            )
                .map_err(Error::<T>::from)?;

            let submission: OpaSubmission = submission.into();

            super::apply_signed_operation::<T>(
                submission.correlation_id.into(),
                submission.payload,
            )
                .map_err(Error::<T>::from)?;

            // Return a successful DispatchResultWithPostInfo
            Ok(())
        }
    }
}
