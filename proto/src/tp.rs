use std::ops::Deref;

use common::{
    ledger::{Depdendencies, InputAddress, Processor, StateInput},
    models::ChronicleTransaction,
};
use crypto::digest::Digest;
use k256::ecdsa::VerifyingKey;
use sawtooth_sdk::{
    messages::processor::TpProcessRequest,
    processor::handler::{ApplyError, TransactionContext, TransactionHandler},
};

pub fn get_prefix() -> String {
    let mut sha = crypto::sha2::Sha512::new();
    sha.input_str("chronicle");
    sha.result_str()[..6].to_string()
}

pub struct ChronicleTransactionHandler {
    family_name: String,
    family_versions: Vec<String>,
    namespaces: Vec<String>,
}

impl ChronicleTransactionHandler {
    pub fn new() -> ChronicleTransactionHandler {
        ChronicleTransactionHandler {
            family_name: "chronicle".into(),
            family_versions: vec!["1.0".into()],
            namespaces: vec![get_prefix()],
        }
    }
}

impl TransactionHandler for ChronicleTransactionHandler {
    fn family_name(&self) -> String {
        self.family_name.clone()
    }

    fn family_versions(&self) -> Vec<String> {
        self.family_versions.clone()
    }

    fn namespaces(&self) -> Vec<String> {
        self.namespaces.clone()
    }

    fn apply(
        &self,
        request: &TpProcessRequest,
        context: &mut dyn TransactionContext,
    ) -> Result<(), ApplyError> {
        let _signer = request
            .header
            .clone()
            .map(|h| {
                VerifyingKey::from_sec1_bytes(
                    &hex::decode(h.signer_public_key)
                        .map_err(|e| ApplyError::InvalidTransaction(e.to_string()))?,
                )
                .map_err(|e| ApplyError::InvalidTransaction(e.to_string()))
            })
            .into_option()
            .ok_or(ApplyError::InvalidTransaction(String::from(
                "Invalid header, missing signer public key",
            )))?;

        let tx: ChronicleTransaction = serde_cbor::from_slice(request.get_payload())
            .map_err(|e| ApplyError::InternalError(e.to_string()))?;

        let (input, output) = tx.dependencies();

        let input = context
            .get_state_entries(&input.into_iter().map(|x| x.0).collect::<Vec<_>>())?
            .into_iter()
            .map(|(address, data)| StateInput::new(InputAddress(address), data));

        let output = tx.process(input)?;

        Ok(())
    }
}
