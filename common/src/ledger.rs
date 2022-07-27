use futures::{stream, SinkExt, Stream, StreamExt};

use json::JsonValue;
use serde::ser::SerializeSeq;
use tracing::{debug, instrument};
use uuid::Uuid;

use crate::{
    context::PROV,
    prov::{
        operations::{
            ActivityExists, ActivityUses, ActsOnBehalfOf, AgentExists, ChronicleOperation,
            CreateNamespace, EndActivity, EntityDerive, EntityExists, EntityHasEvidence,
            RegisterKey, SetAttributes, StartActivity, WasAssociatedWith, WasGeneratedBy,
        },
        to_json_ld::ToJson,
        ActivityId, AgentId, ChronicleIri, ChronicleTransactionId, EntityId, IdentityId, NamePart,
        NamespaceId, ParseIriError, ProcessorError, ProvModel,
    },
};

use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender};
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    fmt::Display,
    pin::Pin,
    str::{from_utf8, FromStr},
    sync::{Arc, Mutex},
};

#[derive(Debug)]
pub enum SubmissionError {
    Implementation {
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },
    Processor {
        source: ProcessorError,
    },
}

#[derive(Debug)]
pub enum SubscriptionError {
    Implementation {
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },
}

impl Display for SubscriptionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Implementation { .. } => write!(f, "Subscription error"),
        }
    }
}

impl std::error::Error for SubscriptionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Implementation { source } => Some(source.as_ref()),
        }
    }
}

impl From<ProcessorError> for SubmissionError {
    fn from(source: ProcessorError) -> Self {
        SubmissionError::Processor { source }
    }
}

impl Display for SubmissionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Implementation { .. } => write!(f, "Ledger error"),
            Self::Processor { source: _ } => write!(f, "Processor error"),
        }
    }
}

impl std::error::Error for SubmissionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Implementation { source } => Some(source.as_ref()),
            Self::Processor { source } => Some(source),
        }
    }
}

#[async_trait::async_trait(?Send)]
pub trait LedgerWriter {
    async fn submit(
        &mut self,
        tx: &[ChronicleOperation],
    ) -> Result<ChronicleTransactionId, SubmissionError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Offset {
    Genesis,
    Identity(String),
}

impl Offset {
    pub fn map<T, F>(&self, f: F) -> Option<T>
    where
        F: FnOnce(&str) -> T,
    {
        if let Offset::Identity(x) = self {
            Some(f(x))
        } else {
            None
        }
    }
}

impl From<&str> for Offset {
    fn from(offset: &str) -> Self {
        let x = offset;
        Offset::Identity(x.to_owned())
    }
}

#[async_trait::async_trait]
pub trait LedgerReader {
    /// Subscribe to state updates from this ledger, starting at [offset]
    async fn state_updates(
        self,
        offset: Offset,
    ) -> Result<
        Pin<Box<dyn Stream<Item = (Offset, Box<ProvModel>, ChronicleTransactionId)> + Send>>,
        SubscriptionError,
    >;
}

/// An in memory ledger implementation for development and testing purposes
#[derive(Debug, Clone)]
pub struct InMemLedger {
    kv: RefCell<HashMap<LedgerAddress, JsonValue>>,
    chan: UnboundedSender<(Offset, ProvModel, ChronicleTransactionId)>,
    reader: Option<InMemLedgerReader>,
    head: u64,
}

impl InMemLedger {
    pub fn new() -> InMemLedger {
        let (tx, rx) = futures::channel::mpsc::unbounded();

        InMemLedger {
            kv: HashMap::new().into(),
            chan: tx,
            reader: Some(InMemLedgerReader {
                chan: Arc::new(Mutex::new(Some(rx).into())),
            }),
            head: 0u64,
        }
    }

