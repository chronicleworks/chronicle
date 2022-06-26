use chrono::{DateTime, Utc};
use custom_error::custom_error;
use json::JsonValue;
use json_ld::{context::Local, Document, JsonContext, NoLoader};

use serde::Serialize;
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    convert::Infallible,
    fmt::Display,
};
use tokio::task::JoinError;
use uuid::Uuid;

use crate::attributes::{Attribute, Attributes};

use super::{
    id,
    operations::{
        ActivityUses, ActsOnBehalfOf, ChronicleOperation, CreateActivity, CreateAgent,
        CreateEntity, CreateNamespace, DerivationType, EndActivity, EntityAttach, EntityDerive,
        GenerateEntity, RegisterKey, SetAttributes, StartActivity,
    },
    ActivityId, AgentId, DomaintypeId, EntityId, EvidenceId, IdentityId, Name, NamePart,
    NamespaceId, PublicKeyPart, UuidPart,
};

pub mod to_json_ld;

custom_error! {pub ProcessorError
    Address{} = "Invalid address",
    Compaction{source: CompactionError} = "Json Ld Error",
    Expansion{inner: String} = "Json Ld Error",
    IRef{source: iref::Error} = "Invalid IRI",
    NotAChronicleIri{source: id::ParseIriError } = "Not a Chronicle IRI",
    Tokio{source: JoinError} = "Tokio Error",
    MissingId{object: JsonValue} = "Missing @id",
    MissingProperty{iri: String, object: JsonValue} = "Missing property",
    NotANode{} = "Json LD object is not a node",
    NotAnObject{} = "Chronicle value is not a json object",
    Time{source: chrono::ParseError} = "Unparsable date/time",
    Json{source: json::JsonError} = "Malformed JSON",
    SerdeJson{source: serde_json::Error } = "Malformed JSON",
    Utf8{source: std::str::Utf8Error} = "State is not valid utf8",
}

impl From<Infallible> for ProcessorError {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}

custom_error! {pub ChronicleTransactionIdError
    InvalidTransactionId {id: String} = "Invalid transaction id",
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct ChronicleTransactionId(String);

impl Display for ChronicleTransactionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&*self.0)
    }
}
impl From<Uuid> for ChronicleTransactionId {
    fn from(u: Uuid) -> Self {
        Self(u.to_string())
    }
}

