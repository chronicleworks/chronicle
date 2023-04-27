use std::{collections::BTreeSet, fmt};

use crate::{
    prov::AgentId,
    signing::{DirectoryStoredKeys, SignerError},
};

use k256::ecdsa::{signature::Signer, Signature, SigningKey, VerifyingKey};
use openssl::sha::Sha512;
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
#[derive(Serialize, Deserialize, PartialEq, Eq, Clone)]
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

impl std::fmt::Debug for JwtId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.debug_struct("JwtId")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

/// Claims from a JWT, referenced in creating an AgentId for a Chronicle user
#[derive(Clone, Deserialize, Serialize)]
pub struct JwtClaims(pub Map<String, Value>);

impl std::fmt::Debug for JwtClaims {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        let claims = self
            .0
            .iter()
            .map(|(k, _v)| (k, "***SECRET***"))
            .collect::<std::collections::HashMap<_, _>>();
        write!(f, "JwtClaims({:?})", claims)
    }
}

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
    pub fn from_jwt_claims(
        JwtClaims(claims): &JwtClaims,
        id_keys: &BTreeSet<String>,
    ) -> Result<Self, IdentityError> {
        const ZERO: [u8; 1] = [0];

        let mut hasher = Sha512::new();
        for id_key in id_keys {
            if let Some(Value::String(claim_value)) = claims.get(id_key) {
                hasher.update(id_key.as_bytes());
                hasher.update(&ZERO);
                hasher.update(claim_value.as_bytes());
                hasher.update(&ZERO);
            } else {
                warn!(
                    "For constructing Chronicle identity no {id_key:?} field among JWT claims: {claims:#?}"
                );
                return Err(IdentityError::JwtClaims);
            }
        }

        Ok(Self::JWT(JwtId::new(
            &hex::encode(hasher.finish()),
            Value::Object(claims.to_owned()),
        )))
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
    pub identity: String,
    pub signature: Signature,
    pub verifying_key: VerifyingKey,
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
    use crate::prov::{ExternalId, ExternalIdPart};
    use serde_json::json;

    fn external_id_from_jwt_claims<'a>(claim_strings: impl Iterator<Item = &'a str>) -> ExternalId {
        const ZERO: [u8; 1] = [0];
        let mut hasher = Sha512::new();
        claim_strings.for_each(|s| {
            hasher.update(s.as_bytes());
            hasher.update(&ZERO);
        });
        hex::encode(hasher.finish()).into()
    }

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
        let auth_id =
            AuthId::from_jwt_claims(&claims, &BTreeSet::from(["name".to_string()])).unwrap();
        insta::assert_json_snapshot!(auth_id, @r###"
        {
          "type": "jwt",
          "id": "6e7f57aeab5edb9bf5863ba2d749715b6f9a9079f3b8c81b6207d437c005b5b9f6f14de53c34c38ee0b1cc77fa6e02b5cef694faf5aaf028b58c15b3c4ee1cb0",
          "claims": {
            "name": "abcdef"
          }
        }
        "###);

        if let AuthId::JWT(JwtId { id, .. }) = auth_id {
            assert_eq!(
                &external_id_from_jwt_claims(vec!["name", "abcdef"].into_iter()),
                id.external_id_part()
            );
        } else {
            panic!("did not receive expected JWT identity: {auth_id}");
        }
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
        let auth_id =
            AuthId::from_jwt_claims(&claims, &BTreeSet::from(["sub".to_string()])).unwrap();

        insta::assert_json_snapshot!(auth_id, @r###"
        {
          "type": "jwt",
          "id": "13cc0854e3c226984a47e3159be9d71dae9796586ae15c493a7dcb79c2c511be7b311a238439a6922b779014b2bc71f351ff388fcac012d4f20f161720fa0dcf",
          "claims": {
            "sub": "John Doe"
          }
        }
        "###);

        if let AuthId::JWT(JwtId { id, .. }) = auth_id {
            assert_eq!(
                &external_id_from_jwt_claims(vec!["sub", "John Doe"].into_iter()),
                id.external_id_part()
            );
        } else {
            panic!("did not receive expected JWT identity: {auth_id}");
        }
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
        let auth_id_result =
            AuthId::from_jwt_claims(&claims, &BTreeSet::from(["externalId".to_string()]));
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

    #[test]
    fn test_jwt_claims_custom_debug() {
        let claims = JwtClaims(
            json!({
                "key": "value",
            })
            .as_object()
            .unwrap()
            .to_owned(),
        );
        insta::assert_debug_snapshot!(claims, @r###"JwtClaims({"key": "***SECRET***"})"###);
    }

    #[test]
    fn test_jwt_id_custom_debug() {
        let jwt_id = AuthId::JWT(JwtId {
            id: AgentId::from_external_id("abcdef"),
            claims: json!({
                "key": "value"
            }),
        });
        insta::assert_debug_snapshot!(jwt_id, @r###"
        JWT(
            JwtId {
                id: AgentId(
                    ExternalId(
                        "abcdef",
                    ),
                ),
                ..
            },
        )
        "###);
    }
}
