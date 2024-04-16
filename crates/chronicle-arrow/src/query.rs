use std::{collections::HashMap, sync::Arc};

use crate::{
	meta::{
		agent_attribution_struct, agent_delegation_struct, association_struct, attribution_struct,
		derivation_struct,
	},
	ChronicleArrowError, DomainTypeMeta,
};
use arrow::array::{ArrayBuilder, StringBuilder, StructBuilder};
use arrow_array::{
	Array, BooleanArray, Int64Array, ListArray, RecordBatch, StringArray, TimestampNanosecondArray,
};
use arrow_buffer::{Buffer, ToByteSlice};
use arrow_data::ArrayData;
use arrow_schema::{DataType, Field};
use chronicle_persistence::{
	query::{
		Activity, Agent, Attribution, Derivation, Entity, Generation, Namespace, Usage,
		WasInformedBy,
	},
	schema::{
		activity, agent, attribution, derivation, entity, entity_attribute, generation, namespace,
		usage, wasinformedby,
	},
};
use chrono::{DateTime, Utc};
use common::{
	attributes::{Attribute, Attributes},
	domain::PrimitiveType,
	prov::{operations::DerivationType, DomaintypeId, ExternalIdPart},
};
use diesel::{
	pg::PgConnection,
	prelude::*,
	r2d2::{ConnectionManager, Pool},
};
use uuid::Uuid;

#[derive(Default, Debug)]
pub struct DerivationRef {
	pub target: String,
	pub activity: String,
}

#[derive(Default, Debug)]
pub struct EntityAttributionRef {
	pub agent: String,
	pub role: Option<String>,
}

#[derive(Default, Debug)]
pub struct EntityAndReferences {
	pub(crate) id: String,
	pub(crate) namespace_name: String,
	pub(crate) namespace_uuid: [u8; 16],
	pub(crate) attributes: Attributes,
	pub(crate) was_generated_by: Vec<String>,
	pub(crate) was_attributed_to: Vec<EntityAttributionRef>,
	pub(crate) was_derived_from: Vec<DerivationRef>,
	pub(crate) had_primary_source: Vec<DerivationRef>,
	pub(crate) was_quoted_from: Vec<DerivationRef>,
	pub(crate) was_revision_of: Vec<DerivationRef>,
}

impl EntityAndReferences {
	#[tracing::instrument(skip(items, meta))]
	pub fn to_record_batch(
		items: impl Iterator<Item = Self>,
		meta: &DomainTypeMeta,
	) -> Result<RecordBatch, ChronicleArrowError> {
		let mut attributes_map: HashMap<String, (PrimitiveType, Vec<Option<serde_json::Value>>)> =
			HashMap::new();

		for (attribute_name, primitive_type) in meta.attributes.iter() {
			attributes_map.insert(attribute_name.clone(), (*primitive_type, vec![]));
		}

		let mut id_vec = Vec::new();
		let mut namespace_name_vec = Vec::new();
		let mut namespace_uuid_vec = Vec::new();
		let mut was_generated_by_vec = Vec::new();
		let mut was_attributed_to_vec = Vec::new();
		let mut was_derived_from_vec = Vec::new();
		let mut had_primary_source_vec = Vec::new();
		let mut was_quoted_from_vec = Vec::new();
		let mut was_revision_of_vec = Vec::new();

		for item in items {
			id_vec.push(item.id);
			namespace_name_vec.push(item.namespace_name);
			namespace_uuid_vec.push(Uuid::from_bytes(item.namespace_uuid).to_string());
			was_generated_by_vec.push(item.was_generated_by);
			was_attributed_to_vec.push(item.was_attributed_to);
			was_derived_from_vec.push(item.was_derived_from);
			had_primary_source_vec.push(item.had_primary_source);
			was_quoted_from_vec.push(item.was_quoted_from);
			was_revision_of_vec.push(item.was_revision_of);
			for (key, (_primitive_type, values)) in attributes_map.iter_mut() {
				if let Some(attribute) = item.attributes.get_attribute(key) {
					values.push(Some(attribute.value.clone().into()));
				} else {
					values.push(None);
				}
			}
		}

		let was_generated_by_array = vec_vec_string_to_list_array(was_generated_by_vec)?;
		let was_attributed_to_array = attributions_to_list_array(was_attributed_to_vec)?;
		let was_derived_from_array = derivations_to_list_array(was_derived_from_vec)?;
		let had_primary_source_array = derivations_to_list_array(had_primary_source_vec)?;
		let was_quoted_from_array = derivations_to_list_array(was_quoted_from_vec)?;
		let was_revision_of_array = derivations_to_list_array(was_revision_of_vec)?;

		let mut fields = vec![
			(
				"namespace_name".to_string(),
				Arc::new(StringArray::from(namespace_name_vec)) as Arc<dyn arrow_array::Array>,
			),
			(
				"namespace_uuid".to_string(),
				Arc::new(StringArray::from(namespace_uuid_vec)) as Arc<dyn arrow_array::Array>,
			),
			("id".to_string(), Arc::new(StringArray::from(id_vec)) as Arc<dyn arrow_array::Array>),
		];
		// Dynamically generate fields for attribute key/values based on their primitive type
		for (key, (primitive_type, values)) in attributes_map {
			tracing::trace!("Key: {}, Primitive Type: {:?}", key, primitive_type);
			let array: Arc<dyn arrow_array::Array> = match primitive_type {
				PrimitiveType::String => {
					tracing::debug!("Converting String attribute values for key: {}", key);
					Arc::new(StringArray::from_iter(
						values.iter().map(|v| v.as_ref().map(|v| v.as_str()).unwrap_or_default()),
					)) as Arc<dyn arrow_array::Array>
				},
				PrimitiveType::Int => {
					tracing::debug!("Converting Int attribute values for key: {}", key);
					Arc::new(Int64Array::from_iter(
						values.iter().map(|v| v.as_ref().map(|v| v.as_i64()).unwrap_or_default()),
					)) as Arc<dyn arrow_array::Array>
				},
				PrimitiveType::Bool => {
					tracing::debug!("Converting Bool attribute values for key: {}", key);
					Arc::new(BooleanArray::from_iter(
						values.iter().map(|v| v.as_ref().map(|v| v.as_bool()).unwrap_or_default()),
					)) as Arc<dyn arrow_array::Array>
				},
				_ => {
					tracing::warn!("Unsupported attribute primitive type for key: {}", key);
					continue;
				},
			};
			fields.push((key, array as Arc<dyn arrow_array::Array>));
		}

		fields.extend(vec![
			(
				"was_generated_by".to_string(),
				Arc::new(was_generated_by_array) as Arc<dyn arrow_array::Array>,
			),
			(
				"was_attributed_to".to_string(),
				Arc::new(was_attributed_to_array) as Arc<dyn arrow_array::Array>,
			),
			(
				"was_derived_from".to_string(),
				Arc::new(was_derived_from_array) as Arc<dyn arrow_array::Array>,
			),
			(
				"had_primary_source".to_string(),
				Arc::new(had_primary_source_array) as Arc<dyn arrow_array::Array>,
			),
			(
				"was_quoted_from".to_string(),
				Arc::new(was_quoted_from_array) as Arc<dyn arrow_array::Array>,
			),
			(
				"was_revision_of".to_string(),
				Arc::new(was_revision_of_array) as Arc<dyn arrow_array::Array>,
			),
		]);

		let hashed_fields = fields.into_iter().collect::<HashMap<_, _>>();

		let mut columns = Vec::new();

		for field in meta.schema.fields() {
			let field_name = field.name();
			match hashed_fields.get(field_name) {
				Some(array) => columns.push(array.clone()),
				None =>
					return Err(ChronicleArrowError::SchemaFieldNotFound(field_name.to_string())),
			}
		}

		RecordBatch::try_new(meta.schema.clone(), columns).map_err(ChronicleArrowError::from)
	}
}

