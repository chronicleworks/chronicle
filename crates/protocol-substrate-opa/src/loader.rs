use common::opa::{
	codec::PolicyV1,
	std::{PolicyLoader, PolicyLoaderError},
	OpaSettings,
};
use opa::bundle::Bundle;
use protocol_substrate::{SubstrateClient, SubxtClientError};
use subxt::{
	ext::{scale_value::Composite, sp_core::blake2_128},
	PolkadotConfig,
};
use tracing::{debug, error, info, instrument, warn};

use crate::{transaction::OpaTransaction, OpaEventCodec, OpaSubstrateClient};

pub struct SubstratePolicyLoader {
	settings: OpaSettings,
	policy: Option<Vec<u8>>,
	client: OpaSubstrateClient<protocol_substrate::PolkadotConfig>,
	addr_string: String,
}

impl SubstratePolicyLoader {
	pub fn new(
		settings: OpaSettings,
		client: &SubstrateClient<PolkadotConfig, OpaEventCodec<PolkadotConfig>, OpaTransaction>,
	) -> Self {
		Self {
			addr_string: settings.policy_address.to_string(),
			settings,
			policy: None,
			client: client.clone(),
		}
	}

	#[instrument(level = "debug", skip(self), fields(
        policy_address = % self.settings.policy_address, entrypoint = % self.settings.entrypoint
    ))]
	async fn load_bundle_from_chain(&mut self) -> Result<Vec<u8>, SubxtClientError> {
		if let Some(policy) = self.policy.as_ref() {
			return Ok(policy.clone());
		}
		let load_policy_from = self.settings.policy_address;
		debug!(policy_address=?load_policy_from, "Loading policy from address");
		let load_policy_from = subxt::ext::scale_value::serde::to_value(load_policy_from)?;
		loop {
			tracing::debug!(target: "protocol_substrate_opa::loader", "Loading policy from storage.");
			let call = subxt::dynamic::runtime_api_call(
				"Opa",
				"get_policy",
				Composite::unnamed(vec![load_policy_from.clone()]),
			);

			let policy: PolicyV1 = self
				.client
				.client
				.runtime_api()
				.at_latest()
				.await?
				.call(call)
				.await
				.map_err(SubxtClientError::from)
				.and_then(|r| r.as_type::<PolicyV1>().map_err(SubxtClientError::from))?;

			if let Some(policy) = Some(policy) {
				return Ok(policy.into_vec());
			} else {
				warn!("Policy not found, retrying in 2 seconds");
				tokio::time::sleep(std::time::Duration::from_secs(2)).await;
				continue;
			}
		}
	}
}

#[async_trait::async_trait]
impl PolicyLoader for SubstratePolicyLoader {
	fn set_address(&mut self, _address: &str) {
		unimplemented!()
	}

	fn set_rule_name(&mut self, _name: &str) {
		unimplemented!()
	}

	fn set_entrypoint(&mut self, _entrypoint: &str) {
		unimplemented!()
	}

	fn get_address(&self) -> &str {
		&self.addr_string
	}

	fn get_rule_name(&self) -> &str {
		&self.settings.policy_name
	}

	fn get_entrypoint(&self) -> &str {
		&self.settings.entrypoint
	}

	fn get_policy(&self) -> &[u8] {
		self.policy.as_ref().unwrap()
	}

	async fn load_policy(&mut self) -> Result<(), PolicyLoaderError> {
		let bundle = self
			.load_bundle_from_chain()
			.await
			.map_err(|e| PolicyLoaderError::Substrate(e.into()))?;
		info!(fetched_policy_bytes=?bundle.len(), "Fetched policy");
		if bundle.is_empty() {
			error!("Policy not found: {}", self.get_rule_name());
			return Err(PolicyLoaderError::MissingPolicy(self.get_rule_name().to_string()));
		}
		self.load_policy_from_bundle(&Bundle::from_bytes(&*bundle)?)
	}

	fn load_policy_from_bytes(&mut self, policy: &[u8]) {
		self.policy = Some(policy.to_vec())
	}

	fn hash(&self) -> String {
		hex::encode(blake2_128(self.policy.as_ref().unwrap()))
	}
}
