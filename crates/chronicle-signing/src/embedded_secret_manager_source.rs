use async_trait::async_trait;
use k256::{
	pkcs8::{EncodePrivateKey, LineEnding},
	SecretKey,
};
use rand::{rngs::StdRng, SeedableRng};
use secret_vault::{Secret, SecretMetadata, SecretVaultRef, SecretVaultResult, SecretsSource};
use secret_vault_value::SecretValue;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use tracing::debug;

use crate::SecretError;

pub struct EmbeddedSecretManagerSource {
	secrets: Arc<Mutex<HashMap<SecretVaultRef, Vec<u8>>>>,
	deterministic: bool,
}

impl EmbeddedSecretManagerSource {
	pub fn new() -> Self {
		Self { secrets: Arc::new(Mutex::new(HashMap::new())), deterministic: false }
	}

	pub fn new_deterministic() -> Self {
		Self { secrets: Arc::new(Mutex::new(HashMap::new())), deterministic: true }
	}
}

fn new_signing_key(deterministic: bool, index: usize) -> Result<Vec<u8>, SecretError> {
	let secret = if deterministic {
		SecretKey::random(StdRng::seed_from_u64(index as _))
	} else {
		SecretKey::random(StdRng::from_entropy())
	};
	let privpem = secret
		.to_pkcs8_pem(LineEnding::CRLF)
		.map_err(|_| SecretError::InvalidPrivateKey)?;

	Ok(privpem.as_bytes().into())
}

#[async_trait]
impl SecretsSource for EmbeddedSecretManagerSource {
	fn name(&self) -> String {
		"EmbeddedSecretManager".to_string()
	}

	// Simply create and cache a new signing key for each novel reference
	async fn get_secrets(
		&self,
		references: &[SecretVaultRef],
	) -> SecretVaultResult<HashMap<SecretVaultRef, Secret>> {
		debug!(get_secrets=?references, "Getting secrets from embedded source");

		let mut result_map: HashMap<SecretVaultRef, Secret> = HashMap::new();
		let mut secrets = self.secrets.lock().await;
		for (index, secret_ref) in references.iter().enumerate() {
			let secret = secrets.entry(secret_ref.clone()).or_insert_with(|| {
				let secret = new_signing_key(self.deterministic, index).unwrap();
				secret.to_vec()
			});

			let secret_value = SecretValue::from(secret);
			let metadata = SecretMetadata::create_from_ref(secret_ref);

			result_map.insert(secret_ref.clone(), Secret::new(secret_value, metadata));
		}

		Ok(result_map)
	}
}
