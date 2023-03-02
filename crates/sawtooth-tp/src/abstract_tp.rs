use chronicle_protocol::{address::SawtoothAddress, protocol::messages::Submission};
use common::{
    identity::SignedIdentity,
    ledger::OperationState,
    opa::ExecutorContext,
    prov::{operations::ChronicleOperation, ChronicleTransaction},
};
use sawtooth_sdk::{
    messages::processor::TpProcessRequest,
    processor::handler::{ApplyError, ContextError, TransactionContext},
};
use tracing::instrument;
// Sawtooth's &mut dyn TransactionContext is highly inconvenient to work with in
// an async environment, so we will use an effects model instead,
// TP inputs can be determined synchronously, so we split processing into a sync
// and async part which returns effects
#[async_trait::async_trait]
pub trait TP {
    fn tp_parse(request: &TpProcessRequest) -> Result<Submission, ApplyError>;
    fn tp_state(
        context: &mut dyn TransactionContext,
        operations: &ChronicleTransaction,
    ) -> Result<OperationState<SawtoothAddress>, ApplyError>;
    async fn tp_operations(request: Submission) -> Result<ChronicleTransaction, ApplyError>;
    async fn tp(
        opa_executor: &ExecutorContext,
        request: &TpProcessRequest,
        submission: Submission,
        operations: ChronicleTransaction,
        state: OperationState<SawtoothAddress>,
    ) -> Result<TPSideEffects, ApplyError>;
    async fn enforce_opa(
        opa_executor: &ExecutorContext,
        identity: &SignedIdentity,
        operation: &ChronicleOperation,
        state: &OperationState<SawtoothAddress>,
    ) -> Result<(), ApplyError>;
}

#[derive(Debug)]
pub enum TPSideEffect {
    SetState {
        address: String,
        value: Vec<u8>,
    },
    AddEvent {
        event_type: String,
        attributes: Vec<(String, String)>,
        data: Vec<u8>,
    },
}

pub struct TPSideEffects {
    effects: Vec<TPSideEffect>,
}

impl TPSideEffects {
    pub fn new() -> Self {
        Self { effects: vec![] }
    }

    #[instrument(name = "set_state_entry", level = "trace", skip(self, data))]
    pub fn set_state_entry(&mut self, address: String, data: Vec<u8>) {
        self.effects.push(TPSideEffect::SetState {
            address,
            value: data,
        });
    }

    #[instrument(name = "add_event", level = "trace", skip(self, data))]
    pub fn add_event(
        &mut self,
        event_type: String,
        attributes: Vec<(String, String)>,
        data: Vec<u8>,
    ) {
        self.effects.push(TPSideEffect::AddEvent {
            event_type,
            attributes,
            data,
        });
    }

    #[instrument(name = "apply_effects", level = "debug", skip(self, ctx), fields (effects = ?self.effects))]
    pub fn apply(self, ctx: &mut dyn TransactionContext) -> Result<(), ContextError> {
        for effect in self.effects.into_iter() {
            match effect {
                TPSideEffect::SetState { address, value } => {
                    ctx.set_state_entry(address.clone(), value.clone())?;
                }
                TPSideEffect::AddEvent {
                    event_type,
                    attributes,
                    data,
                } => {
                    ctx.add_event(event_type, attributes, data.as_slice())?;
                }
            }
        }

        Ok(())
    }
}

impl Default for TPSideEffects {
    fn default() -> Self {
        Self::new()
    }
}

impl IntoIterator for TPSideEffects {
    type IntoIter = std::vec::IntoIter<Self::Item>;
    type Item = TPSideEffect;

    fn into_iter(self) -> Self::IntoIter {
        self.effects.into_iter()
    }
}
