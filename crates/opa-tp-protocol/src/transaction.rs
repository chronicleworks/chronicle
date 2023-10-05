use std::{convert::Infallible, sync::Arc};

use async_stl_client::{
    ledger::{LedgerTransaction, TransactionId},
    sawtooth::MessageBuilder,
};
use chronicle_signing::{BatcherKnownKeyNamesSigner, ChronicleSigning, SecretError};
use k256::ecdsa::VerifyingKey;
use prost::Message;

use crate::{
    async_stl_client::sawtooth::TransactionPayload,
    messages::Submission,
    state::{key_address, policy_address, policy_meta_address},
};

#[derive(Debug, Clone)]
pub enum OpaSubmitTransaction {
    BootstrapRoot(Submission, ChronicleSigning),
    RotateRoot(Submission, ChronicleSigning),
    RegisterKey(Submission, ChronicleSigning, String, bool),
    RotateKey(Submission, ChronicleSigning, String),
    SetPolicy(Submission, ChronicleSigning, String),
}

impl OpaSubmitTransaction {
    pub fn bootstrap_root(submission: Submission, sawtooth_signer: &ChronicleSigning) -> Self {
        Self::BootstrapRoot(submission, sawtooth_signer.to_owned())
    }

    pub fn rotate_root(submission: Submission, sawtooth_signer: &ChronicleSigning) -> Self {
        Self::RotateRoot(submission, sawtooth_signer.to_owned())
    }

    pub fn register_key(
        name: impl AsRef<str>,
        submission: Submission,
        sawtooth_signer: &ChronicleSigning,
        overwrite_existing: bool,
    ) -> Self {
        Self::RegisterKey(
            submission,
            sawtooth_signer.to_owned(),
            name.as_ref().to_owned(),
            overwrite_existing,
        )
    }

    pub fn rotate_key(
        name: impl AsRef<str>,
        submission: Submission,
        sawtooth_signer: &ChronicleSigning,
    ) -> Self {
        Self::RegisterKey(
            submission,
            sawtooth_signer.to_owned(),
            name.as_ref().to_owned(),
            false,
        )
    }

    pub fn set_policy(
        name: impl AsRef<str>,
        submission: Submission,
        sawtooth_signer: &ChronicleSigning,
    ) -> Self {
        Self::SetPolicy(
            submission,
            sawtooth_signer.to_owned(),
            name.as_ref().to_owned(),
        )
    }
}

#[async_trait::async_trait]
impl TransactionPayload for OpaSubmitTransaction {
    type Error = Infallible;

    /// Envelope a payload of `ChronicleOperations` and `SignedIdentity` in a `Submission` protocol buffer,
    /// along with placeholders for protocol version info and a tracing span id.
    async fn to_bytes(&self) -> Result<Vec<u8>, Infallible> {
        Ok(match self {
            Self::BootstrapRoot(submission, _) => submission,
            Self::RotateRoot(submission, _) => submission,
            Self::RegisterKey(submission, _, _, _) => submission,
            Self::RotateKey(submission, _, _) => submission,
            Self::SetPolicy(submission, _, _) => submission,
        }
        .encode_to_vec())
    }
}

#[async_trait::async_trait]
impl LedgerTransaction for OpaSubmitTransaction {
    type Error = SecretError;

    async fn sign(&self, bytes: Arc<Vec<u8>>) -> Result<Vec<u8>, Self::Error> {
        let signer = match self {
            Self::BootstrapRoot(_, signer) => signer,
            Self::RotateRoot(_, signer) => signer,
            Self::RegisterKey(_, signer, _, _) => signer,
            Self::RotateKey(_, signer, _) => signer,
            Self::SetPolicy(_, signer, _) => signer,
        };
        signer.batcher_sign(&bytes).await
    }

    async fn verifying_key(&self) -> Result<VerifyingKey, Self::Error> {
        let signer = match self {
            Self::BootstrapRoot(_, signer) => signer,
            Self::RotateRoot(_, signer) => signer,
            Self::RegisterKey(_, signer, _, _) => signer,
            Self::RotateKey(_, signer, _) => signer,
            Self::SetPolicy(_, signer, _) => signer,
        };

        signer.batcher_verifying().await
    }

    fn addresses(&self) -> Vec<String> {
        match self {
            Self::BootstrapRoot(_, _) => {
                vec![key_address("root")]
            }
            Self::RotateRoot(_, _) => {
                vec![key_address("root")]
            }
            Self::RegisterKey(_, _, name, _) => {
                vec![key_address("root"), key_address(name.clone())]
            }
            Self::RotateKey(_, _, name) => {
                vec![key_address("root"), key_address(name.clone())]
            }
            Self::SetPolicy(_, _, name) => {
                vec![
                    key_address("root"),
                    policy_meta_address(name.clone()),
                    policy_address(name.clone()),
                ]
            }
        }
    }

    async fn as_sawtooth_tx(
        &self,
        message_builder: &MessageBuilder,
    ) -> Result<(async_stl_client::messages::Transaction, TransactionId), Self::Error> {
        let signer = match self {
            Self::BootstrapRoot(_, signer) => signer,
            Self::RotateRoot(_, signer) => signer,
            Self::RegisterKey(_, signer, _, _) => signer,
            Self::RotateKey(_, signer, _) => signer,
            Self::SetPolicy(_, signer, _) => signer,
        }
        .clone();

        message_builder
            .make_sawtooth_transaction(
                self.addresses(),
                self.addresses(),
                vec![],
                self,
                signer.batcher_verifying().await?,
                |bytes| {
                    let signer = signer.clone();
                    let bytes = bytes.to_vec();
                    async move { signer.batcher_sign(&bytes).await }
                },
            )
            .await
    }
}
