use crate::identity::{AuthId, IdentityError, OpaData};
use k256::sha2::{Digest, Sha256};
use opa::{bundle::Bundle, wasm::Opa};
use opa_tp_protocol::{
    address::{FAMILY, VERSION},
    async_sawtooth_sdk::{
        error::SawtoothCommunicationError, ledger::LedgerReader,
        zmq_client::ZmqRequestResponseSawtoothChannel,
    },
    state::policy_address,
    OpaLedger,
};
use rust_embed::RustEmbed;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;
use tracing::{debug, error, info, instrument};
use url::Url;

#[derive(Debug, Error)]
pub enum PolicyLoaderError {
    #[error("Failed to read embedded OPA policies")]
    EmbeddedOpaPolicies,

    #[error("Policy not found: {0}")]
    MissingPolicy(String),

    #[error("Io error: {0}")]
    OpaBundleError(#[from] opa::bundle::Error),

    #[error("Error loading OPA policy: {0}")]
    SawtoothCommunicationError(#[from] SawtoothCommunicationError),
}

#[async_trait::async_trait]
pub trait PolicyLoader {
    /// Set address of OPA policy
    fn set_address(&mut self, address: &str);

    /// Set OPA policy
    fn set_rule_name(&mut self, policy: &str);

    /// Set entrypoint for OPA policy
    fn set_entrypoint(&mut self, entrypoint: &str);

    fn get_address(&self) -> &str;

    fn get_rule_name(&self) -> &str;

    fn get_entrypoint(&self) -> &str;

    fn get_policy(&self) -> &[u8];

    /// Load OPA policy from address set in `PolicyLoader`
    async fn load_policy(&mut self) -> Result<(), PolicyLoaderError>;

    /// Load OPA policy from provided bytes
    fn load_policy_from_bytes(&mut self, policy: &[u8]);

    /// Return a built OPA instance from the cached policy
    #[instrument(level = "trace", skip(self), ret)]
    fn build_opa(&self) -> Result<Opa, OpaExecutorError> {
        Ok(Opa::new().build(self.get_policy())?)
    }

    /// Load OPA policy from provided policy bundle
    fn load_policy_from_bundle(&mut self, bundle: &Bundle) -> Result<(), PolicyLoaderError> {
        let rule = self.get_rule_name();
        self.load_policy_from_bytes(
            bundle
                .wasm_policies
                .iter()
                .find(|p| p.entrypoint == rule)
                .map(|p| p.bytes.as_ref())
                .ok_or(PolicyLoaderError::MissingPolicy(rule.to_string()))?,
        );
        Ok(())
    }

    fn hash(&self) -> String;
}

pub struct SawtoothPolicyLoader {
    policy_id: String,
    address: String,
    policy: Option<Vec<u8>>,
    entrypoint: String,
    ledger: OpaLedger,
}

impl SawtoothPolicyLoader {
    pub fn new(address: &Url, policy_id: &str, entrypoint: &str) -> Self {
        Self {
            policy_id: policy_id.to_owned(),
            address: String::default(),
            policy: None,
            entrypoint: entrypoint.to_owned(),
            ledger: OpaLedger::new(
                ZmqRequestResponseSawtoothChannel::new(address),
                FAMILY,
                VERSION,
            ),
        }
    }

    fn sawtooth_address(&self, policy: impl AsRef<str>) -> String {
        policy_address(policy)
    }

    #[instrument(level = "debug", skip(self))]
    async fn load_bundle_from_chain(&mut self) -> Result<Vec<u8>, SawtoothCommunicationError> {
        if let Some(policy) = self.policy.as_ref() {
            return Ok(policy.clone());
        }
        let load_policy_from = self.sawtooth_address(&self.policy_id);
        debug!(load_policy_from=?load_policy_from);

        loop {
            let res = self.ledger.get_state_entry(&load_policy_from).await;

            if let Err(res) = &res {
                error!(error=?res, "Failed to load policy from chain");
                self.ledger.reconnect().await;
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                continue;
            }

            return Ok(res.unwrap());
        }
    }
}

#[async_trait::async_trait]
impl PolicyLoader for SawtoothPolicyLoader {
    fn set_address(&mut self, address: &str) {
        self.address = address.to_owned()
    }

    fn set_rule_name(&mut self, name: &str) {
        self.policy_id = name.to_owned()
    }

    fn set_entrypoint(&mut self, entrypoint: &str) {
        self.entrypoint = entrypoint.to_owned()
    }

    fn get_address(&self) -> &str {
        &self.address
    }

    fn get_rule_name(&self) -> &str {
        &self.policy_id
    }

    fn get_entrypoint(&self) -> &str {
        &self.entrypoint
    }

    fn get_policy(&self) -> &[u8] {
        self.policy.as_ref().unwrap()
    }

    async fn load_policy(&mut self) -> Result<(), PolicyLoaderError> {
        let bundle = self.load_bundle_from_chain().await?;
        info!(loaded_policy_bytes=?bundle.len(), "Loaded policy");
        if bundle.is_empty() {
            error!("Policy not found: {}", self.get_rule_name());
            return Err(PolicyLoaderError::MissingPolicy(
                self.get_rule_name().to_string(),
            ));
        }
        self.load_policy_from_bundle(&Bundle::from_bytes(&*bundle)?)
    }

    fn load_policy_from_bytes(&mut self, policy: &[u8]) {
        self.policy = Some(policy.to_vec())
    }

    fn hash(&self) -> String {
        hex::encode(Sha256::digest(self.policy.as_ref().unwrap()))
    }
}

/// OPA policy loader for policies passed via CLI or embedded in Chronicle
#[derive(Clone, Default)]
pub struct CliPolicyLoader {
    address: String,
    rule_name: String,
    entrypoint: String,
    policy: Vec<u8>,
}

impl CliPolicyLoader {
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }

    #[instrument(level = "trace", skip(self), ret)]
    async fn get_policy_from_file(&mut self) -> Result<Vec<u8>, PolicyLoaderError> {
        let bundle = Bundle::from_file(self.get_address())?;

        self.load_policy_from_bundle(&bundle)?;

        Ok(self.get_policy().to_vec())
    }

    /// Create a loaded [`CliPolicyLoader`] from name of an embedded dev policy and entrypoint
    pub fn from_embedded_policy(policy: &str, entrypoint: &str) -> Result<Self, PolicyLoaderError> {
        if let Some(file) = EmbeddedOpaPolicies::get("bundle.tar.gz") {
            let bytes = file.data.as_ref();
            let bundle = Bundle::from_bytes(bytes)?;
            let mut loader = CliPolicyLoader::new();
            loader.set_rule_name(policy);
            loader.set_entrypoint(entrypoint);
            loader.load_policy_from_bundle(&bundle)?;
            Ok(loader)
        } else {
            Err(PolicyLoaderError::EmbeddedOpaPolicies)
        }
    }

    /// Create a loaded [`CliPolicyLoader`] from an OPA policy's bytes and entrypoint
    pub fn from_policy_bytes(
        policy: &str,
        entrypoint: &str,
        bytes: &[u8],
    ) -> Result<Self, PolicyLoaderError> {
        let mut loader = CliPolicyLoader::new();
        loader.set_rule_name(policy);
        loader.set_entrypoint(entrypoint);
        let bundle = Bundle::from_bytes(bytes)?;
        loader.load_policy_from_bundle(&bundle)?;
        Ok(loader)
    }
}

#[async_trait::async_trait]
impl PolicyLoader for CliPolicyLoader {
    fn set_address(&mut self, address: &str) {
        self.address = address.to_owned()
    }

