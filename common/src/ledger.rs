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
        let mut output = vec![];

        let mut deps_addresses: Vec<LedgerAddress> = vec![];

        for tx in tx {
            let deps = tx.dependencies();

            let input: Vec<StateInput> = self
                .kv
                .borrow()
                .iter()
                .filter(|(k, _v)| deps.contains(k))
                .map(|(_addr, json)| StateInput::new(json.to_string().as_bytes().into()))
                .into_iter()
                .collect();

            debug!(
                input_chronicle_addresses=?deps,
            );

            let (mut tx_output, updated_model) = tx.process(model, input).await.unwrap();

            output.append(&mut tx_output);
            model = updated_model;
            deps_addresses = deps.into_iter().collect();
        }

        //Merge state output (last update wins) and sort by address, so push into a btree then iterate back to a vector
        let output = output
            .into_iter()
            .map(|state| (state.address.clone(), state))
            .collect::<BTreeMap<_, _>>()
            .into_iter()
            .map(|x| x.1)
            .map(|s| match s.address.is_specified(&deps_addresses) {
                true => Ok(s),
                _ => Err(ProcessorError::Address {}),
            })
            .collect::<Result<Vec<_>, ProcessorError>>()?;

        for output in output {
            let state = json::parse(from_utf8(&output.data).unwrap()).unwrap();
            debug!(output_address=?output.address);
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

    fn is_specified(&self, dependencies: &[LedgerAddress]) -> bool {
        dependencies.iter().any(|addr| self == addr)
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

#[cfg(test)]
pub mod test {
    use serde_json::json;
    use std::collections::BTreeMap;

    use crate::{
        attributes::{Attribute, Attributes},
        ledger::{InMemLedger, LedgerWriter},
        prov::{
            operations::{
                ActivityUses, ActsOnBehalfOf, ChronicleOperation, CreateActivity, CreateEntity,
                CreateNamespace, DerivationType, EntityDerive, GenerateEntity, RegisterKey,
                SetAttributes, StartActivity,
            },
            ActivityId, AgentId, DomaintypeId, EntityId, NamePart, NamespaceId,
        },
    };
    use uuid::Uuid;
    fn uuid() -> Uuid {
        let bytes = [
            0xa1, 0xa2, 0xa3, 0xa4, 0xb1, 0xb2, 0xc1, 0xc2, 0xd1, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6,
            0xd7, 0xd8,
        ];
        Uuid::from_slice(&bytes).unwrap()
    }

    // #[tokio::test]
    // async fn test_entity_attach() -> Result<(), String> {
    //     let namespace: NamespaceId = NamespaceId::from_name("testns", uuid());
    //     let id = EntityId::from_name("test_entity");
    //     let agent = AgentId::from_name("test_agent");
    //     let op: ChronicleOperation = ChronicleOperation::EntityAttach(EntityAttach {
    //         namespace,
    //         identityid: None,
    //         id,
    //         locator: None,
    //         agent,
    //         signature: None,
    //         signature_time: None,
    //     });
    //     let mut l = InMemLedger::new();
    //     let tx = vec![op];
    //     let id = l.submit(&tx).await;
    //     if id.is_err() {
    //         Err(format!("error: {id:?}"))
    //     } else {
    //         Ok(())
    //     }
    // }

    #[tokio::test]
    async fn test_create_namespace() -> Result<(), String> {
        let name = "testns";
        let id = NamespaceId::from_name(name, uuid());

        let op = ChronicleOperation::CreateNamespace(CreateNamespace::new(id, name, uuid()));
        let mut l = InMemLedger::new();
        let tx = vec![op];
        let id = l.submit(&tx).await;
        if id.is_err() {
            Err(format!("error: {id:?}"))
        } else {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_create_agent() -> Result<(), String> {
        let uuid = uuid();
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid);
        let name: crate::prov::Name =
            crate::prov::NamePart::name_part(&crate::prov::AgentId::from_name("test_agent"))
                .clone();
        let op: ChronicleOperation =
            super::ChronicleOperation::CreateAgent(crate::prov::operations::CreateAgent {
                namespace,
                name,
            });
        let mut l = InMemLedger::new();
        let tx = vec![op];
        let id = l.submit(&tx).await;
        if id.is_err() {
            Err(format!("error: {id:?}"))
        } else {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_agent_acts_on_behalf_of() -> Result<(), String> {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid());
        let id = AgentId::from_name("test_agent");
        let delegate_id = AgentId::from_name("test_delegate");
        let activity_id = Some(ActivityId::from_name("test_activity"));

        let op: ChronicleOperation = ChronicleOperation::AgentActsOnBehalfOf(ActsOnBehalfOf {
            namespace,
            id,
            delegate_id,
            activity_id,
        });
        let mut l = InMemLedger::new();
        let tx = vec![op];
        let id = l.submit(&tx).await;
        if id.is_err() {
            Err(format!("error: {id:?}"))
        } else {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_register_key() -> Result<(), String> {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid());
        let id = crate::prov::AgentId::from_name("test_agent");
        let publickey =
            "02197db854d8c6a488d4a0ef3ef1fcb0c06d66478fae9e87a237172cf6f6f7de23".to_string();

        let op: ChronicleOperation = ChronicleOperation::RegisterKey(RegisterKey {
            namespace,
            id,
            publickey,
        });
        let mut l = InMemLedger::new();
        let tx = vec![op];
        let id = l.submit(&tx).await;
        if id.is_err() {
            Err(format!("error: {id:?}"))
        } else {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_create_activity() -> Result<(), String> {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid());
        let name = NamePart::name_part(&ActivityId::from_name("test_activity")).to_owned();

        let op: ChronicleOperation =
            ChronicleOperation::CreateActivity(CreateActivity { namespace, name });
        let mut l = InMemLedger::new();
        let tx = vec![op];
        let id = l.submit(&tx).await;
        if id.is_err() {
            Err(format!("error: {id:?}"))
        } else {
            Ok(())
        }
    }

    #[tokio::test]
    async fn start_activity() -> Result<(), String> {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid());
        let id = ActivityId::from_name("test_activity");
        let agent = AgentId::from_name("test_agent");
        let time = chrono::DateTime::<chrono::Utc>::from_utc(
            chrono::NaiveDateTime::from_timestamp(61, 0),
            chrono::Utc,
        );
        let op: ChronicleOperation = ChronicleOperation::StartActivity(StartActivity {
            namespace,
            id,
            agent,
            time,
        });
        let mut l = InMemLedger::new();
        let tx = vec![op];
        let id = l.submit(&tx).await;
        if id.is_err() {
            Err(format!("error: {id:?}"))
        } else {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_end_activity() -> Result<(), String> {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid());
        let id = ActivityId::from_name("test_activity");
        let agent = crate::prov::AgentId::from_name("test_agent");
        let time = chrono::DateTime::<chrono::Utc>::from_utc(
            chrono::NaiveDateTime::from_timestamp(61, 0),
            chrono::Utc,
        );
        let op: ChronicleOperation =
            super::ChronicleOperation::EndActivity(crate::prov::operations::EndActivity {
                namespace,
                id,
                agent,
                time,
            });
        let mut l = InMemLedger::new();
        let tx = vec![op];
        let id = l.submit(&tx).await;
        if id.is_err() {
            Err(format!("error: {id:?}"))
        } else {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_activity_uses() -> Result<(), String> {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid());
        let id = EntityId::from_name("test_entity");
        let activity = ActivityId::from_name("test_activity");
        let op: ChronicleOperation = ChronicleOperation::ActivityUses(ActivityUses {
            namespace,
            id,
            activity,
        });
        let mut l = InMemLedger::new();
        let tx = vec![op];
        let id = l.submit(&tx).await;
        if id.is_err() {
            Err(format!("error: {id:?}"))
        } else {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_create_entity() -> Result<(), String> {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid());
        let id = NamePart::name_part(&EntityId::from_name("test_entity")).to_owned();
        let op: ChronicleOperation = ChronicleOperation::CreateEntity(CreateEntity {
            namespace,
            name: id,
        });
        let mut l = InMemLedger::new();
        let tx = vec![op];
        let id = l.submit(&tx).await;
        if id.is_err() {
            Err(format!("error: {id:?}"))
        } else {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_generate_entity() -> Result<(), String> {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid());
        let id = EntityId::from_name("test_entity");
        let activity = ActivityId::from_name("test_activity");
        let op: ChronicleOperation = ChronicleOperation::GenerateEntity(GenerateEntity {
            namespace,
            id,
            activity,
        });
        let mut l = InMemLedger::new();
        let tx = vec![op];
        let id = l.submit(&tx).await;
        if id.is_err() {
            Err(format!("error: {id:?}"))
        } else {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_entity_derive() -> Result<(), String> {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid());
        let id = EntityId::from_name("test_entity");
        let used_id = EntityId::from_name("test_used_entity");
        let activity_id = Some(ActivityId::from_name("test_activity"));
        let typ = Some(DerivationType::Revision);
        let op: ChronicleOperation = ChronicleOperation::EntityDerive(EntityDerive {
            namespace,
            id,
            used_id,
            activity_id,
            typ,
        });
        let mut l = InMemLedger::new();
        let tx = vec![op];
        let id = l.submit(&tx).await;
        if id.is_err() {
            Err(format!("error: {id:?}"))
        } else {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_set_attributes_entity() -> Result<(), String> {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid());
        let id = EntityId::from_name("test_entity");
        let domain = DomaintypeId::from_name("test_domain");
        let attributes = Attributes {
            typ: Some(domain),
            attributes: BTreeMap::new(),
        };
        let op: ChronicleOperation = ChronicleOperation::SetAttributes(SetAttributes::Entity {
            namespace,
            id,
            attributes,
        });
        let mut l = InMemLedger::new();
        let tx = vec![op];
        let id = l.submit(&tx).await;
        if id.is_err() {
            Err(format!("error: {id:?}"))
        } else {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_set_attributes_entity_multiple_attributes() -> Result<(), String> {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid());
        let id = EntityId::from_name("test_entity");
        let domain = DomaintypeId::from_name("test_domain");
        let attrs = {
            let mut h: BTreeMap<String, Attribute> = BTreeMap::new();

            let attr = Attribute {
                typ: "Bool".to_string(),
                value: json!("Bool"),
            };
            h.insert("bool_attribute".to_string(), attr);

            let attr = Attribute {
                typ: "String".to_string(),
                value: json!("String"),
            };
            h.insert("string_attribute".to_string(), attr);

            let attr = Attribute {
                typ: "Int".to_string(),
                value: json!("Int"),
            };
            h.insert("int_attribute".to_string(), attr);

            h
        };

        let attributes = Attributes {
            typ: Some(domain),
            attributes: attrs,
        };
        let op: ChronicleOperation = ChronicleOperation::SetAttributes(SetAttributes::Entity {
            namespace,
            id,
            attributes,
        });
        let mut l = InMemLedger::new();
        let tx = vec![op];
        let id = l.submit(&tx).await;
        if id.is_err() {
            Err(format!("error: {id:?}"))
        } else {
            Ok(())
        }
    }
}
