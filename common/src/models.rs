use iref::{AsIri};
use json::{object, JsonValue};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone)]
pub struct NamespaceId(String);

impl<S> From<S> for NamespaceId
where
    S: AsIri,
{
    fn from(iri: S) -> Self {
        Self(iri.as_iri().to_string())
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone)]
pub struct EntityId(String);

impl<S> From<S> for EntityId
where
    S: AsIri,
{
    fn from(iri: S) -> Self {
        Self(iri.as_iri().to_string())
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone)]
pub struct AgentId(String);

impl<S> From<S> for AgentId
where
    S: AsIri,
{
    fn from(iri: S) -> Self {
        Self(iri.as_iri().to_string())
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone)]
pub struct ActivityId(String);

impl<S> From<S> for ActivityId
where
    S: AsIri,
{
    fn from(iri: S) -> Self {
        Self(iri.as_iri().to_string())
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct CreateNamespace {
    pub id: NamespaceId,
    pub name: String,
    pub uuid: Uuid,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct CreateAgent {
    pub namespace: NamespaceId,
    pub name: String,
    pub id: AgentId,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct RegisterKey {
    pub namespace: NamespaceId,
    pub id: AgentId,
    pub publickey: String,
    pub name: String,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct CreateActivity {
    pub namespace: NamespaceId,
    pub id: ActivityId,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct StartActivity {
    pub namespace: NamespaceId,
    pub id: ActivityId,
    pub agent: AgentId,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct ActivityUses {
    pub namespace: NamespaceId,
    pub id: ActivityId,
    pub entity: EntityId,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct GenerateEntity {
    pub namespace: NamespaceId,
    pub id: EntityId,
    pub activity: ActivityId,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub enum ChronicleTransaction {
    CreateNamespace(CreateNamespace),
    CreateAgent(CreateAgent),
    RegisterKey(RegisterKey),
    CreateActivity(CreateActivity),
    StartActivity(StartActivity),
    ActivityUses(ActivityUses),
    GenerateEntity(GenerateEntity),
}

#[derive(Debug, Clone)]
pub struct Namespace {
    pub id: NamespaceId,
    pub uuid: Uuid,
    pub name: String,
}

impl Namespace {
    pub fn new(id: NamespaceId, uuid: Uuid, name: String) -> Self {
        Self { id, uuid, name }
    }
}

#[derive(Debug, Clone)]
pub struct Agent {
    pub id: AgentId,
    pub namespaceid: NamespaceId,
    pub name: String,
    pub publickey: Option<String>,
}

impl Agent {
    pub fn new(
        id: AgentId,
        namespaceid: NamespaceId,
        name: String,
        publickey: Option<String>,
    ) -> Self {
        Self {
            id,
            namespaceid,
            name,
            publickey,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Activity {
    pub id: ActivityId,
    pub ns: NamespaceId,
}

impl Activity {
    pub fn new(id: ActivityId, ns: NamespaceId) -> Self {
        Self { id, ns }
    }
}

#[derive(Debug, Clone)]
pub struct Entity {
    id: EntityId,
    ns: NamespaceId,
}

impl Entity {
    pub fn new(id: EntityId, ns: NamespaceId) -> Self {
        Self { id, ns }
    }
}

#[derive(Debug, Default)]
pub struct ProvModel {
    pub namespaces: HashMap<NamespaceId, Namespace>,
    pub agents: HashMap<AgentId, Agent>,
    pub activities: HashMap<ActivityId, Activity>,
    pub entities: HashMap<EntityId, Entity>,
    pub wasAssociatedWith: HashMap<ActivityId, AgentId>,
    pub wasAttributedTo: HashMap<EntityId, AgentId>,
    pub wasGeneratedBy: HashMap<EntityId, ActivityId>,
    pub uses: HashMap<ActivityId, EntityId>,
}

impl ProvModel {
    pub fn from_tx<'a, I>(tx: I) -> Self
    where
        I: IntoIterator<Item = &'a ChronicleTransaction>,
    {
        let mut model = Self::default();
        for tx in tx {
            model.apply(tx);
        }

        model
    }

    /// Transform a sequence of ChronicleTransaction events into a provenance model,
    /// If a statement requires a subject or object that does not currently exist in the model, then we create it
    pub fn apply(&mut self, tx: &ChronicleTransaction) {
        let tx = tx.to_owned();
        match tx {
            ChronicleTransaction::CreateNamespace(CreateNamespace { id, name, uuid }) => {
                self.namespaces
                    .insert(id.clone(), Namespace::new(id, uuid, name).into());
            }
            ChronicleTransaction::CreateAgent(CreateAgent {
                namespace,
                id,
                name,
            }) => {
                self.agents
                    .insert(id.clone(), Agent::new(id, namespace, name, None));
            }
            ChronicleTransaction::RegisterKey(RegisterKey {
                namespace,
                id,
                publickey,
                name,
            }) => {
                if !self.agents.contains_key(&id) {
                    self.agents
                        .insert(id.clone(), Agent::new(id.clone(), namespace, name, None));
                }
                self.agents
                    .get_mut(&id)
                    .map(|x| x.publickey = Some(publickey));
            }
            ChronicleTransaction::CreateActivity(CreateActivity { namespace, id }) => {
                if !self.activities.contains_key(&id) {
                    self.activities
                        .insert(id.clone(), Activity::new(id, namespace));
                }
            }
            ChronicleTransaction::StartActivity(StartActivity {
                namespace,
                id,
                agent,
            }) => {
                if !self.activities.contains_key(&id) {
                    self.activities
                        .insert(id.clone(), Activity::new(id.clone(), namespace));
                }

                self.wasAssociatedWith.insert(id, agent);
            }
            ChronicleTransaction::ActivityUses(ActivityUses {
                namespace,
                id,
                entity,
            }) => {
                if !self.activities.contains_key(&id) {
                    self.activities
                        .insert(id.clone(), Activity::new(id.clone(), namespace.clone()));
                }
                if !self.entities.contains_key(&entity) {
                    self.entities
                        .insert(entity.clone(), Entity::new(entity.clone(), namespace));
                }

                self.uses.insert(id, entity);
            }
            ChronicleTransaction::GenerateEntity(GenerateEntity {
                namespace,
                id,
                activity,
            }) => {
                if !self.activities.contains_key(&activity) {
                    self.activities.insert(
                        activity.clone(),
                        Activity::new(activity.clone(), namespace.clone()),
                    );
                }
                if !self.entities.contains_key(&id) {
                    self.entities
                        .insert(id.clone(), Entity::new(id.clone(), namespace));
                }

                self.wasGeneratedBy.insert(id, activity);
            }
        };
    }

    /// Write the model out as a JSON-LD document in expanded form
    pub fn to_json(&self) -> ExpandedJson {
        let doc = object! {};

        ExpandedJson(doc)
    }
}

pub struct ExpandedJson(JsonValue);