    pub fn reader(&mut self) -> InMemLedgerReader {
        self.reader.take().unwrap()
    }
}

impl Default for InMemLedger {
    fn default() -> Self {
        Self::new()
    }
}

type SharedLedger = Option<UnboundedReceiver<(Offset, ProvModel, ChronicleTransactionId)>>;

#[derive(Debug, Clone)]
pub struct InMemLedgerReader {
    chan: Arc<Mutex<RefCell<SharedLedger>>>,
}

#[async_trait::async_trait]
impl LedgerReader for InMemLedgerReader {
    async fn state_updates(
        self,
        _offset: Offset,
    ) -> Result<
        Pin<Box<dyn Stream<Item = (Offset, Box<ProvModel>, ChronicleTransactionId)> + Send>>,
        SubscriptionError,
    > {
        let chan = self.chan.lock().unwrap().take().unwrap();
        let stream = stream::unfold(chan, |mut chan| async move {
            chan.next()
                .await
                .map(|(offset, prov, uuid)| ((offset, prov.into(), uuid), chan))
        });

        Ok(stream.boxed())
    }
}

/// An inefficient serialiser implementation for an in memory ledger, used for snapshot assertions of ledger state,
/// <v4 of json-ld doesn't use serde_json for whatever reason, so we reconstruct the ledger as a serde json map
impl serde::Serialize for InMemLedger {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut array = serializer
            .serialize_seq(Some(self.kv.borrow().len()))
            .unwrap();
        let mut keys = self.kv.borrow().keys().cloned().collect::<Vec<_>>();

        keys.sort();
        for k in keys {
            array.serialize_element(&k).ok();
            let v =
                serde_json::value::to_value(self.kv.borrow().get(&k).unwrap().to_string()).unwrap();
            array.serialize_element(&v).ok();
        }
        array.end()
    }
}

#[async_trait::async_trait(?Send)]
impl LedgerWriter for InMemLedger {
    async fn submit(
        &mut self,
        tx: &[ChronicleOperation],
    ) -> Result<ChronicleTransactionId, SubmissionError> {
        let id = ChronicleTransactionId::from(Uuid::new_v4());

        let mut model = ProvModel::default();
        let mut state = OperationState::new();
        let mut state_deps: Vec<LedgerAddress> = vec![];

        for tx in tx {
            let deps = tx.dependencies();

            state.append_input(
                self.kv
                    .borrow()
                    .iter()
                    .map(|(k, v)| (k.to_owned(), v.to_string().as_bytes().to_vec())),
            );

            debug!(
                input_chronicle_addresses=?deps,
            );

            let (tx_output, updated_model) = tx.process(model, state.input()).await.unwrap();

            state.append_output(
                tx_output
                    .into_iter()
                    .map(|output| output.into())
                    .collect::<BTreeMap<_, _>>()
                    .into_iter(),
            );
            model = updated_model;
            state_deps.append(
                &mut deps
                    .iter()
                    .filter(|d| !state_deps.contains(d))
                    .cloned()
                    .collect::<Vec<_>>(),
            )
        }

        state
            .dirty()
            .map(|output: StateOutput| {
                if state_deps.contains(&output.address) {
                    Ok(output)
                } else {
                    Err(SubmissionError::Processor {
                        source: ProcessorError::Address {},
                    })
                }
            })
            .collect::<Result<Vec<_>, SubmissionError>>()
            .into_iter()
            .flat_map(|v: Vec<StateOutput>| v.into_iter())
            .for_each(|output| {
                let state = json::parse(from_utf8(&output.data).unwrap()).unwrap();
                debug!(output_address=?output.address);
                self.kv.borrow_mut().insert(output.address, state);
            });

        self.chan
            .send((Offset::from(&*self.head.to_string()), model, id.clone()))
            .await
            .ok();

        self.head += 1;
        Ok(id)
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, PartialOrd, Ord)]
pub struct LedgerAddress {
    // Namespaces do not have a namespace
    namespace: Option<NamespaceId>,
    resource: ChronicleIri,
}

pub trait NameSpacePart {
    fn namespace_part(&self) -> Option<NamespaceId>;
}

impl NameSpacePart for LedgerAddress {
    fn namespace_part(&self) -> Option<NamespaceId> {
        self.namespace.clone()
    }
}

pub trait ResourcePart {
    fn resource_part(&self) -> ChronicleIri;
}

impl ResourcePart for LedgerAddress {
    fn resource_part(&self) -> ChronicleIri {
        self.resource.clone()
    }
}

impl LedgerAddress {
    fn from_ld(ns: Option<&str>, resource: &str) -> Result<Self, ParseIriError> {
        Ok(Self {
            namespace: if let Some(ns) = ns {
                Some(ChronicleIri::from_str(ns)?.namespace()?)
            } else {
                None
            },
            resource: ChronicleIri::from_str(resource)?,
        })
    }

    fn namespace(ns: &NamespaceId) -> Self {
        Self {
            namespace: None,
            resource: ns.clone().into(),
        }
    }

