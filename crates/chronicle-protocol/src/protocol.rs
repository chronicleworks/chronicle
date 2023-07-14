use std::io::Cursor;

use async_stl_client::{
    error::SawtoothCommunicationError,
    ledger::{LedgerEvent, Span},
};
use common::{
    identity::SignedIdentity,
    prov::{
        operations::ChronicleOperation, to_json_ld::ToJson, CompactionError, Contradiction,
        ExpandedJson, PayloadError, ProcessorError, ProvModel,
    },
};
use prost::Message;
use tracing::span;

use thiserror::Error;

use self::messages::event::OptionContradiction;

#[derive(Debug)]
pub struct ChronicleOperationEvent(pub Result<ProvModel, Contradiction>, pub SignedIdentity);

impl From<ChronicleOperationEvent> for Result<ProvModel, Contradiction> {
    fn from(val: ChronicleOperationEvent) -> Self {
        val.0
    }
}

#[async_trait::async_trait]
impl LedgerEvent for ChronicleOperationEvent {
    async fn deserialize(
        buf: &[u8],
    ) -> Result<(Self, Span), async_stl_client::error::SawtoothCommunicationError>
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

        let identity = {
            if event.identity.is_empty() {
                SignedIdentity::new_no_identity()
            } else {
                serde_json::from_str(&event.identity).map_err(|e| {
                    SawtoothCommunicationError::LedgerEventParse { source: e.into() }
                })?
            }
        };
        Ok((Self(model, identity), Span::Span(span_id.into_u64())))
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

static PROTOCOL_VERSION: &str = "2";

// Include the `submission` module, which is
// generated from ./protos/submission.proto.
pub mod messages {
    #![allow(clippy::derive_partial_eq_without_eq)]

    include!(concat!(env!("OUT_DIR"), "/_.rs"));
}

pub async fn chronicle_committed(
    span: u64,
    delta: ProvModel,
    identity: &SignedIdentity,
) -> Result<messages::Event, ProtocolError> {
    Ok(messages::Event {
        version: PROTOCOL_VERSION.to_owned(),
        delta: serde_json::to_string(&delta.to_json().compact_stable_order().await?)?,
        span_id: span,
        identity: serde_json::to_string(identity)?,
        ..Default::default()
    })
}

pub fn chronicle_contradicted(
    span: u64,
    contradiction: &Contradiction,
    identity: &SignedIdentity,
) -> Result<messages::Event, ProtocolError> {
    Ok(messages::Event {
        version: PROTOCOL_VERSION.to_owned(),
        span_id: span,
        option_contradiction: Some(OptionContradiction::Contradiction(serde_json::to_string(
            &contradiction,
        )?)),
        identity: serde_json::to_string(identity)?,
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
/// strings to a vector of `ChronicleOperation`s.
/// Operates for version 1 of the protocol.
pub async fn chronicle_operations_from_submission_v1(
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

/// Convert a `Submission` payload from a vector of
/// strings to a vector of `ChronicleOperation`s.
/// Operates for version 2 of the protocol.
pub async fn chronicle_operations_from_submission_v2(
    submission_body: String,
) -> Result<Vec<ChronicleOperation>, ProcessorError> {
    use serde_json::{json, Value};
    let json = serde_json::from_str(&submission_body)?;
    if let Value::Object(map) = json {
        if let Some(version) = map.get("version") {
            if version == &json!(1) {
                if let Some(Value::Array(ops_json)) = map.get("ops") {
                    let mut ops = Vec::with_capacity(ops_json.len());
                    for op in ops_json {
                        ops.push(ChronicleOperation::from_json(ExpandedJson(op.clone())).await?);
                    }
                    Ok(ops)
                } else {
                    Err(PayloadError::OpsNotAList.into())
                }
            } else {
                Err(PayloadError::VersionUnknown.into())
            }
        } else {
            Err(PayloadError::VersionMissing.into())
        }
    } else {
        Err(PayloadError::NotAnObject.into())
    }
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
    use crate::protocol::{
        chronicle_operations_from_submission_v1, chronicle_operations_from_submission_v2,
        ChronicleOperation,
    };
    use chrono::{NaiveDateTime, TimeZone, Utc};
    use common::prov::{
        operations::{EndActivity, StartActivity},
        to_json_ld::ToJson,
        ActivityId, NamespaceId,
    };
    use serde_json::{json, Value};
    use std::{
        collections::hash_map::DefaultHasher,
        hash::{Hash, Hasher},
    };
    use uuid::Uuid;

    fn construct_operations() -> Vec<ChronicleOperation> {
        let mut hasher = DefaultHasher::new();
        "foo".hash(&mut hasher);
        let n1 = hasher.finish();
        "bar".hash(&mut hasher);
        let n2 = hasher.finish();
        let uuid = Uuid::from_u64_pair(n1, n2);

        let base_ms = 1234567654321;
        let activity_start =
            Utc.from_utc_datetime(&NaiveDateTime::from_timestamp_millis(base_ms).unwrap());
        let activity_end =
            Utc.from_utc_datetime(&NaiveDateTime::from_timestamp_millis(base_ms + 12345).unwrap());

        let start = ChronicleOperation::StartActivity(StartActivity {
            namespace: NamespaceId::from_external_id("test-namespace", uuid),
            id: ActivityId::from_external_id("test-activity"),
            time: activity_start,
        });
        let end = ChronicleOperation::EndActivity(EndActivity {
            namespace: NamespaceId::from_external_id("test-namespace", uuid),
            id: ActivityId::from_external_id("test-activity"),
            time: activity_end,
        });

        vec![start, end]
    }

    #[tokio::test]
    async fn deserialize_submission_v1() {
        let operations_expected = construct_operations();

        let submission_body = operations_expected
            .iter()
            .map(|operation| serde_json::to_string(&operation.to_json().0).unwrap())
            .collect();

        let operations_actual = chronicle_operations_from_submission_v1(submission_body)
            .await
            .unwrap();

        assert_eq!(operations_expected, operations_actual);
    }

    #[tokio::test]
    async fn deserialize_submission_v2() {
        let operations_expected = construct_operations();

        let submission_body =
            serde_json::to_string(&json!({"version": 1, "ops": operations_expected
            .iter()
            .map(|operation| operation.to_json().0)
            .collect::<Vec<Value>>()}))
            .unwrap();

        let operations_actual = chronicle_operations_from_submission_v2(submission_body)
            .await
            .unwrap();

        assert_eq!(operations_expected, operations_actual);
    }
}
