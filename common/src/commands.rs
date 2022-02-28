use std::{path::PathBuf, pin::Pin, sync::Arc};

use chrono::{DateTime, Utc};
use derivative::*;
use futures::AsyncRead;
use iref::IriBuf;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{ledger::Offset, prov::ProvModel};

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
        namespace: String,
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

#[derive(Derivative)]
#[derivative(Debug, Clone)]
pub enum PathOrFile {
    Path(PathBuf),
    File(#[derivative(Debug = "ignore")] Arc<Pin<Box<dyn AsyncRead + Sync + Send>>>), //Non serialisable variant, used in process
}

impl Serialize for PathOrFile {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            PathOrFile::Path(path) => path.serialize(serializer),
            _ => {
                unreachable!()
            }
        }
    }
}

impl<'de> Deserialize<'de> for PathOrFile {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(PathOrFile::Path(PathBuf::deserialize(deserializer)?))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EntityCommand {
    Attach {
        name: String,
        namespace: String,
        file: PathOrFile,
        locator: Option<String>,
        agent: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryCommand {
    pub namespace: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncCommand {
    pub correlation_id: Uuid,
    pub offset: Offset,
    pub prov: ProvModel,
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
    Prov(IriBuf, Vec<ProvModel>, Uuid),
}
