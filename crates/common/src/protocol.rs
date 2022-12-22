use std::io::Cursor;

use prost::Message;
use tracing::span;

use crate::prov::{
    operations::ChronicleOperation, to_json_ld::ToJson, ChronicleTransaction, CompactionError,
    Contradiction, ExpandedJson, ProcessorError, ProvModel, SignedIdentity,
};

use thiserror::Error;

use self::messages::event::OptionContradiction;

#[derive(Error, Debug)]
pub enum ProtocolError {
    #[error("Protobuf deserialization error {source}")]
    ProtobufDeserialize {
        #[from]
        source: prost::DecodeError,
    },
    #[error("Protobuf serialization error {source}")]
    ProtobufSerialize {
        #[from]
        source: prost::EncodeError,
    },
    #[error("Serde de/serialization error {source}")]
    JsonSerialize {
        #[from]
        source: serde_json::Error,
    },
    #[error("Problem applying delta {source}")]
    ProcessorError {
        #[from]
        source: ProcessorError,
    },
    #[error("Could not compact json {source}")]
    Compaction {
        #[from]
        source: CompactionError,
    },
}

static PROTOCOL_VERSION: &str = "1";

// Include the `submission` module, which is
// generated from ./protos/submission.proto.
pub mod messages {
    #![allow(clippy::derive_partial_eq_without_eq)]

    include!(concat!(env!("OUT_DIR"), "/_.rs"));
}

pub async fn chronicle_committed(
    span: u64,
    delta: ProvModel,
) -> Result<messages::Event, ProtocolError> {
    Ok(messages::Event {
        version: PROTOCOL_VERSION.to_owned(),
        delta: serde_json::to_string(&delta.to_json().compact_stable_order().await?)?,
        span_id: span,
        ..Default::default()
    })
}

pub fn chronicle_contradicted(
    span: u64,
    contradiction: &Contradiction,
) -> Result<messages::Event, ProtocolError> {
    Ok(messages::Event {
        version: PROTOCOL_VERSION.to_owned(),
        span_id: span,
        option_contradiction: Some(OptionContradiction::Contradiction(serde_json::to_string(
            &contradiction,
        )?)),
        ..Default::default()
    })
}

pub async fn deserialize_event(
    buf: &[u8],
) -> Result<(span::Id, Result<ProvModel, Contradiction>), ProtocolError> {
    let event = messages::Event::decode(buf)?;
    // Spans of zero panic, so assign a dummy value until we thread the span correctly
    let span_id = {
        if event.span_id == 0 {
            span::Id::from_u64(0xffffffffffffffff)
        } else {
            span::Id::from_u64(event.span_id)
        }
    };
    let model = match (event.delta, event.option_contradiction) {
        (_, Some(OptionContradiction::Contradiction(contradiction))) => {
            Err(serde_json::from_str::<Contradiction>(&contradiction)?)
        }
        (delta, None) => {
            let mut model = ProvModel::default();
            model.apply_json_ld_str(&delta).await?;

            Ok(model)
        }
    };
    Ok((span_id, model))
}

impl messages::Event {
    pub async fn get_contradiction(&self) -> Result<Option<Contradiction>, ProtocolError> {
        Ok(self
            .option_contradiction
            .as_ref()
            .map(|OptionContradiction::Contradiction(s)| serde_json::from_str(s))
            .transpose()?)
    }

    pub async fn get_delta(&self) -> Result<ProvModel, ProtocolError> {
        let mut model = ProvModel::default();
        model.apply_json_ld_str(&self.delta).await?;

        Ok(model)
    }
}

/// Envelope a payload of `ChronicleOperations` and `SignedIdentity` in a `Submission` protocol buffer,
/// along with placeholders for protocol version info and a tracing span id.
pub async fn create_operation_submission_request(
    payload: &ChronicleTransaction,
) -> Result<messages::Submission, ProtocolError> {
    let mut submission = messages::Submission {
        version: PROTOCOL_VERSION.to_string(),
        span_id: 0u64,
        ..Default::default()
    };

    let mut ops = Vec::with_capacity(payload.tx.len());
    for op in &payload.tx {
        let op_json = op.to_json();
        let compact_json_string = op_json.compact().await?.0.to_string();
        // using `unwrap` to work around `MessageBuilder::make_sawtooth_transaction`,
        // which calls here from `sawtooth-protocol::messages` being non-fallible
        ops.push(compact_json_string);
    }
    submission.body = ops;

    let identity = serde_json::to_string(&payload.identity)?;
    submission.identity = identity;

    Ok(submission)
}

/// `Submission` protocol buffer serializer
pub fn serialize_submission(submission: &messages::Submission) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.reserve(submission.encoded_len());
    submission.encode(&mut buf).unwrap();
    buf
}

/// `Submission` protocol buffer deserializer
pub fn deserialize_submission(buf: &[u8]) -> Result<messages::Submission, prost::DecodeError> {
    messages::Submission::decode(&mut Cursor::new(buf))
}