#[derive(Default)]
pub struct AssociationRef {
	pub(crate) responsible_agent: String,
	pub(crate) responsible_role: Option<String>,
	pub(crate) delegate_agent: Option<String>,
	pub(crate) delegate_role: Option<String>,
}

#[derive(Default)]
pub struct ActivityAndReferences {
	pub(crate) id: String,
	pub(crate) namespace_name: String,
	pub(crate) namespace_uuid: [u8; 16],
	pub(crate) started: Option<DateTime<Utc>>,
	pub(crate) ended: Option<DateTime<Utc>>,
	pub(crate) attributes: Attributes,
	pub(crate) used: Vec<String>,
	pub(crate) generated: Vec<String>,
	pub(crate) was_informed_by: Vec<String>,
	pub(crate) was_associated_with: Vec<AssociationRef>,
}

impl ActivityAndReferences {
	#[tracing::instrument(skip(items, meta))]
	pub fn to_record_batch(
		items: impl Iterator<Item = ActivityAndReferences>,
		meta: &DomainTypeMeta,
	) -> Result<RecordBatch, ChronicleArrowError> {
		let mut attributes_map: HashMap<String, (PrimitiveType, Vec<Option<serde_json::Value>>)> =
			HashMap::new();

		for (attribute_name, primitive_type) in meta.attributes.iter() {
			attributes_map.insert(attribute_name.to_string(), (*primitive_type, vec![]));
		}

		let mut id_vec = Vec::new();
		let mut namespace_name_vec = Vec::new();
		let mut namespace_uuid_vec = Vec::new();
		let mut started_vec = Vec::new();
		let mut ended_vec = Vec::new();
		let mut used_vec = Vec::new();
		let mut generated_vec = Vec::new();
		let mut was_informed_by_vec = Vec::new();
		let mut was_associated_with_vec = Vec::new();

		for item in items {
			id_vec.push(item.id);
			namespace_name_vec.push(item.namespace_name);
			namespace_uuid_vec.push(Uuid::from_bytes(item.namespace_uuid).to_string());
			started_vec.push(item.started.map(|dt| dt.timestamp_nanos_opt().unwrap_or_default()));
			ended_vec.push(item.ended.map(|dt| dt.timestamp_nanos_opt().unwrap_or_default()));
			used_vec.push(item.used);
			generated_vec.push(item.generated);
			was_informed_by_vec.push(item.was_informed_by);
			was_associated_with_vec.push(item.was_associated_with);

			for (key, (_primitive_type, values)) in attributes_map.iter_mut() {
				if let Some(attribute) = item.attributes.get_attribute(key) {
					values.push(Some(attribute.value.clone().into()));
				} else {
					values.push(None);
				}
			}
		}

		let used_array = vec_vec_string_to_list_array(used_vec)?;
		let generated_array = vec_vec_string_to_list_array(generated_vec)?;
		let was_informed_by_array = vec_vec_string_to_list_array(was_informed_by_vec)?;
		let was_associated_with_array = associations_to_list_array(was_associated_with_vec)?;

		let mut fields = vec![
			(
				"namespace_name".to_string(),
				Arc::new(StringArray::from(namespace_name_vec)) as Arc<dyn arrow_array::Array>,
			),
			(
				"namespace_uuid".to_string(),
				Arc::new(StringArray::from(namespace_uuid_vec)) as Arc<dyn arrow_array::Array>,
			),
			("id".to_string(), Arc::new(StringArray::from(id_vec)) as Arc<dyn arrow_array::Array>),
		];

		// Dynamically generate fields for attribute key/values based on their primitive type
		for (key, (primitive_type, values)) in attributes_map {
			let array: Arc<dyn arrow_array::Array> = match primitive_type {
				PrimitiveType::String => {
					tracing::debug!("Converting String attribute values for key: {}", key);
					Arc::new(StringArray::from(
						values
							.iter()
							.map(|v| v.as_ref().map(|v| v.as_str()).unwrap_or_default())
							.collect::<Vec<_>>(),
					)) as Arc<dyn arrow_array::Array>
				},
				PrimitiveType::Int => {
					tracing::debug!("Converting Int attribute values for key: {}", key);
					Arc::new(Int64Array::from(
						values
							.iter()
							.map(|v| v.as_ref().map(|v| v.as_i64()).unwrap_or_default())
							.collect::<Vec<_>>(),
					)) as Arc<dyn arrow_array::Array>
				},
				PrimitiveType::Bool => {
					tracing::debug!("Converting Bool attribute values for key: {}", key);
					Arc::new(BooleanArray::from(
						values
							.iter()
							.map(|v| v.as_ref().map(|v| v.as_bool()).unwrap_or_default())
							.collect::<Vec<_>>(),
					)) as Arc<dyn arrow_array::Array>
				},
				_ => {
					tracing::warn!("Unsupported attribute primitive type for key: {}", key);
					continue;
				},
			};
			fields.push((key, array as Arc<dyn arrow_array::Array>));
		}

		fields.extend(vec![
			(
				"started".to_string(),
				Arc::new(TimestampNanosecondArray::with_timezone_opt(
					started_vec.into(),
					Some("UTC".to_string()),
				)) as Arc<dyn arrow_array::Array>,
			),
			(
				"ended".to_string(),
				Arc::new(TimestampNanosecondArray::with_timezone_opt(
					ended_vec.into(),
					Some("UTC".to_string()),
				)) as Arc<dyn arrow_array::Array>,
			),
			("used".to_string(), Arc::new(used_array) as Arc<dyn arrow_array::Array>),
			("generated".to_string(), Arc::new(generated_array) as Arc<dyn arrow_array::Array>),
			(
				"was_informed_by".to_string(),
				Arc::new(was_informed_by_array) as Arc<dyn arrow_array::Array>,
			),
			(
				"was_associated_with".to_string(),
				Arc::new(was_associated_with_array) as Arc<dyn arrow_array::Array>,
			),
		]);

		let hashed_fields = fields.into_iter().collect::<HashMap<_, _>>();

		let mut columns = Vec::new();
		for field in meta.schema.fields() {
			let field_name = field.name();
			match hashed_fields.get(field_name) {
				Some(array) => columns.push(array.clone()),
				None =>
					return Err(ChronicleArrowError::SchemaFieldNotFound(field_name.to_string())),
			}
		}

		RecordBatch::try_new(meta.schema.clone(), columns).map_err(ChronicleArrowError::from)
	}
}

