use std::{path::PathBuf, pin::Pin, sync::Arc};

use chrono::{DateTime, Utc};
use derivative::*;
use futures::AsyncRead;

use serde::{Deserialize, Serialize};

use crate::{
    attributes::Attributes,
    prov::{
        operations::{ChronicleOperation, DerivationType},
        ActivityId, AgentId, ChronicleIri, ChronicleTransactionId, EntityId, ExternalId,
        NamespaceId, ProvModel, Role,
    },
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NamespaceCommand {
    Create { external_id: ExternalId },
}

#[derive(Clone, Serialize, Deserialize)]
pub enum KeyImport {
    FromPath { path: PathBuf },
    FromPEMBuffer { buffer: Vec<u8> },
}

impl std::fmt::Debug for KeyImport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FromPath { path } => write!(f, "KeyImport::FromPath {{ path: {:?} }}", path),
            Self::FromPEMBuffer { .. } => {
                write!(f, "KeyImport::FromPEMBuffer {{ buffer: ***SECRET*** }}")
            }
        }
    }
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
        external_id: ExternalId,
        namespace: ExternalId,
        attributes: Attributes,
    },
    RegisterKey {
        id: AgentId,
        namespace: ExternalId,
        registration: KeyRegistration,
    },
    UseInContext {
        id: AgentId,
        namespace: ExternalId,
    },
    Delegate {
        id: AgentId,
        delegate: AgentId,
        activity: Option<ActivityId>,
        namespace: ExternalId,
        role: Option<Role>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActivityCommand {
    Create {
        external_id: ExternalId,
        namespace: ExternalId,
        attributes: Attributes,
    },
    Instant {
        id: ActivityId,
        namespace: ExternalId,
        time: Option<DateTime<Utc>>,
        agent: Option<AgentId>,
    },
    Start {
        id: ActivityId,
        namespace: ExternalId,
        time: Option<DateTime<Utc>>,
        agent: Option<AgentId>,
    },
    End {
        id: ActivityId,
        namespace: ExternalId,
        time: Option<DateTime<Utc>>,
        agent: Option<AgentId>,
    },
    Use {
        id: EntityId,
        namespace: ExternalId,
        activity: ActivityId,
    },
    Generate {
        id: EntityId,
        namespace: ExternalId,
        activity: ActivityId,
    },
    WasInformedBy {
        id: ActivityId,
        namespace: ExternalId,
        informing_activity: ActivityId,
    },
    Associate {
        id: ActivityId,
        namespace: ExternalId,
        responsible: AgentId,
        role: Option<Role>,
    },
}

impl ActivityCommand {
    pub fn create(
        external_id: impl AsRef<str>,
        namespace: impl AsRef<str>,
        attributes: Attributes,
    ) -> Self {
        Self::Create {
            external_id: external_id.as_ref().into(),
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
        id: ActivityId,
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

    pub fn instant(
        id: ActivityId,
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

    pub fn r#use(id: EntityId, namespace: impl AsRef<str>, activity: ActivityId) -> Self {
        Self::Use {
            id,
            namespace: namespace.as_ref().into(),
            activity,
        }
    }

    pub fn was_informed_by(
        id: ActivityId,
        namespace: impl AsRef<str>,
        informing_activity: ActivityId,
    ) -> Self {
        Self::WasInformedBy {
            id,
            namespace: namespace.as_ref().into(),
            informing_activity,
        }
    }

    pub fn generate(id: EntityId, namespace: impl AsRef<str>, activity: ActivityId) -> Self {
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
        external_id: ExternalId,
        namespace: ExternalId,
        attributes: Attributes,
    },
    Attribute {
        id: EntityId,
        namespace: ExternalId,
        responsible: AgentId,
        role: Option<Role>,
    },
    Derive {
        id: EntityId,
        namespace: ExternalId,
        derivation: DerivationType,
        activity: Option<ActivityId>,
        used_entity: EntityId,
    },
}

impl EntityCommand {
    pub fn create(
        external_id: impl AsRef<str>,
        namespace: impl AsRef<str>,
        attributes: Attributes,
    ) -> Self {
        Self::Create {
            external_id: external_id.as_ref().into(),
            namespace: namespace.as_ref().into(),
            attributes,
        }
    }

    pub fn detach(
        id: EntityId,
        namespace: impl AsRef<str>,
        derivation: DerivationType,
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
pub struct DepthChargeCommand {
    pub namespace: NamespaceId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportCommand {
    pub namespace: NamespaceId,
    pub operations: Vec<ChronicleOperation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiCommand {
    NameSpace(NamespaceCommand),
    Agent(AgentCommand),
    Activity(ActivityCommand),
    Entity(EntityCommand),
    Query(QueryCommand),
    DepthCharge(DepthChargeCommand),
    Import(ImportCommand),
}

#[derive(Debug)]
pub enum ApiResponse {
    /// The api has successfully executed the operation, but has no useful output
    Unit,
    /// The operation will not result in any data changes
    AlreadyRecorded {
        subject: ChronicleIri,
        prov: Box<ProvModel>,
    },
    /// The api has validated the command and submitted a transaction to a ledger
    Submission {
        subject: ChronicleIri,
        prov: Box<ProvModel>,
        tx_id: ChronicleTransactionId,
    },
    /// The api has successfully executed the query
    QueryReply { prov: Box<ProvModel> },
    /// The api has submitted the import transactions to a ledger
    ImportSubmitted {
        prov: Box<ProvModel>,
        tx_id: ChronicleTransactionId,
    },
    /// The api has submitted the depth charge transaction to a ledger
    DepthChargeSubmitted { tx_id: ChronicleTransactionId },
}

impl ApiResponse {
    pub fn submission(
        subject: impl Into<ChronicleIri>,
        prov: ProvModel,
        tx_id: ChronicleTransactionId,
    ) -> Self {
        ApiResponse::Submission {
            subject: subject.into(),
            prov: Box::new(prov),
            tx_id,
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

    pub fn already_recorded(subject: impl Into<ChronicleIri>, prov: ProvModel) -> Self {
        ApiResponse::AlreadyRecorded {
            subject: subject.into(),
            prov: Box::new(prov),
        }
    }

    pub fn depth_charge_submission(tx_id: ChronicleTransactionId) -> Self {
        ApiResponse::DepthChargeSubmitted { tx_id }
    }

    pub fn import_submitted(prov: ProvModel, tx_id: ChronicleTransactionId) -> Self {
        ApiResponse::ImportSubmitted {
            prov: Box::new(prov),
            tx_id,
        }
    }
}

#[cfg(test)]
mod test {
    use k256::{
        pkcs8::{EncodePrivateKey, LineEnding},
        SecretKey,
    };
    use rand_core::SeedableRng;

    use crate::commands::{KeyImport, KeyRegistration};

    fn key_from_seed(seed: u8) -> String {
        let secret: SecretKey = SecretKey::random(rand::rngs::StdRng::from_seed([seed; 32]));

        secret.to_pkcs8_pem(LineEnding::CRLF).unwrap().to_string()
    }

    #[test]
    fn test_key_import_custom_debug() {
        let pk = key_from_seed(0);

        let from_pem_buffer = KeyRegistration::ImportSigning(KeyImport::FromPEMBuffer {
            buffer: pk.as_bytes().into(),
        });

        insta::assert_debug_snapshot!(from_pem_buffer, @r###"
        ImportSigning(
            KeyImport::FromPEMBuffer { buffer: ***SECRET*** },
        )
        "###);

        // `FromPath` `path` should be visible in `Debug`
        let from_path = KeyRegistration::ImportSigning(KeyImport::FromPath {
            path: "some_path".into(),
        });

        insta::assert_debug_snapshot!(from_path, @r###"
        ImportSigning(
            KeyImport::FromPath { path: "some_path" },
        )
        "###);
    }
}
