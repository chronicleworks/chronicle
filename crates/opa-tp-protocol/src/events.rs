use prost::Message;

use crate::messages::{self, OpaEvent};

pub fn opa_event(span_id: u64, value: Result<serde_json::Value, String>) -> Vec<u8> {
    OpaEvent {
        version: crate::PROTOCOL_VERSION.to_string(),
        span_id,
        payload: {
            match value {
                Ok(value) => Some(messages::opa_event::Payload::Operation(value.to_string())),
                Err(error) => Some(messages::opa_event::Payload::Operation(
                    serde_json::json!({ "error": error }).to_string(),
                )),
            }
        },
    }
    .encode_to_vec()
}