#[derive(Default)]
pub struct ActedOnBehalfOfRef {
	pub(crate) agent: String,
	pub(crate) role: Option<String>,
	pub(crate) activity: Option<String>,
}

#[derive(Default)]
pub struct AgentAttributionRef {
	pub(crate) entity: String,
	pub(crate) role: Option<String>,
}

#[derive(Default)]
pub struct AgentAndReferences {
	pub(crate) id: String,
	pub(crate) namespace_name: String,
	pub(crate) namespace_uuid: [u8; 16],
	pub(crate) attributes: Attributes,
	pub(crate) acted_on_behalf_of: Vec<ActedOnBehalfOfRef>,
	pub(crate) was_attributed_to: Vec<AgentAttributionRef>,
}

impl AgentAndReferences {
	#[tracing::instrument(skip(items, meta))]
	pub fn to_record_batch(
		items: impl Iterator<Item = AgentAndReferences>,
		meta: &DomainTypeMeta,
	) -> Result<RecordBatch, ChronicleArrowError> {
		let mut attributes_map: HashMap<String, (PrimitiveType, Vec<Option<serde_json::Value>>)> =
			HashMap::new();

		for (attribute_name, primitive_type) in meta.attributes.iter() {
			attributes_map.insert(attribute_name.to_string(), (*primitive_type, vec![]));
		}

		let mut id_vec = Vec::new();
		let mut namespace_name_vec = Vec::new();
		let mut namespace_uuid_vec = Vec::new();
		let mut acted_on_behalf_of_vec = Vec::new();
		let mut was_attributed_to_vec = Vec::new();

		for item in items {
			id_vec.push(item.id);
			namespace_name_vec.push(item.namespace_name);

			namespace_uuid_vec.push(Uuid::from_bytes(item.namespace_uuid).to_string());
			acted_on_behalf_of_vec.push(item.acted_on_behalf_of);
			was_attributed_to_vec.push(item.was_attributed_to);

			for (key, (_primitive_type, values)) in attributes_map.iter_mut() {
				if let Some(attribute) = item.attributes.get_attribute(key) {
					values.push(Some(attribute.value.clone().into()));
				} else {
					values.push(None);
				}
			}
		}

		let acted_on_behalf_of_array =
			agent_acted_on_behalf_of_to_list_array(acted_on_behalf_of_vec)?;
		let was_attributed_to_array = agent_attributions_to_list_array(was_attributed_to_vec)?;

		let mut fields = vec![
			(
				"namespace_name".to_string(),
				Arc::new(StringArray::from(namespace_name_vec)) as Arc<dyn arrow_array::Array>,
			),
			(
				"namespace_uuid".to_string(),
				Arc::new(StringArray::from(namespace_uuid_vec)) as Arc<dyn arrow_array::Array>,
			),
			("id".to_string(), Arc::new(StringArray::from(id_vec)) as Arc<dyn arrow_array::Array>),
		];

		// Dynamically generate fields for attribute key/values based on their primitive type
		for (key, (primitive_type, values)) in attributes_map {
			let array: Arc<dyn arrow_array::Array> = match primitive_type {
				PrimitiveType::String => {
					tracing::debug!("Converting String attribute values for key: {}", key);
					Arc::new(StringArray::from_iter(
						values.iter().map(|v| v.as_ref().map(|v| v.as_str()).unwrap_or_default()),
					)) as Arc<dyn arrow_array::Array>
				},
				PrimitiveType::Int => {
					tracing::debug!("Converting Int attribute values for key: {}", key);
					Arc::new(Int64Array::from_iter(
						values.iter().map(|v| v.as_ref().map(|v| v.as_i64()).unwrap_or_default()),
					)) as Arc<dyn arrow_array::Array>
				},
				PrimitiveType::Bool => {
					tracing::debug!("Converting Bool attribute values for key: {}", key);
					Arc::new(BooleanArray::from_iter(
						values.iter().map(|v| v.as_ref().map(|v| v.as_bool()).unwrap_or_default()),
					)) as Arc<dyn arrow_array::Array>
				},
				_ => {
					tracing::warn!("Unsupported attribute primitive type for key: {}", key);
					continue;
				},
			};
			fields.push((key, array as Arc<dyn arrow_array::Array>));
		}

		fields.extend(vec![
			(
				"acted_on_behalf_of".to_string(),
				Arc::new(acted_on_behalf_of_array) as Arc<dyn arrow_array::Array>,
			),
			(
				"was_attributed_to".to_string(),
				Arc::new(was_attributed_to_array) as Arc<dyn arrow_array::Array>,
			),
		]);

		let hashed_fields = fields.into_iter().collect::<HashMap<_, _>>();

		let mut columns = Vec::new();
		for field in meta.schema.fields() {
			let field_name = field.name();
			match hashed_fields.get(field_name) {
				Some(array) => columns.push(array.clone()),
				None =>
					return Err(ChronicleArrowError::SchemaFieldNotFound(field_name.to_string())),
			}
		}

		RecordBatch::try_new(meta.schema.clone(), columns).map_err(ChronicleArrowError::from)
	}
}
fn agent_acted_on_behalf_of_to_list_array(
	agent_attributions: Vec<Vec<ActedOnBehalfOfRef>>,
) -> Result<ListArray, ChronicleArrowError> {
	let offsets: Vec<i32> = std::iter::once(0)
		.chain(agent_attributions.iter().map(|v| v.len() as i32))
		.scan(0, |state, len| {
			*state += len;
			Some(*state)
		})
		.collect();

	let agent_builder = StringBuilder::new();
	let role_builder = StringBuilder::new();
	let activity_builder = StringBuilder::new();

	let fields = vec![
		Field::new("agent", DataType::Utf8, false),
		Field::new("activity", DataType::Utf8, true),
		Field::new("role", DataType::Utf8, true),
	];
	let field_builders = vec![
		Box::new(agent_builder) as Box<dyn ArrayBuilder>,
		Box::new(activity_builder) as Box<dyn ArrayBuilder>,
		Box::new(role_builder) as Box<dyn ArrayBuilder>,
	];

	let mut builder = StructBuilder::new(fields, field_builders);

	for acted_on_behalf_of in agent_attributions.into_iter().flatten() {
		builder
			.field_builder::<StringBuilder>(0)
			.expect("Failed to get agent field builder")
			.append_value(&acted_on_behalf_of.agent);
		builder
			.field_builder::<StringBuilder>(1)
			.expect("Failed to get activity field builder")
			.append_option(acted_on_behalf_of.activity.as_deref());
		builder
			.field_builder::<StringBuilder>(2)
			.expect("Failed to get role field builder")
			.append_option(acted_on_behalf_of.role.as_deref());

		builder.append(true);
	}

	let values_array = builder.finish();

	let data_type = DataType::new_list(agent_delegation_struct(), false);
	let offsets_buffer = Buffer::from(offsets.to_byte_slice());

	let list_array = ListArray::from(
		ArrayData::builder(data_type)
			.add_child_data(values_array.to_data())
			.len(offsets.len() - 1)
			.null_count(0)
			.add_buffer(offsets_buffer)
			.build()?,
	);

	Ok(list_array)
}

