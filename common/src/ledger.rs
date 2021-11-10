use std::cell::RefCell;
use std::collections::HashMap;
use std::convert::Infallible;
use std::hint::unreachable_unchecked;
use std::rc::Rc;
use std::slice::SliceIndex;
use std::str::Utf8Error;

use async_std::task::block_on;
use chrono::{DateTime, Utc};
use custom_error::custom_error;
use futures::future::{self, BoxFuture};
use futures::{FutureExt, Stream};
use iref::{AsIri, Iri, IriBuf, IriRef};
use json::object::Object;
use json::JsonValue;
use json_ld::context::RemoteContext;
use json_ld::util::AsJson;
use json_ld::{context::Local, Document, JsonContext, NoLoader};
use json_ld::{Error, ErrorCode, Indexed, Loader, Node, Reference, RemoteDocument};

use crate::models::{
    Activity, ActivityId, Agent, AgentId, ChronicleTransaction, Entity, EntityId, Namespace,
    NamespaceId, ProvModel,
};
use crate::vocab::{Chronicle, Prov};

custom_error! {pub SubmissionError
    Implementation{source: Box<dyn std::error::Error>} = "Ledger error",
}

custom_error! {pub ProcessorError
    JsonLd{source: json_ld::Error} = "Json Ld Error",
    MissingId{object: JsonValue} = "Missing @id",
    MissingProperty{object: JsonValue} = "Missing property",
    NotANode{} = "Json LD object is not a node",
    Time{source: chrono::ParseError} = "Unparsable date/time",
    Json{source: json::JsonError} = "Malformed JSON",
    Utf8{source: std::str::Utf8Error} = "State is not valid utf8",
}

impl From<Infallible> for ProcessorError {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}

pub trait LedgerWriter {
    fn submit(&self, tx: Vec<&ChronicleTransaction>) -> Result<(), SubmissionError>;
}

pub trait LedgerReader {
    fn namespace_updates(&self, namespace: NamespaceId) -> Box<dyn Stream<Item = ProvModel>>;
}

/// An in memory ledger implementation for development and testing purposes
#[derive(Debug, Default)]
pub struct InMemLedger {
    kv: RefCell<HashMap<String, Vec<u8>>>,
}

