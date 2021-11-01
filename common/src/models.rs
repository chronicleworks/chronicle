use async_std::task::block_on;
use chrono::{DateTime, Utc};
use iref::{AsIri, Iri};
use json::{object, JsonValue};
use json_ld::{context::Local, Document, JsonContext, NoLoader};
use multimap::MultiMap;
use std::collections::HashMap;
use uuid::Uuid;

use crate::vocab::{Chronicle, Prov};

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone)]
pub struct NamespaceId(String);

impl std::ops::Deref for NamespaceId {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl NamespaceId {
    /// Decompose a namespace id into its constituent parts, we need to preserve the type better to justify this implementation
    pub fn decompose(&self) -> (&str, Uuid) {
        if let &[_, _, _, name, uuid, ..] = &self.0.split(":").collect::<Vec<_>>()[..] {
            return (name, Uuid::parse_str(uuid).unwrap());
        }

        unreachable!();
    }
}

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

impl std::ops::Deref for EntityId {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

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

impl std::ops::Deref for AgentId {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AgentId {
    /// Extract the agent name from an id
    pub fn decompose(&self) -> &str {
        if let &[_, _, _, name, ..] = &self.0.split(":").collect::<Vec<_>>()[..] {
            return name;
        }

        unreachable!();
    }
}

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

impl std::ops::Deref for ActivityId {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ActivityId {
    /// Extract the activity name from an id
    pub fn decompose(&self) -> &str {
        if let &[_, _, _, name, ..] = &self.0.split(":").collect::<Vec<_>>()[..] {
            return name;
        }

        unreachable!();
    }
}

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
    pub name: String,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct StartActivity {
    pub namespace: NamespaceId,
    pub id: ActivityId,
    pub agent: AgentId,
    pub time: DateTime<Utc>,
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
    pub namespaceid: NamespaceId,
    pub name: String,
    pub started: Option<DateTime<Utc>>,
    pub ended: Option<DateTime<Utc>>,
}

impl Activity {
    pub fn new(id: ActivityId, ns: NamespaceId, name: &str) -> Self {
        Self {
            id,
            namespaceid: ns,
            name: name.to_owned(),
            started: None,
            ended: None,
        }
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
    pub was_associated_with: MultiMap<ActivityId, AgentId>,
    pub was_attributed_to: MultiMap<EntityId, AgentId>,
    pub was_generated_by: MultiMap<EntityId, ActivityId>,
    pub uses: MultiMap<ActivityId, EntityId>,
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

    fn namespace_context(&mut self, ns: &NamespaceId) {
        let (namespacename, uuid) = ns.decompose();

        self.namespaces.insert(
            ns.clone(),
            Namespace {
                id: ns.clone(),
                uuid: uuid,
                name: namespacename.to_owned(),
            },
        );
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
                self.namespace_context(&namespace);
                self.agents
                    .insert(id.clone(), Agent::new(id, namespace, name, None));
            }
            ChronicleTransaction::RegisterKey(RegisterKey {
                namespace,
                id,
                publickey,
                name,
            }) => {
                self.namespace_context(&namespace);

                if !self.agents.contains_key(&id) {
                    self.agents
                        .insert(id.clone(), Agent::new(id.clone(), namespace, name, None));
                }
                self.agents
                    .get_mut(&id)
                    .map(|x| x.publickey = Some(publickey));
            }
            ChronicleTransaction::CreateActivity(CreateActivity {
                namespace,
                id,
                name,
            }) => {
                self.namespace_context(&namespace);

                if !self.activities.contains_key(&id) {
                    self.activities
                        .insert(id.clone(), Activity::new(id, namespace, &name));
                }
            }
            ChronicleTransaction::StartActivity(StartActivity {
                namespace,
                id,
                agent,
                time,
            }) => {
                self.namespace_context(&namespace);
                if !self.activities.contains_key(&id) {
                    let activity_name = id.decompose();
                    let mut activity = Activity::new(id.clone(), namespace.clone(), activity_name);
                    activity.started = Some(time);
                    self.activities.insert(id.clone(), activity);
                }

                if !self.agents.contains_key(&agent) {
                    let agent_name = agent.decompose();
                    let agentmodel =
                        Agent::new(agent.clone(), namespace, agent_name.to_owned(), None);
                    self.agents.insert(agent.clone(), agentmodel);
                }

                self.was_associated_with
                    .entry(id)
                    .or_insert_vec(vec![])
                    .push(agent.clone())
            }
            ChronicleTransaction::ActivityUses(ActivityUses {
                namespace,
                id,
                entity,
            }) => {
                self.namespace_context(&namespace);
                if !self.activities.contains_key(&id) {
                    let activity_name = id.decompose();
                    self.activities.insert(
                        id.clone(),
                        Activity::new(id.clone(), namespace.clone(), activity_name),
                    );
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
                    let activity_name = activity.decompose();
                    self.activities.insert(
                        activity.clone(),
                        Activity::new(activity.clone(), namespace.clone(), activity_name),
                    );
                }
                if !self.entities.contains_key(&id) {
                    self.entities
                        .insert(id.clone(), Entity::new(id.clone(), namespace));
                }

                self.was_generated_by.insert(id, activity);
            }
        };
    }

