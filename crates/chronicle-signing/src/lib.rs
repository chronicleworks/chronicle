use k256::{
    ecdsa::{
        signature::{Signer, Verifier},
        Signature, SigningKey, VerifyingKey,
    }
};
use secret_vault::{
    errors::SecretVaultError, FilesSource, FilesSourceOptions, MultipleSecretsSources, SecretName,
    SecretNamespace, SecretVaultBuilder, SecretVaultRef, SecretVaultView,
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};
use thiserror::Error;
use tracing::instrument;
use url::Url;

mod embedded_secret_manager_source;
mod vault_secret_manager_source;

pub static CHRONICLE_NAMESPACE: &str = "chronicle";
pub static BATCHER_NAMESPACE: &str = "batcher";
pub static OPA_NAMESPACE: &str = "opa";
pub static CHRONICLE_PK: &str = "chronicle-pk";
pub static BATCHER_PK: &str = "batcher-pk";
pub static OPA_PK: &str = "opa-pk";

#[derive(Error, Debug)]
pub enum SecretError {
    #[error("Invalid public key")]
    InvalidPublicKey,
    #[error("Invalid private key")]
    InvalidPrivateKey,
    #[error("No public key found")]
    NoPublicKeyFound,
    #[error("No private key found")]
    NoPrivateKeyFound,
    #[error("Decoding failure")]
    DecodingFailure,

    #[error("Vault {source}")]
    SecretVault {
        #[from]
        #[source]
        source: SecretVaultError,
    },

    #[error("Bad BIP39 seed")]
    BadSeed,
}

pub enum ChronicleSecretsOptions {
    // Connect to hashicorp vault for secrets
    Vault(vault_secret_manager_source::VaultSecretManagerSourceOptions),
    // Generate secrets from entropy in memory on demand
    Embedded,

    //Seed secrets with name using a map of secret name to BIP39 seed phrase
    Seeded(BTreeMap<String, [u8; 32]>),
    //Filesystem based keys
    Filesystem(PathBuf),
}

impl ChronicleSecretsOptions {
    // Get secrets from Hashicorp vault
    pub fn stored_in_vault(
        vault_url: &Url,
        token: &str,
        mount_path: &str,
    ) -> ChronicleSecretsOptions {
        ChronicleSecretsOptions::Vault(
            vault_secret_manager_source::VaultSecretManagerSourceOptions::new(
                vault_url.clone(),
                token,
                mount_path,
            ),
        )
    }

    // Load secrets from filesystem at path
    pub fn stored_at_path(path: &Path) -> ChronicleSecretsOptions {
        ChronicleSecretsOptions::Filesystem(path.to_owned())
    }

    // Generate secrets in memory on demand
    pub fn generate_in_memory() -> ChronicleSecretsOptions {
        ChronicleSecretsOptions::Embedded
    }

    // Use supplied seeds, or fall back to entropy
    pub fn seeded(seeds: BTreeMap<String, [u8; 32]>) -> ChronicleSecretsOptions {
        ChronicleSecretsOptions::Seeded(seeds)
    }
}

#[derive(Clone)]
pub struct ChronicleSigning {
    vault: Arc<tokio::sync::Mutex<Box<dyn SecretVaultView + Send + Sync>>>,
}

impl core::fmt::Debug for ChronicleSigning {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ChronicleSecrets").finish()
    }
}