fn agent_attributions_to_list_array(
	agent_attributions: Vec<Vec<AgentAttributionRef>>,
) -> Result<ListArray, ChronicleArrowError> {
	let offsets: Vec<i32> = std::iter::once(0)
		.chain(agent_attributions.iter().map(|v| v.len() as i32))
		.scan(0, |state, len| {
			*state += len;
			Some(*state)
		})
		.collect();

	let entity_builder = StringBuilder::new();
	let role_builder = StringBuilder::new();

	let fields =
		vec![Field::new("entity", DataType::Utf8, false), Field::new("role", DataType::Utf8, true)];
	let field_builders = vec![
		Box::new(entity_builder) as Box<dyn ArrayBuilder>,
		Box::new(role_builder) as Box<dyn ArrayBuilder>,
	];

	let mut builder = StructBuilder::new(fields, field_builders);

	for agent_attribution in agent_attributions.into_iter().flatten() {
		builder
			.field_builder::<StringBuilder>(0)
			.unwrap()
			.append_value(agent_attribution.entity);
		builder
			.field_builder::<StringBuilder>(1)
			.unwrap()
			.append_option(agent_attribution.role.map(|r| r.to_string()));

		builder.append(true);
	}

	let values_array = builder.finish();

	let data_type = DataType::new_list(agent_attribution_struct(), false);
	let offsets_buffer = Buffer::from(offsets.to_byte_slice());

	let list_array = ListArray::from(
		ArrayData::builder(data_type)
			.add_child_data(values_array.to_data())
			.len(offsets.len() - 1)
			.null_count(0)
			.add_buffer(offsets_buffer)
			.build()?,
	);

	Ok(list_array)
}

