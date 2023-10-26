use async_stl_client::{
	error::SawtoothCommunicationError,
	ledger::{LedgerEvent, Span},
};
use prost::Message;
use serde_json::json;

use crate::{
	messages::{self, OpaEvent},
	state::OpaOperationEvent,
};

#[async_trait::async_trait]
impl LedgerEvent for OpaOperationEvent {
	async fn deserialize(buf: &[u8]) -> Result<(Self, Span), SawtoothCommunicationError>
	where
		Self: Sized,
	{
		let ev = OpaEvent::decode(buf)?;
		if let Some(payload) = ev.payload {
			match payload {
				messages::opa_event::Payload::Operation(value) => {
					let value = serde_json::from_str(&value)?;
					Ok((value, Span::Span(ev.span_id)))
				},
				messages::opa_event::Payload::Error(value) =>
					Ok((OpaOperationEvent::Error(value), Span::Span(ev.span_id))),
			}
		} else {
			Err(SawtoothCommunicationError::MalformedMessage)
		}
	}
}

pub fn opa_event(span_id: u64, value: OpaOperationEvent) -> Result<Vec<u8>, serde_json::Error> {
	Ok(OpaEvent {
		version: crate::PROTOCOL_VERSION.to_string(),
		span_id,
		payload: {
			match value {
				OpaOperationEvent::Error(e) => Some(messages::opa_event::Payload::Error(
					serde_json::to_string(&json!({ "error": e }))?,
				)),
				value =>
					Some(messages::opa_event::Payload::Operation(serde_json::to_string(&value)?)),
			}
		},
	}
	.encode_to_vec())
}
