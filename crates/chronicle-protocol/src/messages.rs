use common::{
    k256::ecdsa::SigningKey,
    prov::{to_json_ld::ToJson, ChronicleTransaction},
};
use opa_tp_protocol::state::{policy_address, policy_meta_address};
use serde_json::json;

use crate::{
    address::SawtoothAddress,
    protocol::ProtocolError,
    sawtooth::submission::{BodyVariant, IdentityVariant},
    settings::sawtooth_settings_address,
    PROTOCOL_VERSION, SUBMISSION_BODY_VERSION,
};

use super::sawtooth::*;
use async_stl_client::{
    ledger::{LedgerTransaction, TransactionId},
    sawtooth::{MessageBuilder, TransactionPayload},
};
use prost::Message;

#[derive(Debug, Clone)]
pub struct ChronicleSubmitTransaction {
    pub tx: ChronicleTransaction,
    pub signer: SigningKey,
    pub policy_name: Option<String>,
}

#[async_trait::async_trait]
impl TransactionPayload for ChronicleSubmitTransaction {
    type Error = ProtocolError;

    /// Envelope a payload of `ChronicleOperations` and `SignedIdentity` in a `Submission` protocol buffer,
    /// along with placeholders for protocol version info and a tracing span id.
    async fn to_bytes(&self) -> Result<Vec<u8>, ProtocolError> {
        let mut submission = Submission {
            version: PROTOCOL_VERSION.to_string(),
            span_id: 0u64,
            ..Default::default()
        };

        let mut ops = Vec::with_capacity(self.tx.tx.len());
        for op in &self.tx.tx {
            let op_json = op.to_json().compact().await?;
            ops.push(op_json);
        }

        let ops_json =
            serde_json::to_string(&json!({"version": SUBMISSION_BODY_VERSION, "ops": ops}))?;
        let identity_json = serde_json::to_string(&self.tx.identity)?;

        submission.body_variant = Some(BodyVariant::Body(BodyMessageV1 { payload: ops_json }));
        submission.identity_variant = Some(IdentityVariant::Identity(IdentityMessageV1 {
            payload: identity_json,
        }));
        Ok(submission.encode_to_vec())
    }
}

impl ChronicleSubmitTransaction {
    pub fn new(tx: ChronicleTransaction, signer: SigningKey, policy_name: Option<String>) -> Self {
        Self {
            tx,
            signer,
            policy_name,
        }
    }
}

#[async_trait::async_trait]
impl LedgerTransaction for ChronicleSubmitTransaction {
    fn signer(&self) -> &SigningKey {
        &self.signer
    }

    fn addresses(&self) -> Vec<String> {
        self.tx
            .tx
            .iter()
            .flat_map(|op| op.dependencies())
            .map(|dep| SawtoothAddress::from(&dep).to_string())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect()
    }

    async fn as_sawtooth_tx(
        &self,
        message_builder: &MessageBuilder,
    ) -> (async_stl_client::messages::Transaction, TransactionId) {
        //Ensure we append any opa policy binary address and meta address to the
        //list of addresses, along with the settings address
        let mut addresses: Vec<_> = self
            .addresses()
            .into_iter()
            .chain(vec![
                sawtooth_settings_address("chronicle.opa.policy_name"),
                sawtooth_settings_address("chronicle.opa.entrypoint"),
            ])
            .collect();

        if self.policy_name.is_some() {
            addresses = addresses
                .into_iter()
                .chain(vec![
                    policy_address(self.policy_name.as_ref().unwrap()),
                    policy_meta_address(self.policy_name.as_ref().unwrap()),
                ])
                .collect();
        }
        message_builder
            .make_sawtooth_transaction(addresses.clone(), addresses, vec![], self, self.signer())
            .await
    }
}
