use chrono::{DateTime, Utc};
use custom_error::custom_error;
use futures::TryFutureExt;
use iref::{AsIri, Iri, IriBuf};
use json::{object, JsonValue};
use json_ld::{
    context::Local, util::AsJson, Document, Indexed, JsonContext, NoLoader, Node, Reference,
};
use serde::Serialize;
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    convert::Infallible,
};
use tokio::task::JoinError;
use uuid::Uuid;

use super::{
    vocab::{Chronicle, Prov},
    ActivityId, AgentId, DomaintypeId, EntityId, NamespaceId,
};

custom_error! {pub ProcessorError
    Compaction{source: CompactionError} = "Json Ld Error",
    Expansion{inner: String} = "Json Ld Error",
    Tokio{source: JoinError} = "Tokio Error",
    MissingId{object: JsonValue} = "Missing @id",
    MissingProperty{iri: String, object: JsonValue} = "Missing property",
    NotANode{} = "Json LD object is not a node",
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
pub struct EndActivity {
    pub namespace: NamespaceId,
    pub id: ActivityId,
    pub agent: AgentId,
    pub time: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct ActivityUses {
    pub namespace: NamespaceId,
    pub id: EntityId,
    pub activity: ActivityId,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct GenerateEntity {
    pub namespace: NamespaceId,
    pub id: EntityId,
    pub activity: ActivityId,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct EntityAttach {
    pub namespace: NamespaceId,
    pub id: EntityId,
    pub agent: AgentId,
    pub signature: String,
    pub locator: Option<String>,
    pub signature_time: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub enum Domaintype {
    Entity {
        namespace: NamespaceId,
        id: EntityId,
        domaintype: Option<DomaintypeId>,
    },
    Agent {
        namespace: NamespaceId,
        id: AgentId,
        domaintype: Option<DomaintypeId>,
    },
    Activity {
        namespace: NamespaceId,
        id: ActivityId,
        domaintype: Option<DomaintypeId>,
    },
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub enum ChronicleTransaction {
    CreateNamespace(CreateNamespace),
    CreateAgent(CreateAgent),
    RegisterKey(RegisterKey),
    CreateActivity(CreateActivity),
    StartActivity(StartActivity),
    EndActivity(EndActivity),
    ActivityUses(ActivityUses),
    GenerateEntity(GenerateEntity),
    EntityAttach(EntityAttach),
    Domaintype(Domaintype),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Agent {
    pub id: AgentId,
    pub namespaceid: NamespaceId,
    pub name: String,
    pub domaintypeid: Option<DomaintypeId>,
    pub publickey: Option<String>,
}

impl Agent {
    pub fn new(
        id: AgentId,
        namespaceid: NamespaceId,
        name: String,
        publickey: Option<String>,
        domaintypeid: Option<DomaintypeId>,
    ) -> Self {
        Self {
            id,
            namespaceid,
            name,
            publickey,
            domaintypeid,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Activity {
    pub id: ActivityId,
    pub namespaceid: NamespaceId,
    pub name: String,
    pub domaintypeid: Option<DomaintypeId>,
    pub started: Option<DateTime<Utc>>,
    pub ended: Option<DateTime<Utc>>,
}

impl Activity {
    pub fn new(
        id: ActivityId,
        ns: NamespaceId,
        name: &str,
        domaintypeid: Option<DomaintypeId>,
    ) -> Self {
        Self {
            id,
            namespaceid: ns,
            name: name.to_owned(),
            started: None,
            ended: None,
            domaintypeid,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Entity {
    Unsigned {
        id: EntityId,
        namespaceid: NamespaceId,
        name: String,
        domaintypeid: Option<DomaintypeId>,
    },
    Signed {
        id: EntityId,
        namespaceid: NamespaceId,
        name: String,
        domaintypeid: Option<DomaintypeId>,
        signature: String,
        locator: Option<String>,
        signature_time: DateTime<Utc>,
    },
}

impl Entity {
    pub fn unsigned(
        id: EntityId,
        namespaceid: &NamespaceId,
        name: &str,
        domaintypeid: Option<DomaintypeId>,
    ) -> Self {
        Self::Unsigned {
            id,
            namespaceid: namespaceid.to_owned(),
            name: name.to_owned(),
            domaintypeid,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Unsigned { name, .. } | Self::Signed { name, .. } => name,
        }
    }

    pub fn id(&self) -> &EntityId {
        match self {
            Self::Unsigned { id, .. } | Self::Signed { id, .. } => id,
        }
    }

    pub fn namespaceid(&self) -> &NamespaceId {
        match self {
            Self::Unsigned { namespaceid, .. } | Self::Signed { namespaceid, .. } => namespaceid,
        }
    }

    pub fn domaintypeid(&self) -> &Option<DomaintypeId> {
        match self {
            Self::Unsigned { domaintypeid, .. } | Self::Signed { domaintypeid, .. } => domaintypeid,
        }
    }

    pub fn set_domaintypeid(&mut self, newtypeid: Option<DomaintypeId>) {
        match self {
            Self::Unsigned { domaintypeid, .. } | Self::Signed { domaintypeid, .. } => {
                *domaintypeid = newtypeid
            }
        }
    }

    pub fn sign(
        self,
        signature: String,
        locator: Option<String>,
        signature_time: DateTime<Utc>,
    ) -> Self {
        match self {
            Self::Unsigned {
                id,
                namespaceid,
                name,
                domaintypeid,
                ..
            }
            | Self::Signed {
                id,
                namespaceid,
                name,
                domaintypeid,
                ..
            } => Self::Signed {
                id,
                namespaceid,
                name,
                signature,
                locator,
                signature_time,
                domaintypeid,
            },
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvModel {
    pub namespaces: HashMap<NamespaceId, Namespace>,
    pub agents: HashMap<(NamespaceId, AgentId), Agent>,
    pub activities: HashMap<(NamespaceId, ActivityId), Activity>,
    pub entities: HashMap<(NamespaceId, EntityId), Entity>,
    pub was_associated_with: HashMap<(NamespaceId, ActivityId), HashSet<(NamespaceId, AgentId)>>,
    pub was_attributed_to: HashMap<(NamespaceId, EntityId), HashSet<(NamespaceId, AgentId)>>,
    pub was_generated_by: HashMap<(NamespaceId, EntityId), HashSet<(NamespaceId, ActivityId)>>,
    pub used: HashMap<(NamespaceId, ActivityId), HashSet<(NamespaceId, EntityId)>>,
}

impl ProvModel {
    /// Apply a sequence of `ChronicleTransaction` to an empty model, then return it
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

        for (id, links) in other.was_associated_with {
            self.was_associated_with
                .entry(id.clone())
                .and_modify(|map| {
                    for link in links {
                        map.insert(link);
                    }
                });
        }
        for (id, links) in other.was_attributed_to {
            self.was_attributed_to
                .entry(id.clone())
                .and_modify(|map| {
                    for link in links.clone() {
                        map.insert(link);
                    }
                })
                .or_insert(links);
        }
        for (id, links) in other.was_generated_by {
            self.was_generated_by
                .entry(id.clone())
                .and_modify(|map| {
                    for link in links.clone() {
                        map.insert(link);
                    }
                })
                .or_insert(links);
        }
        for (id, links) in other.used {
            self.used
                .entry(id.clone())
                .and_modify(|map| {
                    for link in links.clone() {
                        map.insert(link);
                    }
                })
                .or_insert(links);
        }
    }

    pub fn associate_with(
        &mut self,
        namespace: &NamespaceId,
        activity: &ActivityId,
        agent: &AgentId,
    ) {
        self.was_associated_with
            .entry((namespace.clone(), activity.clone()))
            .or_insert_with(HashSet::new)
            .insert((namespace.to_owned(), agent.clone()));
    }

    pub fn generate_by(&mut self, namespace: NamespaceId, entity: EntityId, activity: &ActivityId) {
        self.was_generated_by
            .entry((namespace.clone(), entity))
            .or_insert_with(HashSet::new)
            .insert((namespace, activity.clone()));
    }

    pub fn used(&mut self, namespace: NamespaceId, activity: ActivityId, entity: &EntityId) {
        self.used
            .entry((namespace.clone(), activity))
            .or_insert_with(HashSet::new)
            .insert((namespace, entity.clone()));
    }

    pub fn namespace_context(&mut self, ns: &NamespaceId) {
        let (namespacename, uuid) = ns.decompose();

        self.namespaces.insert(
            ns.clone(),
            Namespace {
                id: ns.clone(),
                uuid,
                name: namespacename.to_owned(),
            },
        );
    }

    /// Transform a sequence of ChronicleTransaction events into a provenance model,
    /// If a statement requires a subject or object that does not currently exist in the model, then we create it
    pub fn apply(&mut self, tx: &ChronicleTransaction) {
        let tx = tx.to_owned();
        match tx {
            ChronicleTransaction::CreateNamespace(CreateNamespace {
                id,
                name: _,
                uuid: _,
            }) => {
                self.namespace_context(&id);
            }
            ChronicleTransaction::CreateAgent(CreateAgent {
                namespace,
                id,
                name,
            }) => {
                self.namespace_context(&namespace);
                self.agents.insert(
                    (namespace.clone(), id.clone()),
                    Agent::new(id, namespace, name, None, None),
                );
            }
            ChronicleTransaction::RegisterKey(RegisterKey {
                namespace,
                id,
                publickey,
                name,
            }) => {
                self.namespace_context(&namespace);

                self.agents
                    .entry((namespace.clone(), id.clone()))
                    .or_insert_with(|| Agent::new(id.clone(), namespace.clone(), name, None, None));
                self.agents
                    .get_mut(&(namespace.clone(), id))
                    .map(|x| x.publickey = Some(publickey));
            }
            ChronicleTransaction::CreateActivity(CreateActivity {
                namespace,
                id,
                name,
            }) => {
                self.namespace_context(&namespace);

                self.activities
                    .entry((namespace.clone(), id.clone()))
                    .or_insert_with(|| Activity::new(id, namespace, &name, None));
            }
            ChronicleTransaction::StartActivity(StartActivity {
                namespace,
                id,
                agent,
                time,
            }) => {
                self.namespace_context(&namespace);

                self.agents
                    .entry((namespace.clone(), agent.clone()))
                    .or_insert(Agent::new(
                        agent.clone(),
                        namespace.clone(),
                        agent.decompose().to_owned(),
                        None,
                        None,
                    ));

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
                        let mut activity =
                            Activity::new(id.clone(), namespace.clone(), id.decompose(), None);
                        activity.started = Some(time);
                        activity
                    });

                self.associate_with(&namespace, &id, &agent);
            }
            ChronicleTransaction::EndActivity(EndActivity {
                namespace,
                id,
                agent,
                time,
            }) => {
                self.namespace_context(&namespace);

                self.agents
                    .entry((namespace.clone(), agent.clone()))
                    .or_insert_with(|| {
                        Agent::new(
                            agent.clone(),
                            namespace.clone(),
                            agent.decompose().to_owned(),
                            None,
                            None,
                        )
                    });

                // Set our end data, and also the start date if this is a new resource, or the existing resource does not specify a start time
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
                        let mut activity =
                            Activity::new(id.clone(), namespace.clone(), id.decompose(), None);
                        activity.ended = Some(time);
                        activity.started = Some(time);
                        activity
                    });

                self.associate_with(&namespace, &id, &agent);
            }
            ChronicleTransaction::ActivityUses(ActivityUses {
                namespace,
                id,
                activity,
            }) => {
                self.namespace_context(&namespace);
                if !self
                    .activities
                    .contains_key(&(namespace.clone(), activity.clone()))
                {
                    let activity_name = activity.decompose();
                    self.add_activity(Activity::new(
                        activity.clone(),
                        namespace.clone(),
                        activity_name,
                        None,
                    ));
                }

                if !self.entities.contains_key(&(namespace.clone(), id.clone())) {
                    let name = id.decompose();

                    self.add_entity(Entity::unsigned(id.clone(), &namespace, name, None));
                }

                self.used(namespace, activity, &id);
            }
            ChronicleTransaction::GenerateEntity(GenerateEntity {
                namespace,
                id,
                activity,
            }) => {
                self.namespace_context(&namespace);
                if !self
                    .activities
                    .contains_key(&(namespace.clone(), activity.clone()))
                {
                    let activity_name = activity.decompose();
                    self.add_activity(Activity::new(
                        activity.clone(),
                        namespace.clone(),
                        activity_name,
                        None,
                    ));
                }
                if !self.entities.contains_key(&(namespace.clone(), id.clone())) {
                    let name = id.decompose();
                    self.add_entity(Entity::unsigned(id.clone(), &namespace, name, None));
                }

                self.generate_by(namespace, id, &activity)
            }
            ChronicleTransaction::EntityAttach(EntityAttach {
                namespace,
                id,
                agent,
                signature,
                locator,
                signature_time,
            }) => {
                self.namespace_context(&namespace);

                if !self.entities.contains_key(&(namespace.clone(), id.clone())) {
                    let name = id.decompose();
                    self.add_entity(Entity::unsigned(id.clone(), &namespace, name, None));
                }

                if !self
                    .agents
                    .contains_key(&(namespace.clone(), agent.clone()))
                {
                    let agent_name = agent.decompose();
                    self.add_agent(Agent::new(
                        agent.clone(),
                        namespace.clone(),
                        agent_name.to_owned(),
                        None,
                        None,
                    ));
                }

                let unsigned = self.entities.remove(&(namespace.clone(), id)).unwrap();

                self.add_entity(unsigned.sign(signature, locator, signature_time));
            }
            ChronicleTransaction::Domaintype(Domaintype::Entity {
                namespace,
                id,
                domaintype,
            }) => {
                self.namespace_context(&namespace);

                self.entities
                    .entry((namespace.clone(), id.clone()))
                    .and_modify(|entity| entity.set_domaintypeid(domaintype.clone()))
                    .or_insert(Entity::unsigned(
                        id.clone(),
                        &namespace,
                        id.decompose(),
                        domaintype,
                    ));
            }
            ChronicleTransaction::Domaintype(Domaintype::Activity {
                namespace,
                id,
                domaintype,
            }) => {
                self.namespace_context(&namespace);

                self.activities
                    .entry((namespace.clone(), id.clone()))
                    .and_modify(|mut acitvity| {
                        acitvity.domaintypeid = domaintype.clone();
                    })
                    .or_insert(Activity::new(
                        id.clone(),
                        namespace,
                        id.decompose(),
                        domaintype,
                    ));
            }
            ChronicleTransaction::Domaintype(Domaintype::Agent {
                namespace,
                id,
                domaintype,
            }) => {
                self.namespace_context(&namespace);

                self.agents
                    .entry((namespace.clone(), id.clone()))
                    .and_modify(|mut agent| {
                        agent.domaintypeid = domaintype.clone();
                    })
                    .or_insert(Agent::new(
                        id.clone(),
                        namespace,
                        id.decompose().to_string(),
                        None,
                        domaintype,
                    ));
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

        for ((_, id), agent) in self.agents.iter() {
            let mut typ = vec![];
            typ.push(Iri::from(Prov::Agent).to_string());
            if let Some(x) = agent.domaintypeid.as_ref() {
                typ.push(x.to_string())
            }

            let mut agentdoc = object! {
                "@id": (*id.as_str()),
                "@type": typ,
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
                "@id": JsonValue::String(agent.namespaceid.to_string()),
            });

            agentdoc
                .insert(Iri::from(Chronicle::HasNamespace).as_str(), values)
                .ok();

            doc.push(agentdoc);
        }

        for ((namespace, id), activity) in self.activities.iter() {
            let mut typ = vec![];
            typ.push(Iri::from(Prov::Activity).to_string());
            if let Some(x) = activity.domaintypeid.as_ref() {
                typ.push(x.to_string())
            }

            let mut activitydoc = object! {
                "@id": (*id.as_str()),
                "@type": typ,
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

            activity.ended.map(|time| {
                let mut values = json::Array::new();
                values.push(object! {"@value": time.to_rfc3339()});

                activitydoc
                    .insert("http://www.w3.org/ns/prov#endedAtTime", values)
                    .ok();
            });

            self.was_associated_with
                .get(&(namespace.to_owned(), id.to_owned()))
                .map(|asoc| {
                    let mut ids = json::Array::new();

                    for (_, id) in asoc.iter() {
                        ids.push(object! {"@id": id.as_str()});
                    }

                    activitydoc
                        .insert(&Iri::from(Prov::WasAssociatedWith).to_string(), ids)
                        .ok();
                });

            self.used
                .get(&(namespace.to_owned(), id.to_owned()))
                .map(|asoc| {
                    let mut ids = json::Array::new();

                    for (_, id) in asoc.iter() {
                        ids.push(object! {"@id": id.as_str()});
                    }

                    activitydoc
                        .insert(&Iri::from(Prov::Used).to_string(), ids)
                        .ok();
                });

            let mut values = json::Array::new();

            values.push(object! {
                "@id": JsonValue::String(activity.namespaceid.to_string()),
            });

            activitydoc
                .insert(Iri::from(Chronicle::HasNamespace).as_str(), values)
                .ok();

            doc.push(activitydoc);
        }

        for ((namespace, id), entity) in self.entities.iter() {
            let mut typ = vec![Iri::from(Prov::Entity).to_string()];
            if let Some(x) = entity.domaintypeid().as_ref() {
                typ.push(x.to_string())
            }

            let mut entitydoc = object! {
                "@id": (*id.as_str()),
                "@type": typ,
                "http://www.w3.org/2000/01/rdf-schema#label": [{
                   "@value": entity.name()
                }]
            };

            self.was_generated_by
                .get(&(namespace.to_owned(), id.to_owned()))
                .map(|asoc| {
                    let mut ids = json::Array::new();

                    for (_, id) in asoc.iter() {
                        ids.push(object! {"@id": id.as_str()});
                    }

                    entitydoc
                        .insert(Iri::from(Prov::WasGeneratedBy).as_str(), ids)
                        .ok();
                });

            if let Entity::Signed {
                signature,
                signature_time,
                locator,
                ..
            } = entity
            {
                entitydoc
                    .insert(
                        Iri::from(Chronicle::Signature).as_str(),
                        signature.to_owned(),
                    )
                    .ok();

                entitydoc
                    .insert(
                        Iri::from(Chronicle::SignedAtTime).as_str(),
                        signature_time.to_rfc3339(),
                    )
                    .ok();

                if let Some(locator) = locator.as_ref() {
                    entitydoc
                        .insert(Iri::from(Chronicle::Locator).as_str(), locator.to_owned())
                        .ok();
                }
            }

            let mut values = json::Array::new();

            values.push(object! {
                "@id": JsonValue::String(entity.namespaceid().to_string()),
            });

            entitydoc
                .insert(Iri::from(Chronicle::HasNamespace).as_str(), values)
                .ok();

            doc.push(entitydoc);
        }

        ExpandedJson(doc.into())
    }

    pub async fn apply_json_ld_bytes(self, buf: &[u8]) -> Result<Self, ProcessorError> {
        self.apply_json_ld(json::parse(std::str::from_utf8(buf)?)?)
            .await
    }

    /// Take a Json-Ld input document, assuming it is in compact form, expand it and apply the state to the prov model
    /// Replace @context with our resource context
    /// We rely on reified @types, so subclassing must also include supertypes
    pub async fn apply_json_ld(mut self, mut json: JsonValue) -> Result<Self, ProcessorError> {
        json.remove("@context");
        json.insert("@context", crate::context::PROV.clone()).ok();

        let output = json
            .expand::<JsonContext, _>(&mut NoLoader)
            .map_err(|e| ProcessorError::Expansion {
                inner: e.to_string(),
            })
            .await?;

        for o in output {
            let o = o
                .try_cast::<Node>()
                .map_err(|_| ProcessorError::NotANode {})?
                .into_inner();
            if o.has_type(&Reference::Id(Chronicle::NamespaceType.as_iri().into())) {
                self.apply_node_as_namespace(&o)?;
            }
            if o.has_type(&Reference::Id(Prov::Agent.as_iri().into())) {
                self.apply_node_as_agent(&o)?;
            } else if o.has_type(&Reference::Id(Prov::Activity.as_iri().into())) {
                self.apply_node_as_activity(&o)?;
            } else if o.has_type(&Reference::Id(Prov::Entity.as_iri().into())) {
                self.apply_node_as_entity(&o)?;
            }
        }

        Ok(self)
    }

    /// Extract the types and find the first that is not prov::, as we currently only alow zero or one domain types
    /// this should be sufficient
    fn extract_domain_type(node: &Node) -> Result<Option<DomaintypeId>, ProcessorError> {
        Ok(node
            .types()
            .iter()
            .filter_map(|x| x.as_iri())
            .filter(|x| x.as_str().contains("domaintype"))
            .map(|x| x.into())
            .next())
    }

    fn apply_node_as_namespace(&mut self, ns: &Node) -> Result<(), ProcessorError> {
        let ns = ns.id().ok_or_else(|| ProcessorError::MissingId {
            object: ns.as_json(),
        })?;

        self.namespace_context(&NamespaceId::new(ns.as_str()));

        Ok(())
    }

    fn apply_node_as_agent(&mut self, agent: &Node) -> Result<(), ProcessorError> {
        let id = AgentId::new(
            agent
                .id()
                .ok_or_else(|| ProcessorError::MissingId {
                    object: agent.as_json(),
                })?
                .to_string(),
        );

        let namespaceid = extract_namespace(agent)?;
        self.namespace_context(&namespaceid);
        let name = id.decompose().to_owned();

        let publickey = extract_scalar_prop(&Chronicle::HasPublicKey, agent)
            .ok()
            .and_then(|x| x.as_str().map(|x| x.to_string()));

        let domaintypeid = Self::extract_domain_type(agent)?;

        let agent = Agent::new(id, namespaceid, name, publickey, domaintypeid);

        self.add_agent(agent);

        Ok(())
    }

    fn apply_node_as_activity(&mut self, activity: &Node) -> Result<(), ProcessorError> {
        let id = ActivityId::new(
            activity
                .id()
                .ok_or_else(|| ProcessorError::MissingId {
                    object: activity.as_json(),
                })?
                .to_string(),
        );

        let namespaceid = extract_namespace(activity)?;
        self.namespace_context(&namespaceid);
        let name = id.decompose().to_owned();

        let started = extract_scalar_prop(&Prov::StartedAtTime, activity)
            .ok()
            .and_then(|x| x.as_str().map(DateTime::parse_from_rfc3339));

        let ended = extract_scalar_prop(&Prov::EndedAtTime, activity)
            .ok()
            .and_then(|x| x.as_str().map(DateTime::parse_from_rfc3339));

        let used = extract_reference_ids(&Prov::Used, activity)?
            .into_iter()
            .map(|id| EntityId::new(id.as_str()));

        let wasassociatedwith = extract_reference_ids(&Prov::WasAssociatedWith, activity)?
            .into_iter()
            .map(|id| AgentId::new(id.as_str()));

        let domaintypeid = Self::extract_domain_type(activity)?;

        let mut activity = Activity::new(id, namespaceid.clone(), &name, domaintypeid);

        if let Some(started) = started {
            activity.started = Some(DateTime::<Utc>::from(started?));
        }

        if let Some(ended) = ended {
            activity.ended = Some(DateTime::<Utc>::from(ended?));
        }

        for entity in used {
            self.used(namespaceid.clone(), activity.id.to_owned(), &entity);
        }

        for agent in wasassociatedwith {
            self.associate_with(&namespaceid, &activity.id, &agent);
        }

        self.add_activity(activity);

        Ok(())
    }

    fn apply_node_as_entity(&mut self, entity: &Node) -> Result<(), ProcessorError> {
        let id = EntityId::new(
            entity
                .id()
                .ok_or_else(|| ProcessorError::MissingId {
                    object: entity.as_json(),
                })?
                .to_string(),
        );

        let namespaceid = extract_namespace(entity)?;
        self.namespace_context(&namespaceid);
        let name = id.decompose().to_owned();

        let signature = extract_scalar_prop(&Chronicle::Signature, entity)
            .ok()
            .and_then(|x| x.as_str());

        let signature_time = extract_scalar_prop(&Chronicle::SignedAtTime, entity)
            .ok()
            .and_then(|x| x.as_str().map(DateTime::parse_from_rfc3339));

        let locator = extract_scalar_prop(&Chronicle::Locator, entity)
            .ok()
            .and_then(|x| x.as_str());

        let generatedby = extract_reference_ids(&Prov::WasGeneratedBy, entity)?
            .into_iter()
            .map(|id| ActivityId::new(id.as_str()));

        let domaintypeid = Self::extract_domain_type(entity)?;

        let entity = {
            if let (Some(signature), Some(signature_time)) = (signature, signature_time) {
                Entity::Signed {
                    name,
                    namespaceid: namespaceid.clone(),
                    id,
                    signature: signature.to_owned(),
                    locator: locator.map(|x| x.to_owned()),
                    signature_time: DateTime::<Utc>::from(signature_time?),
                    domaintypeid,
                }
            } else {
                Entity::Unsigned {
                    name,
                    namespaceid: namespaceid.clone(),
                    id,
                    domaintypeid,
                }
            }
        };
        for activity in generatedby {
            self.generate_by(namespaceid.clone(), entity.id().clone(), &activity);
        }

        self.add_entity(entity);

        Ok(())
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
            .insert((entity.namespaceid().clone(), entity.id().clone()), entity);
    }
}

fn extract_reference_ids(iri: &dyn AsIri, node: &Node) -> Result<Vec<IriBuf>, ProcessorError> {
    let ids: Result<Vec<_>, _> = node
        .get(&Reference::Id(iri.as_iri().into()))
        .map(|o| {
            o.id().ok_or_else(|| ProcessorError::MissingId {
                object: node.as_json(),
            })
        })
        .map(|id| {
            id.and_then(|id| {
                id.as_iri().ok_or_else(|| ProcessorError::MissingId {
                    object: node.as_json(),
                })
            })
        })
        .map(|id| id.map(|id| id.to_owned()))
        .collect();

    ids
}

fn extract_scalar_prop<'a>(
    iri: &dyn AsIri,
    node: &'a Node,
) -> Result<&'a Indexed<json_ld::object::Object>, ProcessorError> {
    node.get_any(&Reference::Id(iri.as_iri().into()))
        .ok_or_else(|| ProcessorError::MissingProperty {
            iri: iri.as_iri().as_str().to_string(),
            object: node.as_json(),
        })
}

fn extract_namespace(agent: &Node) -> Result<NamespaceId, ProcessorError> {
    Ok(NamespaceId::new(
        extract_scalar_prop(&Chronicle::HasNamespace, agent)?
            .id()
            .ok_or(ProcessorError::MissingId {
                object: agent.as_json(),
            })?
            .to_string(),
    ))
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
            .compact(&processed_context, &mut NoLoader)
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

pub struct CompactedJson(pub JsonValue);

impl std::ops::Deref for CompactedJson {
    type Target = JsonValue;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Property testing of prov models created transactionally and round tripped via JSON / LD
#[cfg(test)]
pub mod test {
    use chrono::Utc;
    use json::JsonValue;
    use proptest::prelude::*;

    use uuid::Uuid;

    use crate::prov::{
        vocab::Chronicle, ChronicleTransaction, CreateActivity, CreateAgent, CreateNamespace,
        Domaintype, DomaintypeId, EndActivity, GenerateEntity, ProvModel, RegisterKey,
    };

    use super::{ActivityUses, CompactedJson, EntityAttach, NamespaceId, StartActivity};

    prop_compose! {
        fn a_name()(name in "[-A-Za-z0-9+]+") -> String {
            name
        }
    }

    // Choose from a limited selection of names so that we get multiple references
    prop_compose! {
        fn name()(names in prop::collection::vec(a_name(), 5), index in (0..5usize)) -> String {
            names.get(index).unwrap().to_owned()
        }
    }

    // Choose from a limited selection of domain types
    prop_compose! {
        fn domain_type_id()(names in prop::collection::vec(a_name(), 5), index in (0..5usize)) -> DomaintypeId {
            Chronicle::domaintype(names.get(index).unwrap()).into()
        }
    }

    prop_compose! {
        fn a_namespace()
            (uuid in prop::collection::vec(0..255u8, 16),
             name in name()) -> NamespaceId {
            Chronicle::namespace(&name,&Uuid::from_bytes(uuid.as_slice().try_into().unwrap())).into()
        }
    }

    // Choose from a limited selection of namespaces so that we get multiple references
    prop_compose! {
        fn namespace()(namespaces in prop::collection::vec(a_namespace(), 2), index in (0..2usize)) -> NamespaceId {
            namespaces.get(index).unwrap().to_owned()
        }
    }

    prop_compose! {
        fn create_namespace()(id in namespace()) -> CreateNamespace {
            let (name,uuid) = id.decompose();
            CreateNamespace {
                id: id.clone(),
                uuid,
                name: name.to_owned(),
            }
        }
    }

    prop_compose! {
        fn create_agent() (name in name(),namespace in namespace()) -> CreateAgent {
            let id = Chronicle::agent(&name).into();
            CreateAgent {
                namespace,
                name,
                id,
            }
        }
    }

    prop_compose! {
        fn register_key() (name in name(),namespace in namespace(), publickey in "[0-9a-f]{64}") -> RegisterKey {
            let id = Chronicle::agent(&name).into();
            RegisterKey {
                namespace,
                name,
                id,
                publickey
            }
        }
    }

    prop_compose! {
        fn create_activity() (name in name(),namespace in namespace()) -> CreateActivity {
            let id = Chronicle::activity(&name).into();
            CreateActivity {
                namespace,
                name,
                id,
            }
        }
    }

    // Create times for start between 2-1 years in the past, to ensure start <= end
    prop_compose! {
        fn start_activity() (name in name(),namespace in namespace(), offset in (0..10)) -> StartActivity {
            let id = Chronicle::activity(&name).into();

            let today = Utc::today().and_hms_micro(0, 0,0,0);

            StartActivity {
                namespace,
                agent: Chronicle::agent(&name).into(),
                id,
                time: today - chrono::Duration::days(offset as _)
            }
        }
    }

    // Create times for start between 2-1 years in the past, to ensure start <= end
    prop_compose! {
        fn end_activity() (name in name(),namespace in namespace(), offset in (0..10)) -> EndActivity {
            let id = Chronicle::activity(&name).into();

            let today = Utc::today().and_hms_micro(0, 0,0,0);

            EndActivity {
                namespace,
                agent: Chronicle::agent(&name).into(),
                id,
                time: today - chrono::Duration::days(offset as _)
            }
        }
    }

    prop_compose! {
        fn activity_uses() (activity_name in name(), entity_name in name(),namespace in namespace()) -> ActivityUses {
            let activity = Chronicle::activity(&activity_name).into();
            let id = Chronicle::entity(&entity_name).into();


            ActivityUses {
                namespace,
                id,
                activity
            }
        }
    }

    prop_compose! {
        fn generate_entity() (activity_name in name(), entity_name in name(),namespace in namespace()) -> GenerateEntity {
            let activity = Chronicle::activity(&activity_name).into();
            let id = Chronicle::entity(&entity_name).into();


            GenerateEntity {
                namespace,
                id,
                activity
            }
        }
    }

    prop_compose! {
        fn entity_attach() (
            offset in (0..10u32),
            signature in "[0-9a-f]{64}",
            locator in proptest::option::of(any::<String>()),
            agent_name in name(),
            name in name(),
            namespace in namespace()) -> EntityAttach {
            let id = Chronicle::entity(&name).into();
            let agent = Chronicle::agent(&agent_name).into();

            let signature_time = Utc::today().and_hms_micro(offset, 0,0,0);

            EntityAttach {
                namespace,
                id,
                locator,
                agent,
                signature,
                signature_time
            }
        }
    }

    prop_compose! {
        fn set_domain_type() (name in name(), namespace in namespace()) -> impl Strategy<Value = Domaintype> {
            let entityname = name.clone();
            let entityns = namespace.clone();

            let activityname = name.clone();
            let activityns = namespace.clone();

            let agentname = name;
            let agentns = namespace;

            prop_oneof![
                domain_type_id().prop_map(move |domaintypeid| Domaintype::Entity{
                    id: Chronicle::entity(&entityname).into(),
                    domaintype: Some(domaintypeid),
                    namespace:  entityns.clone(),
                }),
                domain_type_id().prop_map(move |domaintypeid| Domaintype::Activity{
                    id: Chronicle::activity(&activityname.clone()).into(),
                    domaintype: Some(domaintypeid),
                    namespace: activityns.clone(),
                }),
                domain_type_id().prop_map(move |domaintypeid| Domaintype::Agent{
                    id: Chronicle::agent(&agentname.clone()).into(),
                    domaintype: Some(domaintypeid),
                    namespace: agentns.clone()
                })
            ]
        }
    }

    fn transaction() -> impl Strategy<Value = ChronicleTransaction> {
        prop_oneof![
            1 => create_namespace().prop_map(ChronicleTransaction::CreateNamespace),
            2 => create_agent().prop_map(ChronicleTransaction::CreateAgent),
            2 => register_key().prop_map(ChronicleTransaction::RegisterKey),
            4 => create_activity().prop_map(ChronicleTransaction::CreateActivity),
            4 => start_activity().prop_map(ChronicleTransaction::StartActivity),
            4 => end_activity().prop_map(ChronicleTransaction::EndActivity),
            4 => activity_uses().prop_map(ChronicleTransaction::ActivityUses),
            4 => generate_entity().prop_map(ChronicleTransaction::GenerateEntity),
            2 => entity_attach().prop_map(ChronicleTransaction::EntityAttach),
            2 => set_domain_type().prop_flat_map(|x| x.prop_map(ChronicleTransaction::Domaintype)),
        ]
    }

    fn transaction_seq() -> impl Strategy<Value = Vec<ChronicleTransaction>> {
        proptest::collection::vec(transaction(), 1..50)
    }

    fn compact_json(prov: &ProvModel) -> CompactedJson {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async move { prov.to_json().compact().await })
            .unwrap()
    }

    fn prov_from_json_ld(json: JsonValue) -> ProvModel {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async move {
            let prov = ProvModel::default();
            prov.apply_json_ld(json).await.unwrap()
        })
    }

    proptest! {
       #![proptest_config(ProptestConfig {
            max_shrink_iters: std::u32::MAX, verbose: 0, .. ProptestConfig::default()
        })]
        #[test]
        fn test_transactions(tx in transaction_seq()) {
            let mut prov = ProvModel::default();

            // Apply each transaction in order
            for tx in tx.iter() {
                prov.apply(tx);
            }

            // Key registration overwrites public key, so we only assert the last one
            let mut regkey_assertion:  Box<dyn FnOnce()->Result<(), TestCaseError>> = Box::new(|| {Ok(())});

            // Now assert the final prov object matches what we would expect from the input transactions
            for tx in tx.iter() {
                match tx {
                    ChronicleTransaction::CreateNamespace(CreateNamespace{id,name,uuid}) => {
                        prop_assert!(prov.namespaces.contains_key(id));
                        let ns = prov.namespaces.get(id).unwrap();
                        prop_assert_eq!(&ns.id, id);
                        prop_assert_eq!(&ns.name, name);
                        prop_assert_eq!(&ns.uuid, uuid);
                    },
                    ChronicleTransaction::CreateAgent(
                        CreateAgent { namespace, name, id }) => {
                        let agent = &prov.agents.get(&(namespace.to_owned(),id.to_owned()));
                        prop_assert!(agent.is_some());
                        let agent = agent.unwrap();
                        prop_assert_eq!(&agent.name, name);
                        prop_assert_eq!(&agent.namespaceid, namespace);
                    },
                    ChronicleTransaction::RegisterKey(
                        RegisterKey { namespace, name, id, publickey}) => {
                            regkey_assertion = Box::new(|| {
                                let agent = &prov.agents.get(&(namespace.clone(),id.clone()));
                                prop_assert!(agent.is_some());
                                let agent = agent.unwrap();
                                prop_assert_eq!(&agent.name, &name.clone());
                                prop_assert_eq!(&agent.namespaceid, &namespace.clone());
                                prop_assert!(agent.publickey.is_some());
                                prop_assert_eq!(&agent.publickey.clone().unwrap(), &publickey.clone());
                                Ok(())
                            })
                        },
                    ChronicleTransaction::CreateActivity(
                        CreateActivity { namespace, id, name }) => {
                        let activity = &prov.activities.get(&(namespace.clone(),id.clone()));
                        prop_assert!(activity.is_some());
                        let activity = activity.unwrap();
                        prop_assert_eq!(&activity.name, name);
                        prop_assert_eq!(&activity.namespaceid, namespace);
                    },
                    ChronicleTransaction::StartActivity(
                        StartActivity { namespace, id, agent, time }) =>  {
                        let activity = &prov.activities.get(&(namespace.clone(),id.clone()));
                        prop_assert!(activity.is_some());
                        let activity = activity.unwrap();
                        prop_assert_eq!(&activity.name, id.decompose());
                        prop_assert_eq!(&activity.namespaceid, namespace);

                        prop_assert!(activity.started == Some(time.to_owned()));
                        prop_assert!(activity.ended.is_none() || activity.ended.unwrap() >= activity.started.unwrap());

                        prop_assert!(prov.was_associated_with.get(
                            &(namespace.to_owned(),id.to_owned()))
                            .unwrap()
                            .contains(&(namespace.to_owned(),agent.to_owned())));
                    },
                    ChronicleTransaction::EndActivity(
                        EndActivity { namespace, id, agent, time }) => {
                        let activity = &prov.activities.get(&(namespace.to_owned(),id.to_owned()));
                        prop_assert!(activity.is_some());
                        let activity = activity.unwrap();
                        prop_assert_eq!(&activity.name, id.decompose());
                        prop_assert_eq!(&activity.namespaceid, namespace);

                        prop_assert!(activity.ended == Some(time.to_owned()));
                        prop_assert!(activity.started.unwrap() <= *time);

                        prop_assert!(prov.was_associated_with.get(
                            &(namespace.clone(),id.clone()))
                            .unwrap()
                            .contains(&(namespace.to_owned(),agent.to_owned())));
                    }
                    ChronicleTransaction::ActivityUses(
                        ActivityUses { namespace, id, activity }) => {
                        let activity_id = activity;
                        let entity = &prov.entities.get(&(namespace.to_owned(),id.to_owned()));
                        prop_assert!(entity.is_some());
                        let entity = entity.unwrap();
                        prop_assert_eq!(&entity.name(), &id.decompose());
                        prop_assert_eq!(&entity.namespaceid(), &namespace);

                        let activity = &prov.activities.get(&(namespace.to_owned(),activity_id.to_owned()));
                        prop_assert!(activity.is_some());
                        let activity = activity.unwrap();
                        prop_assert_eq!(&activity.name, &activity_id.decompose());
                        prop_assert_eq!(&activity.namespaceid, namespace);

                        prop_assert!(prov.used.get(
                            &(namespace.clone(),activity.id.clone()))
                            .unwrap()
                            .contains(&(namespace.to_owned(),id.to_owned())));

                    },
                    ChronicleTransaction::GenerateEntity(GenerateEntity{namespace, id, activity}) => {
                        let activity_id = activity;
                        let entity = &prov.entities.get(&(namespace.to_owned(),id.to_owned()));
                        prop_assert!(entity.is_some());
                        let entity = entity.unwrap();
                        prop_assert_eq!(&entity.name(), &id.decompose());
                        prop_assert_eq!(&entity.namespaceid(), &namespace);

                        let activity = &prov.activities.get(&(namespace.to_owned(),activity.to_owned()));
                        prop_assert!(activity.is_some());
                        let activity = activity.unwrap();
                        prop_assert_eq!(&activity.name, &activity_id.decompose());
                        prop_assert_eq!(&activity.namespaceid, namespace);

                        prop_assert!(prov.was_generated_by.get(
                            &(namespace.clone(),id.clone()))
                            .unwrap()
                            .contains(&(namespace.to_owned(),activity.id.to_owned())));
                    }
                    ChronicleTransaction::EntityAttach(
                        EntityAttach{
                        namespace,
                        id,
                        locator: _,
                        agent,
                        signature: _,
                        signature_time: _
                    }) =>  {
                        let agent_id = agent;
                        let entity = &prov.entities.get(&(namespace.to_owned(),id.to_owned()));
                        prop_assert!(entity.is_some());
                        let entity = entity.unwrap();
                        prop_assert_eq!(&entity.name(), &id.decompose());
                        prop_assert_eq!(&entity.namespaceid(), &namespace);

                        let agent = &prov.agents.get(&(namespace.to_owned(),agent.to_owned()));
                        prop_assert!(agent.is_some());
                        let agent = agent.unwrap();
                        prop_assert_eq!(&agent.name, agent_id.decompose());
                        prop_assert_eq!(&agent.namespaceid, namespace);

                    },
                    ChronicleTransaction::Domaintype(
                        Domaintype::Entity  { namespace, id, domaintype }) => {
                        let entity = &prov.entities.get(&(namespace.to_owned(),id.to_owned()));
                        prop_assert!(entity.is_some());
                        let entity = entity.unwrap();

                        prop_assert_eq!(entity.domaintypeid(), domaintype);
                    },
                    ChronicleTransaction::Domaintype(Domaintype::Activity { namespace, id, domaintype }) => {
                        let activity = &prov.activities.get(&(namespace.to_owned(),id.to_owned()));
                        prop_assert!(activity.is_some());
                        let activity = activity.unwrap();

                        prop_assert_eq!(&activity.domaintypeid, domaintype);
                    },
                    ChronicleTransaction::Domaintype(Domaintype::Agent { namespace, id, domaintype }) => {
                        let agent = &prov.agents.get(&(namespace.to_owned(),id.to_owned()));
                        prop_assert!(agent.is_some());
                        let agent = agent.unwrap();

                        prop_assert_eq!(&agent.domaintypeid, domaintype);
                    },
                }
            }
            (regkey_assertion)()?;


            // Test that serialisation to and from JSON-LD is symmetric
            let json = compact_json(&prov).0;
            let serialized_prov = prov_from_json_ld(json.clone());

            prop_assert_eq!(&prov,&serialized_prov,"Prov reserialisation {} ",json)
        }
    }
}