    fn in_namespace(ns: &NamespaceId, resource: impl Into<ChronicleIri>) -> Self {
        Self {
            namespace: Some(ns.clone()),
            resource: resource.into(),
        }
    }
}

#[derive(Debug)]
pub struct StateInput {
    data: Vec<u8>,
}

impl StateInput {
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }
}

#[derive(Debug)]
pub struct StateOutput {
    pub address: LedgerAddress,
    pub data: Vec<u8>,
}

impl StateOutput {
    pub fn new(address: LedgerAddress, data: Vec<u8>) -> Self {
        Self { address, data }
    }
}

impl From<StateOutput> for (LedgerAddress, Vec<u8>) {
    fn from(output: StateOutput) -> (LedgerAddress, Vec<u8>) {
        (output.address, output.data)
    }
}

/// Hold a cache of `LedgerWriter::submit` input and output address data
pub struct OperationState {
    input: BTreeMap<LedgerAddress, Vec<u8>>,
    output: BTreeMap<LedgerAddress, Vec<u8>>,
}

impl Default for OperationState {
    fn default() -> Self {
        Self::new()
    }
}

impl OperationState {
    pub fn new() -> Self {
        Self {
            input: BTreeMap::new(),
            output: BTreeMap::new(),
        }
    }

    /// Load input values into `OperationState`
    pub fn append_input(&mut self, input: impl Iterator<Item = (LedgerAddress, Vec<u8>)>) {
        let input_values: Vec<(LedgerAddress, Vec<u8>)> = input.collect();
        self.input.extend(input_values);
    }

    /// Return the byte vectors of input data held in `OperationState`
    /// as a vector of `StateInput`s
    pub fn input(&self) -> Vec<StateInput> {
        self.input.values().cloned().map(StateInput::new).collect()
    }

    /// Load processed output address and data values into maps in `OperationState`
    pub fn append_output(&mut self, outputs: impl Iterator<Item = (LedgerAddress, Vec<u8>)>) {
        let mut output = outputs.collect::<BTreeMap<_, _>>();
        self.output.append(&mut output);
    }

    /// Check if the data associated with an address has changed in processing
    /// while outputting a stream of dirty `StateOutput`s
    pub fn dirty(self) -> impl Iterator<Item = StateOutput> {
        self.output
            .into_iter()
            .filter(|(addr, data)| match (self.input.get(addr), &data) {
                (Some(input), output) if input != *output => true,
                (None, _) => true,
                _ => false,
            })
            .map(|(k, v)| StateOutput::new(k, v))
            .collect::<Vec<_>>()
            .into_iter()
    }
}

