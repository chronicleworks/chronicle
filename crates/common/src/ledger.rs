use derivative::Derivative;
use futures::{stream, SinkExt, Stream, StreamExt};

use json::JsonValue;
use serde::ser::SerializeSeq;
use tracing::{debug, instrument, trace};
use uuid::Uuid;

use crate::prov::{
    operations::{
        ActivityExists, ActivityUses, ActsOnBehalfOf, AgentExists, ChronicleOperation,
        CreateNamespace, EndActivity, EntityDerive, EntityExists, EntityHasEvidence, RegisterKey,
        SetAttributes, StartActivity, WasAssociatedWith, WasGeneratedBy, WasInformedBy,
    },
    to_json_ld::ToJson,
    ActivityId, AgentId, ChronicleIri, ChronicleTransaction, ChronicleTransactionId, Contradiction,
    EntityId, ExternalIdPart, IdentityId, NamespaceId, ParseIriError, ProcessorError, ProvModel,
};

use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender};
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap, HashSet},
    fmt::{Display, Formatter},
    pin::Pin,
    str::FromStr,
    sync::{Arc, Mutex},
};

#[derive(Derivative, Clone)]
#[derivative(Debug)]
pub enum SubmissionError {
    Implementation {
        #[derivative(Debug = "ignore")]
        source: Arc<Box<dyn std::error::Error + Send + Sync + 'static>>,
        tx_id: ChronicleTransactionId,
    },
    Processor {
        source: Arc<ProcessorError>,
        tx_id: ChronicleTransactionId,
    },
    Contradiction {
        source: Contradiction,
        tx_id: ChronicleTransactionId,
    },
}

impl SubmissionError {
    pub fn tx_id(&self) -> &ChronicleTransactionId {
        match self {
            SubmissionError::Implementation { tx_id, .. } => tx_id,
            SubmissionError::Processor { tx_id, .. } => tx_id,
            SubmissionError::Contradiction { tx_id, .. } => tx_id,
        }
    }

    pub fn processor(tx_id: &ChronicleTransactionId, source: ProcessorError) -> SubmissionError {
        SubmissionError::Processor {
            source: Arc::new(source),
            tx_id: tx_id.clone(),
        }
    }

    pub fn contradiction(tx_id: &ChronicleTransactionId, source: Contradiction) -> SubmissionError {
        SubmissionError::Contradiction {
            source,
            tx_id: tx_id.clone(),
        }
    }

    pub fn implementation(
        tx_id: &ChronicleTransactionId,
        source: Arc<Box<dyn std::error::Error + Send + Sync + 'static>>,
    ) -> SubmissionError {
        SubmissionError::Implementation {
            source,
            tx_id: tx_id.clone(),
        }
    }
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

impl Display for SubmissionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Implementation { source, .. } => write!(f, "Ledger error {source} "),
            Self::Processor { source, .. } => write!(f, "Processor error {source} "),
            Self::Contradiction { source, .. } => write!(f, "Contradiction: {source}"),
        }
    }
}

impl std::error::Error for SubmissionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Implementation { source, .. } => Some(&**source.as_ref()),
            Self::Processor { source, .. } => Some(source),
            Self::Contradiction { source, .. } => Some(source),
        }
    }
}

pub type SubmitResult = Result<ChronicleTransactionId, SubmissionError>;

#[derive(Debug, Clone)]
pub struct Commit {
    pub tx_id: ChronicleTransactionId,
    pub offset: Offset,
    pub delta: Box<ProvModel>,
}

impl Commit {
    pub fn new(tx_id: ChronicleTransactionId, offset: Offset, delta: Box<ProvModel>) -> Self {
        Commit {
            tx_id,
            offset,
            delta,
        }
    }
}

pub type CommitResult = Result<Commit, (ChronicleTransactionId, Contradiction)>;

#[async_trait::async_trait(?Send)]
pub trait LedgerWriter {
    async fn submit(
        &mut self,
        tx: &ChronicleTransaction,
    ) -> Result<ChronicleTransactionId, SubmissionError>;
}

#[derive(Debug, Clone)]
pub enum SubmissionStage {
    Submitted(SubmitResult),
    Committed(CommitResult),
}

impl SubmissionStage {
    pub fn submitted_error(r: &SubmissionError) -> Self {
        SubmissionStage::Submitted(Err(r.clone()))
    }
    pub fn submitted(r: &ChronicleTransactionId) -> Self {
        SubmissionStage::Submitted(Ok(r.clone()))
    }

