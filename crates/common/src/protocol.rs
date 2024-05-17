use std::io::Cursor;

use prost::Message;
use thiserror::Error;
use tracing::span;

use crate::{
    identity::SignedIdentity,
    prov::{
        ChronicleTransaction, CompactionError, Contradiction, ExpandedJson,
        operations::ChronicleOperation, ProcessorError, ProvModel, to_json_ld::ToJson,
    },
};

use self::messages::event::OptionContradiction;

#[derive(Error, Debug)]
pub enum ProtocolError {
    #[error("Protobuf deserialization error {source}")]
    ProtobufDeserialize {
        #[from]
        #[source]
        source: prost::DecodeError,
    },
    #[error("Protobuf serialization error {source}")]
    ProtobufSerialize {
        #[from]
        #[source]
        source: prost::EncodeError,
    },
    #[error("Serde de/serialization error {source}")]
    JsonSerialize {
        #[from]
        #[source]
        source: serde_json::Error,
    },
    #[error("Problem applying delta {source}")]
    ProcessorError {
        #[from]
        #[source]
        source: ProcessorError,
    },
    #[error("Could not compact json {source}")]
    Compaction {
        #[from]
        #[source]
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
