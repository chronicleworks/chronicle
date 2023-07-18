use async_stl_client::{
    ledger::{LedgerTransaction, TransactionId},
    sawtooth::MessageBuilder,
};
use k256::ecdsa::SigningKey;

use crate::{
    messages::Submission,
    state::{key_address, policy_address, policy_meta_address},
};

#[derive(Debug, Clone)]
pub enum OpaSubmitTransaction {
    BootstrapRoot(Submission, SigningKey),
    RotateRoot(Submission, SigningKey),
    RegisterKey(Submission, SigningKey, String, bool),
    RotateKey(Submission, SigningKey, String),
    SetPolicy(Submission, SigningKey, String),
}

impl OpaSubmitTransaction {
    pub fn bootstrap_root(submission: Submission, sawtooth_signer: &SigningKey) -> Self {
        Self::BootstrapRoot(submission, sawtooth_signer.to_owned())
    }

    pub fn rotate_root(submission: Submission, sawtooth_signer: &SigningKey) -> Self {
        Self::RotateRoot(submission, sawtooth_signer.to_owned())
    }

    pub fn register_key(
        name: impl AsRef<str>,
        submission: Submission,
        sawtooth_signer: &SigningKey,
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
        sawtooth_signer: &SigningKey,
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
        sawtooth_signer: &SigningKey,
    ) -> Self {
        Self::SetPolicy(
            submission,
            sawtooth_signer.to_owned(),
            name.as_ref().to_owned(),
        )
    }
}

#[async_trait::async_trait]
impl LedgerTransaction for OpaSubmitTransaction {
    fn signer(&self) -> &SigningKey {
        match self {
            Self::BootstrapRoot(_, signer) => signer,
            Self::RotateRoot(_, signer) => signer,
            Self::RegisterKey(_, signer, _, _) => signer,
            Self::RotateKey(_, signer, _) => signer,
            Self::SetPolicy(_, signer, _) => signer,
        }
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
    ) -> (async_stl_client::messages::Transaction, TransactionId) {
        message_builder
            .make_sawtooth_transaction(
                self.addresses(),
                self.addresses(),
                vec![],
                match self {
                    Self::BootstrapRoot(submission, _) => submission,
                    Self::RotateRoot(submission, _) => submission,
                    Self::RegisterKey(submission, _, _, _) => submission,
                    Self::RotateKey(submission, _, _) => submission,
                    Self::SetPolicy(submission, _, _) => submission,
                },
                self.signer(),
            )
            .await
    }
}
