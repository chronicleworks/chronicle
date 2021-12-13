use std::path::PathBuf;

use chrono::{DateTime, Utc};
use iref::IriBuf;

use crate::prov::ProvModel;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NamespaceCommand {
    Create { name: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KeyImport {
    FromPath { path: PathBuf },
    FromPEMBuffer { buffer: Vec<u8> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KeyRegistration {
    Generate,
    ImportVerifying(KeyImport),
    ImportSigning(KeyImport),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentCommand {
    Create {
        name: String,
        namespace: String,
        domaintype: Option<String>,
    },
    RegisterKey {
        name: String,
        namespace: String,
        registration: KeyRegistration,
    },
    Use {
        name: String,
        namespace: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActivityCommand {
    Create {
        name: String,
        namespace: String,
        domaintype: Option<String>,
    },
    Start {
        name: String,
        namespace: String,
        time: Option<DateTime<Utc>>,
        agent: Option<String>,
    },
    End {
        name: Option<String>,
        namespace: Option<String>,
        time: Option<DateTime<Utc>>,
        agent: Option<String>,
    },
    Use {
        name: String,
        namespace: String,
        domaintype: Option<String>,
        activity: Option<String>,
    },
    Generate {
        name: String,
        namespace: String,
        domaintype: Option<String>,
        activity: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EntityCommand {
    Attach {
        name: String,
        namespace: String,
        file: PathBuf,
        locator: Option<String>,
        agent: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryCommand {
    pub namespace: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiCommand {
    NameSpace(NamespaceCommand),
    Agent(AgentCommand),
    Activity(ActivityCommand),
    Entity(EntityCommand),
    Query(QueryCommand),
}

#[derive(Debug)]
pub enum ApiResponse {
    Unit,
    /// Context iri (i.e the subject resource) and delta
    Prov(IriBuf, Vec<ProvModel>),
}