impl ChronicleOperation {
    /// Compute dependencies for a chronicle operation, input and output addresses are always symmetric
    pub fn dependencies(&self) -> Vec<LedgerAddress> {
        match self {
            ChronicleOperation::CreateNamespace(CreateNamespace { id, .. }) => {
                vec![LedgerAddress::namespace(id)]
            }
            ChronicleOperation::AgentExists(AgentExists {
                namespace, name, ..
            }) => {
                vec![
                    LedgerAddress::namespace(namespace),
                    LedgerAddress::in_namespace(namespace, AgentId::from_name(name)),
                ]
            }
            // Key registration requires identity + agent
            ChronicleOperation::RegisterKey(RegisterKey {
                namespace,
                id,
                publickey,
                ..
            }) => vec![
                LedgerAddress::namespace(namespace),
                LedgerAddress::in_namespace(namespace, id.clone()),
                LedgerAddress::in_namespace(
                    namespace,
                    IdentityId::from_name(id.name_part(), publickey),
                ),
            ],
            ChronicleOperation::ActivityExists(ActivityExists {
                namespace, name, ..
            }) => {
                vec![
                    LedgerAddress::namespace(namespace),
                    LedgerAddress::in_namespace(namespace, ActivityId::from_name(name)),
                ]
            }
            ChronicleOperation::StartActivity(StartActivity { namespace, id, .. }) => {
                vec![
                    LedgerAddress::namespace(namespace),
                    LedgerAddress::in_namespace(namespace, id.clone()),
                ]
            }
            ChronicleOperation::WasAssociatedWith(WasAssociatedWith {
                id,
                namespace,
                activity_id,
                agent_id,
                ..
            }) => vec![
                LedgerAddress::namespace(namespace),
                LedgerAddress::in_namespace(namespace, id.clone()),
                LedgerAddress::in_namespace(namespace, activity_id.clone()),
                LedgerAddress::in_namespace(namespace, agent_id.clone()),
            ],
            ChronicleOperation::EndActivity(EndActivity { namespace, id, .. }) => {
                vec![
                    LedgerAddress::namespace(namespace),
                    LedgerAddress::in_namespace(namespace, id.clone()),
                ]
            }
            ChronicleOperation::ActivityUses(ActivityUses {
                namespace,
                id,
                activity,
            }) => {
                vec![
                    LedgerAddress::namespace(namespace),
                    LedgerAddress::in_namespace(namespace, activity.clone()),
                    LedgerAddress::in_namespace(namespace, id.clone()),
                ]
            }
            ChronicleOperation::EntityExists(EntityExists { namespace, name }) => {
                vec![
                    LedgerAddress::namespace(namespace),
                    LedgerAddress::in_namespace(namespace, EntityId::from_name(name)),
                ]
            }
            ChronicleOperation::WasGeneratedBy(WasGeneratedBy {
                namespace,
                id,
                activity,
            }) => vec![
                LedgerAddress::namespace(namespace),
                LedgerAddress::in_namespace(namespace, activity.clone()),
                LedgerAddress::in_namespace(namespace, id.clone()),
            ],
            ChronicleOperation::EntityHasEvidence(EntityHasEvidence {
                namespace,
                id,
                agent,
                ..
            }) => {
                vec![
                    LedgerAddress::namespace(namespace),
                    LedgerAddress::in_namespace(namespace, agent.clone()),
                    LedgerAddress::in_namespace(namespace, id.clone()),
                ]
            }
            ChronicleOperation::AgentActsOnBehalfOf(ActsOnBehalfOf {
                namespace,
                id,
                delegate_id,
                activity_id,
                ..
            }) => vec![
                Some(LedgerAddress::namespace(namespace)),
                activity_id
                    .as_ref()
                    .map(|activity_id| LedgerAddress::in_namespace(namespace, activity_id.clone())),
                Some(LedgerAddress::in_namespace(namespace, delegate_id.clone())),
                Some(LedgerAddress::in_namespace(namespace, id.clone())),
            ]
            .into_iter()
            .flatten()
            .collect(),
            ChronicleOperation::EntityDerive(EntityDerive {
                namespace,
                id,
                used_id,
                activity_id,
                ..
            }) => vec![
                Some(LedgerAddress::namespace(namespace)),
                activity_id
                    .as_ref()
                    .map(|activity_id| LedgerAddress::in_namespace(namespace, activity_id.clone())),
                Some(LedgerAddress::in_namespace(namespace, used_id.clone())),
                Some(LedgerAddress::in_namespace(namespace, id.clone())),
            ]
            .into_iter()
            .flatten()
            .collect(),
            ChronicleOperation::SetAttributes(SetAttributes::Agent { id, namespace, .. }) => {
                vec![
                    LedgerAddress::namespace(namespace),
                    LedgerAddress::in_namespace(namespace, id.clone()),
                ]
            }
            ChronicleOperation::SetAttributes(SetAttributes::Entity { id, namespace, .. }) => {
                vec![
                    LedgerAddress::namespace(namespace),
                    LedgerAddress::in_namespace(namespace, id.clone()),
                ]
            }
            ChronicleOperation::SetAttributes(SetAttributes::Activity {
                id, namespace, ..
            }) => {
                vec![
                    LedgerAddress::namespace(namespace),
                    LedgerAddress::in_namespace(namespace, id.clone()),
                ]
            }
        }
    }

