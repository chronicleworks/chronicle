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
                // Not semantically the same thing as we re-use f
                SigningKey::from_pkcs8_pem(&secret)
                    .map_err(|_| SecretError::InvalidPrivateKey)
                    .map(&f),
                secret,
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
                SigningKey::from_pkcs8_pem(&secret)
                    .map_err(|_| SecretError::InvalidPrivateKey)
                    .map(|signing_key| f(signing_key.verifying_key())),
                secret,
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
                SigningKey::from_pkcs8_pem(&secret)
                    .map_err(|_| SecretError::InvalidPrivateKey)
                    .map(|signing_key| signing_key.verifying_key()),
                secret,
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
