use k256::{
    ecdsa::{SigningKey, VerifyingKey},
    pkcs8::{DecodePrivateKey, DecodePublicKey, EncodePrivateKey, LineEnding},
    SecretKey,
};
use rand::prelude::StdRng;
use rand_core::SeedableRng;
use thiserror::Error;
use tracing::{debug, error, info};

use std::path::{Path, PathBuf};

#[derive(Error, Debug)]
pub enum SignerError {
    #[error("Invalid validator address {source}")]
    InvalidValidatorAddress {
        #[from]
        source: url::ParseError,
    },
    #[error("Invalid key store directory {source}")]
    Io {
        #[from]
        source: std::io::Error,
    },
    #[error("Invalid glob {source}")]
    Pattern {
        #[from]
        source: glob::PatternError,
    },
    #[error("Invalid file encoding {source}")]
    Encoding {
        #[from]
        source: std::string::FromUtf8Error,
    },
    #[error("Invalid public key")]
    InvalidPublicKey,
    #[error("Invalid private key")]
    InvalidPrivateKey,
    #[error("No public key found")]
    NoPublicKeyFound,
    #[error("No private key found")]
    NoPrivateKeyFound,
}

// TODO:
// This is a temporary solution to allow for matching on different KMS types
// in order to retrieve different methods for retrieving keys/signatures.
pub enum KMS<'a> {
    Directory(&'a DirectoryStoredKeys),
}

// TODO:
// Placeholder for a more generic solution to signing
pub fn directory_signing_key(key: SigningKey) -> Result<SigningKey, SignerError> {
    Ok(key)
}

#[derive(Debug, Clone)]
pub struct DirectoryStoredKeys {
    base: PathBuf,
}

impl DirectoryStoredKeys {
    pub fn new<P>(base: P) -> Result<Self, SignerError>
    where
        P: AsRef<Path>,
    {
        debug!(init_keystore_at = ?base.as_ref());
        Ok(Self {
            base: base.as_ref().to_path_buf(),
        })
    }

    pub fn chronicle_signing<F, T>(&self, f: F) -> Result<T, SignerError>
    where
        F: Fn(SigningKey) -> Result<T, SignerError>,
    {
        let signing_key = Self::signing_key_at(&self.base)?;
        f(signing_key)
    }

    /// Return the verifying key associated with the Chronicle user
    pub fn chronicle_verifying(&self) -> Result<VerifyingKey, SignerError> {
        Self::signing_key_at(&self.base).map(|signing| signing.verifying_key())
    }

    pub fn import_chronicle(
        &self,
        signing: Option<&Path>,
        verifying: Option<&Path>,
    ) -> Result<(), SignerError> {
        std::fs::create_dir_all(&self.base)?;
        if let Some(signing) = signing {
            std::fs::copy(
                Path::new(signing),
                Path::join(&self.base, Path::new("key.priv.pem")),
            )?;
        }

        if let Some(verifying) = verifying {
            std::fs::copy(
                Path::new(verifying),
                Path::join(&self.base, Path::new("key.pub.pem")),
            )?;
        }

        Ok(())
    }

    pub fn generate_chronicle(&self) -> Result<(), SignerError> {
        info!(generate_chronicle_key_at = ?self.base);
        std::fs::create_dir_all(&self.base)?;
        Self::new_signing_key(&self.base)
    }

    fn new_signing_key(secretpath: &Path) -> Result<(), SignerError> {
        debug!(generate_secret_at = ?secretpath);
        let secret = SecretKey::random(StdRng::from_entropy());

        let privpem = secret
            .to_pkcs8_pem(LineEnding::CRLF)
            .map_err(|_| SignerError::InvalidPrivateKey)?;

        std::fs::write(
            Path::join(Path::new(&secretpath), Path::new("key.priv.pem")),
            privpem.as_bytes(),
        )?;

        Ok(())
    }

    fn signing_key_at(path: &Path) -> Result<SigningKey, SignerError> {
        debug!(load_signing_key_at = ?path);
        SigningKey::from_pkcs8_pem(&std::fs::read_to_string(Path::join(
            path,
            Path::new("key.priv.pem"),
        ))?)
        .map_err(|_| SignerError::InvalidPrivateKey)
    }

    #[allow(dead_code)]
    fn verifying_key_at(path: &Path) -> Result<VerifyingKey, SignerError> {
        debug!(load_verifying_key_at = ?path);
        VerifyingKey::from_public_key_pem(&std::fs::read_to_string(Path::join(
            path,
            Path::new("key.pub.pem"),
        ))?)
        .map_err(|_| SignerError::InvalidPublicKey)
    }
}
