use arrow_array::{Array, RecordBatch};

use common::{
	attributes::Attributes,
	prov::{
		operations::{ChronicleOperation, SetAttributes},
		ActivityId, EntityId, NamespaceId,
	},
};

use futures::StreamExt;

use crate::ChronicleArrowError;

use super::{string_list_column, with_implied};

fn get_used(
	record_batch: &RecordBatch,
	row_index: usize,
) -> Result<Vec<String>, ChronicleArrowError> {
	string_list_column(record_batch, "used", row_index)
}

fn get_generated(
	record_batch: &RecordBatch,
	row_index: usize,
) -> Result<Vec<String>, ChronicleArrowError> {
	string_list_column(record_batch, "generated", row_index)
}

fn get_was_informed_by(
	record_batch: &RecordBatch,
	row_index: usize,
) -> Result<Vec<String>, ChronicleArrowError> {
	string_list_column(record_batch, "was_informed_by", row_index)
}

fn opt_time_column(
	record_batch: &RecordBatch,
	column_name: &str,
	row_index: usize,
) -> Result<Option<chrono::DateTime<chrono::Utc>>, ChronicleArrowError> {
	let column_index = record_batch
		.schema()
		.index_of(column_name)
		.map_err(|_| ChronicleArrowError::MissingColumn(column_name.to_string()))?;
	let column = record_batch.column(column_index);

	if let Some(timestamp_array) =
		column.as_any().downcast_ref::<arrow_array::TimestampNanosecondArray>()
	{
		let naive_time = timestamp_array.value_as_datetime(row_index);
		let time = naive_time
			.map(|nt| chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(nt, chrono::Utc));
		Ok(time)
	} else {
		Ok(None)
	}
}

fn get_started(
	record_batch: &RecordBatch,
	row_index: usize,
) -> Result<Option<chrono::DateTime<chrono::Utc>>, ChronicleArrowError> {
	opt_time_column(record_batch, "started", row_index)
}

fn get_ended(
	record_batch: &RecordBatch,
	row_index: usize,
) -> Result<Option<chrono::DateTime<chrono::Utc>>, ChronicleArrowError> {
	opt_time_column(record_batch, "ended", row_index)
}

pub fn activity_operations(
	ns: &NamespaceId,
	id: &str,
	attributes: Attributes,
	row_index: usize,
	record_batch: &RecordBatch,
) -> Result<Vec<ChronicleOperation>, ChronicleArrowError> {
	let mut operations = vec![
		ChronicleOperation::activity_exists(ns.clone(), ActivityId::from_external_id(id)),
		ChronicleOperation::set_attributes(SetAttributes::activity(
			ns.clone(),
			ActivityId::from_external_id(id),
			attributes,
		)),
	];

	let generated_ids = get_generated(record_batch, row_index)?;

	for entity_id in generated_ids {
		operations.push(ChronicleOperation::was_generated_by(
			ns.clone(),
			EntityId::from_external_id(&entity_id),
			ActivityId::from_external_id(id),
		));
	}

	let used_ids = get_used(record_batch, row_index)?;

	for used_id in used_ids {
		operations.push(ChronicleOperation::activity_used(
			ns.clone(),
			ActivityId::from_external_id(id),
			EntityId::from_external_id(&used_id),
		));
	}

	let was_informed_by_ids = get_was_informed_by(record_batch, row_index)?;

	for informed_by_id in was_informed_by_ids {
		operations.push(ChronicleOperation::was_informed_by(
			ns.clone(),
			ActivityId::from_external_id(id),
			ActivityId::from_external_id(&informed_by_id),
		));
	}

	let started = get_started(record_batch, row_index)?;

	if let Some(started) = started {
		operations.push(ChronicleOperation::start_activity(
			ns.clone(),
			ActivityId::from_external_id(id),
			started,
		));
	}

	let ended = get_ended(record_batch, row_index)?;

	if let Some(ended) = ended {
		operations.push(ChronicleOperation::end_activity(
			ns.clone(),
			ActivityId::from_external_id(id),
			ended,
		));
	}

	Ok(with_implied(operations))
}
