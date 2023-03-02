use std::io::Cursor;

use async_sawtooth_sdk::{error::SawtoothCommunicationError, ledger::LedgerEvent};
use common::{
    identity::SignedIdentity,
    prov::{
        operations::ChronicleOperation, to_json_ld::ToJson, CompactionError, Contradiction,
        ExpandedJson, ProcessorError, ProvModel,
    },
};
use prost::Message;
use tracing::span;

use thiserror::Error;

use self::messages::event::OptionContradiction;

#[derive(Debug)]
pub struct ChronicleOperationEvent(pub Result<ProvModel, Contradiction>);

impl From<ChronicleOperationEvent> for Result<ProvModel, Contradiction> {
    fn from(val: ChronicleOperationEvent) -> Self {
        val.0
    }
}

#[async_trait::async_trait]
impl LedgerEvent for ChronicleOperationEvent {
    async fn deserialize(
        buf: &[u8],
    ) -> Result<(Self, u64), async_sawtooth_sdk::error::SawtoothCommunicationError>
    where
        Self: Sized,
    {
        let event = messages::Event::decode(buf)
            .map_err(|e| SawtoothCommunicationError::LedgerEventParse { source: e.into() })?;
        // Spans of zero panic, so assign a dummy value until we thread the span correctly
        let span_id = {
            if event.span_id == 0 {
                span::Id::from_u64(0xffffffffffffffff)
            } else {
                span::Id::from_u64(event.span_id)
            }
        };
        let model = match (event.delta, event.option_contradiction) {
            (_, Some(OptionContradiction::Contradiction(contradiction))) => Err(
                serde_json::from_str::<Contradiction>(&contradiction).map_err(|e| {
                    SawtoothCommunicationError::LedgerEventParse { source: e.into() }
                })?,
            ),
            (delta, None) => {
                let mut model = ProvModel::default();
                model.apply_json_ld_str(&delta).await.map_err(|e| {
                    SawtoothCommunicationError::LedgerEventParse { source: e.into() }
                })?;

                Ok(model)
            }
        };
        Ok((Self(model), span_id.into_u64()))
    }
}

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
        let json = serde_json::from_str(op)?;
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