fn attributions_to_list_array(
	attributions: Vec<Vec<EntityAttributionRef>>,
) -> Result<ListArray, ChronicleArrowError> {
	let offsets: Vec<i32> = std::iter::once(0)
		.chain(attributions.iter().map(|v| v.len() as i32))
		.scan(0, |state, len| {
			*state += len;
			Some(*state)
		})
		.collect();

	let agent_builder = StringBuilder::new();
	let role_builder = StringBuilder::new();

	let fields =
		vec![Field::new("agent", DataType::Utf8, false), Field::new("role", DataType::Utf8, true)];
	let field_builders = vec![
		Box::new(agent_builder) as Box<dyn ArrayBuilder>,
		Box::new(role_builder) as Box<dyn ArrayBuilder>,
	];

	let mut builder = StructBuilder::new(fields, field_builders);

	for attribution in attributions.into_iter().flatten() {
		builder
			.field_builder::<StringBuilder>(0)
			.unwrap()
			.append_value(attribution.agent);
		builder
			.field_builder::<StringBuilder>(1)
			.unwrap()
			.append_option(attribution.role);

		builder.append(true)
	}

	let values_array = builder.finish();

	let data_type = DataType::new_list(attribution_struct(), false);
	let offsets_buffer = Buffer::from(offsets.to_byte_slice());

	let list_array = ListArray::from(
		ArrayData::builder(data_type)
			.add_child_data(values_array.to_data())
			.len(offsets.len() - 1)
			.null_count(0)
			.add_buffer(offsets_buffer)
			.build()?,
	);

	Ok(list_array)
}

fn associations_to_list_array(
	associations: Vec<Vec<AssociationRef>>,
) -> Result<ListArray, ChronicleArrowError> {
	let offsets: Vec<i32> = std::iter::once(0)
		.chain(associations.iter().map(|v| v.len() as i32))
		.scan(0, |state, len| {
			*state += len;
			Some(*state)
		})
		.collect();

	let fields = vec![
		Field::new("responsible_agent", DataType::Utf8, false),
		Field::new("responsible_role", DataType::Utf8, true),
		Field::new("delegate_agent", DataType::Utf8, true),
		Field::new("delegate_role", DataType::Utf8, true),
	];
	let field_builders = vec![
		Box::new(StringBuilder::new()) as Box<dyn ArrayBuilder>,
		Box::new(StringBuilder::new()) as Box<dyn ArrayBuilder>,
		Box::new(StringBuilder::new()) as Box<dyn ArrayBuilder>,
		Box::new(StringBuilder::new()) as Box<dyn ArrayBuilder>,
	];

	let mut builder = StructBuilder::new(fields, field_builders);

	for association in associations.into_iter().flatten() {
		builder
			.field_builder::<StringBuilder>(0)
			.expect("Failed to get field builder for responsible_agent")
			.append_value(&association.responsible_agent);
		if let Some(role) = &association.responsible_role {
			builder
				.field_builder::<StringBuilder>(1)
				.expect("Failed to get field builder for responsible_role")
				.append_value(role);
		} else {
			builder
				.field_builder::<StringBuilder>(1)
				.expect("Failed to get field builder for responsible_role")
				.append_null();
		}
		if let Some(agent) = &association.delegate_agent {
			builder
				.field_builder::<StringBuilder>(2)
				.expect("Failed to get field builder for delegate_agent")
				.append_value(agent);
		} else {
			builder
				.field_builder::<StringBuilder>(2)
				.expect("Failed to get field builder for delegate_agent")
				.append_null();
		}
		if let Some(role) = &association.delegate_role {
			builder
				.field_builder::<StringBuilder>(3)
				.expect("Failed to get field builder for delegate_role")
				.append_value(role);
		} else {
			builder
				.field_builder::<StringBuilder>(3)
				.expect("Failed to get field builder for delegate_role")
				.append_null();
		}

		builder.append(true);
	}

	let values_array = builder.finish();

	let data_type = DataType::new_list(association_struct(), false);
	let offsets_buffer = Buffer::from(offsets.to_byte_slice());

	let list_array = ListArray::from(
		ArrayData::builder(data_type.clone())
			.add_child_data(values_array.to_data())
			.len(offsets.len() - 1)
			.null_count(0)
			.add_buffer(offsets_buffer)
			.build()?,
	);

	Ok(list_array)
}

