#![cfg_attr(feature = "strict", deny(warnings))]






pub use api::{Api, UuidGen};
pub use chronicle_persistence::Store;
pub use chronicle_persistence::StoreError;
use chronicle_signing::ChronicleKnownKeyNamesSigner;
use common::{
    identity::{AuthId, IdentityError, SignedIdentity},
};
pub use dispatch::ApiDispatch;
pub use error::ApiError;


pub mod chronicle_graphql;
pub mod commands;

pub mod import;
mod error;
mod api;
mod dispatch;

pub trait ChronicleSigned {
    /// Get the user identity's [`SignedIdentity`]
    fn signed_identity<S: ChronicleKnownKeyNamesSigner>(
        &self,
        store: &S,
    ) -> Result<SignedIdentity, IdentityError>;
}

impl ChronicleSigned for AuthId {
    fn signed_identity<S: ChronicleKnownKeyNamesSigner>(
        &self,
        store: &S,
    ) -> Result<SignedIdentity, IdentityError> {
        let signable = self.to_string();
        let signature = futures::executor::block_on(store.chronicle_sign(signable.as_bytes()))
            .map_err(|e| IdentityError::Signing(e.into()))?;
        let public_key = futures::executor::block_on(store.chronicle_verifying())
            .map_err(|e| IdentityError::Signing(e.into()))?;

        Ok(SignedIdentity {
            identity: signable,
            signature: signature.into(),
            verifying_key: Some(public_key.to_bytes().to_vec()),
        })
    }
}