    fn set_rule_name(&mut self, name: &str) {
        self.rule_name = name.to_owned()
    }

    fn set_entrypoint(&mut self, entrypoint: &str) {
        self.entrypoint = entrypoint.to_owned()
    }

    fn get_address(&self) -> &str {
        &self.address
    }

    fn get_rule_name(&self) -> &str {
        &self.rule_name
    }

    fn get_entrypoint(&self) -> &str {
        &self.entrypoint
    }

    fn get_policy(&self) -> &[u8] {
        &self.policy
    }

    fn load_policy_from_bytes(&mut self, policy: &[u8]) {
        self.policy = policy.to_vec()
    }

    async fn load_policy(&mut self) -> Result<(), PolicyLoaderError> {
        self.policy = self.get_policy_from_file().await?;
        Ok(())
    }

    fn hash(&self) -> String {
        hex::encode(Sha256::digest(&self.policy))
    }
}

#[derive(Debug, Error)]
pub enum OpaExecutorError {
    #[error("Access denied")]
    AccessDenied,

    #[error("Identity error: {0}")]
    IdentityError(#[from] IdentityError),

    #[error("Error loading OPA policy: {0}")]
    PolicyLoaderError(#[from] PolicyLoaderError),

    #[error("Error evaluating OPA policy: {0}")]
    OpaEvaluationError(#[from] anyhow::Error),
}

#[async_trait::async_trait]
pub trait OpaExecutor {
    /// Evaluate the loaded OPA instance against the provided identity and context
    async fn evaluate(&mut self, id: &AuthId, context: &OpaData) -> Result<(), OpaExecutorError>;
}

#[derive(Clone, Debug)]
pub struct ExecutorContext {
    executor: Arc<Mutex<WasmtimeOpaExecutor>>,
    hash: String,
}

impl ExecutorContext {
    #[instrument(skip(self), level = "trace", ret(Debug))]
    pub async fn evaluate(&self, id: &AuthId, context: &OpaData) -> Result<(), OpaExecutorError> {
        self.executor.lock().await.evaluate(id, context).await
    }

    pub fn from_loader<L: PolicyLoader>(loader: &L) -> Result<Self, OpaExecutorError> {
        Ok(Self {
            executor: Arc::new(Mutex::new(WasmtimeOpaExecutor::from_loader(loader)?)),
            hash: loader.hash(),
        })
    }

    pub fn hash(&self) -> &str {
        &self.hash
    }
}

#[derive(Debug)]
pub struct WasmtimeOpaExecutor {
    opa: Opa,
    entrypoint: String,
}

impl WasmtimeOpaExecutor {
    /// Build a `WasmtimeOpaExecutor` from the `PolicyLoader` provided
    pub fn from_loader<L: PolicyLoader>(loader: &L) -> Result<Self, OpaExecutorError> {
        Ok(Self {
            opa: loader.build_opa()?,
            entrypoint: loader.get_entrypoint().to_owned(),
        })
    }
}

#[async_trait::async_trait]
impl OpaExecutor for WasmtimeOpaExecutor {
    #[instrument(level = "trace", skip(self))]
    async fn evaluate(&mut self, id: &AuthId, context: &OpaData) -> Result<(), OpaExecutorError> {
        self.opa.set_data(context)?;
        let input = id.identity()?;
        match self.opa.eval(&self.entrypoint, &input)? {
            true => Ok(()),
            false => Err(OpaExecutorError::AccessDenied),
        }
    }
}

#[derive(RustEmbed)]
#[folder = "../../policies"]
#[include = "bundle.tar.gz"]
struct EmbeddedOpaPolicies;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::IdentityContext;
    use serde_json::Value;
    use std::collections::BTreeSet;

    fn chronicle_id() -> AuthId {
        AuthId::chronicle()
    }

    fn chronicle_user_opa_data() -> OpaData {
        OpaData::Operation(IdentityContext::new(
            AuthId::chronicle(),
            Value::default(),
            Value::default(),
        ))
    }

    fn allow_chronicle_and_anonymous() -> (String, String) {
        let policy_name = "allow_transactions".to_string();
        let entrypoint = "allow_transactions/allowed_users".to_string();
        (policy_name, entrypoint)
    }

    fn anonymous_user() -> AuthId {
        AuthId::anonymous()
    }

    fn anonymous_user_opa_data() -> OpaData {
        OpaData::Operation(IdentityContext::new(
            AuthId::anonymous(),
            Value::default(),
            Value::default(),
        ))
    }

    #[test]
    fn policy_loader_invalid_rule() {
        let (_policy, entrypoint) = allow_chronicle_and_anonymous();
        let invalid_rule = "a_rule_that_does_not_exist";
        match CliPolicyLoader::from_embedded_policy(invalid_rule, &entrypoint) {
            Err(e) => {
                insta::assert_snapshot!(e.to_string(), @"Policy not found: a_rule_that_does_not_exist")
            }
            _ => panic!("expected error"),
        }
    }

    #[tokio::test]
    async fn opa_executor_allow_chronicle_and_anonymous() -> Result<(), OpaExecutorError> {
        let (policy, entrypoint) = allow_chronicle_and_anonymous();
        let loader = CliPolicyLoader::from_embedded_policy(&policy, &entrypoint)?;
        let mut executor = WasmtimeOpaExecutor::from_loader(&loader).unwrap();
        assert!(executor
            .evaluate(&chronicle_id(), &chronicle_user_opa_data())
            .await
            .is_ok());

        assert!(executor
            .evaluate(&anonymous_user(), &anonymous_user_opa_data())
            .await
            .is_ok());
        Ok(())
    }

    fn jwt_user() -> AuthId {
        let claims = crate::identity::JwtClaims(
            serde_json::json!({
                "sub": "abcdef",
            })
            .as_object()
            .unwrap()
            .to_owned(),
        );
        AuthId::from_jwt_claims(&claims, &BTreeSet::from(["sub".to_string()])).unwrap()
    }

    fn jwt_user_opa_data() -> OpaData {
        OpaData::Operation(IdentityContext::new(
            jwt_user(),
            Value::default(),
            Value::default(),
        ))
    }

    #[tokio::test]
    async fn opa_executor_deny_unauthorized() -> Result<(), OpaExecutorError> {
        let (policy, entrypoint) = allow_chronicle_and_anonymous();
        let loader = CliPolicyLoader::from_embedded_policy(&policy, &entrypoint)?;
        let mut executor = WasmtimeOpaExecutor::from_loader(&loader)?;
        match executor.evaluate(&jwt_user(), &jwt_user_opa_data()).await {
            Err(e) => assert_eq!(e.to_string(), OpaExecutorError::AccessDenied.to_string()),
            _ => panic!("expected error"),
        }
        Ok(())
    }
}