fn derivations_to_list_array(
	derivations: Vec<Vec<DerivationRef>>,
) -> Result<ListArray, ChronicleArrowError> {
	let offsets: Vec<i32> = std::iter::once(0)
		.chain(derivations.iter().map(|v| v.len() as i32))
		.scan(0, |state, len| {
			*state += len;
			Some(*state)
		})
		.collect();

	let fields = vec![
		Field::new("target", DataType::Utf8, false),
		Field::new("activity", DataType::Utf8, false),
	];
	let field_builders = vec![
		Box::new(StringBuilder::new()) as Box<dyn ArrayBuilder>,
		Box::new(StringBuilder::new()) as Box<dyn ArrayBuilder>,
	];

	let mut builder = StructBuilder::new(fields, field_builders);

	for derivation in derivations.into_iter().flatten() {
		builder
			.field_builder::<StringBuilder>(0)
			.unwrap()
			.append_value(derivation.target);
		builder
			.field_builder::<StringBuilder>(1)
			.unwrap()
			.append_value(derivation.activity);

		builder.append(true)
	}

	let values_array = builder.finish();

	let data_type = DataType::new_list(derivation_struct(), false);
	let offsets_buffer = Buffer::from(offsets.to_byte_slice());

	let list_array = ListArray::from(
		ArrayData::builder(data_type)
			.add_child_data(values_array.to_data())
			.len(offsets.len() - 1)
			.null_count(0)
			.add_buffer(offsets_buffer)
			.build()?,
	);

	Ok(list_array)
}

fn vec_vec_string_to_list_array(
	vec_vec_string: Vec<Vec<String>>,
) -> Result<ListArray, ChronicleArrowError> {
	let offsets: Vec<i32> = std::iter::once(0)
		.chain(vec_vec_string.iter().map(|v| v.len() as i32))
		.scan(0, |state, len| {
			*state += len;
			Some(*state)
		})
		.collect();
	let values: Vec<String> = vec_vec_string.into_iter().flatten().collect();

	let values_array = Arc::new(StringArray::from(values)) as Arc<dyn arrow_array::Array>;
	// Create an OffsetBuffer from the offsets
	let offsets_buffer = Buffer::from(offsets.to_byte_slice());
	let data_type = DataType::new_list(DataType::Utf8, false);
	let list_array = ListArray::from(
		ArrayData::builder(data_type)
			.add_child_data(values_array.to_data())
			.len(offsets.len() - 1)
			.null_count(0)
			.add_buffer(offsets_buffer)
			.build()?,
	);

	Ok(list_array)
}

// Returns a list of all indexed domain types from entities, activities and agents , note that these
// may no longer be present in the domain definition
#[tracing::instrument(skip(pool))]
pub fn term_types(
	pool: &Pool<ConnectionManager<PgConnection>>,
) -> Result<Vec<DomaintypeId>, ChronicleArrowError> {
	let mut connection = pool.get()?;
	let types = entity::table
		.select(entity::domaintype)
		.distinct()
		.union(agent::table.select(agent::domaintype).distinct())
		.union(activity::table.select(activity::domaintype).distinct())
		.load::<Option<String>>(&mut connection)?;

	let mut unique_types = types.into_iter().collect::<Vec<_>>();
	unique_types.sort();
	unique_types.dedup();

	Ok(unique_types
		.into_iter()
		.filter_map(|x| x.map(DomaintypeId::from_external_id))
		.collect())
}
pub fn entity_count_by_type(
	pool: &Pool<ConnectionManager<PgConnection>>,
	typ: Vec<&str>,
) -> Result<i64, ChronicleArrowError> {
	let mut connection = pool.get()?;
	let count = entity::table
		.filter(entity::domaintype.eq_any(typ))
		.count()
		.get_result(&mut connection)?;
	Ok(count)
}

#[tracing::instrument(skip(pool))]
pub fn agent_count_by_type(
	pool: &Pool<ConnectionManager<PgConnection>>,
	typ: Vec<&str>,
) -> Result<i64, ChronicleArrowError> {
	let mut connection = pool.get()?;
	let count = agent::table
		.filter(agent::domaintype.eq_any(typ))
		.count()
		.get_result(&mut connection)?;
	Ok(count)
}

#[tracing::instrument(skip(pool))]
pub fn activity_count_by_type(
	pool: &Pool<ConnectionManager<PgConnection>>,
	typ: Vec<&str>,
) -> Result<i64, ChronicleArrowError> {
	let mut connection = pool.get()?;
	let count = activity::table
		.filter(activity::domaintype.eq_any(typ))
		.count()
		.get_result(&mut connection)?;
	Ok(count)
}

