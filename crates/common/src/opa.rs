use crate::identity::{AuthId, IdentityError};
use opa::{bundle::Bundle, wasm::Opa};
use opa_tp_protocol::state::policy_address;
use protobuf::{ProtobufEnum, ProtobufError};
use rust_embed::RustEmbed;
use sawtooth_sdk::{
    messages::{
        client_state::{ClientStateGetRequest, ClientStateGetResponse},
        validator::Message_MessageType,
    },
    messaging::{
        stream::{MessageConnection, MessageReceiver, MessageSender, ReceiveError, SendError},
        zmq_stream::{ZmqMessageConnection, ZmqMessageSender},
    },
};
use std::sync::Arc;
use thiserror::Error;
use tokio::{runtime::Handle, sync::Mutex};
use tracing::{debug, error, instrument, trace};
use url::Url;

#[derive(Debug, Error)]
pub enum PolicyLoaderError {
    #[error("Failed to read embedded OPA policies")]
    EmbeddedOpaPolicies,

    #[error("Policy not found with entrypoint: {0}")]
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

    /// Set entrypoint for OPA policy
    fn set_entrypoint(&mut self, entrypoint: &str);

    fn get_address(&self) -> &str;

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
        let entrypoint = self.get_entrypoint();
        self.load_policy_from_bytes(
            bundle
                .wasm_policies
                .iter()
                .find(|p| p.entrypoint == entrypoint.replace('.', "/"))
                .map(|p| p.bytes.as_ref())
                .ok_or(PolicyLoaderError::MissingPolicy(entrypoint.to_string()))?,
        );
        Ok(())
    }
}

