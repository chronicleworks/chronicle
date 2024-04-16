use std::{
	collections::HashMap,
	sync::{Arc, Mutex},
};

use arrow_schema::{Schema, SchemaBuilder};
use common::domain::{
	ActivityDef, AgentDef, ChronicleDomainDef, EntityDef, PrimitiveType, TypeName,
};

fn field_for_domain_primitive(prim: &PrimitiveType) -> Option<arrow_schema::DataType> {
	match prim {
		PrimitiveType::String => Some(arrow_schema::DataType::Utf8),
		PrimitiveType::Int => Some(arrow_schema::DataType::Int64),
		PrimitiveType::Bool => Some(arrow_schema::DataType::Boolean),
		PrimitiveType::JSON => Some(arrow_schema::DataType::Binary),
	}
}

#[tracing::instrument]
fn schema_for_namespace() -> Schema {
	let mut builder = SchemaBuilder::new();

	builder.push(arrow_schema::Field::new("name", arrow_schema::DataType::Utf8, false));
	builder.push(arrow_schema::Field::new("uuid", arrow_schema::DataType::Utf8, false));

	builder.finish()
}

pub fn attribution_struct() -> arrow_schema::DataType {
	arrow_schema::DataType::Struct(
		vec![
			arrow_schema::Field::new("agent", arrow_schema::DataType::Utf8, false),
			arrow_schema::Field::new("role", arrow_schema::DataType::Utf8, true),
		]
		.into(),
	)
}

pub fn derivation_struct() -> arrow_schema::DataType {
	arrow_schema::DataType::Struct(
		vec![
			arrow_schema::Field::new("target", arrow_schema::DataType::Utf8, false),
			arrow_schema::Field::new("activity", arrow_schema::DataType::Utf8, false),
		]
		.into(),
	)
}

pub fn association_struct() -> arrow_schema::DataType {
	arrow_schema::DataType::Struct(
		vec![
			arrow_schema::Field::new("responsible_agent", arrow_schema::DataType::Utf8, false),
			arrow_schema::Field::new("responsible_role", arrow_schema::DataType::Utf8, true),
			arrow_schema::Field::new("delegate_agent", arrow_schema::DataType::Utf8, true),
			arrow_schema::Field::new("delegate_role", arrow_schema::DataType::Utf8, true),
		]
		.into(),
	)
}

pub fn agent_delegation_struct() -> arrow_schema::DataType {
	arrow_schema::DataType::Struct(
		vec![
			arrow_schema::Field::new("agent", arrow_schema::DataType::Utf8, false),
			arrow_schema::Field::new("activity", arrow_schema::DataType::Utf8, true),
			arrow_schema::Field::new("role", arrow_schema::DataType::Utf8, true),
		]
		.into(),
	)
}

pub fn agent_attribution_struct() -> arrow_schema::DataType {
	arrow_schema::DataType::Struct(
		vec![
			arrow_schema::Field::new("entity", arrow_schema::DataType::Utf8, false),
			arrow_schema::Field::new("role", arrow_schema::DataType::Utf8, true),
		]
		.into(),
	)
}

pub fn schema_for_entity(entity: &EntityDef) -> Schema {
	let mut builder = SchemaBuilder::new();

	builder.push(arrow_schema::Field::new("namespace_name", arrow_schema::DataType::Utf8, false));
	builder.push(arrow_schema::Field::new("namespace_uuid", arrow_schema::DataType::Utf8, false));
	builder.push(arrow_schema::Field::new("id", arrow_schema::DataType::Utf8, false));

	for attribute in &entity.attributes {
		if let Some(data_type) = field_for_domain_primitive(&attribute.primitive_type) {
			builder.push(arrow_schema::Field::new(
				&attribute.preserve_inflection(),
				data_type,
				true,
			));
		}
	}

	builder.push(arrow_schema::Field::new(
		"was_generated_by",
		arrow_schema::DataType::new_list(arrow_schema::DataType::Utf8, false),
		false,
	));

	builder.push(arrow_schema::Field::new(
		"was_attributed_to",
		arrow_schema::DataType::new_list(attribution_struct(), false),
		false,
	));

	builder.push(arrow_schema::Field::new(
		"was_derived_from",
		arrow_schema::DataType::new_list(derivation_struct(), false),
		false,
	));

	builder.push(arrow_schema::Field::new(
		"had_primary_source",
		arrow_schema::DataType::new_list(derivation_struct(), false),
		false,
	));

	builder.push(arrow_schema::Field::new(
		"was_quoted_from",
		arrow_schema::DataType::new_list(derivation_struct(), false),
		false,
	));

	builder.push(arrow_schema::Field::new(
		"was_revision_of",
		arrow_schema::DataType::new_list(derivation_struct(), false),
		false,
	));

	builder.finish()
}

pub fn schema_for_activity(activity: &ActivityDef) -> Schema {
	let mut builder = SchemaBuilder::new();

	builder.push(arrow_schema::Field::new("namespace_name", arrow_schema::DataType::Utf8, false));
	builder.push(arrow_schema::Field::new("namespace_uuid", arrow_schema::DataType::Utf8, false));
	builder.push(arrow_schema::Field::new("id", arrow_schema::DataType::Utf8, false));

	for attribute in &activity.attributes {
		if let Some(typ) = field_for_domain_primitive(&attribute.primitive_type) {
			builder.push(arrow_schema::Field::new(&attribute.preserve_inflection(), typ, true));
		}
	}

	builder.push(arrow_schema::Field::new(
		"started",
		arrow_schema::DataType::Timestamp(arrow_schema::TimeUnit::Nanosecond, Some("UTC".into())),
		true,
	));

	builder.push(arrow_schema::Field::new(
		"ended",
		arrow_schema::DataType::Timestamp(arrow_schema::TimeUnit::Nanosecond, Some("UTC".into())),
		true,
	));

	builder.push(arrow_schema::Field::new(
		"used",
		arrow_schema::DataType::new_list(arrow_schema::DataType::Utf8, false),
		true,
	));

	builder.push(arrow_schema::Field::new(
		"generated",
		arrow_schema::DataType::new_list(arrow_schema::DataType::Utf8, false),
		true,
	));

	builder.push(arrow_schema::Field::new(
		"was_informed_by",
		arrow_schema::DataType::new_list(arrow_schema::DataType::Utf8, false),
		false,
	));

	builder.push(arrow_schema::Field::new(
		"was_associated_with",
		arrow_schema::DataType::new_list(association_struct(), false),
		false,
	));

	builder.finish()
}