// Returns a tuple of an iterator over entities of the specified domain types and their relations,
// the number of returned records and the total number of records
#[tracing::instrument(skip(pool))]
pub fn load_entities_by_type(
	pool: &Pool<ConnectionManager<PgConnection>>,
	typ: &Option<DomaintypeId>,
	attributes: &Vec<(String, PrimitiveType)>,
	position: u64,
	max_records: u64,
) -> Result<(impl Iterator<Item = EntityAndReferences>, u64, u64), ChronicleArrowError> {
	let mut connection = pool.get()?;

	let mut entities_and_references = Vec::new();

	let entities_and_namespaces: Vec<(Entity, Namespace)> = if let Some(typ_value) = typ {
		entity::table
			.inner_join(namespace::table.on(entity::namespace_id.eq(namespace::id)))
			.filter(entity::domaintype.eq(typ_value.external_id_part()))
			.order(entity::id)
			.select((Entity::as_select(), Namespace::as_select()))
			.offset(position as i64)
			.limit(max_records as i64)
			.load::<(Entity, Namespace)>(&mut connection)?
	} else {
		entity::table
			.inner_join(namespace::table.on(entity::namespace_id.eq(namespace::id)))
			.filter(entity::domaintype.is_null())
			.order(entity::id)
			.select((Entity::as_select(), Namespace::as_select()))
			.offset(position as i64)
			.limit(max_records as i64)
			.load::<(Entity, Namespace)>(&mut connection)?
	};

	let (entities, namespaces): (Vec<Entity>, Vec<Namespace>) =
		entities_and_namespaces.into_iter().unzip();

	let entity_ids: Vec<i32> = entities.iter().map(|entity| entity.id).collect();
	let attribute_names: Vec<String> = attributes.iter().map(|(name, _)| name.clone()).collect();

	let loaded_attributes: Vec<(i32, String, serde_json::Value)> = entity_attribute::table
		.filter(entity_attribute::entity_id.eq_any(&entity_ids))
		.filter(entity_attribute::typename.eq_any(&attribute_names))
		.select((entity_attribute::entity_id, entity_attribute::typename, entity_attribute::value))
		.load::<(i32, String, String)>(&mut connection)?
		.into_iter()
		.map(|(entity_id, typename, value)| {
			let parsed_value: serde_json::Value = serde_json::from_str(&value).unwrap_or_default();
			(entity_id, typename, parsed_value)
		})
		.collect();

	let mut attributes_map: HashMap<i32, Vec<Attribute>> = HashMap::new();
	for (entity_id, typename, value) in loaded_attributes {
		let attribute = Attribute::new(&typename, value);
		attributes_map.entry(entity_id).or_default().push(attribute);
	}

	let fetched_records: u64 = entities.len() as u64;
	// Load generations
	let mut generation_map: HashMap<i32, Vec<String>> = Generation::belonging_to(&entities)
		.inner_join(activity::table)
		.select((generation::generated_entity_id, activity::external_id))
		.load::<(i32, String)>(&mut connection)?
		.into_iter()
		.fold(HashMap::new(), |mut acc: HashMap<i32, Vec<String>>, (id, external_id)| {
			acc.entry(id).or_default().push(external_id);
			acc
		});

	let mut attribution_map: HashMap<i32, Vec<_>> = Attribution::belonging_to(&entities)
		.inner_join(agent::table)
		.select((attribution::agent_id, agent::external_id, attribution::role.nullable()))
		.load::<(i32, String, Option<String>)>(&mut connection)?
		.into_iter()
		.fold(HashMap::new(), |mut acc: HashMap<i32, Vec<_>>, (id, external_id, role)| {
			acc.entry(id)
				.or_default()
				.push(EntityAttributionRef { agent: external_id, role });
			acc
		});

	let mut derivation_map: HashMap<(i32, DerivationType), Vec<_>> =
		Derivation::belonging_to(&entities)
			.inner_join(activity::table.on(derivation::activity_id.eq(activity::id)))
			.inner_join(entity::table.on(derivation::used_entity_id.eq(entity::id)))
			.select((
				derivation::used_entity_id,
				activity::external_id,
				entity::external_id,
				derivation::typ,
			))
			.load::<(i32, String, String, i32)>(&mut connection)?
			.into_iter()
			.map(|(entity_id, activity_external_id, entity_external_id, derivation_type)| {
				DerivationType::try_from(derivation_type)
					.map(|derivation_type| {
						(entity_id, activity_external_id, entity_external_id, derivation_type)
					})
					.map_err(|e| ChronicleArrowError::InvalidValue(e.to_string()))
			})
			.collect::<Result<Vec<_>, ChronicleArrowError>>()?
			.into_iter()
			.fold(
				HashMap::new(),
				|mut acc: HashMap<(i32, DerivationType), Vec<_>>,
				 (entity_id, activity_external_id, entity_external_id, derivation_type)| {
					acc.entry((entity_id, derivation_type)).or_default().push(DerivationRef {
						activity: activity_external_id,
						target: entity_external_id,
					});
					acc
				},
			);

	for (entity, ns) in entities.into_iter().zip(namespaces) {
		let entity_id = entity.id;
		entities_and_references.push(EntityAndReferences {
			id: entity.external_id,
			namespace_name: ns.external_id,
			namespace_uuid: Uuid::parse_str(&ns.uuid)?.into_bytes(),
			attributes: Attributes::new(
				entity.domaintype.map(DomaintypeId::from_external_id),
				attributes_map.remove(&entity_id).unwrap_or_default(),
			),
			was_generated_by: generation_map.remove(&entity_id).unwrap_or_default(),
			was_attributed_to: attribution_map.remove(&entity_id).unwrap_or_default(),
			was_derived_from: derivation_map
				.remove(&(entity_id, DerivationType::None))
				.unwrap_or_default(),
			was_quoted_from: derivation_map
				.remove(&(entity_id, DerivationType::Quotation))
				.unwrap_or_default(),
			had_primary_source: derivation_map
				.remove(&(entity_id, DerivationType::PrimarySource))
				.unwrap_or_default(),
			was_revision_of: derivation_map
				.remove(&(entity_id, DerivationType::Revision))
				.unwrap_or_default(),
		});
	}

	tracing::debug!(?fetched_records);

	Ok((entities_and_references.into_iter(), fetched_records, fetched_records))
}