    /// Take input states and apply them to the prov model, then apply transaction,
    /// then transform to the compact representation and write each resource to the output state,
    /// also return the aggregate model so we can emit it as an event
    #[instrument(skip(self, model, input))]
    pub async fn process(
        &self,
        mut model: ProvModel,
        input: Vec<StateInput>,
    ) -> Result<(Vec<StateOutput>, ProvModel), ProcessorError> {
        for input in input {
            let graph = json::parse(std::str::from_utf8(&input.data)?)?;
            debug!(input_model=%graph);
            let resource = json::object! {
                "@context":  PROV.clone(),
                "@graph": [graph],
            };
            model = model.apply_json_ld(resource).await?;
        }

        model.apply(self);
        let mut json_ld = model.to_json().compact_stable_order().await?;
        debug!(result_model=%json_ld);

        Ok((
            if let Some(graph) = json_ld.get("@graph").and_then(|g| g.as_array()) {
                // Separate graph into discrete outputs
                graph
                    .iter()
                    .map(|resource| {
                        Ok(StateOutput {
                            address: LedgerAddress::from_ld(
                                resource
                                    .get("namespace")
                                    .and_then(|resource| resource.as_str()),
                                resource
                                    .get("@id")
                                    .and_then(|id| id.as_str())
                                    .ok_or(ProcessorError::NotANode {})?,
                            )?,
                            data: serde_json::to_string(resource).unwrap().into_bytes(),
                        })
                    })
                    .collect::<Result<Vec<_>, ProcessorError>>()?
            } else {
                // Remove context and return resource
                json_ld
                    .as_object_mut()
                    .map(|graph| graph.remove("@context"));

                vec![StateOutput {
                    address: LedgerAddress::from_ld(
                        json_ld
                            .get("namespace")
                            .and_then(|resource| resource.as_str()),
                        json_ld
                            .get("@id")
                            .and_then(|id| id.as_str())
                            .ok_or(ProcessorError::NotANode {})?,
                    )?,
                    data: serde_json::to_string(&json_ld).unwrap().into_bytes(),
                }]
            },
            model,
        ))
    }
}

/// Ensure ledgerwriter only writes dirty values back
#[cfg(test)]
pub mod test {
    use std::{collections::BTreeMap, str::from_utf8};

    use crate::{
        ledger::{InMemLedger, StateInput},
        prov::{
            operations::{ActsOnBehalfOf, AgentExists, ChronicleOperation, CreateNamespace},
            ActivityId, AgentId, DelegationId, Name, NamePart, NamespaceId, ProvModel, Role,
        },
    };
    use uuid::Uuid;

    use super::{LedgerAddress, OperationState, StateOutput};
    fn uuid() -> Uuid {
        let bytes = [
            0xa1, 0xa2, 0xa3, 0xa4, 0xb1, 0xb2, 0xc1, 0xc2, 0xd1, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6,
            0xd7, 0xd8,
        ];
        Uuid::from_slice(&bytes).unwrap()
    }

    fn create_namespace_id_helper(tag: Option<i32>) -> NamespaceId {
        let name = if tag.is_none() || tag == Some(0) {
            "testns".to_string()
        } else {
            format!("testns{}", tag.unwrap())
        };
        NamespaceId::from_name(&name, uuid())
    }

    fn create_namespace_helper(tag: Option<i32>) -> ChronicleOperation {
        let id = create_namespace_id_helper(tag);
        let name = &id.name_part().to_string();
        ChronicleOperation::CreateNamespace(CreateNamespace::new(id, name, uuid()))
    }

