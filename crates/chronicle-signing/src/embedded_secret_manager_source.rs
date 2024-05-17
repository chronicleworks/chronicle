use async_trait::async_trait;
use k256::{
    pkcs8::{EncodePrivateKey, LineEnding},
    SecretKey,
};
use rand::{rngs::StdRng, SeedableRng};
use secret_vault::{Secret, SecretMetadata, SecretVaultRef, SecretVaultResult, SecretsSource};
use secret_vault_value::SecretValue;
use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};
use tokio::sync::Mutex;
use tracing::debug;

use crate::SecretError;

pub struct EmbeddedSecretManagerSource {
    secrets: Arc<Mutex<HashMap<SecretVaultRef, Vec<u8>>>>,
    seeds: BTreeMap<String, [u8; 32]>,
}

impl EmbeddedSecretManagerSource {
    pub fn new() -> Self {
        Self { secrets: Arc::new(Mutex::new(HashMap::new())), seeds: BTreeMap::default() }
    }

    pub fn new_seeded(seeds: BTreeMap<String, [u8; 32]>) -> Self {
        Self { secrets: Arc::new(Mutex::new(HashMap::new())), seeds }
    }
}

fn new_signing_key(name: &str, seeds: &BTreeMap<String, [u8; 32]>) -> Result<Vec<u8>, SecretError> {
    let secret = if let Some(seed) = seeds.get(name) {
        SecretKey::from_be_bytes(seed).map_err(|_| SecretError::BadSeed)?
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
        for secret_ref in references.iter() {
            let secret = secrets.entry(secret_ref.clone()).or_insert_with(|| {
                let secret =
                    new_signing_key(secret_ref.key.secret_name.as_ref(), &self.seeds).unwrap();
                secret.to_vec()
            });

            let secret_value = SecretValue::from(secret);
            let metadata = SecretMetadata::create_from_ref(secret_ref);

            result_map.insert(secret_ref.clone(), Secret::new(secret_value, metadata));
        }

        Ok(result_map)
    }
}
