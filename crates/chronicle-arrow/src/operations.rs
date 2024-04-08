use api::{
	commands::{ApiCommand, ImportCommand},
	ApiDispatch,
};
use arrow::array::AsArray;
use arrow_array::{Array, BooleanArray, Int64Array, RecordBatch, StringArray};
use arrow_flight::{FlightData, FlightDescriptor, FlightEndpoint, FlightInfo, SchemaAsIpc, Ticket};
use arrow_ipc::writer::{DictionaryTracker, IpcDataGenerator, IpcWriteOptions};
use arrow_schema::ArrowError;

use common::{
	attributes::{Attribute, Attributes},
	domain::TypeName,
	identity::AuthId,
	prov::{
		operations::{ChronicleOperation, DerivationType, SetAttributes},
		ActivityId, AgentId, EntityId, NamespaceId, Role,
	},
};
use diesel::{r2d2::ConnectionManager, PgConnection};
use futures::{
	stream::{self, BoxStream},
	StreamExt,
};
use r2d2::Pool;
use std::sync::Arc;
use tokio::task::spawn_blocking;
use tonic::Status;
use tracing::instrument;
use uuid::Uuid;

use crate::{
	meta::{get_domain_type_meta_from_cache, DomainTypeMeta, Term},
	query::{
		activity_count_by_type, agent_count_by_type, entity_count_by_type, ActedOnBehalfOfRef,
		AgentAttributionRef, DerivationRef, EntityAttributionRef,
	},
	ChronicleArrowError, ChronicleTicket,
};

#[tracing::instrument(skip(record_batch))]
pub async fn process_record_batch(
	descriptor_path: &Vec<String>,
	record_batch: RecordBatch,
	api: &ApiDispatch,
) -> Result<(), ChronicleArrowError> {
	let domain_type_meta = get_domain_type_meta_from_cache(descriptor_path)
		.ok_or(ChronicleArrowError::MetadataNotFound)?;

	let attribute_columns = domain_type_meta
		.schema
		.fields()
		.iter()
		.filter_map(|field| {
			if field.name().ends_with("Attribute") {
				Some(field.name().clone())
			} else {
				None
			}
		})
		.collect::<Vec<String>>();

	match domain_type_meta.term {
		Term::Entity => {
			create_chronicle_entity(&domain_type_meta.typ, &record_batch, &attribute_columns, api)
				.await?
		},
		Term::Activity => {
			create_chronicle_activity(&domain_type_meta.typ, &record_batch, &attribute_columns, api)
				.await?
		},
		Term::Agent => {
			create_chronicle_agent(&domain_type_meta.typ, &record_batch, &attribute_columns, api)
				.await?
		},
		Term::Namespace => create_chronicle_namespace(&record_batch, api).await?,
	}
	Ok(())
}

#[tracing::instrument(skip(descriptor, meta, batch))]
pub fn batch_to_flight_data(
	descriptor: &FlightDescriptor,
	meta: &DomainTypeMeta,
	batch: RecordBatch,
) -> Result<Vec<FlightData>, ArrowError> {
	let options = IpcWriteOptions::default();

	let schema_flight_data: FlightData =
		std::convert::Into::<FlightData>::into(SchemaAsIpc::new(&meta.schema, &options))
			.with_descriptor(descriptor.clone());

	let data_gen = IpcDataGenerator::default();
	let mut dictionary_tracker = DictionaryTracker::new(false);

	let (encoded_dictionaries, encoded_batch) =
		data_gen.encoded_batch(&batch, &mut dictionary_tracker, &options)?;

	let dictionaries: Vec<FlightData> = encoded_dictionaries.into_iter().map(Into::into).collect();
	let flight_data: FlightData = encoded_batch.into();

	let mut stream = vec![schema_flight_data];
	stream.extend(dictionaries);
	stream.push(flight_data);

	Ok(stream)
}

async fn create_chronicle_namespace(
	record_batch: &RecordBatch,
	api: &ApiDispatch,
) -> Result<(), ChronicleArrowError> {
	let uuid = record_batch
		.column_by_name("uuid")
		.ok_or(ChronicleArrowError::MissingColumn("uuid".to_string()))?;
	let name = record_batch
		.column_by_name("name")
		.ok_or(ChronicleArrowError::MissingColumn("name".to_string()))?;

	Ok(())
}