impl ChronicleSigning {
    pub async fn new(
        // Secrets are namespace / name pairs
        required_secret_names: Vec<(String, String)>,
        // Secret stores are namespaced
        options: Vec<(String, ChronicleSecretsOptions)>,
    ) -> Result<Self, SecretError> {
        let mut multi_source = MultipleSecretsSources::new();
        let required_secret_refs: Vec<_> = required_secret_names
            .into_iter()
            .map(|(namespace, name)| {
                SecretVaultRef::new(SecretName::new(name))
                    .with_namespace(SecretNamespace::new(namespace))
            })
            .collect();

        for options in options {
            match options {
                (namespace, ChronicleSecretsOptions::Embedded) => {
                    let source = embedded_secret_manager_source::EmbeddedSecretManagerSource::new();
                    multi_source =
                        multi_source.add_source(&SecretNamespace::new(namespace), source);
                }
                (namespace, ChronicleSecretsOptions::Seeded(seeds)) => {
                    let source =
                        embedded_secret_manager_source::EmbeddedSecretManagerSource::new_seeded(
                            seeds,
                        );
                    multi_source =
                        multi_source.add_source(&SecretNamespace::new(namespace), source);
                }
                (namespace, ChronicleSecretsOptions::Vault(options)) => {
                    let source =
                        vault_secret_manager_source::VaultSecretManagerSource::with_options(
                            options,
                        )
                            .await?;
                    multi_source =
                        multi_source.add_source(&SecretNamespace::new(namespace), source);
                }
                (namespace, ChronicleSecretsOptions::Filesystem(path)) => {
                    let source = FilesSource::with_options(FilesSourceOptions {
                        root_path: Some(path.into_boxed_path()),
                    });
                    multi_source =
                        multi_source.add_source(&SecretNamespace::new(namespace), source);
                }
            }
        }

        let vault = SecretVaultBuilder::with_source(multi_source)
            .with_secret_refs(required_secret_refs.iter().collect())
            .build()?;

        vault.refresh().await?;
        Ok(Self { vault: Arc::new(tokio::sync::Mutex::new(Box::new(vault.viewer()))) })
    }
}

#[async_trait::async_trait]
pub trait WithSecret {
    async fn with_signing_key<T, F>(
        &self,
        secret_namespace: &str,
        secret_name: &str,
        f: F,
    ) -> Result<T, SecretError>
        where
            F: Fn(SigningKey) -> T,
            F: Send,
            T: Send;
    async fn with_verifying_key<T, F>(
        &self,
        secret_namespace: &str,
        secret_name: &str,
        f: F,
    ) -> Result<T, SecretError>
        where
            F: Fn(VerifyingKey) -> T,
            F: Send,
            T: Send;

    async fn verifying_key(
        &self,
        secret_namespace: &str,
        secret_name: &str,
    ) -> Result<VerifyingKey, SecretError>;
}

#[async_trait::async_trait]
pub trait OwnedSecret {
    async fn copy_signing_key(
        &self,
        secret_namespace: &str,
        secret_name: &str,
    ) -> Result<SigningKey, SecretError>;
}

#[async_trait::async_trait]
impl<T: WithSecret + ?Sized + Send + Sync> OwnedSecret for T {
    async fn copy_signing_key(
        &self,
        secret_namespace: &str,
        secret_name: &str,
    ) -> Result<SigningKey, SecretError> {
        let secret =
            WithSecret::with_signing_key(self, secret_namespace, secret_name, |secret| secret)
                .await?;

        Ok(secret)
    }
}

#[async_trait::async_trait]
impl WithSecret for ChronicleSigning {
    async fn with_signing_key<T, F>(
        &self,
        secret_namespace: &str,
        secret_name: &str,
        f: F,
    ) -> Result<T, SecretError>
        where
            F: Fn(SigningKey) -> T,
            F: Send,
            T: Send,
    {
        let secret_ref = SecretVaultRef::new(SecretName::new(secret_name.to_owned()))
            .with_namespace(secret_namespace.into());
        let secret = self.vault.lock().await.require_secret_by_ref(&secret_ref).await?;

        let signing_result = secret.value.exposed_in_as_str(|secret| {
            (
                // Convert hex encoded seed to SigningKey
                hex::decode(secret.trim_start_matches("0x")).map_err(|_| SecretError::DecodingFailure).and_then(
                   |secret|  SigningKey::from_bytes(&secret)
                    .map_err(|_| SecretError::InvalidPrivateKey))
                    .map(&f),
                    secret
            )
        });

        Ok(signing_result?)
    }

