use crate::{
	identity::{AuthId, IdentityError, OpaData},
	import::FromUrlError,
};
use futures::lock::Mutex;
use opa::{bundle::Bundle, wasm::Opa};

#[cfg(not(feature = "std"))]
use parity_scale_codec::alloc::sync::Arc;
#[cfg(feature = "std")]
use std::sync::Arc;

use rust_embed::RustEmbed;
use thiserror::Error;
use tracing::{error, instrument};

#[derive(Debug, Error)]
pub enum PolicyLoaderError {
	#[error("Failed to read embedded OPA policies")]
	EmbeddedOpaPolicies,

	#[error("Policy not found: {0}")]
	MissingPolicy(String),

	#[error("OPA bundle I/O error: {0}")]
	OpaBundleError(#[from] opa::bundle::Error),

	#[error("Error loading OPA policy: {0}")]
	SawtoothCommunicationError(#[from] anyhow::Error),

	#[error("Error loading policy bundle from URL: {0}")]
	UrlError(#[from] FromUrlError),
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
		Ok(Self { opa: loader.build_opa()?, entrypoint: loader.get_entrypoint().to_owned() })
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