pub async fn create_chronicle_entity(
	domain_type: &Option<Box<dyn TypeName + Send + Sync>>,
	record_batch: &RecordBatch,
	attribute_columns: &Vec<String>,
	api: &ApiDispatch,
) -> Result<(), ChronicleArrowError> {
	create_chronicle_terms(record_batch, Term::Entity, domain_type, attribute_columns, api).await
}

pub async fn create_chronicle_activity(
	domain_type: &Option<Box<dyn TypeName + Send + Sync>>,
	record_batch: &RecordBatch,
	attribute_columns: &Vec<String>,
	api: &ApiDispatch,
) -> Result<(), ChronicleArrowError> {
	create_chronicle_terms(record_batch, Term::Activity, domain_type, attribute_columns, api).await
}

pub async fn create_chronicle_agent(
	domain_type: &Option<Box<dyn TypeName + Send + Sync>>,
	record_batch: &RecordBatch,
	attribute_columns: &Vec<String>,
	api: &ApiDispatch,
) -> Result<(), ChronicleArrowError> {
	create_chronicle_terms(record_batch, Term::Agent, domain_type, attribute_columns, api).await
}

pub async fn create_chronicle_terms(
	record_batch: &RecordBatch,
	record_type: Term,
	domain_type: &Option<Box<dyn TypeName + Send + Sync>>,
	attribute_columns: &Vec<String>,
	api: &ApiDispatch,
) -> Result<(), ChronicleArrowError> {
	let ns_name_column = record_batch
		.column_by_name("namespace_name")
		.ok_or(ChronicleArrowError::MissingColumn("namespace_name".to_string()))?;

	let ns_uuid_column = record_batch
		.column_by_name("namespace_uuid")
		.ok_or(ChronicleArrowError::MissingColumn("namespace_uuid".to_string()))?;

	let id_column = record_batch
		.column_by_name("id")
		.ok_or(ChronicleArrowError::MissingColumn("id".to_string()))?;

	let attribute_columns_refs: Vec<&String> = attribute_columns.iter().collect();
	let attribute_values = attribute_columns_refs
		.iter()
		.map(|column_name| (column_name.to_string(), record_batch.column_by_name(column_name)))
		.filter_map(|(column_name, array_ref)| array_ref.map(|array_ref| (column_name, array_ref)))
		.collect::<Vec<(_, _)>>();

	tracing::debug!(?attribute_columns, "Processing attribute columns");

	let mut operations = Vec::new();
	for row_index in 0..record_batch.num_rows() {
		let ns_name = ns_name_column.as_string::<i32>().value(row_index);
		let ns_uuid = ns_uuid_column.as_string::<i32>().value(row_index);
		let ns_uuid = Uuid::parse_str(ns_uuid).map_err(ChronicleArrowError::from)?;
		let ns: NamespaceId = NamespaceId::from_external_id(ns_name, ns_uuid);

		let id = id_column.as_string::<i32>().value(row_index);

		let mut attributes: Vec<Attribute> = Vec::new();

		for (attribute_name, attribute_array) in attribute_values.iter() {
			tracing::trace!(%attribute_name, row_index, "Appending to attributes");
			if let Some(array) = attribute_array.as_any().downcast_ref::<StringArray>() {
				let value = array.value(row_index);
				attributes.push(Attribute::new(
					attribute_name.clone(),
					serde_json::Value::String(value.to_string()),
				));
			} else if let Some(array) = attribute_array.as_any().downcast_ref::<Int64Array>() {
				let value = array.value(row_index);
				attributes.push(Attribute::new(
					attribute_name.clone(),
					serde_json::Value::Number(value.into()),
				));
			} else if let Some(array) = attribute_array.as_any().downcast_ref::<BooleanArray>() {
				let value = array.value(row_index);
				attributes
					.push(Attribute::new(attribute_name.clone(), serde_json::Value::Bool(value)));
			} else {
				tracing::warn!(%attribute_name, row_index, "Unsupported attribute type");
			}
		}
		let attributes =
			Attributes::new(domain_type.as_ref().map(|x| x.as_domain_type_id()), attributes);

		match record_type {
			Term::Entity => {
				operations.extend(entity_operations(&ns, id, attributes, row_index, record_batch)?);
			},
			Term::Activity => {
				operations.extend(activity_operations(
					&ns,
					id,
					attributes,
					row_index,
					record_batch,
				)?);
			},
			Term::Agent => {
				operations.extend(agent_operations(&ns, id, attributes, row_index, record_batch)?);
			},
			Term::Namespace => {
				// Noop / unreachable
			},
		}
	}

	api.dispatch(ApiCommand::Import(ImportCommand { operations }), AuthId::anonymous())
		.await?;

	Ok(())
}

