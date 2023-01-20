use crate::{
    prov::{AgentId, ExternalIdPart},
    signing::{DirectoryStoredKeys, SignerError},
};

use custom_error::custom_error;
use k256::ecdsa::{signature::Signer, Signature, SigningKey, VerifyingKey};

custom_error! {pub IdentityError
    KeyStore{source: SignerError} = "Invalid key store directory {source}",
    SerdeJson{source: serde_json::Error } = "Malformed JSON {source}",
    Signing{source: k256::ecdsa::Error} = "Signing error {source}",
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub enum AuthId {
    Chronicle,
    Jwt(AgentId),
}

impl AuthId {
    pub fn agent(agent: &AgentId) -> Self {
        Self::Jwt(agent.to_owned())
    }

    pub fn chronicle() -> Self {
        Self::Chronicle
    }

    pub fn signed_identity(
        &self,
        store: &DirectoryStoredKeys,
    ) -> Result<SignedIdentity, IdentityError> {
        let verifying_key = self.verifying_key(store)?;
        let signing_key = self.signing_key(store)?;

        let id_and_type = IdentityContext::new(self);
        let buf = id_and_type.to_sign()?;

        let signature: Signature = signing_key.try_sign(&buf)?;
        let signed_id = SignedIdentity::new(self, signature, verifying_key)?;

        Ok(signed_id)
    }

    fn signing_key(&self, store: &DirectoryStoredKeys) -> Result<SigningKey, IdentityError> {
        match self {
            Self::Chronicle => Ok(store.chronicle_signing()?),
            Self::Jwt(agent_id) => Ok(store.agent_signing(agent_id)?),
        }
    }

    fn verifying_key(&self, store: &DirectoryStoredKeys) -> Result<VerifyingKey, IdentityError> {
        match self {
            Self::Chronicle => Ok(store.chronicle_verifying()?),
            Self::Jwt(agent_id) => Ok(store.agent_verifying(agent_id)?),
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
        let identity_context = serde_json::to_string(&IdentityContext::new(id))?;
        Ok(Self {
            identity_context,
            signature,
            verifying_key,
        })
    }
}

#[derive(Serialize, Deserialize)]
pub(crate) struct IdentityContext {
    id: serde_json::Value,
    typ: String,
}

impl IdentityContext {
    pub(crate) fn new(id: &AuthId) -> Self {
        match id {
            AuthId::Chronicle => Self {
                id: serde_json::Value::from("chronicle"),
                typ: "key".to_owned(),
            },
            AuthId::Jwt(agent_id) => Self {
                id: serde_json::Value::from(agent_id.external_id_part().as_str()),
                typ: "JWT".to_owned(),
            },
        }
    }
    fn to_sign(&self) -> Result<Vec<u8>, IdentityError> {
        Ok(serde_json::to_string(self)?.as_bytes().to_vec())
    }
}

impl TryFrom<&str> for IdentityContext {
    type Error = serde_json::Error;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        serde_json::from_str(s)
    }
}