    fn agent_exists_helper() -> ChronicleOperation {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid());
        let name: Name = NamePart::name_part(&AgentId::from_name("test_agent")).clone();
        ChronicleOperation::AgentExists(AgentExists { namespace, name })
    }

    fn create_agent_acts_on_behalf_of() -> ChronicleOperation {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid());
        let responsible_id = AgentId::from_name("test_agent");
        let delegate_id = AgentId::from_name("test_delegate");
        let activity_id = ActivityId::from_name("test_activity");
        let role = "test_role";
        let id = DelegationId::from_component_ids(
            &delegate_id,
            &responsible_id,
            Some(&activity_id),
            Some(role),
        );
        let role = Role::from(role.to_string());
        ChronicleOperation::AgentActsOnBehalfOf(ActsOnBehalfOf {
            namespace,
            id,
            responsible_id,
            delegate_id,
            activity_id: Some(activity_id),
            role: Some(role),
        })
    }

    async fn transact_helper(
        tx: &ChronicleOperation,
        state: &mut OperationState,
        l: &InMemLedger,
        model: &mut ProvModel,
    ) {
        state.append_input(
            l.kv.borrow()
                .iter()
                .map(|(k, v)| (k.to_owned(), v.to_string().as_bytes().to_vec())),
        );
        let (tx_output, updated_model) = tx.process(model.clone(), state.input()).await.unwrap();
        state.append_output(
            tx_output
                .into_iter()
                .map(|o| o.into())
                .collect::<BTreeMap<_, _>>()
                .into_iter(),
        );
        *model = updated_model;
    }

    fn amend_ledger_helper(output: StateOutput, l: &mut InMemLedger) {
        let state = json::parse(from_utf8(&output.data).unwrap()).unwrap();

        l.kv.borrow_mut().insert(output.address, state);
    }

    /// Create namespace should create one novel output
    #[tokio::test]
    async fn test_dirty_passes_novel_output() -> Result<(), String> {
        let mut state = OperationState::new();
        let mut model = ProvModel::default();
        let mut tx: Vec<ChronicleOperation> = vec![];
        let mut l = InMemLedger::new();

        let op = create_namespace_helper(None);
        tx.push(op);

        let dirty_values = 1;

        for tx in tx {
            transact_helper(&tx, &mut state, &l, &mut model).await;
        }
        let mut dirty_matches = 0;
        for output in state.dirty() {
            dirty_matches += 1;
            amend_ledger_helper(output, &mut l);
        }
        l.head += 1;
        assert!(dirty_matches == dirty_values);
        Ok(())
    }

    /// Repeating an operation should create no novel output
    #[tokio::test]
    async fn test_dirty_matches_non_novel_output() -> Result<(), String> {
        let mut state = OperationState::new();
        let mut model = ProvModel::default();
        let mut tx: Vec<ChronicleOperation> = vec![];
        let mut l = InMemLedger::new();

        // operation - create namespace
        let op = create_namespace_helper(None);
        tx.push(op);

        for tx in tx {
            transact_helper(&tx, &mut state, &l, &mut model).await;
        }

        for output in state.dirty() {
            amend_ledger_helper(output, &mut l);
        }
        l.head += 1;

        // reinitialize state
        let mut state = OperationState::new();
        let mut tx: Vec<ChronicleOperation> = vec![];

        // operation - re-create the same namespace
        let dirty_values = 0;
        let op = create_namespace_helper(None);
        tx.push(op);

        for tx in tx {
            transact_helper(&tx, &mut state, &l, &mut model).await;
        }
        let mut dirty_matches = 0;
        for output in state.dirty() {
            dirty_matches += 1;
            amend_ledger_helper(output, &mut l);
        }

        l.head += 1;
        assert!(dirty_matches == dirty_values);

        // reinitialize state
        let mut state = OperationState::new();
        let mut tx: Vec<ChronicleOperation> = vec![];

        // operation - agent acts on behalf of
        let op = create_agent_acts_on_behalf_of();
        tx.push(op);
        let dirty_values = 4;

        for tx in tx {
            transact_helper(&tx, &mut state, &l, &mut model).await;
        }
        let mut dirty_matches = 0;
        for output in state.dirty() {
            dirty_matches += 1;
            amend_ledger_helper(output, &mut l);
        }
        assert!(dirty_matches == dirty_values);
        l.head += 1;

        // reinitialize state
        let mut state = OperationState::new();
        let mut tx: Vec<ChronicleOperation> = vec![];

        // repeat operation - agent acts on behalf of
        let dirty_values = 0;
        let op = create_agent_acts_on_behalf_of();
        tx.push(op);

        for tx in tx {
            transact_helper(&tx, &mut state, &l, &mut model).await;
        }
        let mut dirty_matches = 0;
        for output in state.dirty() {
            dirty_matches += 1;
            amend_ledger_helper(output, &mut l);
        }
        l.head += 1;
        assert!(dirty_matches == dirty_values);
        Ok(())
    }

    /// Already existing 'dirty' values should not match
    #[tokio::test]
    async fn test_dirty_passes_dirty_output() -> Result<(), String> {
        let mut state = OperationState::new();
        let mut model = ProvModel::default();
        let mut tx: Vec<ChronicleOperation> = vec![];
        let mut l = InMemLedger::new();

        // operation - create namespace
        let op = create_namespace_helper(None);
        tx.push(op);

        for tx in tx {
            transact_helper(&tx, &mut state, &l, &mut model).await;
        }

        for output in state.dirty() {
            amend_ledger_helper(output, &mut l);
        }
        l.head += 1;

        // reinitialize state
        let mut state = OperationState::new();
        let mut tx: Vec<ChronicleOperation> = vec![];

        // operation - create agent
        let op = agent_exists_helper();
        tx.push(op);

        for tx in tx {
            transact_helper(&tx, &mut state, &l, &mut model).await;
        }

        for output in state.dirty() {
            amend_ledger_helper(output, &mut l);
        }
        l.head += 1;

        // reinitialize state
        let mut state = OperationState::new();
        let mut tx: Vec<ChronicleOperation> = vec![];

        // operation - agent acts on behalf of
        // involves a delegation and an activity, writing to
        // a namespace and agent that already exist as inputs,
        // which are amended by the transaction
        let op = create_agent_acts_on_behalf_of();
        let dirty_values = 4;

        tx.push(op);

        for tx in tx {
            transact_helper(&tx, &mut state, &l, &mut model).await;
        }

        let mut dirty_matches = 0;

        for output in state.dirty() {
            dirty_matches += 1;
            amend_ledger_helper(output, &mut l);
        }
        l.head += 1;
        assert!(dirty_matches == dirty_values);
        Ok(())
    }

    fn init_n_unique_ledgeraddresses(n: i32) -> BTreeMap<LedgerAddress, Vec<u8>> {
        let addresses = {
            (0..n)
                .into_iter()
                .map(|i| LedgerAddress::namespace(&create_namespace_id_helper(Some(i))))
                .collect::<Vec<_>>()
        };
        let data: Vec<Vec<u8>> = vec![vec![1], vec![2], vec![3], vec![4], vec![5]];

        addresses
            .into_iter()
            .zip(data.into_iter())
            .collect::<BTreeMap<_, _>>()
    }

    #[test]
    fn test_operationstate_append_output_passes_dirty_values_and_dirty_only_outputs_dirty_values() {
        let mut state = OperationState::new();

        assert!(state.input.is_empty());
        assert!(state.output.is_empty());

        let input = init_n_unique_ledgeraddresses(5);

        state.append_input(input.clone().into_iter().take(3));

        let state_data = state.input.clone();

        assert!(state_data.len() == 3);

        state.append_output(input.clone().into_iter());

        assert!(state.output.len() == 5);

        let dirty_output = state.dirty().collect::<Vec<_>>();
        assert!(dirty_output.len() == 2);

        let dirty_output = dirty_output
            .into_iter()
            .map(|output| output.into())
            .collect::<BTreeMap<LedgerAddress, Vec<u8>>>();
        let dirty_input = input
            .into_iter()
            .skip(3)
            .collect::<BTreeMap<LedgerAddress, Vec<u8>>>();

        dirty_output
            .into_iter()
            .zip(dirty_input.into_iter())
            .for_each(|(left, right)| {
                assert_eq!(left.0, right.0);
                assert_eq!(left.1, right.1);
            });
    }

    #[test]
    fn test_operationstate_input() {
        let mut state = OperationState::new();

        let input = init_n_unique_ledgeraddresses(5);

        state.append_input(input.clone().into_iter());

        let state_data = state.input();
        let input_as_stateinput = input.values().cloned().map(StateInput::new);

        state_data
            .into_iter()
            .zip(input_as_stateinput.into_iter())
            .for_each(|(left, right)| {
                assert_eq!(left.data, right.data);
            });
    }

    #[test]
    fn test_append_output() {
        let mut state = OperationState::new();

        let input = init_n_unique_ledgeraddresses(5);

        state.append_input(input.clone().into_iter());

        assert!(state.input == input);
    }

    #[test]
    fn test_operationstate_append_input_and_input() {
        let mut state = OperationState::new();

        let addresses = {
            (0..5)
                .into_iter()
                .map(|i| LedgerAddress::namespace(&create_namespace_id_helper(Some(i))))
                .collect::<Vec<_>>()
        };
        let data: Vec<Vec<u8>> = vec![vec![1], vec![2], vec![3], vec![4], vec![5]];

        let input = addresses
            .into_iter()
            .zip(data.clone().into_iter())
            .collect::<BTreeMap<_, _>>();

        state.append_input(input.clone().into_iter());

        // check if the input is as expected by comparing with cloned values
        assert_eq!(state.input, input);
        // check `OperationState::input` outputs data consistent with original input data
        assert_eq!(
            state
                .input()
                .into_iter()
                .map(|input| input.data)
                .collect::<Vec<Vec<u8>>>(),
            data
        );
    }

    #[test]
    fn test_from_stateoutput_for_ledger_data_tuple() {
        let id = create_namespace_id_helper(None);
        let addr = LedgerAddress::namespace(&id);
        let data: Vec<u8> = vec![1];
        let output = StateOutput::new(addr.clone(), data.clone());
        let (addr_from_output, data_from_output) = output.into();
        assert_eq!(addr_from_output, addr);
        assert_eq!(data_from_output, data);
    }
}