impl From<&str> for ChronicleTransactionId {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct ChronicleTransaction {
    pub tx: Vec<ChronicleOperation>,
}

impl ChronicleTransaction {
    pub fn new(tx: Vec<ChronicleOperation>) -> Self {
        Self { tx }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Namespace {
    pub id: NamespaceId,
    pub uuid: Uuid,
    pub name: Name,
}

impl Namespace {
    pub fn new(id: NamespaceId, uuid: Uuid, name: &Name) -> Self {
        Self {
            id,
            uuid,
            name: name.to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Agent {
    pub id: AgentId,
    pub namespaceid: NamespaceId,
    pub name: Name,
    pub domaintypeid: Option<DomaintypeId>,
    pub attributes: BTreeMap<String, Attribute>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    pub id: IdentityId,
    pub namespaceid: NamespaceId,
    pub public_key: String,
}

impl Identity {
    pub fn new(namespace: &NamespaceId, agent: &AgentId, public_key: &str) -> Self {
        Self {
            id: IdentityId::from_name(agent.name_part(), public_key),
            namespaceid: namespace.clone(),
            public_key: public_key.to_owned(),
        }
    }
}

impl Agent {
    pub fn has_attributes(self, attributes: Attributes) -> Self {
        let Self {
            id,
            namespaceid,
            name,
            ..
        } = self;

        Self {
            id,
            namespaceid,
            name,
            domaintypeid: attributes.typ,
            attributes: attributes.attributes,
        }
    }

    // Create a prototypical agent from its IRI, we can only determine name
    pub fn exists(namespaceid: NamespaceId, id: AgentId) -> Self {
        Self {
            namespaceid,
            name: id.name_part().to_owned(),
            id,
            domaintypeid: None,
            attributes: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Activity {
    pub id: ActivityId,
    pub namespaceid: NamespaceId,
    pub name: Name,
    pub domaintypeid: Option<DomaintypeId>,
    pub attributes: BTreeMap<String, Attribute>,
    pub started: Option<DateTime<Utc>>,
    pub ended: Option<DateTime<Utc>>,
}

impl Activity {
    pub fn has_attributes(self, attributes: Attributes) -> Self {
        let Self {
            id,
            namespaceid,
            name,
            started,
            ended,
            ..
        } = self;
        Self {
            id,
            namespaceid,
            name,
            started,
            ended,
            domaintypeid: attributes.typ,
            attributes: attributes.attributes,
        }
    }

    // Create a prototypical agent from its IRI, we can only determine name
    pub fn exists(namespaceid: NamespaceId, id: ActivityId) -> Self {
        Self {
            namespaceid,
            name: id.name_part().to_owned(),
            id,
            started: None,
            ended: None,
            domaintypeid: None,
            attributes: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attachment {
    pub id: EvidenceId,
    pub namespaceid: NamespaceId,
    pub signature: String,
    pub signer: IdentityId,
    pub locator: Option<String>,
    pub signature_time: DateTime<Utc>,
}

impl Attachment {
    fn new(
        namespace: NamespaceId,
        entity: &EntityId,
        signer: &IdentityId,
        signature: &str,
        locator: Option<String>,
        signature_time: DateTime<Utc>,
    ) -> Attachment {
        Self {
            id: EvidenceId::from_name(entity.name_part(), signature),
            namespaceid: namespace,
            signature: signature.to_owned(),
            signer: signer.clone(),
            locator,
            signature_time,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entity {
    pub id: EntityId,
    pub namespaceid: NamespaceId,
    pub name: Name,
    pub domaintypeid: Option<DomaintypeId>,
    pub attributes: BTreeMap<String, Attribute>,
}

impl Entity {
    pub fn has_attributes(self, attributes: Attributes) -> Self {
        let Self {
            id,
            namespaceid,
            name,
            ..
        } = self;
        Self {
            id,
            namespaceid,
            name,
            domaintypeid: attributes.typ,
            attributes: attributes.attributes,
        }
    }

    pub fn exists(namespaceid: NamespaceId, id: EntityId) -> Self {
        Self {
            name: id.name_part().to_owned(),
            id,
            namespaceid,
            domaintypeid: None,
            attributes: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Derivation {
    pub generated_id: EntityId,
    pub used_id: EntityId,
    pub activity_id: Option<ActivityId>,
    pub typ: Option<DerivationType>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Delegation {
    pub delegate_id: AgentId,
    pub responsible_id: AgentId,
    pub activity_id: Option<ActivityId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Association {
    pub agent_id: AgentId,
    pub activity_id: ActivityId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Useage {
    pub activity_id: ActivityId,
    pub entity_id: EntityId,
    pub time: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Generation {
    pub activity_id: ActivityId,
    pub generated_id: EntityId,
    pub time: Option<DateTime<Utc>>,
}

type NamespacedId<T> = (NamespaceId, T);
type NamespacedAgent = NamespacedId<AgentId>;
type NamespacedEntity = NamespacedId<EntityId>;
type NamespacedActivity = NamespacedId<ActivityId>;
type NamespacedIdentity = NamespacedId<IdentityId>;
type NamespacedAttachment = NamespacedId<EvidenceId>;

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvModel {
    pub namespaces: HashMap<NamespaceId, Namespace>,
    pub agents: HashMap<NamespacedAgent, Agent>,
    pub activities: HashMap<NamespacedActivity, Activity>,
    pub entities: HashMap<NamespacedEntity, Entity>,
    pub identities: HashMap<NamespacedIdentity, Identity>,
    pub attachments: HashMap<NamespacedAttachment, Attachment>,
    pub has_identity: HashMap<NamespacedAgent, NamespacedIdentity>,
    pub had_identity: HashMap<NamespacedAgent, HashSet<NamespacedIdentity>>,
    pub has_evidence: HashMap<NamespacedEntity, NamespacedAttachment>,
    pub had_attachment: HashMap<NamespacedEntity, HashSet<NamespacedAttachment>>,
    pub association: HashMap<NamespacedActivity, Vec<Association>>,
    pub derivation: HashMap<NamespacedEntity, Vec<Derivation>>,
    pub delegation: HashMap<NamespacedAgent, Vec<Delegation>>,
    pub generation: HashMap<NamespacedEntity, Vec<Generation>>,
    pub useage: HashMap<NamespacedActivity, Vec<Useage>>,
}

impl ProvModel {
    /// Apply a sequence of `ChronicleTransaction` to an empty model, then return it
    pub fn from_tx<'a, I>(tx: I) -> Self
    where
        I: IntoIterator<Item = &'a ChronicleOperation>,
    {
        let mut model = Self::default();
        for tx in tx {
            model.apply(tx);
        }

        model
    }

    /// Merge 2 prov models, consuming the other
    pub fn merge(&mut self, other: ProvModel) {
        for (id, ns) in other.namespaces {
            self.namespaces.insert(id, ns);
        }

        for (id, agent) in other.agents {
            self.agents.insert(id, agent);
        }

        for (id, acitvity) in other.activities {
            self.activities.insert(id, acitvity);
        }

        for (id, entity) in other.entities {
            self.entities.insert(id, entity);
        }

        for (id, identity) in other.identities {
            self.identities.insert(id, identity);
        }

        for (id, attachment) in other.attachments {
            self.attachments.insert(id, attachment);
        }

        for (id, other_link) in other.has_identity {
            self.has_identity.insert(id, other_link);
        }

        for (id, other_link) in other.has_evidence {
            self.has_evidence.insert(id, other_link);
        }

        for (id, links) in other.had_identity {
            self.had_identity
                .entry(id.clone())
                .and_modify(|map| {
                    for link in links.clone() {
                        map.insert(link);
                    }
                })
                .or_insert(links);
        }
        for (id, links) in other.had_attachment {
            self.had_attachment
                .entry(id.clone())
                .and_modify(|map| {
                    for link in links.clone() {
                        map.insert(link);
                    }
                })
                .or_insert(links);
        }
        for (id, mut rhs) in other.association {
            self.association
                .entry(id.clone())
                .and_modify(|xs| xs.append(&mut rhs))
                .or_insert(rhs);
        }

        for (id, mut rhs) in other.generation {
            self.generation
                .entry(id.clone())
                .and_modify(|xs| xs.append(&mut rhs))
                .or_insert(rhs);
        }

        for (id, mut rhs) in other.useage {
            self.useage
                .entry(id.clone())
                .and_modify(|xs| xs.append(&mut rhs))
                .or_insert(rhs);
        }

        for (id, mut rhs) in other.derivation {
            self.derivation
                .entry(id.clone())
                .and_modify(|xs| xs.append(&mut rhs))
                .or_insert(rhs);
        }

        for (id, mut rhs) in other.delegation {
            self.delegation
                .entry(id.clone())
                .and_modify(|xs| xs.append(&mut rhs))
                .or_insert(rhs);
        }
    }

    /// Append a derivation to the model
    pub fn was_derived_from(
        &mut self,
        namespace: NamespaceId,
        typ: Option<DerivationType>,
        used_id: EntityId,
        id: EntityId,
        activity_id: Option<ActivityId>,
    ) {
        self.derivation
            .entry((namespace, id.clone()))
            .or_insert_with(Vec::new)
            .push(Derivation {
                typ,
                generated_id: id,
                used_id,
                activity_id,
            });
    }

    /// Append a delegation to the model
    pub fn acted_on_behalf_of(
        &mut self,
        namespace: NamespaceId,
        responsible_id: AgentId,
        delegate_id: AgentId,
        activity_id: Option<ActivityId>,
    ) {
        self.delegation
            .entry((namespace, responsible_id.clone()))
            .or_insert_with(Vec::new)
            .push(Delegation {
                responsible_id,
                delegate_id,
                activity_id,
            });
    }

    pub fn was_associated_with(
        &mut self,
        namespace: &NamespaceId,
        activity_id: &ActivityId,
        agent_id: &AgentId,
    ) {
        self.association
            .entry((namespace.clone(), activity_id.clone()))
            .or_insert_with(std::vec::Vec::new)
            .push(Association {
                agent_id: agent_id.clone(),
                activity_id: activity_id.clone(),
            });
    }

    pub fn was_generated_by(
        &mut self,
        namespace: NamespaceId,
        generated_id: &EntityId,
        activity_id: &ActivityId,
    ) {
        self.generation
            .entry((namespace, generated_id.clone()))
            .or_insert_with(std::vec::Vec::new)
            .push(Generation {
                activity_id: activity_id.clone(),
                generated_id: generated_id.clone(),
                time: None,
            })
    }

    pub fn used(&mut self, namespace: NamespaceId, activity_id: &ActivityId, entity_id: &EntityId) {
        self.useage
            .entry((namespace, activity_id.clone()))
            .or_insert_with(std::vec::Vec::new)
            .push(Useage {
                activity_id: activity_id.clone(),
                entity_id: entity_id.clone(),
                time: None,
            })
    }

    pub fn had_identity(&mut self, namespace: NamespaceId, agent: &AgentId, identity: &IdentityId) {
        self.had_identity
            .entry((namespace.clone(), agent.clone()))
            .or_insert_with(HashSet::new)
            .insert((namespace, identity.clone()));
    }

    pub fn has_identity(&mut self, namespace: NamespaceId, agent: &AgentId, identity: &IdentityId) {
        self.has_identity.insert(
            (namespace.clone(), agent.clone()),
            (namespace, identity.clone()),
        );
    }

    pub fn had_attachment(
        &mut self,
        namespace: NamespaceId,
        entity: EntityId,
        attachment: &EvidenceId,
    ) {
        self.had_attachment
            .entry((namespace.clone(), entity))
            .or_insert_with(HashSet::new)
            .insert((namespace, attachment.clone()));
    }

    pub fn has_attachment(
        &mut self,
        namespace: NamespaceId,
        entity: EntityId,
        attachment: &EvidenceId,
    ) {
        self.has_evidence
            .insert((namespace.clone(), entity), (namespace, attachment.clone()));
    }

    fn sign(
        &mut self,
        namespace: NamespaceId,
        signer: &IdentityId,
        entity: &EntityId,
        signature: &str,
        locator: Option<String>,
        signature_time: DateTime<Utc>,
    ) {
        let new_attachment = Attachment::new(
            namespace.clone(),
            entity,
            signer,
            signature,
            locator,
            signature_time,
        );

        if let Some((_, old_attachment)) = self
            .has_evidence
            .remove(&(namespace.clone(), entity.clone()))
        {
            self.had_attachment(namespace.clone(), entity.clone(), &old_attachment);
        }

        self.has_attachment(namespace, entity.clone(), &new_attachment.id);
        self.add_attachment(new_attachment);
    }

    fn new_identity(&mut self, namespace: &NamespaceId, agent: &AgentId, signature: &str) {
        let new_identity = Identity::new(namespace, agent, signature);

        if let Some((_, old_identity)) = self
            .has_identity
            .remove(&(namespace.clone(), agent.clone()))
        {
            self.had_identity(namespace.clone(), agent, &old_identity);
        }

        self.has_identity(namespace.clone(), agent, &new_identity.id);
        self.add_identity(new_identity);
    }

    pub fn namespace_context(&mut self, ns: &NamespaceId) {
        let (namespacename, uuid) = (ns.name_part(), ns.uuid_part());

        self.namespaces.insert(
            ns.clone(),
            Namespace {
                id: ns.clone(),
                uuid: uuid.to_owned(),
                name: namespacename.to_owned(),
            },
        );
    }

    /// Transform a sequence of `ChronicleOperation` events into a provenance model,
    /// If a statement requires a subject or object that does not currently exist in the model, then we create it
    pub fn apply(&mut self, tx: &ChronicleOperation) {
        let tx = tx.to_owned();
        match tx {
            ChronicleOperation::CreateNamespace(CreateNamespace {
                id,
                name: _,
                uuid: _,
            }) => {
                self.namespace_context(&id);
            }
            ChronicleOperation::CreateAgent(CreateAgent {
                namespace, name, ..
            }) => {
                let id = AgentId::from_name(&name);
                self.namespace_context(&namespace);
                self.agents.insert(
                    (namespace.clone(), id.clone()),
                    Agent::exists(namespace, id),
                );
            }

            ChronicleOperation::AgentActsOnBehalfOf(ActsOnBehalfOf {
                namespace,
                id,
                delegate_id,
                activity_id,
            }) => {
                self.namespace_context(&namespace);

                self.agents
                    .entry((namespace.clone(), id.clone()))
                    .or_insert_with(|| Agent::exists(namespace.clone(), id.clone()));

                self.agents
                    .entry((namespace.clone(), delegate_id.clone()))
                    .or_insert_with(|| Agent::exists(namespace.clone(), delegate_id.clone()));

                if let Some(activity_id) = activity_id.clone() {
                    self.activities
                        .entry((namespace.clone(), activity_id.clone()))
                        .or_insert_with(|| Activity::exists(namespace.clone(), activity_id));
                }

                self.acted_on_behalf_of(namespace, id, delegate_id, activity_id)
            }
            ChronicleOperation::RegisterKey(RegisterKey {
                namespace,
                id,
                publickey,
                ..
            }) => {
                self.namespace_context(&namespace);

                self.agents
                    .entry((namespace.clone(), id.clone()))
                    .or_insert_with(|| Agent::exists(namespace.clone(), id.clone()));
                self.new_identity(&namespace, &id, &publickey);
            }
            ChronicleOperation::CreateActivity(CreateActivity {
                namespace, name, ..
            }) => {
                let id = ActivityId::from_name(&name);
                self.namespace_context(&namespace);

                self.activities
                    .entry((namespace.clone(), id.clone()))
                    .or_insert_with(|| Activity::exists(namespace, id));
            }
            ChronicleOperation::StartActivity(StartActivity {
                namespace,
                id,
                agent,
                time,
            }) => {
                self.namespace_context(&namespace);

                self.agents
                    .entry((namespace.clone(), agent.clone()))
                    .or_insert_with(|| Agent::exists(namespace.clone(), agent.clone()));

                // Ensure started <= ended
                self.activities
                    .entry((namespace.clone(), id.clone()))
                    .and_modify(|mut activity| {
                        match activity.ended {
                            Some(ended) if ended < time => activity.ended = Some(time),
                            _ => {}
                        };
                        activity.started = Some(time);
                    })
                    .or_insert_with(|| {
                        let mut activity = Activity::exists(namespace.clone(), id.clone());
                        activity.started = Some(time);
                        activity
                    });

                self.was_associated_with(&namespace, &id, &agent);
            }
            ChronicleOperation::EndActivity(EndActivity {
                namespace,
                id,
                agent,
                time,
            }) => {
                self.namespace_context(&namespace);

                self.agents
                    .entry((namespace.clone(), agent.clone()))
                    .or_insert_with(|| Agent::exists(namespace.clone(), agent.clone()));

                // Set our end time, and also the start date if this is a new resource, or the existing resource does not specify a start time
                // Following our inference - an ended activity must have also started, so becomes an instant if the start time is not specified
                // or is greater than the end time
                self.activities
                    .entry((namespace.clone(), id.clone()))
                    .and_modify(|mut activity| {
                        match activity.started {
                            None => activity.started = Some(time),
                            Some(started) if started > time => activity.started = Some(time),
                            _ => {}
                        };
                        activity.ended = Some(time);
                    })
                    .or_insert({
                        let mut activity = Activity::exists(namespace.clone(), id.clone());
                        activity.ended = Some(time);
                        activity.started = Some(time);
                        activity
                    });

                self.was_associated_with(&namespace, &id, &agent);
            }
            ChronicleOperation::ActivityUses(ActivityUses {
                namespace,
                id,
                activity,
            }) => {
                self.namespace_context(&namespace);
                if !self
                    .activities
                    .contains_key(&(namespace.clone(), activity.clone()))
                {
                    self.add_activity(Activity::exists(namespace.clone(), activity.clone()));
                }
                if !self.entities.contains_key(&(namespace.clone(), id.clone())) {
                    self.add_entity(Entity::exists(namespace.clone(), id.clone()));
                }

                self.used(namespace, &activity, &id);
            }
            ChronicleOperation::CreateEntity(CreateEntity {
                namespace, name, ..
            }) => {
                let id = EntityId::from_name(&name);
                self.namespace_context(&namespace);
                self.entities.insert(
                    (namespace.clone(), id.clone()),
                    Entity::exists(namespace, id),
                );
            }
            ChronicleOperation::GenerateEntity(GenerateEntity {
                namespace,
                id,
                activity,
            }) => {
                self.namespace_context(&namespace);
                if !self
                    .activities
                    .contains_key(&(namespace.clone(), activity.clone()))
                {
                    self.add_activity(Activity::exists(namespace.clone(), activity.clone()));
                }
                if !self.entities.contains_key(&(namespace.clone(), id.clone())) {
                    self.add_entity(Entity::exists(namespace.clone(), id.clone()));
                }

                self.was_generated_by(namespace, &id, &activity)
            }
            ChronicleOperation::EntityAttach(EntityAttach {
                namespace,
                id,
                agent,
                identityid,
                signature,
                locator,
                signature_time,
            }) => {
                self.namespace_context(&namespace);

                if !self.entities.contains_key(&(namespace.clone(), id.clone())) {
                    self.add_entity(Entity::exists(namespace.clone(), id.clone()));
                }

                let agent_key = (namespace.clone(), agent.clone());
                if !self.agents.contains_key(&agent_key) {
                    self.add_agent(Agent::exists(namespace.clone(), agent));
                }

                let identity_key = (namespace.clone(), identityid.as_ref().unwrap().clone());

                if !self.identities.contains_key(&identity_key) {
                    let agent = self.agents.get(&agent_key).unwrap().id.clone();
                    let id = identityid.clone().unwrap();
                    let public_key = &id.public_key_part().to_owned();
                    self.add_identity(Identity::new(&namespace, &agent, public_key));
                    self.has_identity(namespace.clone(), &agent, &id);
                }

                let entity = self
                    .entities
                    .get(&(namespace.clone(), id))
                    .unwrap()
                    .id
                    .clone();

                self.sign(
                    namespace,
                    &identityid.unwrap(),
                    &entity,
                    &*signature.unwrap(),
                    locator,
                    signature_time.unwrap(),
                );
            }
            ChronicleOperation::EntityDerive(EntityDerive {
                namespace,
                id,
                typ,
                used_id,
                activity_id,
            }) => {
                self.namespace_context(&namespace);

                // Ensure the generated entity is in the graph
                if !self.entities.contains_key(&(namespace.clone(), id.clone())) {
                    self.add_entity(Entity::exists(namespace.clone(), id.clone()));
                }

                // Enmsure the used entity is in the graph
                if !self
                    .entities
                    .contains_key(&(namespace.clone(), used_id.clone()))
                {
                    self.add_entity(Entity::exists(namespace.clone(), used_id.clone()));
                }

                self.was_derived_from(namespace, typ, used_id, id, activity_id);
            }
            ChronicleOperation::SetAttributes(SetAttributes::Entity {
                namespace,
                id,
                attributes,
            }) => {
                self.namespace_context(&namespace);

                self.entities
                    .entry((namespace.clone(), id.clone()))
                    .and_modify(|entity| {
                        entity.domaintypeid = attributes.typ.clone();
                        entity.attributes = attributes.attributes.clone();
                    })
                    .or_insert_with(|| {
                        Entity::exists(namespace, id.clone()).has_attributes(attributes)
                    });
            }
            ChronicleOperation::SetAttributes(SetAttributes::Activity {
                namespace,
                id,
                attributes,
            }) => {
                self.namespace_context(&namespace);

                self.activities
                    .entry((namespace.clone(), id.clone()))
                    .and_modify(|mut acitvity| {
                        acitvity.domaintypeid = attributes.typ.clone();
                        acitvity.attributes = attributes.attributes.clone();
                    })
                    .or_insert_with(|| {
                        Activity::exists(namespace, id.clone()).has_attributes(attributes)
                    });
            }
            ChronicleOperation::SetAttributes(SetAttributes::Agent {
                namespace,
                id,
                attributes,
            }) => {
                self.namespace_context(&namespace);

                self.agents
                    .entry((namespace.clone(), id.clone()))
                    .and_modify(|mut agent| {
                        agent.domaintypeid = attributes.typ.clone();
                        agent.attributes = attributes.attributes.clone();
                    })
                    .or_insert_with(|| {
                        Agent::exists(namespace, id.clone()).has_attributes(attributes)
                    });
            }
        };
    }

    pub(crate) fn add_attachment(&mut self, attachment: Attachment) {
        self.attachments.insert(
            (attachment.namespaceid.clone(), attachment.id.clone()),
            attachment,
        );
    }

    pub(crate) fn add_identity(&mut self, identity: Identity) {
        self.identities.insert(
            (identity.namespaceid.clone(), identity.id.clone()),
            identity,
        );
    }

    pub(crate) fn add_agent(&mut self, agent: Agent) {
        self.agents
            .insert((agent.namespaceid.clone(), agent.id.clone()), agent);
    }

    pub(crate) fn add_activity(&mut self, activity: Activity) {
        self.activities.insert(
            (activity.namespaceid.clone(), activity.id.clone()),
            activity,
        );
    }

    pub(crate) fn add_entity(&mut self, entity: Entity) {
        self.entities
            .insert((entity.namespaceid.clone(), entity.id.clone()), entity);
    }
}

custom_error::custom_error! {pub CompactionError
    JsonLd{inner: String}                  = "JsonLd", //TODO: contribute Send to the upstream JsonLD error type
    Join{source : JoinError}               = "Tokio",
    Serde{source: serde_json::Error }      = "Serde conversion",
}
pub struct ExpandedJson(pub JsonValue);

impl ExpandedJson {
    pub async fn compact(self) -> Result<CompactedJson, CompactionError> {
        let processed_context = crate::context::PROV
            .process::<JsonContext, _>(&mut NoLoader, None)
            .await
            .map_err(|e| CompactionError::JsonLd {
                inner: e.to_string(),
            })?;

        let output = self
            .0
            .compact_with(
                None,
                &processed_context,
                &mut NoLoader,
                json_ld::compaction::Options {
                    processing_mode: json_ld::ProcessingMode::JsonLd1_1,
                    compact_to_relative: true,
                    compact_arrays: true,
                    ordered: true,
                },
            )
            .await
            .map_err(|e| CompactionError::JsonLd {
                inner: e.to_string(),
            })?;

        Ok(CompactedJson(output))
    }

    pub async fn compact_stable_order(self) -> Result<Value, CompactionError> {
        let mut v: serde_json::Value = serde_json::from_str(&*self.compact().await?.0.to_string())?;

        // Sort @graph by //@id, as objects are unordered
        if let Some(v) = v.pointer_mut("/@graph").and_then(|p| p.as_array_mut()) {
            v.sort_by(|l, r| {
                let lid = l.get("@id").and_then(|o| o.as_str());

                let rid = r.get("@id").and_then(|o| o.as_str());
                lid.cmp(&rid)
            });
        }

        Ok(v)
    }
}
pub mod from_json_ld;

pub struct CompactedJson(pub JsonValue);

impl std::ops::Deref for CompactedJson {
    type Target = JsonValue;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Property testing of prov models created transactionally and round tripped via JSON / LD
#[cfg(test)]
pub mod proptest;
