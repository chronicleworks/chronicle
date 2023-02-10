use std::fmt;

use crate::{
    prov::AgentId,
    signing::{DirectoryStoredKeys, SignerError},
};

use k256::ecdsa::{signature::Signer, Signature, SigningKey, VerifyingKey};
use serde_json::{Map, Value};
use thiserror::Error;
use tracing::warn;

#[derive(Error, Debug)]
pub enum IdentityError {
    #[error("Failed to get agent id from JWT claims")]
    JwtClaims,

    #[error("Invalid key store directory: {0}")]
    KeyStore(#[from] SignerError),

    #[error("Malformed JSON: {0}")]
    SerdeJson(#[from] serde_json::Error),

    #[error("Serialization error: {0}")]
    SerdeJsonSerialize(String),

    #[error("Signing error: {0}")]
    Signing(#[from] k256::ecdsa::Error),
}

/// Contains the scalar ID and identity claims for a user established via JWT
#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct JwtId {
    pub id: AgentId,
    pub claims: Value,
}

impl JwtId {
    fn new(external_id: &str, claims: Value) -> Self {
        Self {
            id: AgentId::from_external_id(external_id),
            claims,
        }
    }
}

/// Claims from a JWT, referenced in creating an AgentId for a Chronicle user
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct JwtClaims(pub Map<String, Value>);

/// Chronicle identity object for authorization
#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
#[serde(rename_all = "lowercase", tag = "type")]
pub enum AuthId {
    Anonymous,
    Chronicle,
    JWT(JwtId),
}

impl fmt::Display for AuthId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Anonymous => write!(f, "Anonymous"),
            Self::Chronicle => write!(f, "Chronicle"),
            Self::JWT(jwt_id) => write!(f, "{}", jwt_id.id),
        }
    }
}

impl TryFrom<&str> for AuthId {
    type Error = serde_json::Error;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        serde_json::from_str(s)
    }
}

impl AuthId {
    /// Establish a Chronicle user via JWT using a provided pointer into the JWT claims,
    /// caching the claims with the JWT user identity
    pub fn from_jwt_claims(claims: &JwtClaims, json_pointer: &str) -> Result<Self, IdentityError> {
        let claims = claims.to_owned();

        if let Some(Value::String(external_id)) =
            Value::Object(claims.0.clone()).pointer(json_pointer)
        {
            Ok(Self::JWT(JwtId::new(
                external_id,
                serde_json::to_value(claims)?,
            )))
        } else {
            warn!(
                "Pointer ({json_pointer}) failed to get externalId from JWT claims: {:#?}",
                claims
            );
            Err(IdentityError::JwtClaims)
        }
    }

    /// Create an Anonymous Chronicle user
    pub fn anonymous() -> Self {
        Self::Anonymous
    }

    /// Create a Chronicle super user
    pub fn chronicle() -> Self {
        Self::Chronicle
    }

    /// Serialize identity to a JSON object containing "type" ("Anonymous", "Chronicle", or "JWT"),
    /// and, in the case of a JWT identity, "id" fields - the Input for an OPA check
    pub fn identity(&self) -> Result<Value, IdentityError> {
        serde_json::to_value(self).map_err(|e| IdentityError::SerdeJsonSerialize(e.to_string()))
    }

    fn signature(&self, store: &DirectoryStoredKeys) -> Result<Signature, IdentityError> {
        let signing_key = self.signing_key(store)?;
        let buf = serde_json::to_string(self)?.as_bytes().to_vec();
        Ok(signing_key.try_sign(&buf)?)
    }

    /// Get the user identity's [`SignedIdentity`]
    pub fn signed_identity(
        &self,
        store: &DirectoryStoredKeys,
    ) -> Result<SignedIdentity, IdentityError> {
        let verifying_key = self.verifying_key(store)?;
        let signature = self.signature(store)?;
        SignedIdentity::new(self, signature, verifying_key)
    }

    /// Use the Chronicle key to sign all identity variants
    fn signing_key(&self, store: &DirectoryStoredKeys) -> Result<SigningKey, IdentityError> {
        Ok(store.chronicle_signing()?)
    }

    /// Use the Chronicle key to verify all identity variants
    fn verifying_key(&self, store: &DirectoryStoredKeys) -> Result<VerifyingKey, IdentityError> {
        Ok(store.chronicle_verifying()?)
    }
}

/// Context data for an OPA check - `operation` and `state` fields are
/// equivalent to GraphQL parent type and path node
#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone, Default)]
struct Context {
    operation: Value,
    state: Value,
}

/// Identity and Context data for an OPA check
#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct IdentityContext {
    identity: AuthId,
    context: Context,
}

impl IdentityContext {
    pub fn new(identity: AuthId, operation: Value, state: Value) -> Self {
        Self {
            identity,
            context: Context { operation, state },
        }
    }
}

/// Contextual data for OPA created either via GraphQL or in the Transaction Processor
#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
#[serde(rename_all = "lowercase", tag = "type")]
pub enum OpaData {
    GraphQL(IdentityContext),
    Operation(IdentityContext),
}