#[derive(Error, Debug)]
pub enum SawtoothCommunicationError {
    #[error("Protobuf error: {0}")]
    Protobuf(#[from] ProtobufError),

    #[error("Receive error: {0}")]
    Receive(#[from] ReceiveError),

    #[error("Send error: {0}")]
    Send(#[from] SendError),

    #[error("Unexpected status: {0}")]
    UnexpectedStatus(i32),

    #[error("ZMQ error: {0}")]
    ZMQ(#[from] zmq::Error),
}

pub struct ZmqRequestResponseSawtoothChannel {
    tx: ZmqMessageSender,
    _rx: MessageReceiver,
}

impl ZmqRequestResponseSawtoothChannel {
    fn new(address: &Url) -> Self {
        let (tx, _rx) = ZmqMessageConnection::new(address.as_str()).create();
        Self { tx, _rx }
    }
}

#[async_trait::async_trait(?Send)]
trait RequestResponseSawtoothChannel {
    fn message_type() -> Message_MessageType;
    async fn receive_one<RX: protobuf::Message, TX: protobuf::Message>(
        &self,
        tx: &TX,
    ) -> Result<RX, SawtoothCommunicationError>;
}

#[async_trait::async_trait(?Send)]
impl RequestResponseSawtoothChannel for ZmqRequestResponseSawtoothChannel {
    fn message_type() -> Message_MessageType {
        Message_MessageType::CLIENT_STATE_GET_REQUEST
    }

    #[instrument(level = "trace", skip(self), ret)]
    async fn receive_one<RX: protobuf::Message, TX: protobuf::Message>(
        &self,
        tx: &TX,
    ) -> Result<RX, SawtoothCommunicationError> {
        let correlation_id = uuid::Uuid::new_v4().to_string();
        let mut bytes = vec![];
        tx.write_to_vec(&mut bytes)?;
        let res = Handle::current().block_on(async move {
            let mut future = self
                .tx
                .send(Self::message_type(), &correlation_id, &bytes)?;

            debug!(send_message=%correlation_id);
            trace!(body=?tx);

            Ok(future.get_timeout(std::time::Duration::from_secs(10))?)
        });

        res.and_then(|res| {
            RX::parse_from_bytes(&res.content).map_err(SawtoothCommunicationError::from)
        })
    }
}

#[derive(Clone)]
pub struct SawtoothPolicyLoader {
    messenger_address: Url,
    address: String,
    policy: Vec<u8>,
    entrypoint: String,
}

impl SawtoothPolicyLoader {
    pub fn new(address: &Url) -> Self {
        Self {
            messenger_address: address.to_owned(),
            address: String::default(),
            policy: Vec::default(),
            entrypoint: String::default(),
        }
    }

    fn sawtooth_address(&self, policy: impl AsRef<str>) -> String {
        policy_address(policy)
    }
    #[instrument(level = "trace", skip(self), ret)]
    async fn get_policy(&mut self) -> Result<Vec<u8>, SawtoothCommunicationError> {
        Handle::current().block_on(async move {
            let mut tx = ClientStateGetRequest::new();
            let address = self.sawtooth_address(&self.address);
            tx.set_address(address);

            let messenger = ZmqRequestResponseSawtoothChannel::new(&self.messenger_address);
            let response: ClientStateGetResponse = messenger.receive_one(&tx).await?;

            debug!(validator_response=?response);

            if response.status.value() == 1 {
                Ok(response.value)
            } else {
                Err(SawtoothCommunicationError::UnexpectedStatus(
                    response.status.value(),
                ))
            }
        })
    }
}

#[async_trait::async_trait]
impl PolicyLoader for SawtoothPolicyLoader {
    fn set_address(&mut self, address: &str) {
        self.address = address.to_owned()
    }

    fn set_entrypoint(&mut self, entrypoint: &str) {
        self.entrypoint = entrypoint.to_owned()
    }

    fn get_address(&self) -> &str {
        &self.address
    }

    fn get_entrypoint(&self) -> &str {
        &self.entrypoint
    }

    fn get_policy(&self) -> &[u8] {
        &self.policy
    }

    async fn load_policy(&mut self) -> Result<(), PolicyLoaderError> {
        self.policy = self.get_policy().await?;
        Ok(())
    }

    fn load_policy_from_bytes(&mut self, policy: &[u8]) {
        self.policy = policy.to_vec()
    }
}

/// OPA policy loader for policies passed via CLI or embedded in Chronicle for development
#[derive(Clone, Default)]
pub struct CliPolicyLoader {
    address: String,
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

    /// Create a loaded [`CliPolicyLoader`] from the filename of an embedded dev policy
    /// in tar.gz format and entrypoint
    pub fn from_embedded_policy(wasm: &str, entrypoint: &str) -> Result<Self, PolicyLoaderError> {
        if let Some(file) = EmbeddedOpaPolicies::get(wasm) {
            let bytes = file.data.as_ref();
            let bundle = Bundle::from_bytes(bytes)?;
            let mut loader = CliPolicyLoader::new();
            loader.set_entrypoint(entrypoint);
            loader.load_policy_from_bundle(&bundle)?;
            Ok(loader)
        } else {
            Err(PolicyLoaderError::EmbeddedOpaPolicies)
        }
    }

    /// Create a loaded [`CliPolicyLoader`] from an OPA policy's bytes and entrypoint
    pub fn from_policy_bytes(entrypoint: &str, policy: &[u8]) -> CliPolicyLoader {
        let mut loader = CliPolicyLoader::new();
        loader.set_entrypoint(entrypoint);
        loader.load_policy_from_bytes(policy);
        loader
    }
}

#[async_trait::async_trait]
impl PolicyLoader for CliPolicyLoader {
    fn set_address(&mut self, address: &str) {
        self.address = address.to_owned()
    }

    fn set_entrypoint(&mut self, entrypoint: &str) {
        self.entrypoint = entrypoint.to_owned()
    }

    fn get_address(&self) -> &str {
        &self.address
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
    async fn evaluate(
        &mut self,
        id: &AuthId,
        context: &serde_json::Value,
    ) -> Result<(), OpaExecutorError>;
}

#[derive(Clone, Debug)]
pub struct ExecutorContext {
    executor: Arc<Mutex<WasmtimeOpaExecutor>>,
}

impl ExecutorContext {
    #[instrument(skip(self), level = "trace", ret(Debug))]
    pub async fn evaluate(
        &self,
        id: &AuthId,
        context: &serde_json::Value,
    ) -> Result<(), OpaExecutorError> {
        self.executor.lock().await.evaluate(id, context).await
    }

    pub fn from_loader<L: PolicyLoader>(loader: &L) -> Result<Self, OpaExecutorError> {
        Ok(Self {
            executor: Arc::new(Mutex::new(WasmtimeOpaExecutor::from_loader(loader)?)),
        })
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
    #[instrument(level = "info", skip(self), ret)]
    async fn evaluate(
        &mut self,
        id: &AuthId,
        context: &serde_json::Value,
    ) -> Result<(), OpaExecutorError> {
        self.opa.set_data(context)?;
        let input = id.identity_context()?;
        match self.opa.eval(&self.entrypoint, &input)? {
            true => Ok(()),
            false => Err(OpaExecutorError::AccessDenied),
        }
    }
}

/// Embedded OPA development policies - See `crates/common/src/policies/` for source
#[derive(RustEmbed)]
#[folder = "$OUT_DIR/opa"]
struct EmbeddedOpaPolicies;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{identity::AuthId, prov::AgentId};

    fn chronicle_id() -> AuthId {
        AuthId::chronicle()
    }

    // Empty placeholder context for OPA check
    fn context() -> serde_json::Value {
        serde_json::json!({})
    }

    // Dev policy and entrypoints
    fn auth_is_authenticated() -> (String, String) {
        (
            "auth.tar.gz".to_string(),
            "auth.is_authenticated".to_string(),
        )
    }
    // '.' and '/' in policy entrypoint style both work
    fn default_allow() -> (String, String) {
        (
            "default_allow.tar.gz".to_string(),
            "default_allow/allow".to_string(),
        )
    }
    fn default_deny() -> (String, String) {
        (
            "default_deny.tar.gz".to_string(),
            "default_deny.allow".to_string(),
        )
    }

    #[tokio::test]
    async fn opa_executor_authorized_id() -> Result<(), OpaExecutorError> {
        let (wasm, entrypoint) = auth_is_authenticated();
        let loader = CliPolicyLoader::from_embedded_policy(&wasm, &entrypoint)?;
        let mut executor = WasmtimeOpaExecutor::from_loader(&loader)?;
        assert!(executor.evaluate(&chronicle_id(), &context()).await.is_ok());
        Ok(())
    }

    fn unauthorized_agent() -> AuthId {
        AuthId::agent(&AgentId::from_external_id("x"))
    }

    #[tokio::test]
    async fn opa_executor_unauthorized_id() -> Result<(), OpaExecutorError> {
        let (wasm, entrypoint) = auth_is_authenticated();
        let loader = CliPolicyLoader::from_embedded_policy(&wasm, &entrypoint)?;
        let mut executor = WasmtimeOpaExecutor::from_loader(&loader)?;
        match executor.evaluate(&unauthorized_agent(), &context()).await {
            Err(e) => assert_eq!(e.to_string(), OpaExecutorError::AccessDenied.to_string()),
            _ => panic!("expected error"),
        }
        Ok(())
    }

    #[test]
    fn policy_loader_invalid_entrypoint() {
        let (wasm, _entrypoint) = auth_is_authenticated();
        let invalid_entrypoint = "x";
        match CliPolicyLoader::from_embedded_policy(&wasm, invalid_entrypoint) {
            Err(e) => {
                insta::assert_snapshot!(e.to_string(), @"Policy not found with entrypoint: x")
            }
            _ => panic!("expected error"),
        }
    }

    #[tokio::test]
    async fn opa_executor_default_allow() -> Result<(), OpaExecutorError> {
        let (wasm, entrypoint) = default_allow();
        let loader = CliPolicyLoader::from_embedded_policy(&wasm, &entrypoint)?;
        let mut executor = WasmtimeOpaExecutor::from_loader(&loader).unwrap();
        assert!(executor.evaluate(&chronicle_id(), &context()).await.is_ok());
        Ok(())
    }

    #[tokio::test]
    async fn opa_executor_default_deny() -> Result<(), OpaExecutorError> {
        let (wasm, entrypoint) = default_deny();
        let loader = CliPolicyLoader::from_embedded_policy(&wasm, &entrypoint)?;
        let mut executor = WasmtimeOpaExecutor::from_loader(&loader)?;
        match executor.evaluate(&chronicle_id(), &context()).await {
            Err(e) => assert_eq!(e.to_string(), OpaExecutorError::AccessDenied.to_string()),
            _ => panic!("expected error"),
        }
        Ok(())
    }
}