    async fn with_verifying_key<T, F>(
        &self,
        secret_namespace: &str,
        secret_name: &str,
        f: F,
    ) -> Result<T, SecretError>
        where
            F: Fn(VerifyingKey) -> T,
            F: Send,
            T: Send,
    {
        let secret_ref = SecretVaultRef::new(SecretName::new(secret_name.to_owned()))
            .with_namespace(secret_namespace.into());
        let secret = self.vault.lock().await.require_secret_by_ref(&secret_ref).await?;

        let signing_result = secret.value.exposed_in_as_str(|secret| {
            (
                // Convert hex encoded seed to SigningKey
                hex::decode(secret.trim_start_matches("0x")).map_err(|_| SecretError::DecodingFailure).and_then(
                    |secret|  SigningKey::from_bytes(&secret)
                    .map_err(|_| SecretError::InvalidPrivateKey))
                    .map(|signing_key| f(signing_key.verifying_key())),
                    secret
            )
        });

        Ok(signing_result?)
    }

    async fn verifying_key(
        &self,
        secret_namespace: &str,
        secret_name: &str,
    ) -> Result<VerifyingKey, SecretError> {
        let secret_ref = SecretVaultRef::new(SecretName::new(secret_name.to_owned()))
            .with_namespace(secret_namespace.into());
        let secret = self.vault.lock().await.require_secret_by_ref(&secret_ref).await?;

        let key = secret.value.exposed_in_as_str(|secret| {
            (
                // Convert hex encoded seed to SigningKey
                hex::decode(secret.trim_start_matches("0x")).map_err(|_| SecretError::DecodingFailure).and_then(
                    |decoded_secret| SigningKey::from_bytes(&decoded_secret)
                    .map_err(|_| SecretError::InvalidPrivateKey))
                    .map(|signing_key| signing_key.verifying_key()),
                    secret
            )
        });

        Ok(key?)
    }
}

/// Trait for signing with a key known by chronicle
#[async_trait::async_trait]
pub trait ChronicleSigner {
    /// Sign data with the a known key and return a signature
    async fn sign(
        &self,
        secret_namespace: &str,
        secret_name: &str,
        data: &[u8],
    ) -> Result<Signature, SecretError>;

    /// Verify a signature with a known key
    async fn verify(
        &self,
        secret_namespace: &str,
        secret_name: &str,
        data: &[u8],
        signature: &[u8],
    ) -> Result<bool, SecretError>;
}

#[async_trait::async_trait]
impl<T: WithSecret + Send + Sync> ChronicleSigner for T {
    /// Sign data with the chronicle key and return a signature
    async fn sign(
        &self,
        secret_namespace: &str,
        secret_name: &str,
        data: &[u8],
    ) -> Result<Signature, SecretError> {
        self.with_signing_key(secret_namespace, secret_name, |signing_key| {
            let s: Signature = signing_key.sign(data);
            s
        })
            .await
    }

    /// Verify a signature with the chronicle key
    async fn verify(
        &self,
        secret_namespace: &str,
        secret_name: &str,
        data: &[u8],
        signature: &[u8],
    ) -> Result<bool, SecretError> {
        self.with_verifying_key(secret_namespace, secret_name, |verifying_key| {
            let signature: Signature = k256::ecdsa::signature::Signature::from_bytes(signature)
                .map_err(|_| SecretError::InvalidPublicKey)?;

            Ok(verifying_key.verify(data, &signature).is_ok())
        })
            .await?
    }
}

/// Trait for signing with a key known by the batcher
#[async_trait::async_trait]
pub trait BatcherKnownKeyNamesSigner {
    /// Sign data with the batcher key and return a signature in low-s form, as this
    /// is required by sawtooth for batcher signatures
    async fn batcher_sign(&self, data: &[u8]) -> Result<Vec<u8>, SecretError>;

    /// Verify a signature with the batcher key
    async fn batcher_verify(&self, data: &[u8], signature: &[u8]) -> Result<bool, SecretError>;

    /// Get the verifying key for the batcher key
    async fn batcher_verifying(&self) -> Result<VerifyingKey, SecretError>;
}

/// Trait for signing with a key known by chronicle
#[async_trait::async_trait]
pub trait ChronicleKnownKeyNamesSigner {
    /// Sign data with the chronicle key and return a signature
    async fn chronicle_sign(&self, data: &[u8]) -> Result<Vec<u8>, SecretError>;

