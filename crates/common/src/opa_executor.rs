use crate::identity::{AuthId, IdentityContext};
use opa::wasm::Opa;
use opa_tp_protocol::state::policy_address;
use protobuf::{ProtobufEnum, ProtobufError};
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
use std::io::Read;
use thiserror::Error;
use tokio::runtime::Handle;
use tracing::{debug, error, instrument, trace};
use url::Url;

#[derive(Debug, Error)]
pub enum PolicyLoaderError {
    #[error("Error loading OPA policy: {0}")]
    SawtoothPolicyLoader(#[from] SawtoothCommunicationError),

    #[error("Error loading OPA policy: {0}")]
    CliPolicyLoader(#[from] CliPolicyLoaderError),
}

#[async_trait::async_trait]
pub trait PolicyLoader {
    fn set_policy_address(&mut self, address: &str);
    fn set_policy_entrypoint(&mut self, entrypoint: &str);
    fn entrypoint(&self) -> &str;
    async fn load_policy(&mut self) -> Result<Vec<u8>, PolicyLoaderError>;
}

#[derive(Error, Debug)]
pub enum SawtoothCommunicationError {
    #[error("Protobuf error {0}")]
    Protobuf(#[from] ProtobufError),

    #[error("Receive error {0}")]
    Receive(#[from] ReceiveError),

    #[error("Send error {0}")]
    Send(#[from] SendError),

    #[error("Unexpected status: {0}")]
    UnexpectedStatus(i32),

    #[error("ZMQ error {0}")]
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

    #[instrument(level = "info", skip(self), ret)]
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
    entrypoint: String,
}

impl SawtoothPolicyLoader {
    pub fn new(address: &Url) -> Self {
        Self {
            messenger_address: address.to_owned(),
            address: String::new(),
            entrypoint: String::new(),
        }
    }

    fn sawtooth_address(&self, policy: impl AsRef<str>) -> String {
        policy_address(policy)
    }

    #[instrument(level = "info", skip(self), ret)]
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
    fn set_policy_address(&mut self, address: &str) {
        self.address = address.to_owned()
    }

    fn set_policy_entrypoint(&mut self, entrypoint: &str) {
        self.entrypoint = entrypoint.to_owned()
    }

    fn entrypoint(&self) -> &str {
        &self.entrypoint
    }

    async fn load_policy(&mut self) -> Result<Vec<u8>, PolicyLoaderError> {
        Ok(self.get_policy().await?)
    }
}

#[derive(Error, Debug)]
pub enum CliPolicyLoaderError {
    #[error("Io error {0}")]
    FileIo(#[source] std::io::Error),

    #[error("Error reading policy {0}")]
    ReadError(#[from] std::io::Error),
}

#[derive(Clone, Default)]
pub struct CliPolicyLoader {
    address: String,
    entrypoint: String,
}

impl CliPolicyLoader {
    pub fn new() -> Self {
        Self {
            address: String::new(),
            entrypoint: String::new(),
        }
    }

    #[instrument(level = "info", skip(self), ret)]
    async fn get_policy(&self) -> Result<Vec<u8>, CliPolicyLoaderError> {
        let mut policy = Vec::<u8>::new();
        {
            let file = std::fs::File::open(&self.address).map_err(CliPolicyLoaderError::FileIo)?;
            let mut buf_reader = std::io::BufReader::new(file);
            buf_reader.read_to_end(&mut policy)?;
        }
        Ok(policy)
    }
}

#[async_trait::async_trait]
impl PolicyLoader for CliPolicyLoader {
    fn set_policy_address(&mut self, address: &str) {
        self.address = address.to_owned()
    }

    fn set_policy_entrypoint(&mut self, entrypoint: &str) {
        self.entrypoint = entrypoint.to_owned()
    }

    fn entrypoint(&self) -> &str {
        &self.entrypoint
    }

    async fn load_policy(&mut self) -> Result<Vec<u8>, PolicyLoaderError> {
        Ok(self.get_policy().await?)
    }
}

#[derive(Debug, Error)]
pub enum OpaExecutorError {
    #[error("Error loading OPA policy")]
    PolicyLoaderError(#[from] PolicyLoaderError),

    #[error("Error evaluating OPA policy")]
    OpaEvaluationError(#[from] anyhow::Error),
}

#[async_trait::async_trait]
pub trait OpaExecutor {
    fn new(policy: &[u8], entrypoint: &str) -> Self;
    async fn evaluate(
        &self,
        id: &AuthId,
        context: &serde_json::Value,
    ) -> Result<bool, OpaExecutorError>;
}

pub struct WasmtimeOpaExecutor {
    policy: Vec<u8>,
    entrypoint: String,
}

#[async_trait::async_trait]
impl OpaExecutor for WasmtimeOpaExecutor {
    fn new(policy: &[u8], entrypoint: &str) -> Self {
        Self {
            policy: policy.to_vec(),
            entrypoint: entrypoint.to_owned(),
        }
    }

    #[instrument(level = "info", skip_all, ret)]
    async fn evaluate(
        &self,
        id: &AuthId,
        context: &serde_json::Value,
    ) -> Result<bool, OpaExecutorError> {
        let mut opa = Opa::new().build(&self.policy)?;

        let input = IdentityContext::new(id);
        opa.set_data(context)?;

        Ok(opa.eval(&self.entrypoint, &input)?)
    }
}

#[cfg(test)]
mod tests {
    use super::{OpaExecutor, OpaExecutorError, WasmtimeOpaExecutor};
    use crate::{
        identity::AuthId,
        opa_executor::{CliPolicyLoader, PolicyLoader},
        prov::AgentId,
    };
    use std::{
        io::{BufReader, Read},
        path::Path,
    };

    // See src/dev_policies/auth.rego for policy source
    fn auth_entrypoint() -> String {
        "auth.is_authenticated".to_string()
    }

    fn chronicle_id() -> AuthId {
        AuthId::chronicle()
    }

    // Empty placeholder context
    fn context() -> serde_json::Value {
        serde_json::json!({})
    }

    #[tokio::test]
    async fn cli_loader() -> Result<(), OpaExecutorError> {
        let mut loader = CliPolicyLoader::new();
        // See src/dev_policies/auth.rego for policy source
        loader.set_policy_address("src/dev_policies/auth.wasm");
        loader.set_policy_entrypoint(&auth_entrypoint());

        let executor = WasmtimeOpaExecutor::new(&loader.load_policy().await?, &loader.entrypoint);

        let result = executor.evaluate(&chronicle_id(), &context()).await?;

        assert!(result);

        Ok(())
    }

    fn policy<P: AsRef<Path>>(path: P) -> Vec<u8> {
        let mut policy = Vec::<u8>::new();

        {
            let file = std::fs::File::open(path).unwrap();
            let mut buf_reader = BufReader::new(file);
            buf_reader.read_to_end(&mut policy).unwrap();
        }

        policy
    }

    #[tokio::test]
    async fn opa_executor_authorized_id() -> Result<(), OpaExecutorError> {
        // See src/dev_policies/auth.rego for source
        let policy = policy("src/dev_policies/auth.wasm");
        let entrypoint = auth_entrypoint();
        let executor = WasmtimeOpaExecutor::new(&policy, &entrypoint);

        let result = executor.evaluate(&chronicle_id(), &context()).await?;

        assert!(result);

        Ok(())
    }

    fn unauthorized_agent() -> AuthId {
        AuthId::agent(&AgentId::from_external_id("x"))
    }

    #[tokio::test]
    async fn opa_executor_unauthorized_id() -> Result<(), OpaExecutorError> {
        // See src/dev_policies/auth.rego for source
        let policy = policy("src/dev_policies/auth.wasm");
        let entrypoint = auth_entrypoint();
        let executor = WasmtimeOpaExecutor::new(&policy, &entrypoint);

        let result = executor.evaluate(&unauthorized_agent(), &context()).await?;

        assert!(!result);

        Ok(())
    }

    #[tokio::test]
    async fn opa_executor_blank_wasm() {
        // Source - empty file with .wasm suffix
        let policy = policy("src/dev_policies/blank.wasm");
        let entrypoint = auth_entrypoint();
        let executor = WasmtimeOpaExecutor::new(&policy, &entrypoint);

        match executor.evaluate(&chronicle_id(), &context()).await {
            Err(OpaExecutorError::OpaEvaluationError(source)) => {
                insta::assert_snapshot!(source.to_string(), @"failed to parse WebAssembly module")
            }
            _ => panic!("expected error"),
        }
    }

    #[tokio::test]
    async fn opa_executor_invalid_wasm() {
        // Source - Base64 encoded image with .wasm suffix
        let policy = policy("src/dev_policies/invalid_content.wasm");
        let entrypoint = auth_entrypoint();
        let executor = WasmtimeOpaExecutor::new(&policy, &entrypoint);

        match executor.evaluate(&chronicle_id(), &context()).await {
            Err(OpaExecutorError::OpaEvaluationError(source)) => {
                insta::assert_snapshot!(source.to_string(), @"failed to parse WebAssembly module")
            }
            _ => panic!("expected error"),
        }
    }

    fn invalid_entrypoint() -> String {
        "x".to_string()
    }

    #[tokio::test]
    async fn opa_executor_invalid_entrypoint() {
        // See src/dev_policies/auth.rego for source
        let policy = policy("src/dev_policies/auth.wasm");
        let entrypoint = invalid_entrypoint();
        let executor = WasmtimeOpaExecutor::new(&policy, &entrypoint);

        match executor.evaluate(&chronicle_id(), &context()).await {
            Err(OpaExecutorError::OpaEvaluationError(source)) => {
                insta::assert_snapshot!(source.to_string(), @"invalid entrypoint `x`")
            }
            _ => panic!("expected error"),
        }
    }

    #[tokio::test]
    async fn opa_executor_default_allow() -> Result<(), OpaExecutorError> {
        // See src/dev_policies/default_allow.rego for source
        let policy = policy("src/dev_policies/default_allow.wasm");
        let entrypoint = "default_allow.allow".to_string();
        let executor = WasmtimeOpaExecutor::new(&policy, &entrypoint);

        let result = executor.evaluate(&chronicle_id(), &context()).await?;

        assert!(result);

        Ok(())
    }

    #[tokio::test]
    async fn opa_executor_default_deny() -> Result<(), OpaExecutorError> {
        // See src/dev_policies/default_deny.rego for source
        let policy = policy("src/dev_policies/default_deny.wasm");
        let entrypoint = "default_deny.allow".to_string();
        let executor = WasmtimeOpaExecutor::new(&policy, &entrypoint);

        let result = executor.evaluate(&chronicle_id(), &context()).await?;

        assert!(!result);

        Ok(())
    }
}