pub fn schema_for_agent(agent: &AgentDef) -> Schema {
	let mut builder = SchemaBuilder::new();
	builder.push(arrow_schema::Field::new("namespace_name", arrow_schema::DataType::Utf8, false));
	builder.push(arrow_schema::Field::new("namespace_uuid", arrow_schema::DataType::Utf8, false));

	builder.push(arrow_schema::Field::new("id", arrow_schema::DataType::Utf8, false));
	for attribute in &agent.attributes {
		if let Some(typ) = field_for_domain_primitive(&attribute.primitive_type) {
			builder.push(arrow_schema::Field::new(&attribute.preserve_inflection(), typ, true));
		}
	}

	builder.push(arrow_schema::Field::new(
		"acted_on_behalf_of",
		arrow_schema::DataType::new_list(agent_delegation_struct(), false),
		true,
	));

	builder.push(arrow_schema::Field::new(
		"was_attributed_to",
		arrow_schema::DataType::new_list(agent_attribution_struct(), false),
		false,
	));

	builder.finish()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum Term {
	Namespace,
	Entity,
	Activity,
	Agent,
}

use std::str::FromStr;

impl FromStr for Term {
	type Err = ();

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s {
			"Namespace" => Ok(Term::Namespace),
			"Entity" => Ok(Term::Entity),
			"Activity" => Ok(Term::Activity),
			"Agent" => Ok(Term::Agent),
			_ => Err(()),
		}
	}
}

use std::fmt;

impl fmt::Display for Term {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Term::Namespace => write!(f, "Namespace"),
			Term::Entity => write!(f, "Entity"),
			Term::Activity => write!(f, "Activity"),
			Term::Agent => write!(f, "Agent"),
		}
	}
}

pub(crate) struct DomainTypeMeta {
	pub schema: Arc<Schema>,
	pub term: Term,
	pub typ: Option<Box<dyn common::domain::TypeName + Send + Sync>>,
	pub attributes: Vec<(String, PrimitiveType)>,
}

lazy_static::lazy_static! {
	static ref SCHEMA_CACHE: Mutex<HashMap<Vec<String>, Arc<DomainTypeMeta>>> =
		Mutex::new(HashMap::new());
}

pub fn get_domain_type_meta_from_cache(
	descriptor_path: &Vec<String>,
) -> Option<Arc<DomainTypeMeta>> {
	let cache = SCHEMA_CACHE.lock().unwrap();
	cache.get(descriptor_path).cloned()
}

#[tracing::instrument(skip(domain_type, type_name, schema), fields(term, schema = ?schema, type_name = type_name))]
pub fn cache_metadata(
	term: Term,
	domain_type: Box<dyn TypeName + Send + Sync>,
	type_name: String,
	attributes: Vec<(String, PrimitiveType)>,
	schema: Schema,
) {
	let mut cache = SCHEMA_CACHE.lock().expect("Failed to lock SCHEMA_CACHE");
	let domain_type_meta = Arc::new(DomainTypeMeta {
		schema: schema.into(),
		term,
		typ: Some(domain_type),
		attributes,
	});
	cache.insert(vec![term.to_string(), type_name], domain_type_meta);
}

pub fn cache_namespace_schema() {
	let mut cache = SCHEMA_CACHE.lock().unwrap();
	cache.insert(
		vec!["Namespace".to_string()],
		Arc::new(DomainTypeMeta {
			schema: schema_for_namespace().into(),
			term: Term::Namespace,
			typ: None,
			attributes: vec![],
		}),
	);
}

#[tracing::instrument(skip(domain_def))]
pub fn cache_domain_schemas(domain_def: &ChronicleDomainDef) {
	for entity in &domain_def.entities {
		let schema = schema_for_entity(entity);

		let attributes = entity
			.attributes
			.iter()
			.map(|attr| (attr.preserve_inflection(), attr.primitive_type))
			.collect();
		cache_metadata(
			Term::Entity,
			Box::new(entity.clone()),
			entity.as_type_name(),
			attributes,
			schema,
		);
	}

	for agent in &domain_def.agents {
		let schema = schema_for_agent(agent);

		let attributes = agent
			.attributes
			.iter()
			.map(|attr| (attr.preserve_inflection(), attr.primitive_type))
			.collect();
		cache_metadata(
			Term::Agent,
			Box::new(agent.clone()),
			agent.as_type_name(),
			attributes,
			schema,
		);
	}

	for activity in &domain_def.activities {
		let schema = schema_for_activity(activity);

		let attributes = activity
			.attributes
			.iter()
			.map(|attr| (attr.preserve_inflection(), attr.primitive_type))
			.collect();
		cache_metadata(
			Term::Activity,
			Box::new(activity.clone()),
			activity.as_type_name(),
			attributes,
			schema,
		);
	}
}
