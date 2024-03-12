use std::collections::HashMap;

use crate::ChronicleArrowError;
use chronicle_persistence::query::{Activity, Agent, Attribution, Derivation, Generation};
use chronicle_persistence::schema::agent::{self, table};
use chronicle_persistence::schema::{activity, attribution, derivation, entity, generation};
use chronicle_persistence::{query::Entity, Store};
use common::attributes::Attributes;
use common::prov::DomaintypeId;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};

#[derive(Default)]
pub struct EntityAndReferences {
	pub(crate) id: String,
	pub(crate) namespace: String,
	pub(crate) attributes: Attributes,
	pub(crate) was_generated_by: Vec<String>,
	pub(crate) was_attributed_to: Vec<String>,
	pub(crate) was_derived_from: Vec<String>,
	pub(crate) had_primary_source: Vec<String>,
	pub(crate) was_quoted_from: Vec<String>,
}

#[derive(Default)]
pub struct ActivityAndReferences {
	pub(crate) id: String,
	pub(crate) namespace: String,
	pub(crate) attributes: Attributes,
	pub(crate) used: Vec<String>,
	pub(crate) generated: Vec<String>,
	pub(crate) was_associated_with: Vec<String>,
	pub(crate) was_started_by: Vec<String>,
	pub(crate) was_ended_by: Vec<String>,
}

#[derive(Default)]
pub struct AgentAndReferences {
	pub(crate) id: String,
	pub(crate) namespace: String,
	pub(crate) attributes: Attributes,
	pub(crate) acted_on_behalf_of: Vec<String>,
	pub(crate) was_associated_with: Vec<String>,
	pub(crate) was_attributed_to: Vec<String>,
}

// Returns a list of all indexed domain types from entities, activities and agents , note that these may no longer be present in the domain definition
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

// Returns a tuple of an iterator over entities of the specified domain types and their relations, the number of returned records and the total number of records
#[tracing::instrument(skip(pool))]
pub fn load_entities_by_type(
	pool: &Pool<ConnectionManager<PgConnection>>,
	typ: Vec<&str>,
	position: u64,
	max_records: u64,
) -> Result<(impl Iterator<Item = EntityAndReferences>, u64, u64), ChronicleArrowError> {
	let mut connection = pool.get()?;

	let mut entities_and_references = Vec::new();
	let mut total_records = 0u64;

	// Load entities by type
	let entities: Vec<Entity> = entity::table
		.filter(entity::domaintype.eq_any(typ))
		.order(entity::id)
		.offset(position as i64)
		.limit(max_records as i64)
		.load(&mut connection)?;

	// Load generations
	let mut generation_map: HashMap<i32, Vec<String>> = Generation::belonging_to(&entities)
		.inner_join(activity::table)
		.select((generation::generated_entity_id, activity::external_id))
		.load::<(i32, String)>(&mut connection)?
		.into_iter()
		.fold(HashMap::new(), |mut acc: HashMap<i32, Vec<String>>, (id, external_id)| {
			acc.entry(id).or_insert_with(Vec::new).push(external_id);
			acc
		});

	// Load attributions
	let mut attribution_map: HashMap<i32, Vec<(String, String)>> =
		Attribution::belonging_to(&entities)
			.inner_join(agent::table)
			.select((attribution::agent_id, agent::external_id, attribution::role))
			.load::<(i32, String, String)>(&mut connection)?
			.into_iter()
			.fold(
				HashMap::new(),
				|mut acc: HashMap<i32, Vec<(String, String)>>, (id, external_id, role)| {
					acc.entry(id).or_insert_with(Vec::new).push((external_id, role));
					acc
				},
			);

	for entity in entities {
		let entity_id = entity.id;
		entities_and_references.push(EntityAndReferences {
			id: entity.external_id,
			namespace: entity.namespace_id.to_string(),
			attributes: Attributes::new(
				entity.domaintype.map(DomaintypeId::from_external_id),
				vec![],
			), // Placeholder for attribute loading logic
			was_generated_by: generation_map.remove(&entity_id).unwrap_or_default(),
			..Default::default()
		});

		total_records += 1;
	}

	Ok((entities_and_references.into_iter(), total_records, total_records))
}

#[tracing::instrument(skip(pool))]
pub async fn load_activities_by_type(
	pool: &Pool<ConnectionManager<PgConnection>>,
	typ: Vec<&str>,
	position: u64,
	max_records: u64,
) -> Result<(impl Iterator<Item = ActivityAndReferences>, u64, u64), ChronicleArrowError> {
	let mut connection = pool.get().map_err(ChronicleArrowError::PoolError)?;

	let activities: Vec<Activity> = activity::table
		.filter(activity::domaintype.eq_any(typ))
		.order(activity::id)
		.offset(position as i64)
		.limit(max_records as i64)
		.load(&mut connection)?;

	let total_records = activities.len() as u64;

	let entities_and_references = activities.into_iter().map(|activity| ActivityAndReferences {
		id: activity.external_id,
		namespace: activity.namespace_id.to_string(),
		attributes: Attributes::new(
			activity.domaintype.map(DomaintypeId::from_external_id),
			vec![],
		),
		..Default::default()
	});

	Ok((entities_and_references, total_records, total_records))
}

#[tracing::instrument(skip(pool))]
pub async fn load_agents_by_type(
	pool: &Pool<ConnectionManager<PgConnection>>,
	typ: Vec<&str>,
	position: u64,
	max_records: u64,
) -> Result<(impl Iterator<Item = AgentAndReferences>, u64, u64), ChronicleArrowError> {
	let mut connection = pool.get().map_err(ChronicleArrowError::PoolError)?;

	let agents: Vec<Agent> = agent::table
		.filter(agent::domaintype.eq_any(typ))
		.order(agent::id)
		.offset(position as i64)
		.limit(max_records as i64)
		.load(&mut connection)?;

	let total_records = agents.len() as u64;

	let agents_and_references = agents.into_iter().map(|agent| AgentAndReferences {
		id: agent.external_id,
		namespace: agent.namespace_id.to_string(),
		attributes: Attributes::new(agent.domaintype.map(DomaintypeId::from_external_id), vec![]),
		..Default::default()
	});

	Ok((agents_and_references, total_records, total_records))
}
