use std::{collections::HashMap, sync::Arc};

use crate::{
	meta::{agent_attribution_struct, agent_delegation_struct},
	ChronicleArrowError, DomainTypeMeta,
};
use arrow::array::{ArrayBuilder, StringBuilder, StructBuilder};
use arrow_array::{Array, BooleanArray, Int64Array, ListArray, RecordBatch, StringArray};
use arrow_buffer::{Buffer, ToByteSlice};
use arrow_data::ArrayData;
use arrow_schema::{DataType, Field};
use chronicle_persistence::{
	query::{Agent, Namespace},
	schema::{agent, namespace},
};
use common::{
	attributes::Attributes,
	domain::PrimitiveType,
	prov::{DomaintypeId, ExternalIdPart},
};
use diesel::{
	pg::PgConnection,
	prelude::*,
	r2d2::{ConnectionManager, Pool},
};
use uuid::Uuid;
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
