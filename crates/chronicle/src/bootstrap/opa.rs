use clap::ArgMatches;
use opa::bundle::Bundle;
use tracing::{debug, error, info, instrument};

use common::opa::{
    OpaSettings,
    std::{ExecutorContext, load_bytes_from_url, PolicyLoader, PolicyLoaderError},
};
use protocol_substrate::SubxtClientError;
use protocol_substrate_chronicle::{ChronicleSubstrateClient, SettingsLoader};
use protocol_substrate_opa::{loader::SubstratePolicyLoader, OpaSubstrateClient, policy_hash};

use super::CliError;

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
        Self { ..Default::default() }
    }

    #[instrument(level = "trace", skip(self), ret)]
    async fn get_policy_from_file(&mut self) -> Result<Vec<u8>, PolicyLoaderError> {
        let bundle = Bundle::from_file(self.get_address())?;

        self.load_policy_from_bundle(&bundle)?;

        Ok(self.get_policy().to_vec())
    }

    /// Create a loaded [`CliPolicyLoader`] from name of an embedded dev policy and entrypoint
    pub fn from_embedded_policy(policy: &str, entrypoint: &str) -> Result<Self, PolicyLoaderError> {
        if let Some(file) = common::opa::std::EmbeddedOpaPolicies::get("bundle.tar.gz") {
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
        hex::encode(policy_hash(&self.policy))
    }
}

#[derive(Clone, Default)]
pub struct UrlPolicyLoader {
    policy_id: String,
    address: String,
    policy: Vec<u8>,
    entrypoint: String,
}

impl UrlPolicyLoader {
    pub fn new(url: &str, policy_id: &str, entrypoint: &str) -> Self {
        Self {
            address: url.into(),
            policy_id: policy_id.to_owned(),
            entrypoint: entrypoint.to_owned(),
            ..Default::default()
        }
    }
}

#[async_trait::async_trait]
impl PolicyLoader for UrlPolicyLoader {
    fn set_address(&mut self, address: &str) {
        self.address = address.to_owned();
    }

    fn set_rule_name(&mut self, name: &str) {
        self.policy_id = name.to_owned();
    }

    fn set_entrypoint(&mut self, entrypoint: &str) {
        self.entrypoint = entrypoint.to_owned();
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
        &self.policy
    }

    fn load_policy_from_bytes(&mut self, policy: &[u8]) {
        self.policy = policy.to_vec();
    }

    async fn load_policy(&mut self) -> Result<(), PolicyLoaderError> {
        let address = &self.address;
        let bundle = load_bytes_from_url(address).await?;

        info!(loaded_policy_bytes=?bundle.len(), "Loaded policy bundle");

        if bundle.is_empty() {
            error!("Policy not found: {}", self.get_rule_name());
            return Err(PolicyLoaderError::MissingPolicy(self.get_rule_name().to_string()));
        }

        self.load_policy_from_bundle(&Bundle::from_bytes(&*bundle)?)
    }

    fn hash(&self) -> String {
        hex::encode(policy_hash(&self.policy))
    }
}

trait SetRuleOptions {
    fn rule_addr(&mut self, options: &ArgMatches) -> Result<(), CliError>;
    fn rule_entrypoint(&mut self, options: &ArgMatches) -> Result<(), CliError>;
    fn set_addr_and_entrypoint(&mut self, options: &ArgMatches) -> Result<(), CliError> {
        self.rule_addr(options)?;
        self.rule_entrypoint(options)?;
        Ok(())
    }
}

impl SetRuleOptions for CliPolicyLoader {
    fn rule_addr(&mut self, options: &ArgMatches) -> Result<(), CliError> {
        if let Some(val) = options.get_one::<String>("opa-rule") {
            self.set_address(val);
            Ok(())
        } else {
            Err(CliError::MissingArgument { arg: "opa-rule".to_string() })
        }
    }

    fn rule_entrypoint(&mut self, options: &ArgMatches) -> Result<(), CliError> {
        if let Some(val) = options.get_one::<String>("opa-entrypoint") {
            self.set_entrypoint(val);
            Ok(())
        } else {
            Err(CliError::MissingArgument { arg: "opa-entrypoint".to_string() })
        }
    }
}

