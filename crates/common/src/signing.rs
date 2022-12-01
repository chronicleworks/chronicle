use custom_error::custom_error;
use k256::{
    ecdsa::{SigningKey, VerifyingKey},
    pkcs8::{self, spki, DecodePrivateKey, DecodePublicKey, EncodePrivateKey, LineEnding},
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
    InvalidPublicKey{source: pkcs8::Error}                  = "Invalid public key",
    InvalidPrivateKey{source:  spki::Error}                 = "Invalid public key",
    NoPublicKeyFound{}                                      = "No public key found",
    NoPrivateKeyFound{}                                     = "No private key found",
}

#[derive(Debug, Clone)]
pub struct DirectoryStoredKeys {
    base: PathBuf,
}

impl DirectoryStoredKeys {
    /// Create an object for key storage with a pointer to the provided key store path
    pub fn new<P>(base: P) -> Result<Self, SignerError>
    where
        P: AsRef<Path>,
    {
        debug!(init_keystore_at = ?base.as_ref());
        Ok(Self {
            base: base.as_ref().to_path_buf(),
        })
    }

    /// Return the signing key associated with the Chronicle user
    pub fn chronicle_signing(&self) -> Result<SigningKey, SignerError> {
        Self::signing_key_at(&self.base)
    }

    /// Return the signing key associated with an AgentId
    pub fn agent_signing(&self, agent: &AgentId) -> Result<SigningKey, SignerError> {
        Self::signing_key_at(&self.agent_path(agent))
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

    /// Store a private key and/or public key for an agent, having ensured a directory named after the Agent's external id exists in the local key store
    pub fn store_agent(
        &self,
        agent: &AgentId,
        signing: Option<&Vec<u8>>,
        verifying: Option<&Vec<u8>>,
    ) -> Result<(), SignerError> {
        std::fs::create_dir_all(&self.agent_path(agent))?;

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

    /// Import the private key and/or public key for an agent, having ensured a directory named after the given Agent's external id exists in the local key store
    pub fn import_agent(
        &self,
        agent: &AgentId,
        signing: Option<&Path>,
        verifying: Option<&Path>,
    ) -> Result<(), SignerError> {
        std::fs::create_dir_all(&self.agent_path(agent))?;

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

    /// Ensure a directory at the locally configured key store path exists, and import an existing signing key and/or verifying key
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

    /// Ensure a directory named after the external id of an Agent exists in the local key store and write a new signing key to that path
    pub fn generate_agent(&self, agent: &AgentId) -> Result<(), SignerError> {
        info!(generate_agent_key_at = ?self.agent_path(agent));
        let path = self.agent_path(agent);
        std::fs::create_dir_all(&path)?;
        Self::new_signing_key(&path)
    }

    /// Generate and store a Chronicle super user signing key in the local key store
    pub fn generate_chronicle(&self) -> Result<(), SignerError> {
        info!(generate_chronicle_key_at = ?self.base);
        std::fs::create_dir_all(&self.base)?;
        Self::new_signing_key(&self.base)
    }

    /// Write a new signing key to the given path
    fn new_signing_key(secretpath: &Path) -> Result<(), SignerError> {
        debug!(generate_secret_at = ?secretpath);
        let secret = SecretKey::random(StdRng::from_entropy());

        let privpem = secret.to_pkcs8_pem(LineEnding::CRLF)?;

        std::fs::write(
            Path::join(Path::new(&secretpath), Path::new("key.priv.pem")),
            privpem.as_bytes(),
        )?;

        Ok(())
    }

    /// Return a path to a directory named after the external id of an Agent within the local key store
    fn agent_path(&self, agent: &AgentId) -> PathBuf {
        Path::join(&self.base, Path::new(agent.external_id_part().as_str()))
    }

    /// Derive and return a signing key from the private key stored at the given path
    fn signing_key_at(path: &Path) -> Result<SigningKey, SignerError> {
        debug!(load_signing_key_at = ?path);
        Ok(SigningKey::from_pkcs8_pem(&std::fs::read_to_string(
            Path::join(path, Path::new("key.priv.pem")),
        )?)?)
    }

    /// Derive and return a verifying key from the public key stored at the given path
    fn verifying_key_at(path: &Path) -> Result<VerifyingKey, SignerError> {
        debug!(load_verifying_key_at = ?path);
        Ok(VerifyingKey::from_public_key_pem(
            &std::fs::read_to_string(Path::join(path, Path::new("key.pub.pem")))?,
        )?)
    }
}
