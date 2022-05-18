use futures::{stream, SinkExt, Stream, StreamExt};
use json::JsonValue;
use serde::ser::SerializeSeq;
use tracing::{debug, instrument};
use uuid::Uuid;

use crate::{
    context::PROV,
    prov::{
        operations::{
            ActivityUses, ActsOnBehalfOf, ChronicleOperation, CreateActivity, CreateAgent,
            CreateEntity, CreateNamespace, EndActivity, EntityAttach, EntityDerive, GenerateEntity,
            RegisterKey, SetAttributes, StartActivity,
        },
        AgentId, AttachmentId, ChronicleIri, ChronicleTransactionId, EntityId, IdentityId,
        NamePart, NamespaceId, ProcessorError, ProvModel,
    },
};

use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender};
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    fmt::Display,
    pin::Pin,
    str::from_utf8,
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
            Self::Implementation { .. } => write!(f, "Subecription rror"),
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
#[derive(Debug)]
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
                chan: Some(rx).into(),
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

#[derive(Debug)]
pub struct InMemLedgerReader {
    chan: RefCell<Option<UnboundedReceiver<(Offset, ProvModel, ChronicleTransactionId)>>>,
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
        let stream = stream::unfold(self.chan.take().unwrap(), |mut chan| async move {
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
        let mut output = vec![];

        for tx in tx {
            debug!(?tx, "Processing");

            let deps = tx.dependencies();

            debug!(?deps, "Input addresses");

            let input = self
                .kv
                .borrow()
                .iter()
                .filter(|(k, _v)| deps.contains(k))
                .map(|(_addr, json)| StateInput::new(json.to_string().as_bytes().into()))
                .into_iter()
                .collect();

            debug!(?input, "Processing input state");

            let (mut tx_output, updated_model) = tx.process(model, input).await.unwrap();

            output.append(&mut tx_output);
            model = updated_model;
        }

        //Merge state output (last update wins) and sort by address, so push into a btree then iterate back to a vector
        let output = output
            .into_iter()
            .map(|state| (state.address.clone(), state))
            .collect::<BTreeMap<_, _>>()
            .into_iter()
            .map(|x| x.1)
            .collect::<Vec<_>>();

        for output in output {
            let state = json::parse(from_utf8(&output.data).unwrap()).unwrap();
            debug!(?output.address, "Address");
            debug!(%state, "New state");
            self.kv.borrow_mut().insert(output.address, state);
        }

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
    pub namespace: Option<String>,
    pub resource: String,
}

impl LedgerAddress {
    fn namespace(ns: &NamespaceId) -> Self {
        Self {
            namespace: None,
            resource: ns.to_string(),
        }
    }

    fn in_namespace(ns: &NamespaceId, resource: impl Into<ChronicleIri>) -> Self {
        Self {
            namespace: Some(ns.to_string()),
            resource: resource.into().to_string(),
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

impl ChronicleOperation {
    /// Compute dependencies for a chronicle operation, input and output addresses are always symmetric
    pub fn dependencies(&self) -> Vec<LedgerAddress> {
        match self {
            ChronicleOperation::CreateNamespace(CreateNamespace { id, .. }) => {
                vec![LedgerAddress::namespace(id)]
            }
            ChronicleOperation::CreateAgent(CreateAgent {
                namespace, name, ..
            }) => {
                vec![LedgerAddress::in_namespace(
                    namespace,
                    AgentId::from_name(name),
                )]
            }
            // Key registration requires identity + agent
            ChronicleOperation::RegisterKey(RegisterKey {
                namespace,
                id,
                publickey,
                ..
            }) => vec![
                LedgerAddress::in_namespace(namespace, id.clone()),
                LedgerAddress::in_namespace(
                    namespace,
                    IdentityId::from_name(id.name_part(), publickey),
                ),
            ],
            ChronicleOperation::CreateActivity(CreateActivity {
                namespace, name, ..
            }) => {
                vec![LedgerAddress::in_namespace(
                    namespace,
                    EntityId::from_name(name),
                )]
            }
            ChronicleOperation::StartActivity(StartActivity { namespace, id, .. }) => {
                vec![LedgerAddress::in_namespace(namespace, id.clone())]
            }
            ChronicleOperation::EndActivity(EndActivity { namespace, id, .. }) => {
                vec![LedgerAddress::in_namespace(namespace, id.clone())]
            }
            ChronicleOperation::ActivityUses(ActivityUses {
                namespace,
                id,
                activity,
            }) => {
                vec![
                    LedgerAddress::in_namespace(namespace, activity.clone()),
                    LedgerAddress::in_namespace(namespace, id.clone()),
                ]
            }
            ChronicleOperation::CreateEntity(CreateEntity {
                namespace, name, ..
            }) => {
                vec![LedgerAddress::in_namespace(
                    namespace,
                    EntityId::from_name(name),
                )]
            }
            ChronicleOperation::GenerateEntity(GenerateEntity {
                namespace,
                id,
                activity,
            }) => vec![
                LedgerAddress::in_namespace(namespace, activity.clone()),
                LedgerAddress::in_namespace(namespace, id.clone()),
            ],
            ChronicleOperation::EntityAttach(EntityAttach {
                namespace,
                id,
                agent,
                identityid,
                signature,
                ..
            }) => {
                vec![
                    LedgerAddress::in_namespace(namespace, agent.clone()),
                    LedgerAddress::in_namespace(namespace, id.clone()),
                    LedgerAddress::in_namespace(namespace, identityid.clone()),
                    LedgerAddress::in_namespace(
                        namespace,
                        AttachmentId::from_name(id.name_part(), signature),
                    ),
                ]
            }
            ChronicleOperation::AgentActsOnBehalfOf(ActsOnBehalfOf {
                namespace,
                id,
                delegate_id,
                activity_id,
                ..
            }) => vec![
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
                vec![LedgerAddress::in_namespace(namespace, id.clone())]
            }
            ChronicleOperation::SetAttributes(SetAttributes::Entity { id, namespace, .. }) => {
                vec![LedgerAddress::in_namespace(namespace, id.clone())]
            }
            ChronicleOperation::SetAttributes(SetAttributes::Activity {
                id, namespace, ..
            }) => {
                vec![LedgerAddress::in_namespace(namespace, id.clone())]
            }
        }
    }

    /// Take input states and apply them to the prov model, then apply transaction,
    /// then transform to the compact representation and write each resource to the output state,
    /// also return the aggregate model so we can emit it as an event
    #[instrument]
    pub async fn process(
        &self,
        mut model: ProvModel,
        input: Vec<StateInput>,
    ) -> Result<(Vec<StateOutput>, ProvModel), ProcessorError> {
        debug!(?input, "Transforming state input");

        for input in input {
            let resource = json::object! {
                "@context":  PROV.clone(),
                "@graph": [json::parse(std::str::from_utf8(&input.data)?)?]
            };
            debug!(%resource, "Restore graph / context");
            model = model.apply_json_ld(resource).await?;
        }

        model.apply(self);
        let mut json_ld = model.to_json().compact_stable_order().await?;

        debug!(%json_ld, "Result model");

        Ok((
            if let Some(graph) = json_ld.get("@graph").and_then(|g| g.as_array()) {
                // Separate graph into descrete outpute
                graph
                    .iter()
                    .map(|resource| {
                        Ok(StateOutput {
                            address: LedgerAddress {
                                namespace: resource
                                    .get("namespace")
                                    .and_then(|resource| resource.as_str())
                                    .map(|resource| resource.to_owned()),
                                resource: resource
                                    .get("@id")
                                    .and_then(|id| id.as_str())
                                    .ok_or(ProcessorError::NotANode {})?
                                    .to_owned(),
                            },
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
                    address: LedgerAddress {
                        namespace: json_ld
                            .get("namespace")
                            .and_then(|resource| resource.as_str())
                            .map(|resource| resource.to_owned()),
                        resource: json_ld
                            .get("@id")
                            .and_then(|id| id.as_str())
                            .ok_or(ProcessorError::NotANode {})?
                            .to_owned(),
                    },
                    data: serde_json::to_string(&json_ld).unwrap().into_bytes(),
                }]
            },
            model,
        ))
    }
}

#[cfg(test)]
pub mod test {}