#[instrument()]
pub async fn opa_executor_from_embedded_policy(
    policy_name: &str,
    entrypoint: &str,
) -> Result<ExecutorContext, CliError> {
    let loader = CliPolicyLoader::from_embedded_policy(policy_name, entrypoint)?;
    Ok(ExecutorContext::from_loader(&loader)?)
}

pub async fn read_opa_settings(
    client: &ChronicleSubstrateClient<protocol_substrate::PolkadotConfig>,
) -> Result<Option<OpaSettings>, SubxtClientError> {
    client.load_settings_from_storage().await
}

#[instrument(skip(chronicle_client, opa_client))]
pub async fn opa_executor_from_substrate_state(
    chronicle_client: &ChronicleSubstrateClient<protocol_substrate::PolkadotConfig>,
    opa_client: &OpaSubstrateClient<protocol_substrate::PolkadotConfig>,
) -> Result<(ExecutorContext, Option<OpaSettings>), CliError> {
    let opa_settings = read_opa_settings(chronicle_client).await?;
    debug!(on_chain_opa_policy = ?opa_settings);
    if let Some(opa_settings) = opa_settings {
        let mut loader = SubstratePolicyLoader::new(opa_settings.clone(), opa_client);
        loader.load_policy().await?;

        Ok((ExecutorContext::from_loader(&loader)?, Some(opa_settings)))
    } else {
        Err(CliError::NoOnChainSettings)
    }
}

