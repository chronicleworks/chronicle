use crate::prov::{operations::ChronicleOperation, to_json_ld::ToJson};

// Include the `submission` module, which is generated from ./protos/submission.proto.
mod submission {
    include!(concat!(env!("OUT_DIR"), "/_.rs"));
}

pub fn submit(op: &ChronicleOperation) -> submission::Submission {
    let mut submission = submission::Submission::default();
    let protocol_version = "1".to_string();
    submission.version = protocol_version;
    submission.span_id = "".to_string();
    submission.body = op.to_json().0.to_string();
    submission
}