pub fn load_activities_by_type(
	pool: &Pool<ConnectionManager<PgConnection>>,
	typ: &Option<DomaintypeId>,
	position: u64,
	max_records: u64,
) -> Result<(impl Iterator<Item = ActivityAndReferences>, u64, u64), ChronicleArrowError> {
	let mut connection = pool.get().map_err(ChronicleArrowError::PoolError)?;

	let activities_and_namespaces: Vec<(Activity, Namespace)> = match typ {
		Some(typ_value) => activity::table
			.inner_join(namespace::table.on(activity::namespace_id.eq(namespace::id)))
			.filter(activity::domaintype.eq(typ_value.external_id_part()))
			.order(activity::id)
			.select((Activity::as_select(), Namespace::as_select()))
			.offset(position as i64)
			.limit(max_records as i64)
			.load(&mut connection)?,
		None => activity::table
			.inner_join(namespace::table.on(activity::namespace_id.eq(namespace::id)))
			.filter(activity::domaintype.is_null())
			.order(activity::id)
			.select((Activity::as_select(), Namespace::as_select()))
			.offset(position as i64)
			.limit(max_records as i64)
			.load(&mut connection)?,
	};

	let (activities, namespaces): (Vec<Activity>, Vec<Namespace>) =
		activities_and_namespaces.into_iter().unzip();

	let mut was_informed_by_map: HashMap<i32, Vec<String>> =
		WasInformedBy::belonging_to(&activities)
			.inner_join(activity::table.on(wasinformedby::informing_activity_id.eq(activity::id)))
			.select((wasinformedby::activity_id, activity::external_id))
			.load::<(i32, String)>(&mut connection)?
			.into_iter()
			.fold(HashMap::new(), |mut acc: HashMap<i32, Vec<String>>, (id, external_id)| {
				acc.entry(id).or_default().push(external_id);
				acc
			});

	let mut used_map: HashMap<i32, Vec<String>> = Usage::belonging_to(&activities)
		.inner_join(entity::table.on(usage::entity_id.eq(entity::id)))
		.select((usage::activity_id, entity::external_id))
		.load::<(i32, String)>(&mut connection)?
		.into_iter()
		.fold(HashMap::new(), |mut acc: HashMap<i32, Vec<String>>, (id, external_id)| {
			acc.entry(id).or_default().push(external_id);
			acc
		});

	let mut generated_map: HashMap<i32, Vec<String>> = Generation::belonging_to(&activities)
		.inner_join(entity::table.on(generation::generated_entity_id.eq(entity::id)))
		.select((generation::activity_id, entity::external_id))
		.load::<(i32, String)>(&mut connection)?
		.into_iter()
		.fold(HashMap::new(), |mut acc: HashMap<i32, Vec<String>>, (id, external_id)| {
			acc.entry(id).or_default().push(external_id);
			acc
		});

	let fetched_records = activities.len() as u64;

	let mut activities_and_references = vec![];

	for (activity, ns) in activities.into_iter().zip(namespaces) {
		activities_and_references.push(ActivityAndReferences {
			id: activity.external_id,
			namespace_name: ns.external_id,
			namespace_uuid: Uuid::parse_str(&ns.uuid)?.into_bytes(),
			attributes: Attributes::new(
				activity.domaintype.map(DomaintypeId::from_external_id),
				vec![],
			), // Placeholder for attribute loading logic
			started: activity.started.map(|dt| dt.and_utc()),
			ended: activity.ended.map(|dt| dt.and_utc()),
			was_informed_by: was_informed_by_map.remove(&activity.id).unwrap_or_default(),
			used: used_map.remove(&activity.id).unwrap_or_default(),
			generated: generated_map.remove(&activity.id).unwrap_or_default(),
			..Default::default()
		});
	}
	Ok((activities_and_references.into_iter(), fetched_records, fetched_records))
}

#[tracing::instrument(skip(pool))]
pub fn load_agents_by_type(
	pool: &Pool<ConnectionManager<PgConnection>>,
	typ: &Option<DomaintypeId>,
	position: u64,
	max_records: u64,
) -> Result<(impl Iterator<Item = AgentAndReferences>, u64, u64), ChronicleArrowError> {
	let mut connection = pool.get().map_err(ChronicleArrowError::PoolError)?;

	let agents_and_namespaces: Vec<(Agent, Namespace)> = match typ {
		Some(typ_value) => agent::table
			.inner_join(namespace::table.on(agent::namespace_id.eq(namespace::id)))
			.filter(agent::domaintype.eq(typ_value.external_id_part()))
			.order(agent::id)
			.select((Agent::as_select(), Namespace::as_select()))
			.offset(position as i64)
			.limit(max_records as i64)
			.load(&mut connection)?,
		None => agent::table
			.inner_join(namespace::table.on(agent::namespace_id.eq(namespace::id)))
			.filter(agent::domaintype.is_null())
			.order(agent::id)
			.select((Agent::as_select(), Namespace::as_select()))
			.offset(position as i64)
			.limit(max_records as i64)
			.load(&mut connection)?,
	};

	let total_records = agents_and_namespaces.len() as u64;

	let (agents, namespaces): (Vec<Agent>, Vec<Namespace>) =
		agents_and_namespaces.into_iter().unzip();

	let mut agents_and_references = vec![];

	for (agent, ns) in agents.into_iter().zip(namespaces) {
		agents_and_references.push(AgentAndReferences {
			id: agent.external_id,
			namespace_name: ns.external_id,
			namespace_uuid: Uuid::parse_str(&ns.uuid)?.into_bytes(),
			attributes: Attributes::new(
				agent.domaintype.map(DomaintypeId::from_external_id),
				vec![],
			), // Placeholder for attribute loading logic
			..Default::default()
		});
	}

	Ok((agents_and_references.into_iter(), total_records, total_records))
}