    /// Verify a signature with the chronicle key
    async fn chronicle_verify(&self, data: &[u8], signature: &[u8]) -> Result<bool, SecretError>;

    /// Get the verifying key for the chronicle key
    async fn chronicle_verifying(&self) -> Result<VerifyingKey, SecretError>;
}

/// Trait for signing with a key known by OPA
#[async_trait::async_trait]
pub trait OpaKnownKeyNamesSigner {
    /// Sign data with the OPA key and return a signature
    async fn opa_sign(&self, data: &[u8]) -> Result<Vec<u8>, SecretError>;

    /// Verify a signature with the OPA key
    async fn opa_verify(&self, data: &[u8], signature: &[u8]) -> Result<bool, SecretError>;

    /// Get the verifying key for the OPA key
    async fn opa_verifying(&self) -> Result<VerifyingKey, SecretError>;
}

#[async_trait::async_trait]
impl<T: ChronicleSigner + WithSecret + Send + Sync> BatcherKnownKeyNamesSigner for T {
    // Sign with the batcher key and return a signature in low-s form, as this
    // is required by sawtooth for batcher signatures
    #[instrument(skip(self, data), level = "trace", name = "batcher_sign", fields(
        namespace = BATCHER_NAMESPACE, pk = BATCHER_PK
    ))]
    async fn batcher_sign(&self, data: &[u8]) -> Result<Vec<u8>, SecretError> {
        let s = self.sign(BATCHER_NAMESPACE, BATCHER_PK, data).await?;

        let s = s.normalize_s().unwrap_or(s);

        Ok(s.to_vec())
    }

    #[instrument(skip(self, data, signature), level = "trace", name = "batcher_verify", fields(
        namespace = BATCHER_NAMESPACE, pk = BATCHER_PK
    ))]
    async fn batcher_verify(&self, data: &[u8], signature: &[u8]) -> Result<bool, SecretError> {
        self.verify(BATCHER_NAMESPACE, BATCHER_PK, data, signature).await
    }

    #[instrument(skip(self), level = "trace", name = "batcher_verifying", fields(
        namespace = BATCHER_NAMESPACE, pk = BATCHER_PK
    ))]
    async fn batcher_verifying(&self) -> Result<VerifyingKey, SecretError> {
        self.verifying_key(BATCHER_NAMESPACE, BATCHER_PK).await
    }
}

#[async_trait::async_trait]
impl<T: ChronicleSigner + WithSecret + Send + Sync> ChronicleKnownKeyNamesSigner for T {
    #[instrument(skip(self, data), level = "trace", name = "chronicle_sign", fields(
        namespace = CHRONICLE_NAMESPACE, pk = CHRONICLE_PK
    ))]
    async fn chronicle_sign(&self, data: &[u8]) -> Result<Vec<u8>, SecretError> {
        Ok(self.sign(CHRONICLE_NAMESPACE, CHRONICLE_PK, data).await?.to_vec())
    }

    #[instrument(skip(self, data, signature), level = "trace", name = "chronicle_verify", fields(
        namespace = CHRONICLE_NAMESPACE, pk = CHRONICLE_PK
    ))]
    async fn chronicle_verify(&self, data: &[u8], signature: &[u8]) -> Result<bool, SecretError> {
        self.verify(CHRONICLE_NAMESPACE, CHRONICLE_PK, data, signature).await
    }

    #[instrument(skip(self), level = "trace", name = "chronicle_verifying", fields(
        namespace = CHRONICLE_NAMESPACE, pk = CHRONICLE_PK
    ))]
    async fn chronicle_verifying(&self) -> Result<VerifyingKey, SecretError> {
        self.verifying_key(CHRONICLE_NAMESPACE, CHRONICLE_PK).await
    }
}

