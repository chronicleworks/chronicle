use std::{collections::HashMap, sync::Arc};

use async_trait::*;
use secret_vault::{
    errors::{SecretVaultError, SecretVaultErrorPublicGenericDetails, SecretsSourceError},
    Secret, SecretMetadata, SecretVaultRef, SecretVaultResult, SecretsSource,
};
use secret_vault_value::SecretValue;
use tokio::sync::Mutex;
use tracing::*;
use url::Url;
use vaultrs::{
    client::{VaultClient, VaultClientSettingsBuilder},
    kv2,
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct VaultSecretManagerSourceOptions {
    pub vault_url: Url,
    pub token: String,
    pub mount_path: String,
}

impl VaultSecretManagerSourceOptions {
    pub fn new(vault_url: Url, token: &str, mount_path: &str) -> Self {
        VaultSecretManagerSourceOptions {
            vault_url,
            token: token.to_owned(),
            mount_path: mount_path.to_owned(),
        }
    }
}

#[derive(Clone)]
pub struct VaultSecretManagerSource {
    options: VaultSecretManagerSourceOptions,
    client: Arc<Mutex<VaultClient>>,
}

impl VaultSecretManagerSource {
    pub async fn with_options(options: VaultSecretManagerSourceOptions) -> SecretVaultResult<Self> {
        Ok(VaultSecretManagerSource {
            options: options.clone(),
            client: Arc::new(Mutex::new(
                VaultClient::new(
                    VaultClientSettingsBuilder::default()
                        .address(options.vault_url.as_str())
                        .token(options.token)
                        .build()
                        .unwrap(),
                )
                    .map_err(|e| {
                        SecretVaultError::SecretsSourceError(
                            SecretsSourceError::new(
                                SecretVaultErrorPublicGenericDetails::new(format!("{}", e)),
                                format!("Vault error: {}", e),
                            )
                                .with_root_cause(Box::new(e)),
                        )
                    })?,
            )),
        })
    }
}

#[async_trait]
impl SecretsSource for VaultSecretManagerSource {
    fn name(&self) -> String {
        "HashiVaultSecretManager".to_string()
    }

    async fn get_secrets(
        &self,
        references: &[SecretVaultRef],
    ) -> SecretVaultResult<HashMap<SecretVaultRef, Secret>> {
        let mut result_map: HashMap<SecretVaultRef, Secret> = HashMap::new();
        let client = &*self.client.lock().await;

        let mut results = vec![];
        for secret_ref in references {
            results.push((
                secret_ref.clone(),
                kv2::read(client, &self.options.mount_path, secret_ref.key.secret_name.as_ref())
                    .await,
            ));
        }

        for (secret_ref, result) in results {
            match result {
                Ok(vault_secret) => {
                    let metadata = SecretMetadata::create_from_ref(&secret_ref);
                    result_map.insert(
                        secret_ref.clone(),
                        Secret::new(SecretValue::new(vault_secret), metadata),
                    );
                }
                Err(err) => {
                    error!(
						"Unable to read secret or secret version {}:{}/{:?}: {}.",
						self.options.mount_path,
						&secret_ref.key.secret_name,
						&secret_ref.key.secret_version,
						err
					);
                    return Err(SecretVaultError::SecretsSourceError(SecretsSourceError::new(
                        SecretVaultErrorPublicGenericDetails::new(format!(
                            "Unable to read secret or secret version {}/{:?}: {}.",
                            self.options.mount_path, &secret_ref.key.secret_name, err
                        )),
                        format!(
                            "Unable to read secret or secret version {}/{:?}: {}.",
                            self.options.mount_path, &secret_ref.key.secret_name, err
                        ),
                    )));
                }
            }
        }

        Ok(result_map)
    }
}