    /// Write the model out as a JSON-LD document in expanded form
    pub fn to_json(&self) -> ExpandedJson {
        let mut doc = json::Array::new();

        for (id, ns) in self.namespaces.iter() {
            doc.push(object! {
                "@id": (*id.as_str()),
                "@type": Iri::from(Chronicle::NamespaceType).as_str(),
                "http://www.w3.org/2000/01/rdf-schema#label": [{
                    "@value": ns.name.as_str(),
                }]
            })
        }

        for (id, agent) in self.agents.iter() {
            let mut agentdoc = object! {
                "@id": (*id.as_str()),
                "@type": Iri::from(Prov::Agent).as_str(),
                "http://www.w3.org/2000/01/rdf-schema#label": [{
                   "@value": agent.name.as_str(),
                }]
            };
            agent.publickey.as_ref().map(|publickey| {
                let mut values = json::Array::new();

                values.push(object! {
                    "@value": JsonValue::String(publickey.to_owned()),
                });

                agentdoc
                    .insert(Iri::from(Chronicle::HasPublicKey).as_str(), values)
                    .ok();
            });

            let mut values = json::Array::new();

            values.push(object! {
                "@id": JsonValue::String(agent.namespaceid.0.clone()),
            });

            agentdoc
                .insert(Iri::from(Chronicle::HasNamespace).as_str(), values)
                .ok();

            doc.push(agentdoc);
        }

        for (id, activity) in self.activities.iter() {
            let mut activitydoc = object! {
                "@id": (*id.as_str()),
                "@type": Iri::from(Prov::Activity).as_str(),
                "http://www.w3.org/2000/01/rdf-schema#label": [{
                   "@value": activity.name.as_str(),
                }]
            };

            activity.started.map(|time| {
                let mut values = json::Array::new();
                values.push(object! {"@value": time.to_rfc3339()});

                activitydoc
                    .insert("http://www.w3.org/ns/prov#startedAtTime", values)
                    .ok();
            });

            self.was_associated_with.get_vec(&id).map(|asoc| {
                let mut ids = json::Array::new();

                for id in asoc.iter() {
                    ids.push(object! {"@id": id.as_str()});
                }

                activitydoc
                    .insert("http://www.w3.org/ns/prov#wasAssociatedWith", ids)
                    .ok();
            });

            let mut values = json::Array::new();

            values.push(object! {
                "@id": JsonValue::String(activity.namespaceid.0.clone()),
            });

            activitydoc
                .insert(Iri::from(Chronicle::HasNamespace).as_str(), values)
                .ok();

            doc.push(activitydoc);
        }

        ExpandedJson(doc.into())
    }
}

pub struct ExpandedJson(pub JsonValue);

impl ExpandedJson {
    pub fn compact(self) -> Result<CompactedJson, json_ld::Error> {
        let processed_context =
            block_on(crate::context::PROV.process::<JsonContext, _>(&mut NoLoader, None))?;

        // Compaction.
        let output = block_on(self.0.compact(&processed_context, &mut NoLoader))?;

        Ok(CompactedJson(output))
    }
}

pub struct CompactedJson(pub JsonValue);

impl std::ops::Deref for CompactedJson {
    type Target = JsonValue;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