#[async_trait::async_trait]
impl<T: ChronicleSigner + WithSecret + Send + Sync> OpaKnownKeyNamesSigner for T {
    #[instrument(skip(self), level = "trace", name = "opa_sign", fields(
        namespace = OPA_NAMESPACE, pk = OPA_PK
    ))]
    async fn opa_sign(&self, data: &[u8]) -> Result<Vec<u8>, SecretError> {
        let s = self.sign(OPA_NAMESPACE, OPA_PK, data).await?;

        let s = s.normalize_s().unwrap_or(s);

        Ok(s.to_vec())
    }

    #[instrument(skip(self, data, signature), level = "trace", name = "opa_verify", fields(
        namespace = OPA_NAMESPACE, pk = OPA_PK
    ))]
    async fn opa_verify(&self, data: &[u8], signature: &[u8]) -> Result<bool, SecretError> {
        self.verify(OPA_NAMESPACE, OPA_PK, data, signature).await
    }

    #[instrument(skip(self), level = "trace", name = "opa_verifying", fields(
        namespace = OPA_NAMESPACE, pk = OPA_PK
    ))]
    async fn opa_verifying(&self) -> Result<VerifyingKey, SecretError> {
        self.verifying_key(OPA_NAMESPACE, OPA_PK).await
    }
}

pub fn chronicle_secret_names() -> Vec<(String, String)> {
    vec![
        (CHRONICLE_NAMESPACE.to_string(), CHRONICLE_PK.to_string()),
        (BATCHER_NAMESPACE.to_string(), BATCHER_PK.to_string()),
    ]
}

pub fn opa_secret_names() -> Vec<(String, String)> {
    vec![
        (OPA_NAMESPACE.to_string(), OPA_PK.to_string()),
        (BATCHER_NAMESPACE.to_string(), BATCHER_PK.to_string()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use k256::schnorr::signature::Signature;

    #[tokio::test]
    async fn embedded_keys() {
        let secrets = ChronicleSigning::new(
            chronicle_secret_names(),
            vec![(CHRONICLE_NAMESPACE.to_string(), ChronicleSecretsOptions::Embedded)],
        )
            .await
            .unwrap();

        secrets
            .with_signing_key(CHRONICLE_NAMESPACE, "chronicle-pk", |signing_key| {
                assert_eq!(signing_key.to_bytes().len(), 32, "Signing key should be 32 bytes");
            })
            .await
            .unwrap();

        secrets
            .with_verifying_key(CHRONICLE_NAMESPACE, "chronicle-pk", |verifying_key| {
                assert_eq!(verifying_key.to_bytes().len(), 33, "Verifying key should be 33 bytes");
            })
            .await
            .unwrap();

        let sig = secrets
            .sign(CHRONICLE_NAMESPACE, "chronicle-pk", "hello world".as_bytes())
            .await
            .unwrap();

        assert!(secrets
            .verify(CHRONICLE_NAMESPACE, "chronicle-pk", "hello world".as_bytes(), sig.as_bytes())
            .await
            .unwrap());

        assert!(!secrets
            .verify(CHRONICLE_NAMESPACE, "chronicle-pk", "boom".as_bytes(), sig.as_bytes())
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn vault_keys() {
        let secrets = ChronicleSigning::new(
            chronicle_secret_names(),
            vec![(CHRONICLE_NAMESPACE.to_string(), ChronicleSecretsOptions::Embedded)],
        )
            .await
            .unwrap();

        secrets
            .with_signing_key(CHRONICLE_NAMESPACE, "chronicle-pk", |signing_key| {
                assert_eq!(signing_key.to_bytes().len(), 32, "Signing key should be 32 bytes");
            })
            .await
            .unwrap();

        secrets
            .with_verifying_key(CHRONICLE_NAMESPACE, "chronicle-pk", |verifying_key| {
                assert_eq!(verifying_key.to_bytes().len(), 33, "Verifying key should be 33 bytes");
            })
            .await
            .unwrap();

        let sig = secrets
            .sign(CHRONICLE_NAMESPACE, "chronicle-pk", "hello world".as_bytes())
            .await
            .unwrap();

        assert!(secrets
            .verify(CHRONICLE_NAMESPACE, "chronicle-pk", "hello world".as_bytes(), sig.as_bytes())
            .await
            .unwrap());

        assert!(!secrets
            .verify(CHRONICLE_NAMESPACE, "chronicle-pk", "boom".as_bytes(), sig.as_bytes())
            .await
            .unwrap());
    }
}
