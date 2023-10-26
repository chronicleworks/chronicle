use std::sync::Arc;

use chronicle_signing::{BatcherKnownKeyNamesSigner, ChronicleSigning, SecretError};
use common::prov::{to_json_ld::ToJson, ChronicleTransaction};
use k256::ecdsa::VerifyingKey;
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
	pub signer: ChronicleSigning,
	pub policy_name: Option<String>,
}

#[async_trait::async_trait]
impl TransactionPayload for ChronicleSubmitTransaction {
	type Error = ProtocolError;

	/// Envelope a payload of `ChronicleOperations` and `SignedIdentity` in a `Submission` protocol
	/// buffer, along with placeholders for protocol version info and a tracing span id.
	async fn to_bytes(&self) -> Result<Vec<u8>, ProtocolError> {
		let mut submission = Submission {
			version: PROTOCOL_VERSION.to_string(),
			span_id: 0u64,
			..Default::default()
		};

		let mut ops = Vec::with_capacity(self.tx.tx.len());
		for op in &self.tx.tx {
			let op_json = op.to_json();
			let compact_json = op_json.compact_stable_order().await?;
			ops.push(compact_json);
		}

		let ops_json =
			serde_json::to_string(&json!({"version": SUBMISSION_BODY_VERSION, "ops": ops}))?;
		let identity_json = serde_json::to_string(&self.tx.identity)?;
		tracing::debug!(ops_json = %ops_json, identity_json = %identity_json);

		submission.body_variant = Some(BodyVariant::Body(BodyMessageV1 { payload: ops_json }));
		submission.identity_variant =
			Some(IdentityVariant::Identity(IdentityMessageV1 { payload: identity_json }));
		Ok(submission.encode_to_vec())
	}
}

impl ChronicleSubmitTransaction {
	pub fn new(
		tx: ChronicleTransaction,
		signer: ChronicleSigning,
		policy_name: Option<String>,
	) -> Self {
		Self { tx, signer, policy_name }
	}
}

#[async_trait::async_trait]
impl LedgerTransaction for ChronicleSubmitTransaction {
	type Error = SecretError;

	async fn sign(&self, bytes: Arc<Vec<u8>>) -> Result<Vec<u8>, SecretError> {
		self.signer.batcher_sign(&bytes).await
	}

	async fn verifying_key(&self) -> Result<VerifyingKey, SecretError> {
		self.signer.batcher_verifying().await
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
	) -> Result<(async_stl_client::messages::Transaction, TransactionId), Self::Error> {
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
			.make_sawtooth_transaction(
				addresses.clone(),
				addresses,
				vec![],
				self,
				self.signer.batcher_verifying().await?,
				|bytes| {
					let signer = self.signer.clone();
					let bytes = bytes.to_vec();
					async move { signer.batcher_sign(&bytes).await }
				},
			)
			.await
	}
}
