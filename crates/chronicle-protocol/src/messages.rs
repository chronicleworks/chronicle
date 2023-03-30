use common::{
    k256::ecdsa::SigningKey,
    prov::{to_json_ld::ToJson, ChronicleTransaction},
};

use crate::{address::SawtoothAddress, protocol::ProtocolError, PROTOCOL_VERSION};

use super::sawtooth::*;
use async_sawtooth_sdk::{
    ledger::{LedgerTransaction, TransactionId},
    sawtooth::{MessageBuilder, TransactionPayload},
};
use prost::Message;
use sawtooth_sdk::messages::transaction::Transaction;

#[derive(Debug, Clone)]
pub struct ChronicleSubmitTransaction {
    pub tx: ChronicleTransaction,
    pub signer: SigningKey,
    pub on_chain_opa_policy: Option<(String, String)>,
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
            let op_json = op.to_json();
            let compact_json_string = op_json.compact().await?.0.to_string();
            // using `unwrap` to work around `MessageBuilder::make_sawtooth_transaction`,
            // which calls here from `sawtooth-protocol::messages` being non-fallible
            ops.push(compact_json_string);
        }
        submission.body = ops;

        let identity = serde_json::to_string(&self.tx.identity)?;
        submission.identity = identity;

        if let Some((policy_id, policy_entrypoint)) = &self.on_chain_opa_policy {
            submission.policy = Some(OpaPolicy {
                id: policy_id.clone(),
                entrypoint: policy_entrypoint.clone(),
                ..Default::default()
            });
        }

        Ok(submission.encode_to_vec())
    }
}

impl ChronicleSubmitTransaction {
    pub fn new(
        tx: ChronicleTransaction,
        signer: SigningKey,
        on_chain_opa_policy: Option<(String, String)>,
    ) -> Self {
        Self {
            tx,
            signer,
            on_chain_opa_policy,
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
    ) -> (Transaction, TransactionId) {
        //Ensure we append any opa policy binary address and meta address to the list of addresses
        let addresses: Vec<_> = self
            .addresses()
            .into_iter()
            .chain(
                self.on_chain_opa_policy
                    .as_ref()
                    .map(|x| &x.0)
                    .map(|x| {
                        vec![
                            opa_tp_protocol::state::policy_address(x),
                            opa_tp_protocol::state::policy_meta_address(x),
                        ]
                    })
                    .unwrap_or_default(),
            )
            .collect();
        message_builder
            .make_sawtooth_transaction(addresses.clone(), addresses, vec![], self, self.signer())
            .await
    }
}
