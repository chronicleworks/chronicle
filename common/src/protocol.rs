use std::io::Cursor;

use prost::Message;

use crate::prov::{
    operations::ChronicleOperation, to_json_ld::ToJson, ExpandedJson, ProcessorError,
};

// Include the `submission` module, which is
// generated from ./protos/submission.proto.
pub mod submission {
    include!(concat!(env!("OUT_DIR"), "/_.rs"));
}

/// Envelope a payload of `ChronicleOperations`
/// in a `Submission` protocol buffer along with
/// placeholders for protocol version and a
/// tracing span id.
pub fn create_operation_submission_request(
    payload: &[ChronicleOperation],
) -> submission::Submission {
    let mut submission = submission::Submission::default();
    let protocol_version = "1".to_string();
    submission.version = protocol_version;
    submission.span_id = "".to_string();
    let mut ops = Vec::with_capacity(payload.len());
    for op in payload {
        let op_string = op.to_json().0.to_string();
        ops.push(op_string);
    }
    submission.body = ops;
    submission
}

/// `Submission` protocol buffer serializer
pub fn serialize_submission(submission: &submission::Submission) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.reserve(submission.encoded_len());
    submission.encode(&mut buf).unwrap();
    buf
}

/// `Submission` protocol buffer deserializer
pub fn deserialize_submission(buf: &[u8]) -> Result<submission::Submission, prost::DecodeError> {
    submission::Submission::decode(&mut Cursor::new(buf))
}

/// Convert a `Submission` payload from a vector of
/// strings to a vector of `ChronicleOperation`s
pub async fn chronicle_operations_from_submission(
    submission_body: Vec<String>,
) -> Result<Vec<ChronicleOperation>, ProcessorError> {
    let mut ops = Vec::with_capacity(submission_body.len());
    for op in submission_body.iter() {
        let json = json::parse(op)?;
        let exp_json = ExpandedJson(json);
        let op = ChronicleOperation::from_json(exp_json).await?;
        ops.push(op);
    }
    Ok(ops)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::prov::{
        operations::{ActsOnBehalfOf, AgentExists, ChronicleOperation, CreateNamespace},
        ActivityId, AgentId, DelegationId, Name, NamePart, NamespaceId, Role,
    };
    use api::UuidGen;
    use sawtooth_sdk::processor::handler::ApplyError;
    use uuid::Uuid;

    #[derive(Debug, Clone)]
    struct SameUuid;

    impl UuidGen for SameUuid {
        fn uuid() -> Uuid {
            Uuid::parse_str("5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea").unwrap()
        }
    }

    fn create_namespace_id_helper(tag: Option<i32>) -> NamespaceId {
        let name = if tag.is_none() || tag == Some(0) {
            "testns".to_string()
        } else {
            format!("testns{}", tag.unwrap())
        };
        NamespaceId::from_name(&name, SameUuid::uuid())
    }

    fn create_namespace_helper(tag: Option<i32>) -> ChronicleOperation {
        let id = create_namespace_id_helper(tag);
        let name = &id.name_part().to_string();
        ChronicleOperation::CreateNamespace(CreateNamespace::new(id, name, SameUuid::uuid()))
    }

    fn agent_exists_helper() -> ChronicleOperation {
        let namespace: NamespaceId = NamespaceId::from_name("testns", SameUuid::uuid());
        let name: Name = NamePart::name_part(&AgentId::from_name("test_agent")).clone();
        ChronicleOperation::AgentExists(AgentExists { namespace, name })
    }

    fn create_agent_acts_on_behalf_of() -> ChronicleOperation {
        let namespace: NamespaceId = NamespaceId::from_name("testns", SameUuid::uuid());
        let responsible_id = AgentId::from_name("test_agent");
        let delegate_id = AgentId::from_name("test_delegate");
        let activity_id = ActivityId::from_name("test_activity");
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

    #[tokio::test]
    async fn test_submission_serialization_deserialization() -> Result<(), ApplyError> {
        // Example transaction payload of `CreateNamespace`,
        // `AgentExists`, and `AgentActsOnBehalfOf` `ChronicleOperation`s
        let tx = vec![
            create_namespace_helper(None),
            agent_exists_helper(),
            create_agent_acts_on_behalf_of(),
        ];

        // Serialize operations payload to protocol buffer
        let submission = create_operation_submission_request(&tx);
        let serialized_sub = serialize_submission(&submission);

        // Test that serialisation to and from protocol buffer is symmetric
        assert_eq!(
            tx,
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