/// Convert a `Submission` payload from a vector of
/// strings to a vector of `ChronicleOperation`s
pub async fn chronicle_operations_from_submission(
    submission_body: Vec<String>,
) -> Result<Vec<ChronicleOperation>, ProcessorError> {
    let mut ops = Vec::with_capacity(submission_body.len());
    for op in submission_body.iter() {
        let json = json::parse(op)?;
        // The inner json value should be in compacted form,
        // wrapping in `ExpandedJson`, as required by `ChronicleOperation::from_json`
        let op = ChronicleOperation::from_json(ExpandedJson(json)).await?;
        ops.push(op);
    }
    Ok(ops)
}

/// Convert a `Submission` identity from a String
/// to a `SignedIdentity`
pub async fn chronicle_identity_from_submission(
    submission_identity: String,
) -> Result<SignedIdentity, ProcessorError> {
    Ok(serde_json::from_str(&submission_identity)?)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        prov::{
            operations::{ActsOnBehalfOf, AgentExists, ChronicleOperation, CreateNamespace},
            ActivityId, AgentId, AuthId, DelegationId, ExternalId, ExternalIdPart, NamespaceId,
            Role, SignedIdentity,
        },
        signing::DirectoryStoredKeys,
    };
    use api::UuidGen;
    use sawtooth_sdk::processor::handler::ApplyError;
    use temp_dir::TempDir;
    use uuid::Uuid;

    #[derive(Debug, Clone)]
    struct SameUuid;

    impl UuidGen for SameUuid {
        fn uuid() -> Uuid {
            Uuid::parse_str("5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea").unwrap()
        }
    }

    fn create_namespace_id_helper(tag: Option<i32>) -> NamespaceId {
        let external_id = if tag.is_none() || tag == Some(0) {
            "testns".to_string()
        } else {
            format!("testns{}", tag.unwrap())
        };
        NamespaceId::from_external_id(external_id, SameUuid::uuid())
    }

    fn create_namespace_helper(tag: Option<i32>) -> ChronicleOperation {
        let id = create_namespace_id_helper(tag);
        let external_id = &id.external_id_part().to_string();
        ChronicleOperation::CreateNamespace(CreateNamespace::new(id, external_id, SameUuid::uuid()))
    }

    fn agent_exists_helper() -> ChronicleOperation {
        let namespace: NamespaceId = NamespaceId::from_external_id("testns", SameUuid::uuid());
        let external_id: ExternalId =
            ExternalIdPart::external_id_part(&AgentId::from_external_id("test_agent")).clone();
        ChronicleOperation::AgentExists(AgentExists {
            namespace,
            external_id,
        })
    }

    fn create_agent_acts_on_behalf_of() -> ChronicleOperation {
        let namespace: NamespaceId = NamespaceId::from_external_id("testns", SameUuid::uuid());
        let responsible_id = AgentId::from_external_id("test_agent");
        let delegate_id = AgentId::from_external_id("test_delegate");
        let activity_id = ActivityId::from_external_id("test_activity");
        let role = "test_role";
        let id = DelegationId::from_component_ids(
            &delegate_id,
            &responsible_id,
            Some(&activity_id),
            Some(role),
        );
        let role = Role::from(role.to_string());
        ChronicleOperation::AgentActsOnBehalfOf(ActsOnBehalfOf {
            namespace,
            id,
            responsible_id,
            delegate_id,
            activity_id: Some(activity_id),
            role: Some(role),
        })
    }

    fn signed_identity_helper() -> SignedIdentity {
        let keystore = DirectoryStoredKeys::new(TempDir::new().unwrap().path()).unwrap();
        keystore.generate_chronicle().unwrap();
        AuthId::chronicle().signed_identity(&keystore).unwrap()
    }

    #[tokio::test]
    async fn test_submission_serialization_deserialization() -> Result<(), ApplyError> {
        // Example transaction payload of `CreateNamespace`,
        // `AgentExists`, and `AgentActsOnBehalfOf` `ChronicleOperation`s
        let tx = ChronicleTransaction::new(
            vec![
                create_namespace_helper(None),
                agent_exists_helper(),
                create_agent_acts_on_behalf_of(),
            ],
            signed_identity_helper(),
        );

        // Serialize operations payload to protocol buffer
        let submission = create_operation_submission_request(&tx).await.unwrap();
        let serialized_sub = serialize_submission(&submission);

        // Test that serialisation to and from protocol buffer is symmetric
        assert_eq!(
            tx.tx,
            chronicle_operations_from_submission(
                deserialize_submission(&serialized_sub)
                    // handle DecodeError
                    .map_err(|e| ApplyError::InternalError(e.to_string()))?
                    .body
            )
            .await
            // handle ProcessorError
            .map_err(|e| ApplyError::InternalError(e.to_string()))?
        );
        Ok(())
    }
}
