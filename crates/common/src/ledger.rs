use derivative::Derivative;

use opa_tp_protocol::async_sawtooth_sdk::{error::SawtoothCommunicationError, ledger::BlockId};
use tracing::{debug, instrument};

use crate::{
    identity::SignedIdentity,
    prov::{
        operations::{
            ActivityExists, ActivityUses, ActsOnBehalfOf, AgentExists, ChronicleOperation,
            CreateNamespace, EndActivity, EntityDerive, EntityExists, EntityHasEvidence,
            RegisterKey, SetAttributes, StartActivity, WasAssociatedWith, WasAttributedTo,
            WasGeneratedBy, WasInformedBy,
        },
        to_json_ld::ToJson,
        ActivityId, AgentId, ChronicleIri, ChronicleTransactionId, Contradiction, EntityId,
        ExternalIdPart, IdentityId, NamespaceId, ParseIriError, ProcessorError, ProvModel,
    },
};

use std::{
    collections::{BTreeMap, HashSet},
    fmt::{Display, Formatter},
    str::FromStr,
    sync::Arc,
};

#[derive(Derivative, Clone)]
#[derivative(Debug)]
pub enum SubmissionError {
    Communication {
        source: Arc<SawtoothCommunicationError>,
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
            SubmissionError::Communication { tx_id, .. } => tx_id,
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

    pub fn communication(
        tx_id: &ChronicleTransactionId,
        source: SawtoothCommunicationError,
    ) -> SubmissionError {
        SubmissionError::Communication {
            source: Arc::new(source),
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
            Self::Communication { source, .. } => write!(f, "Ledger error {source} "),
            Self::Processor { source, .. } => write!(f, "Processor error {source} "),
            Self::Contradiction { source, .. } => write!(f, "Contradiction: {source}"),
        }
    }
}

impl std::error::Error for SubmissionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Communication { source, .. } => Some(source),
            Self::Processor { source, .. } => Some(source),
            Self::Contradiction { source, .. } => Some(source),
        }
    }
}

pub type SubmitResult = Result<ChronicleTransactionId, SubmissionError>;

#[derive(Debug, Clone)]
pub struct Commit {
    pub tx_id: ChronicleTransactionId,
    pub block_id: BlockId,
    pub delta: Box<ProvModel>,
}

impl Commit {
    pub fn new(tx_id: ChronicleTransactionId, block_id: BlockId, delta: Box<ProvModel>) -> Self {
        Commit {
            tx_id,
            block_id,
            delta,
        }
    }
}

pub type CommitResult = Result<Commit, (ChronicleTransactionId, Contradiction)>;

#[derive(Debug, Clone)]
pub enum SubmissionStage {
    Submitted(SubmitResult),
    Committed(Commit, Box<SignedIdentity>),
    NotCommitted((ChronicleTransactionId, Contradiction, Box<SignedIdentity>)),
}

impl SubmissionStage {
    pub fn submitted_error(r: &SubmissionError) -> Self {
        SubmissionStage::Submitted(Err(r.clone()))
    }

    pub fn submitted(r: &ChronicleTransactionId) -> Self {
        SubmissionStage::Submitted(Ok(r.clone()))
    }

    pub fn committed(commit: Commit, identity: SignedIdentity) -> Self {
        SubmissionStage::Committed(commit, identity.into())
    }

    pub fn not_committed(
        tx: ChronicleTransactionId,
        contradiction: Contradiction,
        identity: SignedIdentity,
    ) -> Self {
        SubmissionStage::NotCommitted((tx, contradiction, identity.into()))
    }