    pub fn committed(commit: Result<Commit, (ChronicleTransactionId, Contradiction)>) -> Self {
        SubmissionStage::Committed(commit)
    }

    pub fn tx_id(&self) -> &ChronicleTransactionId {
        match self {
            Self::Submitted(tx_id) => match tx_id {
                Ok(tx_id) => tx_id,
                Err(e) => e.tx_id(),
            },
            Self::Committed(commit) => match commit {
                Ok(commit) => &commit.tx_id,
                Err((tx_id, _)) => tx_id,
            },
        }
    }
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
    /// Subscribe to state updates from this ledger, starting at `offset`
    async fn state_updates(
        self,
        offset: Offset,
    ) -> Result<Pin<Box<dyn Stream<Item = CommitResult> + Send>>, SubscriptionError>;
}

/// An in memory ledger implementation for development and testing purposes
#[derive(Debug, Clone)]
pub struct InMemLedger {
    kv: RefCell<HashMap<LedgerAddress, JsonValue>>,
    chan: UnboundedSender<CommitResult>,
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

impl std::fmt::Display for InMemLedger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (k, v) in self.kv.borrow().iter() {
            writeln!(f, "{}: {}", k, v.pretty(2))?;
        }
        Ok(())
    }
}

type SharedLedger = Option<UnboundedReceiver<CommitResult>>;

#[derive(Debug, Clone)]
pub struct InMemLedgerReader {
    chan: Arc<Mutex<RefCell<SharedLedger>>>,
}

