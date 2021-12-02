use futures::Stream;
use json::JsonValue;
use serde::ser::SerializeSeq;
use tracing::{debug, instrument};

use crate::prov::{ChronicleTransaction, NamespaceId, ProcessorError, ProvModel};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Display;
use std::str::from_utf8;

#[derive(Debug)]
pub enum SubmissionError {
    Implementation {
        source: Box<dyn std::error::Error + Send>,
    },
    Processor {
        source: ProcessorError,
    },
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
    async fn submit(&mut self, tx: Vec<&ChronicleTransaction>) -> Result<(), SubmissionError>;
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
    #[instrument]
    async fn submit(&mut self, tx: Vec<&ChronicleTransaction>) -> Result<(), SubmissionError> {
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
impl ProvModel {}

impl ChronicleTransaction {
    /// Compute dependencies for a chronicle transaction, input and output addresses are always symmetric
    pub fn dependencies(&self) -> Vec<LedgerAddress> {
        let mut model = ProvModel::default();
        model.apply(self);

        model
            .entities
            .iter()
            .map(|((_, id), o)| (id.to_string(), o.namespaceid()))
            .chain(
                model
                    .activities
                    .iter()
                    .map(|((_, id), o)| (id.to_string(), &o.namespaceid)),
            )
            .chain(
                model
                    .agents
                    .iter()
                    .map(|((_, id), o)| (id.to_string(), &o.namespaceid)),
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
