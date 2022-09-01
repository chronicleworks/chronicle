use std::io::Cursor;

use prost::Message;

use crate::prov::{operations::ChronicleOperation, to_json_ld::ToJson};

// Include the `submission` module, which is generated from ./protos/submission.proto.
mod submission {
    include!(concat!(env!("OUT_DIR"), "/_.rs"));
}

pub fn create_operation_submission_request(op: &ChronicleOperation) -> submission::Submission {
    let mut submission = submission::Submission::default();
    let protocol_version = "1".to_string();
    submission.version = protocol_version;
    submission.span_id = "".to_string();
    submission.body = op.to_json().0.to_string();
    submission
}

pub fn serialize_submission(submission: &submission::Submission) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.reserve(submission.encoded_len());
    submission.encode(&mut buf).unwrap();
    buf
}

pub fn deserialize_submission(buf: &[u8]) -> Result<submission::Submission, prost::DecodeError> {
    submission::Submission::decode(&mut Cursor::new(buf))
}
