use custom_error::custom_error;
use k256::{
    ecdsa::{SigningKey, VerifyingKey},
    pkcs8::{DecodePrivateKey, DecodePublicKey, EncodePrivateKey, LineEnding},
    SecretKey,
};
use rand::prelude::StdRng;
use rand_core::SeedableRng;
use tracing::{debug, error, info};

use std::{
    path::{Path, PathBuf},
    string::FromUtf8Error,
};

use crate::prov::{AgentId, ExternalIdPart};

custom_error! {pub SignerError
    InvalidValidatorAddress{source: url::ParseError}        = "Invalid validator address",
    Io{source: std::io::Error}                              = "Invalid key store directory",
    Pattern{source: glob::PatternError}                     = "Invalid glob ",
    Encoding{source: FromUtf8Error}                         = "Invalid file encoding",
    InvalidPublicKey{}                  = "Invalid public key",
    InvalidPrivateKey{}                  = "Invalid private key",
    NoPublicKeyFound{}                                      = "No public key found",
    NoPrivateKeyFound{}                                     = "No private key found",
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

    pub fn chronicle_signing(&self) -> Result<SigningKey, SignerError> {
        Self::signing_key_at(&self.base)
    }

    pub fn agent_signing(&self, agent: &AgentId) -> Result<SigningKey, SignerError> {
        Self::signing_key_at(&self.agent_path(agent))
    }

    /// Return the verifying key associated with the Chronicle user
    pub fn chronicle_verifying(&self) -> Result<VerifyingKey, SignerError> {
        Self::signing_key_at(&self.base).map(|signing| signing.verifying_key())
    }

    /// If we have a signing key, derive the verifying key from it, else attempt to load an imported verifying key
    pub fn agent_verifying(&self, agent: &AgentId) -> Result<VerifyingKey, SignerError> {
        let path = &self.agent_path(agent);
        Self::signing_key_at(path)
            .map(|signing| signing.verifying_key())
            .or_else(|error| {
                error!(?error, ?path, "Loading signing key");
                Self::verifying_key_at(path)
            })
    }

    pub fn store_agent(
        &self,
        agent: &AgentId,
        signing: Option<&Vec<u8>>,
        verifying: Option<&Vec<u8>>,
    ) -> Result<(), SignerError> {
        std::fs::create_dir_all(self.agent_path(agent))?;

        if let Some(signing) = signing {
            std::fs::write(
                Path::join(&self.agent_path(agent), Path::new("key.priv.pem")),
                signing,
            )?;
        }

        if let Some(verifying) = verifying {
            std::fs::write(
                Path::join(&self.agent_path(agent), Path::new("key.pub.pem")),
                verifying,
            )?;
        }

        Ok(())
    }

    pub fn import_agent(
        &self,
        agent: &AgentId,
        signing: Option<&Path>,
        verifying: Option<&Path>,
    ) -> Result<(), SignerError> {
        std::fs::create_dir_all(self.agent_path(agent))?;

        if let Some(signing) = signing {
            std::fs::copy(
                Path::new(signing),
                Path::join(&self.agent_path(agent), Path::new("key.priv.pem")),
            )?;
        }

        if let Some(verifying) = verifying {
            std::fs::copy(
                Path::new(verifying),
                Path::join(&self.agent_path(agent), Path::new("key.pub.pem")),
            )?;
        }

        Ok(())
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

    pub fn generate_agent(&self, agent: &AgentId) -> Result<(), SignerError> {
        info!(generate_agent_key_at = ?self.agent_path(agent));
        let path = self.agent_path(agent);
        std::fs::create_dir_all(&path)?;
        Self::new_signing_key(&path)
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
            .map_err(|_| SignerError::InvalidPrivateKey {})?;

        std::fs::write(
            Path::join(Path::new(&secretpath), Path::new("key.priv.pem")),
            privpem.as_bytes(),
        )?;

        Ok(())
    }

    fn agent_path(&self, agent: &AgentId) -> PathBuf {
        Path::join(&self.base, Path::new(agent.external_id_part().as_str()))
    }

    fn signing_key_at(path: &Path) -> Result<SigningKey, SignerError> {
        debug!(load_signing_key_at = ?path);
        SigningKey::from_pkcs8_pem(&std::fs::read_to_string(Path::join(
            path,
            Path::new("key.priv.pem"),
        ))?)
        .map_err(|_| SignerError::InvalidPrivateKey {})
    }

    fn verifying_key_at(path: &Path) -> Result<VerifyingKey, SignerError> {
        debug!(load_verifying_key_at = ?path);
        VerifyingKey::from_public_key_pem(&std::fs::read_to_string(Path::join(
            path,
            Path::new("key.pub.pem"),
        ))?)
        .map_err(|_| SignerError::InvalidPublicKey {})
    }
}
