use std::{
    fmt::Display,
    fs::File,
    path::{Path, PathBuf},
};

use serde_json::Value;

use crate::error::ChronicleSynthError;

/// Represents a Synth collection that generates a Chronicle operation or component-generator of an operation collection.
#[derive(Debug)]
pub enum Collection {
    Operation(Operation),
    Generator(Generator),
}

/// `Operation` refers to a Synth schema collection that generates a Chronicle operation.
/// An `Operation` usually has dependencies in the form of component [`Generator`]s.
#[derive(Debug)]
pub enum Operation {
    ActivityExists,
    ActivityUses,
    AgentActsOnBehalfOf,
    AgentExists,
    CreateNamespace,
    EndActivity,
    EntityDerive,
    EntityExists,
    SetActivityAttributes,
    SetAgentAttributes,
    SetEntityAttributes,
    StartActivity,
    WasAssociatedWith,
    WasAssociatedWithHasRole,
    WasAttributedTo,
    WasGeneratedBy,
    WasInformedBy,
    DomainCollection(DomainCollection),
}

/// `Generator` refers to a Synth schema collection that generates a component of a Chronicle
/// operation, as opposed to being an operation itself. A `Generator` should have no dependencies.
#[derive(Debug)]
pub enum Generator {
    ActivityName,
    SecondActivityName,
    AgentName,
    SecondAgentName,
    Attribute,
    Attributes,
    DateTime,
    DomainTypeId,
    EntityName,
    SecondEntityName,
    Namespace,
    NamespaceUuid,
    Role,
    Roles,
    SameNamespaceName,
    SameNamespaceUuid,
    DomainCollection(DomainCollection),
}

/// Represents a Synth collection that is generated specifically for a Chronicle domain.
#[derive(Debug)]
pub struct DomainCollection {
    pub name: String,
    pub schema: Value,
}

impl DomainCollection {
    pub fn new(name: impl Into<String>, schema: Value) -> Self {
        let name = name.into();
        Self { name, schema }
    }
}

pub trait CollectionHandling {
    fn name(&self) -> String
    where
        Self: Display,
    {
        self.to_string()
    }

    fn path(&self) -> PathBuf
    where
        Self: Display,
    {
        Path::new(&format!("{}.json", self)).to_path_buf()
    }

    fn json_schema(&self) -> Result<Value, ChronicleSynthError>
    where
        Self: Display;
}

impl From<Operation> for Collection {
    fn from(operation: Operation) -> Self {
        Self::Operation(operation)
    }
}

impl From<Generator> for Collection {
    fn from(generator: Generator) -> Self {
        Self::Generator(generator)
    }
}

impl Display for Collection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Collection::Operation(operation) => write!(f, "{}", operation),
            Collection::Generator(generator) => write!(f, "{}", generator),
        }
    }
}

impl CollectionHandling for Collection {
    fn json_schema(&self) -> Result<Value, ChronicleSynthError> {
        match self {
            Collection::Operation(operation) => operation.json_schema(),
            Collection::Generator(generator) => generator.json_schema(),
        }
    }
}

impl CollectionHandling for Operation {
    fn json_schema(&self) -> Result<Value, ChronicleSynthError>
    where
        Self: Display,
    {
        match self {
            Self::DomainCollection(domain_collection) => Ok(domain_collection.schema.to_owned()),
            _ => {
                let path = self.path();
                let reader = File::open(path)?;
                let schema: serde_json::Value = serde_json::from_reader(reader)?;
                Ok(schema)
            }
        }
    }
}

impl Display for Operation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::ActivityExists => "activity_exists",
                Self::ActivityUses => "activity_uses",
                Self::AgentActsOnBehalfOf => "agent_acts_on_behalf_of",
                Self::AgentExists => "agent_exists",
                Self::CreateNamespace => "create_namespace",
                Self::EndActivity => "end_activity",
                Self::EntityDerive => "entity_derive",
                Self::EntityExists => "entity_exists",
                Self::SetActivityAttributes => "set_activity_attributes",
                Self::SetAgentAttributes => "set_agent_attributes",
                Self::SetEntityAttributes => "set_entity_attributes",
                Self::StartActivity => "start_activity",
                Self::WasAssociatedWith => "was_associated_with",
                Self::WasAssociatedWithHasRole => "was_associated_with_has_role",
                Self::WasAttributedTo => "was_attributed_to",
                Self::WasGeneratedBy => "was_generated_by",
                Self::WasInformedBy => "was_informed_by",
                Self::DomainCollection(domain_collection) => &domain_collection.name,
            }
        )
    }
}

impl CollectionHandling for Generator {
    fn json_schema(&self) -> Result<Value, ChronicleSynthError>
    where
        Self: Display,
    {
        match self {
            Self::DomainCollection(domain_collection) => Ok(domain_collection.schema.to_owned()),
            _ => {
                let path = self.path();
                let reader = File::open(path)?;
                let schema: serde_json::Value = serde_json::from_reader(reader)?;
                Ok(schema)
            }
        }
    }
}

impl Display for Generator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::ActivityName => "activity_name",
                Self::SecondActivityName => "second_activity_name",
                Self::AgentName => "agent_name",
                Self::SecondAgentName => "second_agent_name",
                Self::Attribute => "attribute",
                Self::Attributes => "attributes",
                Self::DateTime => "date_time",
                Self::DomainTypeId => "domain_type_id",
                Self::EntityName => "entity_name",
                Self::SecondEntityName => "second_entity_name",
                Self::Namespace => "namespace",
                Self::NamespaceUuid => "namespace_uuid",
                Self::Role => "role",
                Self::Roles => "roles",
                Self::SameNamespaceName => "same_namespace_name",
                Self::SameNamespaceUuid => "same_namespace_uuid",
                Self::DomainCollection(dc) => &dc.name,
            }
        )
    }
}
