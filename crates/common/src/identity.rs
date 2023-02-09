use crate::{
    prov::AgentId,
    signing::{DirectoryStoredKeys, SignerError},
};

use k256::ecdsa::{signature::Signer, Signature, SigningKey, VerifyingKey};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum IdentityError {
    #[error("Invalid key store directory {0}")]
    KeyStore(#[from] SignerError),
    #[error("Malformed JSON {0}")]
    SerdeJson(#[from] serde_json::Error),
    #[error("Serialization error {0}")]
    SerdeJsonSerialize(String),
    #[error("Signing error {0}")]
    Signing(#[from] k256::ecdsa::Error),
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
#[serde(tag = "type", content = "id")]
pub enum AuthId {
    Anonymous,
    Chronicle,
    JWT(AgentId),
}

impl TryFrom<&str> for AuthId {
    type Error = serde_json::Error;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        serde_json::from_str(s)
    }
}

impl AuthId {
    pub fn agent(agent: &AgentId) -> Self {
        Self::JWT(agent.to_owned())
    }

    pub fn anonymous() -> Self {
        Self::Anonymous
    }

    pub fn chronicle() -> Self {
        Self::Chronicle
    }

    pub fn identity_context(&self) -> Result<serde_json::Value, IdentityError> {
        serde_json::to_value(self).map_err(|e| IdentityError::SerdeJsonSerialize(e.to_string()))
    }

    fn signature(&self, store: &DirectoryStoredKeys) -> Result<Signature, IdentityError> {
        let signing_key = self.signing_key(store)?;
        let buf = serde_json::to_string(self)?.as_bytes().to_vec();
        Ok(signing_key.try_sign(&buf)?)
    }

    pub fn signed_identity(
        &self,
        store: &DirectoryStoredKeys,
    ) -> Result<SignedIdentity, IdentityError> {
        let verifying_key = self.verifying_key(store)?;
        let signature = self.signature(store)?;
        SignedIdentity::new(self, signature, verifying_key)
    }

    fn signing_key(&self, store: &DirectoryStoredKeys) -> Result<SigningKey, IdentityError> {
        match self {
            Self::Anonymous => Ok(store.agent_signing(&AgentId::from_external_id("Anonymous"))?),
            Self::Chronicle => Ok(store.chronicle_signing()?),
            Self::JWT(agent_id) => Ok(store.agent_signing(agent_id)?),
        }
    }

    fn verifying_key(&self, store: &DirectoryStoredKeys) -> Result<VerifyingKey, IdentityError> {
        match self {
            Self::Anonymous => Ok(store.agent_verifying(&AgentId::from_external_id("Anonymous"))?),
            Self::Chronicle => Ok(store.chronicle_verifying()?),
            Self::JWT(agent_id) => Ok(store.agent_verifying(agent_id)?),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SignedIdentity {
    identity_context: String,
    signature: Signature,
    verifying_key: VerifyingKey,
}

impl SignedIdentity {
    fn new(
        id: &AuthId,
        signature: Signature,
        verifying_key: VerifyingKey,
    ) -> Result<Self, IdentityError> {
        let identity_context = serde_json::to_string(&id)?;
        Ok(Self {
            identity_context,
            signature,
            verifying_key,
        })
    }
}

impl TryFrom<&SignedIdentity> for AuthId {
    type Error = serde_json::Error;

    fn try_from(signed_identity: &SignedIdentity) -> Result<Self, Self::Error> {
        serde_json::from_str(&signed_identity.identity_context)
    }
}