impl OpaData {
    pub fn graphql(identity: &AuthId, parent_type: &Value, resolve_path: &Value) -> Self {
        Self::GraphQL(IdentityContext::new(
            identity.to_owned(),
            parent_type.to_owned(),
            resolve_path.to_owned(),
        ))
    }

    pub fn operation(identity: &AuthId, operation: &Value, state: &Value) -> Self {
        Self::Operation(IdentityContext::new(
            identity.to_owned(),
            operation.to_owned(),
            state.to_owned(),
        ))
    }
}

/// Signed user identity containing the serialized identity, signature, and
/// verifying key. Implements `TryFrom` to deserialize to the user identity object
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SignedIdentity {
    identity: String,
    signature: Signature,
    verifying_key: VerifyingKey,
}

impl SignedIdentity {
    fn new(
        id: &AuthId,
        signature: Signature,
        verifying_key: VerifyingKey,
    ) -> Result<Self, IdentityError> {
        Ok(Self {
            identity: serde_json::to_string(&id)?,
            signature,
            verifying_key,
        })
    }
}

impl TryFrom<&SignedIdentity> for AuthId {
    type Error = serde_json::Error;

    fn try_from(signed_identity: &SignedIdentity) -> Result<Self, Self::Error> {
        serde_json::from_str(&signed_identity.identity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_auth_id_serialization() {
        let auth_id = AuthId::anonymous();
        insta::assert_json_snapshot!(auth_id, @r###"
        {
          "type": "anonymous"
        }
        "###);

        let auth_id = AuthId::chronicle();
        insta::assert_json_snapshot!(auth_id, @r###"
        {
          "type": "chronicle"
        }
        "###);

        let claims = JwtClaims(
            json!({
                "name": "abcdef",
            })
            .as_object()
            .unwrap()
            .to_owned(),
        );
        let auth_id = AuthId::from_jwt_claims(&claims, "/name").unwrap();
        insta::assert_json_snapshot!(auth_id, @r###"
        {
          "type": "jwt",
          "id": "abcdef",
          "claims": {
            "name": "abcdef"
          }
        }
        "###);
    }

    #[test]
    fn test_auth_id_deserialization() {
        let serialized = r#"{"type":"anonymous"}"#;
        let deserialized: AuthId = serde_json::from_str(serialized).unwrap();
        assert_eq!(deserialized, AuthId::Anonymous);

        let serialized = r#"{"type":"chronicle"}"#;
        let deserialized: AuthId = serde_json::from_str(serialized).unwrap();
        assert_eq!(deserialized, AuthId::Chronicle);

        let serialized = r#"{
            "type": "jwt",
            "id": "abcdef",
            "claims": {
              "name": "abcdef"
            }
          }"#;
        let deserialized: AuthId = serde_json::from_str(serialized).unwrap();
        assert_eq!(
            deserialized,
            AuthId::JWT(JwtId {
                id: AgentId::from_external_id("abcdef"),
                claims: json!({
                        "name": "abcdef"
                })
            })
        );
    }

    #[test]
    fn test_auth_id_from_jwt_claims() {
        let claims = JwtClaims(
            json!({
                "sub": "John Doe"
            })
            .as_object()
            .unwrap()
            .to_owned(),
        );
        let auth_id_result = AuthId::from_jwt_claims(&claims, "/sub").unwrap();
        insta::assert_json_snapshot!(auth_id_result, @r###"
        {
          "type": "jwt",
          "id": "John Doe",
          "claims": {
            "sub": "John Doe"
          }
        }
        "###);
    }

    #[test]
    fn test_auth_id_from_jwt_claims_failure() {
        let claims = JwtClaims(
            json!({
                "sub": "John Doe"
            })
            .as_object()
            .unwrap()
            .to_owned(),
        );
        let auth_id_result = AuthId::from_jwt_claims(&claims, "/externalId");
        assert!(auth_id_result.is_err());
        assert_eq!(
            auth_id_result.unwrap_err().to_string(),
            IdentityError::JwtClaims.to_string()
        );
    }

    #[test]
    fn test_opa_data_serialization() {
        let identity = AuthId::Chronicle;
        let operation = json!({
            "resource": "users",
            "action": "read"
        });
        let state = json!([{"id": 1, "name": "Alice"}, {"id": 2, "name": "Bob"}]);
        let context = OpaData::graphql(&identity, &operation, &state);

        let json = serde_json::to_string(&context).unwrap();
        let deserialized_context: OpaData = serde_json::from_str(&json).unwrap();

        assert!(context == deserialized_context);
        insta::assert_json_snapshot!(context, @r###"
        {
          "type": "graphql",
          "identity": {
            "type": "chronicle"
          },
          "context": {
            "operation": {
              "action": "read",
              "resource": "users"
            },
            "state": [
              {
                "id": 1,
                "name": "Alice"
              },
              {
                "id": 2,
                "name": "Bob"
              }
            ]
          }
        }
        "###);
    }
}
