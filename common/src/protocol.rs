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
