use chronicle_signing::{
    ChronicleSigning, OwnedSecret, SecretError, BATCHER_NAMESPACE, BATCHER_PK,
};
use common::opa::{codec::OpaSubmissionV1, OpaSubmission};
use protocol_abstract::LedgerTransaction;
use subxt::ext::sp_core::{crypto::SecretStringError, Pair};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum TransactionError {
    #[error("Secret error: {0}")]
    SecretError(
        #[from]
        #[source]
        SecretError,
    ),
    #[error("Secret string error: {0}")]
    SecretStringError(
        #[from]
        #[source]
        SecretStringError,
    ),
}

#[derive(Clone)]
// Note, the subxt client requires synchronous, infallible access to the signing keypair, so we
// extract it on construction
pub enum OpaTransaction {
    BootstrapRoot(OpaSubmission, ChronicleSigning, subxt::ext::sp_core::ecdsa::Pair),
    RotateRoot(OpaSubmission, ChronicleSigning, subxt::ext::sp_core::ecdsa::Pair),
    RegisterKey(OpaSubmission, ChronicleSigning, String, bool, subxt::ext::sp_core::ecdsa::Pair),
    RotateKey(OpaSubmission, ChronicleSigning, String, subxt::ext::sp_core::ecdsa::Pair),
    SetPolicy(OpaSubmission, ChronicleSigning, String, subxt::ext::sp_core::ecdsa::Pair),
}

impl OpaTransaction {
    pub async fn bootstrap_root(
        opa_submission: OpaSubmission,
        signer: &ChronicleSigning,
    ) -> Result<Self, TransactionError> {
        Ok(Self::BootstrapRoot(
            opa_submission,
            signer.to_owned(),
            subxt::ext::sp_core::ecdsa::Pair::from_seed_slice(
                &signer.copy_signing_key(BATCHER_NAMESPACE, BATCHER_PK).await?.to_bytes(),
            )?,
        ))
    }

    pub async fn rotate_root(
        opa_submission: OpaSubmission,
        signer: &ChronicleSigning,
    ) -> Result<Self, TransactionError> {
        Ok(Self::RotateRoot(
            opa_submission,
            signer.to_owned(),
            subxt::ext::sp_core::ecdsa::Pair::from_seed_slice(
                &signer.copy_signing_key(BATCHER_NAMESPACE, BATCHER_PK).await?.to_bytes(),
            )?,
        ))
    }

    pub async fn register_key(
        name: impl AsRef<str>,
        opa_submission: OpaSubmission,
        signer: &ChronicleSigning,
        overwrite_existing: bool,
    ) -> Result<Self, TransactionError> {
        Ok(Self::RegisterKey(
            opa_submission,
            signer.to_owned(),
            name.as_ref().to_owned(),
            overwrite_existing,
            subxt::ext::sp_core::ecdsa::Pair::from_seed_slice(
                &signer.copy_signing_key(BATCHER_NAMESPACE, BATCHER_PK).await?.to_bytes(),
            )?,
        ))
    }

    pub async fn rotate_key(
        name: impl AsRef<str>,
        opa_submission: OpaSubmission,
        signer: &ChronicleSigning,
    ) -> Result<Self, TransactionError> {
        Ok(Self::RegisterKey(
            opa_submission,
            signer.to_owned(),
            name.as_ref().to_owned(),
            false,
            subxt::ext::sp_core::ecdsa::Pair::from_seed_slice(
                &signer.copy_signing_key(BATCHER_NAMESPACE, BATCHER_PK).await?.to_bytes(),
            )?,
        ))
    }

    pub async fn set_policy(
        name: impl AsRef<str>,
        opa_submission: OpaSubmission,
        signer: &ChronicleSigning,
    ) -> Result<Self, TransactionError> {
        Ok(Self::SetPolicy(
            opa_submission,
            signer.to_owned(),
            name.as_ref().to_owned(),
            subxt::ext::sp_core::ecdsa::Pair::from_seed_slice(
                &signer.copy_signing_key(BATCHER_NAMESPACE, BATCHER_PK).await?.to_bytes(),
            )?,
        ))
    }

    pub fn account_key(&self) -> &subxt::ext::sp_core::ecdsa::Pair {
        match self {
            OpaTransaction::BootstrapRoot(_, _, k) => k,
            OpaTransaction::RotateRoot(_, _, k) => k,
            OpaTransaction::RegisterKey(_, _, _, _, k) => k,
            OpaTransaction::RotateKey(_, _, _, k) => k,
            OpaTransaction::SetPolicy(_, _, _, k) => k,
        }
    }

    pub fn submission(&self) -> &OpaSubmission {
        match self {
            OpaTransaction::BootstrapRoot(o, _, _) => o,
            OpaTransaction::RotateRoot(o, _, _) => o,
            OpaTransaction::RegisterKey(o, _, _, _, _) => o,
            OpaTransaction::RotateKey(o, _, _, _) => o,
            OpaTransaction::SetPolicy(o, _, _, _) => o,
        }
    }
}

#[async_trait::async_trait]
impl LedgerTransaction for OpaTransaction {
    type Error = SecretError;
    type Payload = OpaSubmissionV1;

    async fn as_payload(&self) -> Result<Self::Payload, Self::Error> {
        Ok(match self.clone() {
            OpaTransaction::BootstrapRoot(o, _, _) => o,
            OpaTransaction::RotateRoot(o, _, _) => o,
            OpaTransaction::RegisterKey(o, _, _, _, _) => o,
            OpaTransaction::RotateKey(o, _, _, _) => o,
            OpaTransaction::SetPolicy(o, _, _, _) => o,
        }
            .into())
    }

    fn correlation_id(&self) -> [u8; 16] {
        match self {
            OpaTransaction::BootstrapRoot(o, _, _) => o.correlation_id,
            OpaTransaction::RotateRoot(o, _, _) => o.correlation_id,
            OpaTransaction::RegisterKey(o, _, _, _, _) => o.correlation_id,
            OpaTransaction::RotateKey(o, _, _, _) => o.correlation_id,
            OpaTransaction::SetPolicy(o, _, _, _) => o.correlation_id,
        }
    }
}
