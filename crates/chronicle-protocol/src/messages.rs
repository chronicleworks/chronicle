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

        Ok(submission.encode_to_vec())
    }
}

impl ChronicleSubmitTransaction {
    pub fn new(tx: ChronicleTransaction, signer: SigningKey) -> Self {
        Self { tx, signer }
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
        message_builder
            .make_sawtooth_transaction(
                self.addresses(),
                self.addresses(),
                vec![],
                self,
                self.signer(),
            )
            .await
    }
}