#[instrument()]
pub async fn opa_executor_from_url(
    url: &str,
    policy_name: &str,
    entrypoint: &str,
) -> Result<ExecutorContext, CliError> {
    let mut loader = UrlPolicyLoader::new(url, policy_name, entrypoint);
    loader.load_policy().await?;
    Ok(ExecutorContext::from_loader(&loader)?)
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, io::Write};

    use serde_json::Value;

    use common::{
        identity::{AuthId, IdentityContext, JwtClaims, OpaData},
        opa::std::{EmbeddedOpaPolicies, OpaExecutor, OpaExecutorError, WasmtimeOpaExecutor},
    };

    use super::*;

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

    fn allow_all_users() -> (String, String) {
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

    fn jwt_user() -> AuthId {
        let claims = JwtClaims(
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
        OpaData::Operation(IdentityContext::new(jwt_user(), Value::default(), Value::default()))
    }

    #[test]
    fn policy_loader_invalid_rule() {
        let (_policy, entrypoint) = allow_all_users();
        let invalid_rule = "a_rule_that_does_not_exist";
        match CliPolicyLoader::from_embedded_policy(invalid_rule, &entrypoint) {
            Err(e) => {
                insta::assert_snapshot!(e.to_string(), @"Policy not found: a_rule_that_does_not_exist")
            }
            _ => panic!("expected error"),
        }
    }

    #[tokio::test]
    async fn opa_executor_allow_chronicle_users() -> Result<(), OpaExecutorError> {
        let (policy, entrypoint) = allow_all_users();
        let loader = CliPolicyLoader::from_embedded_policy(&policy, &entrypoint)?;
        let mut executor = WasmtimeOpaExecutor::from_loader(&loader).unwrap();
        assert!(executor.evaluate(&chronicle_id(), &chronicle_user_opa_data()).await.is_ok());
        Ok(())
    }

    #[tokio::test]
    async fn opa_executor_allow_anonymous_users() -> Result<(), OpaExecutorError> {
        let (policy, entrypoint) = allow_all_users();
        let loader = CliPolicyLoader::from_embedded_policy(&policy, &entrypoint)?;
        let mut executor = WasmtimeOpaExecutor::from_loader(&loader).unwrap();
        executor.evaluate(&anonymous_user(), &anonymous_user_opa_data()).await.unwrap();
        Ok(())
    }

    #[tokio::test]
    async fn opa_executor_allow_jwt_users() -> Result<(), OpaExecutorError> {
        let (policy, entrypoint) = allow_all_users();
        let loader = CliPolicyLoader::from_embedded_policy(&policy, &entrypoint)?;
        let mut executor = WasmtimeOpaExecutor::from_loader(&loader)?;
        assert!(executor.evaluate(&jwt_user(), &jwt_user_opa_data()).await.is_ok());
        Ok(())
    }

    const BUNDLE_FILE: &str = "bundle.tar.gz";

    fn embedded_policy_bundle() -> Result<Vec<u8>, PolicyLoaderError> {
        EmbeddedOpaPolicies::get(BUNDLE_FILE)
            .map(|file| file.data.to_vec())
            .ok_or(PolicyLoaderError::EmbeddedOpaPolicies)
    }

    #[tokio::test]
    async fn test_load_policy_from_http_url() {
        let embedded_bundle = embedded_policy_bundle().unwrap();
        let (rule, entrypoint) = allow_all_users();

        let mut server = mockito::Server::new_async().await;
        // Start the mock server and define the response
        let _m = server.mock("GET", "/bundle.tar.gz").with_body(&embedded_bundle).create();

        // Create the URL policy loader
        let mut loader =
            UrlPolicyLoader::new(&format!("{}/bundle.tar.gz", server.url()), &rule, &entrypoint);

        // Load the policy
        let result = loader.load_policy().await;
        assert!(result.is_ok());

        let bundle = Bundle::from_bytes(&embedded_bundle).unwrap();

        // Extract the policy from the bundle we embedded in the binary
        let policy_from_embedded_bundle = bundle
            .wasm_policies
            .iter()
            .find(|p| p.entrypoint == rule)
            .map(|p| p.bytes.as_ref())
            .ok_or(PolicyLoaderError::MissingPolicy(rule.to_string()))
            .unwrap();

        // Get the loaded policy from the url
        let policy_from_url = loader.get_policy();

        assert_eq!(&policy_from_url, &policy_from_embedded_bundle);
    }

    #[tokio::test]
    async fn test_load_policy_from_file_url() {
        let embedded_bundle = embedded_policy_bundle().unwrap();
        let (rule, entrypoint) = allow_all_users();

        let temp_dir = tempfile::tempdir().unwrap();
        let policy_path = temp_dir.path().join("bundle.tar.gz");
        let mut file = std::fs::File::create(&policy_path).unwrap();
        file.write_all(&embedded_bundle).unwrap();

        // Create the file URL policy loader
        let file_url = format!("file://{}", policy_path.to_string_lossy());
        let mut loader = UrlPolicyLoader::new(&file_url, &rule, &entrypoint);

        // Load the policy
        let result = loader.load_policy().await;
        assert!(result.is_ok());

        let bundle = Bundle::from_bytes(&embedded_bundle).unwrap();

        // Extract the policy from the bundle we embedded in the binary
        let policy_from_embedded_bundle = bundle
            .wasm_policies
            .iter()
            .find(|p| p.entrypoint == rule)
            .map(|p| p.bytes.as_ref())
            .ok_or(PolicyLoaderError::MissingPolicy(rule.to_string()))
            .unwrap();

        // Get the loaded policy from the file URL
        let policy_from_file_url = loader.get_policy();

        assert_eq!(policy_from_embedded_bundle, policy_from_file_url);
    }

    #[tokio::test]
    async fn test_load_policy_from_bare_path() {
        let embedded_bundle = embedded_policy_bundle().unwrap();
        let (rule, entrypoint) = allow_all_users();

        let temp_dir = tempfile::tempdir().unwrap();
        let policy_path = temp_dir.path().join("bundle.tar.gz");
        let mut file = std::fs::File::create(&policy_path).unwrap();
        file.write_all(&embedded_bundle).unwrap();

        // Create the bare path policy loader
        let mut loader = UrlPolicyLoader::new(&policy_path.to_string_lossy(), &rule, &entrypoint);

        // Load the policy
        let result = loader.load_policy().await;
        assert!(result.is_ok());

        let bundle = Bundle::from_bytes(&embedded_bundle).unwrap();

        // Extract the policy from the bundle we embedded in the binary
        let policy_from_embedded_bundle = bundle
            .wasm_policies
            .iter()
            .find(|p| p.entrypoint == rule)
            .map(|p| p.bytes.as_ref())
            .ok_or(PolicyLoaderError::MissingPolicy(rule.to_string()))
            .unwrap();

        // Get the loaded policy from the url
        let policy_from_bare_path_url = loader.get_policy();

        assert_eq!(policy_from_embedded_bundle, policy_from_bare_path_url);
    }
}
