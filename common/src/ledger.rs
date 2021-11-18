use std::cell::RefCell;
use std::collections::HashMap;
use std::convert::Infallible;

use std::slice::SliceIndex;
use std::str::from_utf8;

use chrono::{DateTime, Utc};
use custom_error::custom_error;

use futures::{FutureExt, Stream, TryFutureExt};
use iref::{AsIri, IriBuf};

use json::JsonValue;

use json_ld::util::AsJson;
use json_ld::{Document, JsonContext, NoLoader};
use json_ld::{Indexed, Node, Reference};
use serde::ser::SerializeSeq;
use serde::Serialize;

use tokio::task::JoinError;
use tracing::{debug, instrument};

use crate::models::{
    Activity, ActivityId, Agent, AgentId, ChronicleTransaction, CompactionError, Entity, EntityId,
    NamespaceId, ProvModel,
};
use crate::vocab::{Chronicle, Prov};

custom_error! {pub SubmissionError
    Implementation{source: Box<dyn std::error::Error>} = "Ledger error",
    Peocessor{source: ProcessorError} = "Processor error for in mem ledger",
}

custom_error! {pub ProcessorError
    Compaction{source: CompactionError} = "Json Ld Error",
    Expansion{inner: String} = "Json Ld Error",
    Tokio{source: JoinError} = "Tokio Error",
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

#[async_trait::async_trait(?Send)]
pub trait LedgerWriter {
    async fn submit(&self, tx: Vec<&ChronicleTransaction>) -> Result<(), SubmissionError>;
}

pub trait LedgerReader {
    fn namespace_updates(&self, namespace: NamespaceId) -> Box<dyn Stream<Item = ProvModel>>;
}

/// An in memory ledger implementation for development and testing purposes
#[derive(Debug, Default)]
pub struct InMemLedger {
    kv: RefCell<HashMap<LedgerAddress, JsonValue>>,
}

/// An inefficient serialiser implementation for an in memory ledger, used for snapshot assertions of ledger state,
/// <v4 of json-ld doesn't use serde_json for whatever reason, so we reconstruct the ledger as a serde json map
impl Serialize for InMemLedger {
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
    #[instrument]
    async fn submit(&self, tx: Vec<&ChronicleTransaction>) -> Result<(), SubmissionError> {
        for tx in tx {
            debug!(?tx, "Process transaction");

            let output = tx
                .process(
                    tx.dependencies()
                        .iter()
                        .filter_map(|dep| {
                            self.kv
                                .borrow()
                                .get(dep)
                                .map(|json| StateInput::new(json.to_string().as_bytes().into()))
                        })
                        .collect(),
                )
                .await?;

            for output in output {
                let state = json::parse(from_utf8(&output.data).unwrap()).unwrap();
                debug!(?output.address, "Address");
                debug!(%state, "New state");
                self.kv.borrow_mut().insert(output.address, state);
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, PartialOrd, Ord)]
pub struct LedgerAddress {
    pub namespace: String,
    pub resource: String,
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

/// A prov model represented as one or more JSON-LD documents
impl ProvModel {
    /// Take a Json-Ld input document, assuming it is in compact form, expand it and apply the state to the prov model
    /// Replace @context with our resource context
    /// We rely on reified @types, so subclassing must also include supertypes
    async fn apply_json_ld(&mut self, mut json: JsonValue) -> Result<(), ProcessorError> {
        json.remove("@context");
        json.insert("@context", crate::context::PROV.clone()).ok();
        let mut model = ProvModel::default();

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
            if o.has_type(&Reference::Id(Prov::Agent.as_iri().into())) {
                self.apply_node_as_agent(&mut model, &o)?;
            } else if o.has_type(&Reference::Id(Prov::Activity.as_iri().into())) {
                self.apply_node_as_activity(&mut model, &o)?;
            } else if o.has_type(&Reference::Id(Prov::Entity.as_iri().into())) {
                self.apply_node_as_entity(&mut model, &o)?;
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
            .map(DateTime::parse_from_rfc3339);

        let ended = extract_scalar_prop(&Prov::EndedAtTime, activity)?
            .as_str()
            .map(DateTime::parse_from_rfc3339);

        let used = extract_reference_ids(&Prov::Used, activity)?
            .into_iter()
            .map(|id| EntityId::new(id.as_str()));

        let wasassociatedwith = extract_reference_ids(&Prov::Used, activity)?
            .into_iter()
            .map(|id| AgentId::new(id.as_str()));

        let mut activity = Activity::new(id, namespaceid, &name);

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
            .map(DateTime::parse_from_rfc3339);
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

impl ChronicleTransaction {
    /// Compute dependencies for a chronicle transaction, input and output addresses are always symmetric
    pub fn dependencies(&self) -> Vec<LedgerAddress> {
        let mut model = ProvModel::default();
        model.apply(self);

        model
            .entities
            .iter()
            .map(|(id, o)| (id.to_string(), o.namespaceid()))
            .chain(
                model
                    .activities
                    .iter()
                    .map(|(id, o)| (id.to_string(), &o.namespaceid)),
            )
            .chain(
                model
                    .agents
                    .iter()
                    .map(|(id, o)| (id.to_string(), &o.namespaceid)),
            )
            .map(|(resource, namespace)| LedgerAddress {
                resource,
                namespace: namespace.to_string(),
            })
            .collect()
    }
    /// Take out input states and apply them to the prov model, then apply transaction,
    /// then transform to the compact representation and write each resource to the output state
    pub async fn process(
        &self,
        input: Vec<StateInput>,
    ) -> Result<Vec<StateOutput>, ProcessorError> {
        let mut model = ProvModel::default();

        for input in input {
            model
                .apply_json_ld(json::parse(std::str::from_utf8(&input.data)?)?)
                .await?;
        }

        model.apply(self);

        let graph = &model.to_json().compact().await?.0["@graph"];

        Ok(graph
            .members()
            .map(|resource| StateOutput {
                address: LedgerAddress {
                    namespace: resource["namespace"].to_string(),
                    resource: resource["@id"].to_string(),
                },
                data: resource.to_string().into_bytes(),
            })
            .collect())
    }
}

#[cfg(test)]
pub mod test {}
