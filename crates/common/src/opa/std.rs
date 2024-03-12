use futures::lock::Mutex;
use opa::{bundle::Bundle, wasm::Opa};
use rust_embed::RustEmbed;
use url::Url;

use std::{fs::File, io::Read, path::PathBuf, sync::Arc};

use thiserror::Error;
use tracing::{error, instrument};

use crate::identity::{AuthId, IdentityError, OpaData};

use super::{KeyAddress, PolicyAddress, PolicyMetaAddress};

#[derive(RustEmbed)]
#[folder = "../../policies"]
#[include = "bundle.tar.gz"]
pub struct EmbeddedOpaPolicies;

// Prefer these functions over the core ones in std, as they are more efficient
pub fn policy_address(id: impl AsRef<str>) -> PolicyAddress {
	sp_core::blake2_128(format!("opa:policy:binary:{}", id.as_ref()).as_bytes()).into()
}

// Prefer these functions over the core ones in std, as they are more efficient
pub fn policy_meta_address(id: impl AsRef<str>) -> PolicyMetaAddress {
	sp_core::blake2_128(format!("opa:policy:meta:{}", id.as_ref()).as_bytes()).into()
}

// Prefer these functions over the core ones in std, as they are more efficient
pub fn key_address(id: impl AsRef<str>) -> KeyAddress {
	sp_core::blake2_128(format!("opa:keys:{}", id.as_ref()).as_bytes()).into()
}

#[derive(Error, Debug)]
pub enum FromUrlError {
	#[error("HTTP error while attempting to read from URL: {0}")]
	HTTP(
		#[from]
		#[source]
		reqwest::Error,
	),

	#[error("Invalid URL scheme: {0}")]
	InvalidUrlScheme(String),

	#[error("IO error while attempting to read from URL: {0}")]
	IO(
		#[from]
		#[source]
		std::io::Error,
	),
}

pub enum PathOrUrl {
	File(PathBuf),
	Url(Url),
}

pub async fn load_bytes_from_url(url: &str) -> Result<Vec<u8>, FromUrlError> {
	let path_or_url = match url.parse::<Url>() {
		Ok(url) => PathOrUrl::Url(url),
		Err(_) => PathOrUrl::File(PathBuf::from(url)),
	};

	let content = match path_or_url {
		PathOrUrl::File(path) => {
			let mut file = File::open(path)?;
			let mut buf = Vec::new();
			file.read_to_end(&mut buf)?;
			Ok(buf)
		},
		PathOrUrl::Url(url) => match url.scheme() {
			"file" => {
				let mut file = File::open(url.path())?;
				let mut buf = Vec::new();
				file.read_to_end(&mut buf)?;
				Ok(buf)
			},
			"http" | "https" => Ok(reqwest::get(url).await?.bytes().await?.into()),
			_ => Err(FromUrlError::InvalidUrlScheme(url.scheme().to_owned())),
		},
	}?;

	Ok(content)
}

pub fn load_bytes_from_stdin() -> Result<Vec<u8>, std::io::Error> {
	let mut buffer = Vec::new();
	let mut stdin = std::io::stdin();
	let _ = stdin.read_to_end(&mut buffer)?;
	Ok(buffer)
}
#[derive(Debug, Error)]
pub enum OpaExecutorError {
	#[error("Access denied")]
	AccessDenied,

	#[error("Identity error: {0}")]
	IdentityError(
		#[from]
		#[source]
		IdentityError,
	),

	#[error("Error loading OPA policy: {0}")]
	PolicyLoaderError(
		#[from]
		#[source]
		PolicyLoaderError,
	),

	#[error("Error evaluating OPA policy: {0}")]
	OpaEvaluationError(
		#[from]
		#[source]
		anyhow::Error,
	),
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

#[derive(Debug, Error)]
pub enum PolicyLoaderError {
	#[error("Failed to read embedded OPA policies")]
	EmbeddedOpaPolicies,

	#[error("Policy not found: {0}")]
	MissingPolicy(String),

	#[error("OPA bundle I/O error: {0}")]
	OpaBundleError(
		#[from]
		#[source]
		opa::bundle::Error,
	),

	#[error("Error loading OPA policy: {0}")]
	Substrate(
		#[from]
		#[source]
		anyhow::Error,
	),

	#[error("Error loading from URL: {0}")]
	FromUrl(
		#[from]
		#[source]
		FromUrlError,
	),
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
