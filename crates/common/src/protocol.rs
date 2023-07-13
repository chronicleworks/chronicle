use std::io::Cursor;

use prost::Message;
use tracing::span;

use crate::{
    identity::SignedIdentity,
    prov::{
        operations::ChronicleOperation, to_json_ld::ToJson, ChronicleTransaction, CompactionError,
        Contradiction, ExpandedJson, ProcessorError, ProvModel,
    },
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

static PROTOCOL_VERSION: &str = "2";

// Include the `submission` module, which is
// generated from ./protos/submission.proto.
pub mod messages {
    #![allow(clippy::derive_partial_eq_without_eq)]

    include!(concat!(env!("OUT_DIR"), "/_.rs"));
}