#[async_trait::async_trait]
impl LedgerReader for InMemLedgerReader {
    async fn state_updates(
        self,
        _offset: Offset,
    ) -> Result<Pin<Box<dyn Stream<Item = CommitResult> + Send>>, SubscriptionError> {
        let chan = self.chan.lock().unwrap().take().unwrap();
        let stream = stream::unfold(chan, |mut chan| async move {
            chan.next().await.map(|stage| (stage, chan))
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
    #[instrument(skip(self), level="debug" ret(Debug))]
    async fn submit(
        &mut self,
        tx: &ChronicleTransaction,
    ) -> Result<ChronicleTransactionId, SubmissionError> {
        let id = ChronicleTransactionId::from(Uuid::new_v4());

        let mut model = ProvModel::default();
        let mut state = OperationState::new();

        //pre compute and pre-load dependencies
        let deps = &tx
            .tx
            .iter()
            .flat_map(|tx| tx.dependencies())
            .collect::<HashSet<_>>();

        state.update_state(deps.iter().map(|dep| {
            (
                dep.clone(),
                self.kv.borrow().get(dep).map(|dep| dep.to_string()),
            )
        }));

        debug!(
            input_chronicle_addresses=?deps,
        );

        trace!(ledger_state_before = %self);

        for tx in &tx.tx {
            let res = tx.process(model, state.input()).await;

            match res {
                Err(ProcessorError::Contradiction { source }) => {
                    return Err(SubmissionError::contradiction(&id, source))
                }
                //This fake ledger is used for development purposes and testing, so we panic on serious error
                Err(e) => panic!("{e:?}"),
                Ok((tx_output, updated_model)) => {
                    state.update_state(
                        tx_output
                            .into_iter()
                            .map(|output| {
                                debug!(output_state = %output.data);
                                (output.address, Some(output.data))
                            })
                            .collect::<BTreeMap<_, _>>()
                            .into_iter(),
                    );
                    model = updated_model;
                }
            }
        }

        let mut delta = ProvModel::default();
        for output in state
            .dirty()
            .map(|output: StateOutput<LedgerAddress>| {
                trace!(dirty = ?output);
                if deps.contains(&output.address) {
                    Ok(output)
                } else {
                    Err(SubmissionError::processor(&id, ProcessorError::Address {}))
                }
            })
            .collect::<Result<Vec<_>, SubmissionError>>()
            .into_iter()
            .flat_map(|v: Vec<StateOutput<LedgerAddress>>| v.into_iter())
        {
            let state = json::parse(&output.data).unwrap();
            delta
                .apply_json_ld_str(&output.data)
                .await
                .map_err(|e| SubmissionError::processor(&id, e))?;
            self.kv.borrow_mut().insert(output.address, state);
        }

        debug!(delta = %delta.to_json().compact().await.unwrap().pretty());

        trace!(ledger_state_after = %self);

        self.chan
            .send(Ok(Commit::new(
                id.clone(),
                Offset::from(&*self.head.to_string()),
                Box::new(delta),
            )))
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

impl Display for LedgerAddress {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(namespace) = &self.namespace {
            write!(f, "{}:{}", namespace, self.resource)
        } else {
            write!(f, "{}", self.resource)
        }
    }
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

#[derive(Debug, Clone)]
pub struct StateInput {
    data: String,
}

impl StateInput {
    pub fn new(data: String) -> Self {
        Self { data }
    }

    pub fn data(&self) -> &[u8] {
        self.data.as_bytes()
    }
}

#[derive(Debug)]
pub struct StateOutput<T> {
    pub address: T,
    pub data: String,
}

impl<T> StateOutput<T> {
    pub fn new(address: T, data: impl ToString) -> Self {
        Self {
            address,
            data: data.to_string(),
        }
    }

    pub fn address(&self) -> &T {
        &self.address
    }

    pub fn data(&self) -> &[u8] {
        self.data.as_bytes()
    }
}

impl<T> From<StateOutput<T>> for (T, String) {
    fn from(output: StateOutput<T>) -> (T, String) {
        (output.address, output.data)
    }
}

#[derive(Debug, Clone)]
pub struct Version {
    pub(crate) version: u32,
    pub(crate) value: Option<String>,
}

impl Version {
    pub fn write(&mut self, value: Option<String>) {
        if value != self.value {
            self.version += 1;
            self.value = value;
        }
    }
}

/// Hold a cache of `LedgerWriter::submit` input and output address data
pub struct OperationState<T>
where
    T: Ord,
{
    state: BTreeMap<T, Version>,
}

impl<T> Default for OperationState<T>
where
    T: Ord,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T> OperationState<T>
where
    T: Ord,
{
    pub fn new() -> Self {
        Self {
            state: BTreeMap::new(),
        }
    }

    /// Load input values into `OperationState`
    pub fn update_state(&mut self, input: impl Iterator<Item = (T, Option<String>)>) {
        for (address, value) in input {
            self.state
                .entry(address)
                .or_insert_with(|| Version {
                    version: 0,
                    value: value.clone(),
                })
                .write(value);
        }
    }

    /// Return the input data held in `OperationState`
    /// as a vector of `StateInput`s
    pub fn input(&self) -> Vec<StateInput> {
        self.state
            .values()
            .cloned()
            .filter_map(|v| v.value.map(StateInput::new))
            .collect()
    }

    /// Check if the data associated with an address has changed in processing
    /// while outputting a stream of dirty `StateOutput`s
    pub fn dirty(self) -> impl Iterator<Item = StateOutput<T>> {
        self.state
            .into_iter()
            .filter_map(|(addr, data)| {
                if data.version > 0 {
                    data.value.map(|value| (StateOutput::new(addr, value)))
                } else {
                    None
                }
            })
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
                namespace,
                external_id,
                ..
            }) => {
                vec![
                    LedgerAddress::namespace(namespace),
                    LedgerAddress::in_namespace(namespace, AgentId::from_external_id(external_id)),
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
                    IdentityId::from_external_id(id.external_id_part(), publickey),
                ),
            ],
            ChronicleOperation::ActivityExists(ActivityExists {
                namespace,
                external_id,
                ..
            }) => {
                vec![
                    LedgerAddress::namespace(namespace),
                    LedgerAddress::in_namespace(
                        namespace,
                        ActivityId::from_external_id(external_id),
                    ),
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
            ChronicleOperation::EntityExists(EntityExists {
                namespace,
                external_id,
            }) => {
                vec![
                    LedgerAddress::namespace(namespace),
                    LedgerAddress::in_namespace(namespace, EntityId::from_external_id(external_id)),
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
            ChronicleOperation::WasInformedBy(WasInformedBy {
                namespace,
                activity,
                informing_activity,
            }) => {
                vec![
                    LedgerAddress::namespace(namespace),
                    LedgerAddress::in_namespace(namespace, activity.clone()),
                    LedgerAddress::in_namespace(namespace, informing_activity.clone()),
                ]
            }
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
                responsible_id,
                ..
            }) => vec![
                Some(LedgerAddress::namespace(namespace)),
                activity_id
                    .as_ref()
                    .map(|activity_id| LedgerAddress::in_namespace(namespace, activity_id.clone())),
                Some(LedgerAddress::in_namespace(namespace, delegate_id.clone())),
                Some(LedgerAddress::in_namespace(
                    namespace,
                    responsible_id.clone(),
                )),
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
    #[instrument(level = "debug", skip(self, model, input))]
    pub async fn process(
        &self,
        mut model: ProvModel,
        input: Vec<StateInput>,
    ) -> Result<(Vec<StateOutput<LedgerAddress>>, ProvModel), ProcessorError> {
        for input in input {
            let graph = json::parse(&input.data)?;
            debug!(input_model=%graph.pretty(2));
            let resource = json::object! {
                "@graph": [graph],
            };
            model.apply_json_ld(resource).await?;
        }

        model.apply(self)?;
        let mut json_ld = model.to_json().compact_stable_order().await?;

        json_ld
            .as_object_mut()
            .map(|graph| graph.remove("@context"));

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
                            data: serde_json::to_string(resource).unwrap(),
                        })
                    })
                    .collect::<Result<Vec<_>, ProcessorError>>()?
            } else {
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
                    data: serde_json::to_string(&json_ld).unwrap(),
                }]
            },
            model,
        ))
    }
}