impl LedgerWriter for InMemLedger {
    fn submit(&self, tx: Vec<&ChronicleTransaction>) -> Result<(), SubmissionError> {
        for tx in tx {
            let deps = tx.dependencies();
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct LedgerAddress {
    namespace: String,
    resource: String,
}

pub trait Depdendencies {
    fn dependencies(&self) -> Vec<LedgerAddress>;
}

pub struct StateInput {
    address: LedgerAddress,
    data: Vec<u8>,
}

impl StateInput {
    pub fn new(address: LedgerAddress, data: Vec<u8>) -> Self {
        Self { address, data }
    }
}

pub struct StateOutput {
    address: LedgerAddress,
    data: Vec<u8>,
}

impl StateOutput {
    pub fn new(address: LedgerAddress, data: Vec<u8>) -> Self {
        Self { address, data }
    }
}

/// A prov model represented as one or more JSON-LD documents
impl ProvModel {
    /// Take a Json-Ld input document, assuming it is in compact form, expand it and apply the state to the prov model
    /// Replace @context with our resource context
    /// We rely on reified @types, so subclassing must also include supertypes
    fn apply_json_ld(&mut self, json: JsonValue) -> Result<(), ProcessorError> {
        json.remove("@context");
        json.insert("@context", crate::context::PROV.clone());
        let model = ProvModel::default();
        let output = block_on(json.expand::<JsonContext, _>(&mut NoLoader))?;

        for o in output {
            let o = o
                .try_cast::<Node>()
                .map_err(|_| ProcessorError::NotANode {})?
                .inner();
            if o.has_type(&Reference::Id(Prov::Agent.as_iri().into())) {
                self.apply_node_as_agent(&mut model, o)?;
            } else if o.has_type(&Reference::Id(Prov::Activity.as_iri().into())) {
                self.apply_node_as_activity(&mut model, o)?;
            } else if o.has_type(&Reference::Id(Prov::Entity.as_iri().into())) {
                self.apply_node_as_entity(&mut model, o)?;
            }
        }

        Ok(())
    }

    fn apply_node_as_agent(
        &self,
        model: &mut ProvModel,
        agent: &Node,
    ) -> Result<(), ProcessorError> {
        let id = AgentId::new(
            agent
                .id()
                .ok_or_else(|| ProcessorError::MissingId {
                    object: agent.as_json(),
                })?
                .to_string(),
        );

        let namespaceid = extract_namespace(agent)?;
        model.namespace_context(&namespaceid);
        let name = id.decompose().to_owned();

        let publickey = extract_scalar_prop(&Chronicle::HasPublicKey, agent)?
            .as_str()
            .map(|x| x.to_owned());

        model.add_agent(Agent::new(id, namespaceid, name, publickey));

        Ok(())
    }

    fn apply_node_as_activity(
        &self,
        model: &mut ProvModel,
        activity: &Node,
    ) -> Result<(), ProcessorError> {
        let id = ActivityId::new(
            activity
                .id()
                .ok_or_else(|| ProcessorError::MissingId {
                    object: activity.as_json(),
                })?
                .to_string(),
        );

        let namespaceid = extract_namespace(activity)?;
        model.namespace_context(&namespaceid);
        let name = id.decompose().to_owned();

        let started = extract_scalar_prop(&Prov::StartedAtTime, activity)?
            .as_str()
            .map(|x| DateTime::parse_from_rfc3339(x));

        let ended = extract_scalar_prop(&Prov::EndedAtTime, activity)?
            .as_str()
            .map(|x| DateTime::parse_from_rfc3339(x));

        let used = extract_reference_ids(&Prov::Used, activity)?
            .into_iter()
            .map(|id| EntityId::new(id.as_str()));

        let wasassociatedwith = extract_reference_ids(&Prov::Used, activity)?
            .into_iter()
            .map(|id| AgentId::new(id.as_str()));

        let activity = Activity::new(id, namespaceid, &name);

        if let Some(started) = started {
            activity.started = Some(DateTime::<Utc>::from(started?));
        }

        if let Some(ended) = ended {
            activity.ended = Some(DateTime::<Utc>::from(ended?));
        }

        for entity in used {
            model.used(activity.id.to_owned(), &entity);
        }

        for agent in wasassociatedwith {
            model.associate_with(activity.id.to_owned(), &agent);
        }

        model.add_activity(activity);

        Ok(())
    }

    fn apply_node_as_entity(
        &self,
        model: &mut ProvModel,
        entity: &Node,
    ) -> Result<(), ProcessorError> {
        let id = EntityId::new(
            entity
                .id()
                .ok_or_else(|| ProcessorError::MissingId {
                    object: entity.as_json(),
                })?
                .to_string(),
        );

        let namespaceid = extract_namespace(entity)?;
        model.namespace_context(&namespaceid);
        let name = id.decompose().to_owned();

        let signature = extract_scalar_prop(&Chronicle::Signature, entity)?.as_str();
        let signature_time = extract_scalar_prop(&Chronicle::Signature, entity)?
            .as_str()
            .map(|x| DateTime::parse_from_rfc3339(x));
        let locator = extract_scalar_prop(&Chronicle::Signature, entity)?.as_str();

        let generatedby = extract_reference_ids(&Prov::WasGeneratedBy, entity)?
            .into_iter()
            .map(|id| ActivityId::new(id.as_str()));

        let entity = {
            if let (Some(signature), Some(signature_time)) = (signature, signature_time) {
                Entity::Signed {
                    name,
                    namespaceid,
                    id,
                    signature: signature.to_owned(),
                    locator: locator.map(|x| x.to_owned()),
                    signature_time: DateTime::<Utc>::from(signature_time?),
                }
            } else {
                Entity::Unsigned {
                    name,
                    namespaceid,
                    id,
                }
            }
        };
        for activity in generatedby {
            model.generate_by(entity.id().clone(), &activity);
        }

        model.add_entity(entity);

        Ok(())
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
        .map(|id| id.and_then(|id| Ok(id.to_owned())))
        .collect();

    Ok(ids?)
}

fn extract_scalar_prop<'a>(
    iri: &dyn AsIri,
    node: &'a Node,
) -> Result<&'a Indexed<json_ld::object::Object>, ProcessorError> {
    node.get_any(&Reference::Id(iri.as_iri().into()))
        .ok_or_else(|| ProcessorError::MissingProperty {
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

impl Depdendencies for ChronicleTransaction {
    /// Compute dependencies for a chronicle transaction, input and output addresses are always symmetric
    fn dependencies(&self) -> Vec<LedgerAddress> {
        let mut model = ProvModel::default();
        model.apply(self);

        model
            .entities
            .iter()
            .map(|(id, o)| (**id, o.namespaceid()))
            .chain(
                model
                    .activities
                    .iter()
                    .map(|(id, o)| (**id, &o.namespaceid)),
            )
            .chain(model.agents.iter().map(|(id, o)| (**id, &o.namespaceid)))
            .map(|(resource, namespace)| LedgerAddress {
                resource: resource.to_owned(),
                namespace: *namespace.to_owned(),
            })
            .collect()
    }
}

impl ChronicleTransaction {
    /// Take out input states and apply them to the prov model, then apply transaction,
    /// then transform to the compact representation and write each resource to the output state
    fn process(
        &self,
        input: Vec<StateInput>,
        tx: &ChronicleTransaction,
    ) -> Result<Vec<StateOutput>, ProcessorError> {
        let model = ProvModel::default();

        for input in input {
            model.apply_json_ld(json::parse(std::str::from_utf8(&input.data)?)?)?;
        }

        model.apply(&tx);

        let graph = model.to_json().compact()?.0["@graph"];

        Ok(graph
            .members()
            .map(|resource| StateOutput {
                address: LedgerAddress {
                    namespace: resource["namespace"].to_string(),
                    resource: resource["@id"].to_string(),
                },
                data: json::stringify(*resource).into_bytes(),
            })
            .collect())
    }
}