    pub fn tx_id(&self) -> &ChronicleTransactionId {
        match self {
            Self::Submitted(tx_id) => match tx_id {
                Ok(tx_id) => tx_id,
                Err(e) => e.tx_id(),
            },
            Self::Committed(commit, _) => &commit.tx_id,
            Self::NotCommitted((tx_id, _, _)) => tx_id,
        }
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

    /// Return the input data held in `OperationState` for `addresses` as a vector of `StateInput`s
    pub fn opa_context(&self, addresses: HashSet<T>) -> Vec<StateInput> {
        self.state
            .iter()
            .filter(|(addr, _data)| addresses.iter().any(|a| &a == addr))
            .map(|(_, data)| data.clone())
            .filter_map(|v| v.value.map(StateInput::new))
            .collect()
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
            ChronicleOperation::WasAttributedTo(WasAttributedTo {
                id,
                namespace,
                entity_id,
                agent_id,
                ..
            }) => vec![
                LedgerAddress::namespace(namespace),
                LedgerAddress::in_namespace(namespace, id.clone()),
                LedgerAddress::in_namespace(namespace, entity_id.clone()),
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

    /// Apply an operation's input states to the prov model
    pub async fn dependencies_graph(
        &self,
        model: &mut ProvModel,
        input: Vec<StateInput>,
    ) -> Result<serde_json::Value, ProcessorError> {
        for input in input {
            let graph: serde_json::Value = serde_json::from_str(&input.data)?;
            debug!(input_model=%serde_json::to_string_pretty(&graph).map_or_else(|e| format!("error: {e}"), |x| x));
            let resource = serde_json::json!({
                "@graph": [graph],
            });
            model.apply_json_ld(resource).await?;
        }

        model.apply(self)?;
        let mut json_ld = model.to_json().compact_stable_order().await?;

        json_ld
            .as_object_mut()
            .map(|graph| graph.remove("@context"));
        Ok(json_ld)
    }

    /// Take input states and apply them to the prov model, then apply transaction,
    /// then transform to the compact representation and write each resource to the output state,
    #[instrument(level = "debug", skip(self, model, input))]
    pub async fn process(
        &self,
        mut model: ProvModel,
        input: Vec<StateInput>,
    ) -> Result<(Vec<StateOutput<LedgerAddress>>, ProvModel), ProcessorError> {
        let json_ld = self.dependencies_graph(&mut model, input).await?;
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

    /// Apply the list of operation dependencies to the prov model and transform to the compact representation,
    /// then transform the model's @graph to a list of dependency objects with the @id as the key.
    #[instrument(level = "debug", skip(self, model, deps))]
    pub async fn opa_context_state(
        &self,
        mut model: ProvModel,
        deps: Vec<StateInput>,
    ) -> Result<Vec<serde_json::Value>, ProcessorError> {
        if deps.is_empty() {
            // No @graph to process in this case
            Ok(vec![serde_json::Value::default()])
        } else {
            let mut json_ld = self.dependencies_graph(&mut model, deps).await?;
            Ok(
                if let Some(graph) = json_ld.get_mut("@graph").and_then(|g| g.as_array_mut()) {
                    graph
                        .iter_mut()
                        .map(transform_context_graph_object)
                        .collect::<Result<Vec<_>, ProcessorError>>()?
                } else {
                    vec![transform_context_graph_object(&mut json_ld)?]
                },
            )
        }
    }
}

fn transform_context_graph_object(
    value: &mut serde_json::Value,
) -> Result<serde_json::Value, ProcessorError> {
    Ok(serde_json::json!({
        value
            .as_object_mut()
            .and_then(|o| o.remove("@id"))
            .as_ref()
            .and_then(|v| v.as_str())
            .ok_or(ProcessorError::NotANode)?: value
    }))
}

/// Ensure ledgerwriter only writes dirty values back
#[cfg(test)]
pub mod test {

    #[test]
    fn test_transform_context_graph_object() {
        let mut graph_object = serde_json::json!(
            {
                "@id": "http://example.org/library",
                "@type": "ex:Library",
                "ex:contains": "http://example.org/library/the-republic"
            }
        );

        insta::assert_json_snapshot!(
            super::transform_context_graph_object(&mut graph_object).unwrap(),
            @r###"
        {
          "http://example.org/library": {
            "@type": "ex:Library",
            "ex:contains": "http://example.org/library/the-republic"
          }
        }
        "###);
    }
}