/// Ensure ledgerwriter only writes dirty values back
#[cfg(test)]
pub mod test {

    use crate::{
        identity::{AuthId, SignedIdentity},
        ledger::InMemLedger,
        prov::{
            operations::{ActsOnBehalfOf, AgentExists, ChronicleOperation, CreateNamespace},
            to_json_ld::ToJson,
            ActivityId, AgentId, ChronicleTransaction, DelegationId, ExternalId, ExternalIdPart,
            NamespaceId, Role,
        },
        signing::DirectoryStoredKeys,
    };
    use futures::StreamExt;
    use temp_dir::TempDir;
    use uuid::Uuid;

    use super::{LedgerReader, LedgerWriter, Offset};
    fn uuid() -> Uuid {
        let bytes = [
            0xa1, 0xa2, 0xa3, 0xa4, 0xb1, 0xb2, 0xc1, 0xc2, 0xd1, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6,
            0xd7, 0xd8,
        ];
        Uuid::from_slice(&bytes).unwrap()
    }

    fn create_namespace_id_helper(tag: Option<i32>) -> NamespaceId {
        let external_id = if tag.is_none() || tag == Some(0) {
            "testns".to_string()
        } else {
            format!("testns{}", tag.unwrap())
        };
        NamespaceId::from_external_id(external_id, uuid())
    }

    fn create_namespace_helper(tag: Option<i32>) -> ChronicleOperation {
        let id = create_namespace_id_helper(tag);
        let external_id = &id.external_id_part().to_string();
        ChronicleOperation::CreateNamespace(CreateNamespace::new(id, external_id, uuid()))
    }

    fn agent_exists_helper() -> ChronicleOperation {
        let namespace: NamespaceId = NamespaceId::from_external_id("testns", uuid());
        let external_id: ExternalId =
            ExternalIdPart::external_id_part(&AgentId::from_external_id("test_agent")).clone();
        ChronicleOperation::AgentExists(AgentExists {
            namespace,
            external_id,
        })
    }

