use std::path::PathBuf;

use chrono::{DateTime, Utc};

use crate::models::ProvModel;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NamespaceCommand {
    Create { name: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KeyRegistration {
    Generate,
    ImportVerifying { path: PathBuf },
    ImportSigning { path: PathBuf },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentCommand {
    Create {
        name: String,
        namespace: String,
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
    },
    Start {
        name: String,
        namespace: String,
        time: Option<DateTime<Utc>>,
    },
    End {
        name: Option<String>,
        namespace: Option<String>,
        time: Option<DateTime<Utc>>,
    },
    Use {
        name: String,
        namespace: String,
        activity: Option<String>,
    },
    Generate {
        name: String,
        namespace: String,
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
    StartUi {},
}

#[derive(Debug)]
pub enum ApiResponse {
    Unit,
    Prov(ProvModel),
}
