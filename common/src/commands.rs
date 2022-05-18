use std::{path::PathBuf, pin::Pin, sync::Arc};

use chrono::{DateTime, Utc};
use derivative::*;
use futures::AsyncRead;

use serde::{Deserialize, Serialize};

use crate::{
    attributes::Attributes,
    prov::{
        operations::DerivationType, ActivityId, AgentId, ChronicleIri, ChronicleTransactionId,
        EntityId, Name, ProvModel,
    },
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NamespaceCommand {
    Create { name: Name },
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
        name: Name,
        namespace: Name,
        attributes: Attributes,
    },
    RegisterKey {
        id: AgentId,
        namespace: Name,
        registration: KeyRegistration,
    },
    UseInContext {
        id: AgentId,
        namespace: Name,
    },
    Delegate {
        id: AgentId,
        delegate: AgentId,
        activity: Option<ActivityId>,
        namespace: Name,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActivityCommand {
    Create {
        name: Name,
        namespace: Name,
        attributes: Attributes,
    },
    Start {
        id: ActivityId,
        namespace: Name,
        time: Option<DateTime<Utc>>,
        agent: Option<AgentId>,
    },
    End {
        id: Option<ActivityId>,
        namespace: Name,
        time: Option<DateTime<Utc>>,
        agent: Option<AgentId>,
    },
    Use {
        id: EntityId,
        namespace: Name,
        activity: Option<ActivityId>,
    },
    Generate {
        id: EntityId,
        namespace: Name,
        activity: Option<ActivityId>,
    },
}

impl ActivityCommand {
    pub fn create(
        name: impl AsRef<str>,
        namespace: impl AsRef<str>,
        attributes: Attributes,
    ) -> Self {
        Self::Create {
            name: name.as_ref().into(),
            namespace: namespace.as_ref().into(),
            attributes,
        }
    }

    pub fn start(
        id: ActivityId,
        namespace: impl AsRef<str>,
        time: Option<DateTime<Utc>>,
        agent: Option<AgentId>,
    ) -> Self {
        Self::Start {
            id,
            namespace: namespace.as_ref().into(),
            time,
            agent,
        }
    }

    pub fn end(
        id: Option<ActivityId>,
        namespace: impl AsRef<str>,
        time: Option<DateTime<Utc>>,
        agent: Option<AgentId>,
    ) -> Self {
        Self::End {
            id,
            namespace: namespace.as_ref().into(),
            time,
            agent,
        }
    }

    pub fn r#use(id: EntityId, namespace: impl AsRef<str>, activity: Option<ActivityId>) -> Self {
        Self::Use {
            id,
            namespace: namespace.as_ref().into(),
            activity,
        }
    }

    pub fn generate(
        id: EntityId,
        namespace: impl AsRef<str>,
        activity: Option<ActivityId>,
    ) -> Self {
        Self::Generate {
            id,
            namespace: namespace.as_ref().into(),
            activity,
        }
    }
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
    Create {
        name: Name,
        namespace: Name,
        attributes: Attributes,
    },
    Attach {
        id: EntityId,
        namespace: Name,
        file: PathOrFile,
        locator: Option<String>,
        agent: Option<AgentId>,
    },
    Derive {
        id: EntityId,
        namespace: Name,
        derivation: Option<DerivationType>,
        activity: Option<ActivityId>,
        used_entity: EntityId,
    },
}

impl EntityCommand {
    pub fn create(
        name: impl AsRef<str>,
        namespace: impl AsRef<str>,
        attributes: Attributes,
    ) -> Self {
        Self::Create {
            name: name.as_ref().into(),
            namespace: namespace.as_ref().into(),
            attributes,
        }
    }

    pub fn attach(
        id: EntityId,
        namespace: impl AsRef<str>,
        file: PathOrFile,
        locator: Option<String>,
        agent: Option<AgentId>,
    ) -> Self {
        Self::Attach {
            id,
            namespace: namespace.as_ref().into(),
            file,
            locator,
            agent,
        }
    }

    pub fn detach(
        id: EntityId,
        namespace: impl AsRef<str>,
        derivation: Option<DerivationType>,
        activity: Option<ActivityId>,
        used_entity: EntityId,
    ) -> Self {
        Self::Derive {
            id,
            namespace: namespace.as_ref().into(),
            derivation,
            activity,
            used_entity,
        }
    }
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
        subject: ChronicleIri,
        prov: Box<ProvModel>,
        correlation_id: ChronicleTransactionId,
    },
    /// The api has successfully executed the query
    QueryReply { prov: Box<ProvModel> },
}

impl ApiResponse {
    pub fn submission(
        subject: impl Into<ChronicleIri>,
        prov: ProvModel,
        correlation_id: ChronicleTransactionId,
    ) -> Self {
        ApiResponse::Submission {
            subject: subject.into(),
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