fn string_list_column(
	record_batch: &RecordBatch,
	column_name: &str,
	row_index: usize,
) -> Result<Vec<String>, ChronicleArrowError> {
	let column_index = record_batch
		.schema()
		.index_of(column_name)
		.map_err(|_| ChronicleArrowError::MissingColumn(column_name.to_string()))?;
	let column = record_batch.column(column_index);
	if let Some(list_array) = column.as_any().downcast_ref::<arrow_array::ListArray>() {
		if let Some(string_array) =
			list_array.value(row_index).as_any().downcast_ref::<arrow_array::StringArray>()
		{
			Ok((0..string_array.len()).map(|i| string_array.value(i).to_string()).collect())
		} else {
			Ok(vec![])
		}
	} else {
		Ok(vec![])
	}
}

fn struct_2_list_column_opt_string(
	record_batch: &RecordBatch,
	column_name: &str,
	row_index: usize,
	field1_name: &str,
	field2_name: &str,
) -> Result<Vec<(String, Option<String>)>, ChronicleArrowError> {
	let column_index = record_batch
		.schema()
		.index_of(column_name)
		.map_err(|_| ChronicleArrowError::MissingColumn(column_name.to_string()))?;
	let column = record_batch.column(column_index);
	if let Some(list_array) = column.as_any().downcast_ref::<arrow_array::ListArray>() {
		if let Some(struct_array) =
			list_array.value(row_index).as_any().downcast_ref::<arrow_array::StructArray>()
		{
			let field1_index = struct_array
				.column_by_name(field1_name)
				.ok_or_else(|| ChronicleArrowError::MissingColumn(field1_name.to_string()))?;
			let field2_index = struct_array
				.column_by_name(field2_name)
				.ok_or_else(|| ChronicleArrowError::MissingColumn(field2_name.to_string()))?;

			let field1_array = field1_index
				.as_any()
				.downcast_ref::<arrow_array::StringArray>()
				.ok_or_else(|| ChronicleArrowError::ColumnTypeMismatch(field1_name.to_string()))?;
			let field2_array = field2_index.as_any().downcast_ref::<arrow_array::StringArray>();

			Ok((0..struct_array.len())
				.map(|i| {
					(
						field1_array.value(i).to_string(),
						field2_array.map(|arr| arr.value(i).to_string()),
					)
				})
				.collect())
		} else {
			Ok(vec![])
		}
	} else {
		Ok(vec![])
	}
}

fn struct_3_list_column_opt_string(
	record_batch: &RecordBatch,
	column_name: &str,
	row_index: usize,
	field1_name: &str,
	field2_name: &str,
	field3_name: &str,
) -> Result<Vec<(String, Option<String>, Option<String>)>, ChronicleArrowError> {
	let column_index = record_batch
		.schema()
		.index_of(column_name)
		.map_err(|_| ChronicleArrowError::MissingColumn(column_name.to_string()))?;
	let column = record_batch.column(column_index);
	if let Some(list_array) = column.as_any().downcast_ref::<arrow_array::ListArray>() {
		if let Some(struct_array) =
			list_array.value(row_index).as_any().downcast_ref::<arrow_array::StructArray>()
		{
			let field1_index = struct_array
				.column_by_name(field1_name)
				.ok_or_else(|| ChronicleArrowError::MissingColumn(field1_name.to_string()))?;
			let field2_index = struct_array
				.column_by_name(field2_name)
				.ok_or_else(|| ChronicleArrowError::MissingColumn(field2_name.to_string()))?;
			let field3_index = struct_array
				.column_by_name(field3_name)
				.ok_or_else(|| ChronicleArrowError::MissingColumn(field3_name.to_string()))?;

			let field1_array = field1_index
				.as_any()
				.downcast_ref::<arrow_array::StringArray>()
				.ok_or_else(|| ChronicleArrowError::ColumnTypeMismatch(field1_name.to_string()))?;
			let field2_array = field2_index.as_any().downcast_ref::<arrow_array::StringArray>();
			let field3_array = field3_index.as_any().downcast_ref::<arrow_array::StringArray>();

			Ok((0..struct_array.len())
				.map(|i| {
					(
						field1_array.value(i).to_string(),
						field2_array.map(|arr| arr.value(i).to_string()),
						field3_array.map(|arr| arr.value(i).to_string()),
					)
				})
				.collect::<Vec<(String, Option<String>, Option<String>)>>())
		} else {
			Ok(vec![])
		}
	} else {
		Ok(vec![])
	}
}

