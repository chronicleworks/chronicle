use std::{path::PathBuf, pin::Pin, sync::Arc};

use chrono::{DateTime, Utc};
use derivative::*;
use futures::AsyncRead;
use iref::IriBuf;
use serde::{Deserialize, Serialize};

use crate::prov::{ChronicleTransactionId, DerivationType, ProvModel};

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
    UseInContext {
        name: String,
        namespace: String,
    },
    Delegate {
        name: String,
        delegate: String,
        activity: Option<String>,
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
    Derive {
        name: String,
        namespace: String,
        activity: Option<String>,
        used_entity: String,
        typ: Option<DerivationType>,
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
    /// The api has successfully executed the operation, but has no useful output
    Unit,
    /// The api has validated the command and submitted a transaction to a ledger
    Submission {
        subject: IriBuf,
        prov: Box<ProvModel>,
        correlation_id: ChronicleTransactionId,
    },
    /// The api has successfully executed the query
    QueryReply { prov: Box<ProvModel> },
}

impl ApiResponse {
    pub fn submission(
        subject: IriBuf,
        prov: ProvModel,
        correlation_id: ChronicleTransactionId,
    ) -> Self {
        ApiResponse::Submission {
            subject,
            prov: Box::new(prov),
            correlation_id,
        }
    }

    pub fn unit() -> Self {
        ApiResponse::Unit
    }

    pub fn query_reply(prov: ProvModel) -> Self {
        ApiResponse::QueryReply {
            prov: Box::new(prov),
        }
    }
}