    fn create_agent_acts_on_behalf_of() -> ChronicleOperation {
        let namespace: NamespaceId = NamespaceId::from_external_id("testns", uuid());
        let responsible_id = AgentId::from_external_id("test_agent");
        let delegate_id = AgentId::from_external_id("test_delegate");
        let activity_id = ActivityId::from_external_id("test_activity");
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

    fn signed_identity_helper() -> SignedIdentity {
        let keystore = DirectoryStoredKeys::new(TempDir::new().unwrap().path()).unwrap();
        keystore.generate_chronicle().unwrap();
        AuthId::chronicle().signed_identity(&keystore).unwrap()
    }

    #[tokio::test]
    async fn test_delta_incrementally() {
        let mut ledger = InMemLedger::new();
        let reader = ledger.reader();
        let mut reader = reader.state_updates(Offset::Genesis).await.unwrap();

        ledger
            .submit(&ChronicleTransaction::new(vec![], signed_identity_helper()))
            .await
            .ok();

        let res = reader.next().await.unwrap().unwrap();

        // No transaction, so no delta
        insta::assert_toml_snapshot!(res.delta.to_json().compact_stable_order().await.unwrap(), @r###""@context" = 'https://btp.works/chr/1.0/c.jsonld'"###);

        ledger
            .submit(&ChronicleTransaction::new(
                vec![create_namespace_helper(Some(1))],
                signed_identity_helper(),
            ))
            .await
            .ok();

        let res = reader.next().await.unwrap().unwrap();

        // Namespace delta
        insta::assert_toml_snapshot!(res.delta.to_json().compact_stable_order().await.unwrap(), @r###"
        "@context" = 'https://btp.works/chr/1.0/c.jsonld'
        "@id" = 'chronicle:ns:testns1:a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8'
        "@type" = 'chronicle:Namespace'
        externalId = 'testns1'
        "###);

        ledger
            .submit(&ChronicleTransaction::new(
                vec![create_namespace_helper(Some(1))],
                signed_identity_helper(),
            ))
            .await
            .ok();

        let res = reader.next().await.unwrap().unwrap();

        // No delta
        insta::assert_toml_snapshot!(res.delta.to_json().compact_stable_order().await.unwrap(), @r###""@context" = 'https://btp.works/chr/1.0/c.jsonld'"###);

        ledger
            .submit(&ChronicleTransaction::new(
                vec![agent_exists_helper()],
                signed_identity_helper(),
            ))
            .await
            .ok();

        let res = reader.next().await.unwrap().unwrap();

        // Agent delta
        insta::assert_toml_snapshot!(res.delta.to_json().compact_stable_order().await.unwrap(), @r###"
        "@context" = 'https://btp.works/chr/1.0/c.jsonld'

        [["@graph"]]
        "@id" = 'chronicle:agent:test%5Fagent'
        "@type" = 'prov:Agent'
        externalId = 'test_agent'
        namespace = 'chronicle:ns:testns:a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8'

        ["@graph".value]

        [["@graph"]]
        "@id" = 'chronicle:ns:testns:a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8'
        "@type" = 'chronicle:Namespace'
        externalId = 'testns'
        "###);

        ledger
            .submit(&ChronicleTransaction::new(
                vec![agent_exists_helper()],
                signed_identity_helper(),
            ))
            .await
            .ok();

        let res = reader.next().await.unwrap().unwrap();

        // No delta
        insta::assert_toml_snapshot!(res.delta.to_json().compact_stable_order()
          .await.unwrap(), @r###""@context" = 'https://btp.works/chr/1.0/c.jsonld'"###);

        ledger
            .submit(&ChronicleTransaction::new(
                vec![create_agent_acts_on_behalf_of()],
                signed_identity_helper(),
            ))
            .await
            .ok();

        let res = reader.next().await.unwrap().unwrap();

        // No delta
        insta::assert_snapshot!(&*serde_json::to_string_pretty(&res.delta.to_json()
            .compact_stable_order().await.unwrap()).unwrap(),
             @r###"
        {
          "@context": "https://btp.works/chr/1.0/c.jsonld",
          "@graph": [
            {
              "@id": "chronicle:activity:test%5Factivity",
              "@type": "prov:Activity",
              "externalId": "test_activity",
              "namespace": "chronicle:ns:testns:a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8",
              "value": {}
            },
            {
              "@id": "chronicle:agent:test%5Fagent",
              "@type": "prov:Agent",
              "actedOnBehalfOf": [
                "chronicle:agent:test%5Fdelegate"
              ],
              "externalId": "test_agent",
              "namespace": "chronicle:ns:testns:a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8",
              "prov:qualifiedDelegation": {
                "@id": "chronicle:delegation:test%5Fdelegate:test%5Fagent:role=test%5Frole:activity=test%5Factivity"
              },
              "value": {}
            },
            {
              "@id": "chronicle:agent:test%5Fdelegate",
              "@type": "prov:Agent",
              "externalId": "test_delegate",
              "namespace": "chronicle:ns:testns:a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8",
              "value": {}
            },
            {
              "@id": "chronicle:delegation:test%5Fdelegate:test%5Fagent:role=test%5Frole:activity=test%5Factivity",
              "@type": "prov:Delegation",
              "actedOnBehalfOf": [
                "chronicle:agent:test%5Fdelegate"
              ],
              "agent": "chronicle:agent:test%5Fagent",
              "namespace": "chronicle:ns:testns:a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8",
              "prov:hadActivity": {
                "@id": "chronicle:activity:test%5Factivity"
              },
              "prov:hadRole": "test_role"
            },
            {
              "@id": "chronicle:ns:testns:a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8",
              "@type": "chronicle:Namespace",
              "externalId": "testns"
            }
          ]
        }
        "###);
    }
}