fn struct_2_list_column(
	record_batch: &RecordBatch,
	column_name: &str,
	row_index: usize,
	field1_name: &str,
	field2_name: &str,
) -> Result<Vec<(String, String)>, ChronicleArrowError> {
	let column_index = record_batch
		.schema()
		.index_of(column_name)
		.map_err(|_| ChronicleArrowError::MissingColumn(column_name.to_string()))?;
	let column = record_batch.column(column_index);
	if let Some(list_array) = column.as_any().downcast_ref::<arrow_array::ListArray>() {
		if let Some(struct_array) =
			list_array.value(row_index).as_any().downcast_ref::<arrow_array::StructArray>()
		{
			let field1_index = struct_array
				.column_by_name(field1_name)
				.ok_or_else(|| ChronicleArrowError::MissingColumn(field1_name.to_string()))?;
			let field2_index = struct_array
				.column_by_name(field2_name)
				.ok_or_else(|| ChronicleArrowError::MissingColumn(field2_name.to_string()))?;

			if let (Some(field1_array), Some(field2_array)) = (
				field1_index.as_any().downcast_ref::<arrow_array::StringArray>(),
				field2_index.as_any().downcast_ref::<arrow_array::StringArray>(),
			) {
				Ok((0..struct_array.len())
					.map(|i| (field1_array.value(i).to_string(), field2_array.value(i).to_string()))
					.collect())
			} else {
				Ok(vec![])
			}
		} else {
			Ok(vec![])
		}
	} else {
		Ok(vec![])
	}
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

fn get_was_generated_by(
	record_batch: &RecordBatch,
	row_index: usize,
) -> Result<Vec<String>, ChronicleArrowError> {
	string_list_column(record_batch, "was_generated_by", row_index)
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

fn get_used(
	record_batch: &RecordBatch,
	row_index: usize,
) -> Result<Vec<String>, ChronicleArrowError> {
	string_list_column(record_batch, "used", row_index)
}

fn get_agent_attribution(
	record_batch: &RecordBatch,
	row_index: usize,
) -> Result<Vec<AgentAttributionRef>, ChronicleArrowError> {
	Ok(struct_2_list_column_opt_string(
		record_batch,
		"was_attributed_to",
		row_index,
		"entity",
		"role",
	)?
	.into_iter()
	.map(|(entity, role)| AgentAttributionRef { entity, role })
	.collect())
}

fn get_acted_on_behalf_of(
	record_batch: &RecordBatch,
	row_index: usize,
) -> Result<Vec<ActedOnBehalfOfRef>, ChronicleArrowError> {
	Ok(struct_3_list_column_opt_string(
		record_batch,
		"acted_on_behalf_of",
		row_index,
		"agent",
		"role",
		"activity",
	)?
	.into_iter()
	.map(|(agent, role, activity)| ActedOnBehalfOfRef { agent, role, activity })
	.collect())
}

fn get_entity_was_attributed_to(
	record_batch: &RecordBatch,
	row_index: usize,
) -> Result<Vec<EntityAttributionRef>, ChronicleArrowError> {
	Ok(struct_2_list_column_opt_string(
		record_batch,
		"was_attributed_to",
		row_index,
		"agent",
		"role",
	)?
	.into_iter()
	.map(|(agent, role)| EntityAttributionRef { agent, role })
	.collect())
}

fn get_derivation(
	column_name: &str,
	record_batch: &RecordBatch,
	row_index: usize,
) -> Result<Vec<DerivationRef>, ChronicleArrowError> {
	Ok(struct_2_list_column(record_batch, column_name, row_index, "target", "activity")?
		.into_iter()
		.map(|(target, activity)| DerivationRef { target, activity })
		.collect())
}

fn with_implied(operations: Vec<ChronicleOperation>) -> Vec<ChronicleOperation> {
	operations
		.into_iter()
		.flat_map(|op| {
			let mut implied_ops = op.implied_by();
			implied_ops.push(op);
			implied_ops
		})
		.collect()
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

pub fn agent_operations(
	ns: &NamespaceId,
	id: &str,
	attributes: Attributes,
	row_index: usize,
	record_batch: &RecordBatch,
) -> Result<Vec<ChronicleOperation>, ChronicleArrowError> {
	let mut operations = vec![
		ChronicleOperation::agent_exists(ns.clone(), AgentId::from_external_id(id)),
		ChronicleOperation::set_attributes(SetAttributes::agent(
			ns.clone(),
			AgentId::from_external_id(id),
			attributes,
		)),
	];

	let was_attributed_to_refs = get_agent_attribution(record_batch, row_index)?;

	for was_attributed_to_ref in was_attributed_to_refs {
		operations.push(ChronicleOperation::was_attributed_to(
			ns.clone(),
			EntityId::from_external_id(was_attributed_to_ref.entity),
			AgentId::from_external_id(id),
			was_attributed_to_ref.role.map(Role::from),
		));
	}

	let acted_on_behalf_of_refs = get_acted_on_behalf_of(record_batch, row_index)?;

	for acted_on_behalf_of_ref in acted_on_behalf_of_refs {
		operations.push(ChronicleOperation::agent_acts_on_behalf_of(
			ns.clone(),
			AgentId::from_external_id(id),
			AgentId::from_external_id(acted_on_behalf_of_ref.agent),
			acted_on_behalf_of_ref.activity.map(ActivityId::from_external_id),
			acted_on_behalf_of_ref.role.map(Role::from),
		));
	}

	Ok(with_implied(operations))
}

pub fn entity_operations(
	ns: &NamespaceId,
	id: &str,
	attributes: Attributes,
	row_index: usize,
	record_batch: &RecordBatch,
) -> Result<Vec<ChronicleOperation>, ChronicleArrowError> {
	let mut operations = vec![
		ChronicleOperation::entity_exists(ns.clone(), EntityId::from_external_id(id)),
		ChronicleOperation::set_attributes(SetAttributes::entity(
			ns.clone(),
			EntityId::from_external_id(id),
			attributes,
		)),
	];

	let was_generated_by_ids = get_was_generated_by(record_batch, row_index)?;

	for generated_by_id in was_generated_by_ids {
		operations.push(ChronicleOperation::was_generated_by(
			ns.clone(),
			EntityId::from_external_id(id),
			ActivityId::from_external_id(&generated_by_id),
		));
	}

	let was_attributed_to_refs = get_entity_was_attributed_to(record_batch, row_index)?;

	for was_attributed_to_ref in was_attributed_to_refs {
		operations.push(ChronicleOperation::was_attributed_to(
			ns.clone(),
			EntityId::from_external_id(id),
			AgentId::from_external_id(was_attributed_to_ref.agent),
			was_attributed_to_ref.role.map(Role::from),
		))
	}

	let was_derived_from_refs = get_derivation("was_derived_from", record_batch, row_index)?;

	for was_derived_from_ref in was_derived_from_refs {
		operations.push(ChronicleOperation::entity_derive(
			ns.clone(),
			EntityId::from_external_id(id),
			EntityId::from_external_id(was_derived_from_ref.target),
			Some(ActivityId::from_external_id(was_derived_from_ref.activity)),
			DerivationType::None,
		))
	}

	let had_primary_source_refs = get_derivation("had_primary_source", record_batch, row_index)?;

	for had_primary_source_ref in had_primary_source_refs {
		operations.push(ChronicleOperation::entity_derive(
			ns.clone(),
			EntityId::from_external_id(id),
			EntityId::from_external_id(had_primary_source_ref.target),
			Some(ActivityId::from_external_id(had_primary_source_ref.activity)),
			DerivationType::PrimarySource,
		))
	}

	let was_quoted_from_refs = get_derivation("was_quoted_from", record_batch, row_index)?;

	for was_quoted_from_ref in was_quoted_from_refs {
		operations.push(ChronicleOperation::entity_derive(
			ns.clone(),
			EntityId::from_external_id(id),
			EntityId::from_external_id(was_quoted_from_ref.target),
			Some(ActivityId::from_external_id(was_quoted_from_ref.activity)),
			DerivationType::Quotation,
		))
	}

	let was_revision_of_refs = get_derivation("was_revision_of", record_batch, row_index)?;

	for was_revision_of_ref in was_revision_of_refs {
		operations.push(ChronicleOperation::entity_derive(
			ns.clone(),
			EntityId::from_external_id(id),
			EntityId::from_external_id(was_revision_of_ref.target),
			Some(ActivityId::from_external_id(was_revision_of_ref.activity)),
			DerivationType::Revision,
		))
	}

	Ok(with_implied(operations))
}

#[instrument(skip(pool, term, domaintype))]
pub async fn calculate_count_by_metadata_term(
	pool: &Pool<ConnectionManager<PgConnection>>,
	term: &Term,
	domaintype: Option<String>,
) -> Result<i64, Status> {
	let pool = pool.clone();
	match term {
		Term::Entity => {
			spawn_blocking(move || {
				entity_count_by_type(
					&pool,
					domaintype.map(|x| x.to_string()).iter().map(|s| s.as_str()).collect(),
				)
			})
			.await
		},
		Term::Agent => {
			spawn_blocking(move || {
				agent_count_by_type(
					&pool,
					domaintype.map(|x| x.to_string()).iter().map(|s| s.as_str()).collect(),
				)
			})
			.await
		},
		Term::Activity => {
			spawn_blocking(move || {
				activity_count_by_type(
					&pool,
					domaintype.map(|x| x.to_string()).iter().map(|s| s.as_str()).collect(),
				)
			})
			.await
		},
		_ => Ok(Ok(0)),
	}
	.map_err(|e| Status::from_error(e.into()))
	.and_then(|res| res.map_err(|e| Status::from_error(e.into())))
}

pub async fn create_flight_info_for_type(
	pool: Arc<Pool<ConnectionManager<PgConnection>>>,
	domain_items: Vec<impl TypeName + Send + Sync + 'static>,
	term: Term,
	record_batch_size: usize,
) -> BoxStream<'static, Result<FlightInfo, Status>> {
	stream::iter(domain_items.into_iter().map(|item| Ok::<_, tonic::Status>(item)))
		.then(move |item| {
			let pool = pool.clone();
			async move {
				let item = item?; // Handle the Result from the iterator
				let descriptor_path = vec![term.to_string(), item.as_type_name()];
				let metadata =
					get_domain_type_meta_from_cache(&descriptor_path).ok_or_else(|| {
						Status::from_error(Box::new(ChronicleArrowError::MissingSchemaError))
					})?;

				let count = calculate_count_by_metadata_term(
					&pool,
					&term,
					Some(item.as_type_name().to_string()),
				)
				.await?;

				let tickets = (0..count)
					.step_by(record_batch_size as _)
					.map(|start| {
						let end = std::cmp::min(start as usize + record_batch_size, count as usize);

						let ticket_metadata = ChronicleTicket::new(
							term,
							metadata.typ.as_ref().map(|x| x.as_domain_type_id()),
							start as _,
							(end - start as usize) as _,
						);
						Ticket::try_from(ticket_metadata)
							.map_err(|e| Status::from_error(Box::new(ChronicleArrowError::from(e))))
					})
					.collect::<Result<Vec<_>, _>>()?;

				let mut flight_info = FlightInfo::new();

				for ticket in tickets {
					flight_info =
						flight_info.with_endpoint(FlightEndpoint::new().with_ticket(ticket));
				}

				Ok(flight_info
					.with_descriptor(FlightDescriptor::new_path(descriptor_path))
					.try_with_schema(&metadata.schema)
					.map_err(|e| Status::from_error(Box::new(ChronicleArrowError::from(e))))?
					.with_total_records(count))
			}
		})
		.boxed()
}
