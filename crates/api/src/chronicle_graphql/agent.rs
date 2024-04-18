use async_graphql::Context;
use chronicle_persistence::{
	queryable::{Agent, Entity, Namespace},
	Store,
};
use common::prov::Role;
use diesel::prelude::*;

use crate::chronicle_graphql::DatabaseContext;

pub async fn namespace<'a>(
	namespace_id: i32,
	ctx: &Context<'a>,
) -> async_graphql::Result<Namespace> {
	use chronicle_persistence::schema::namespace::{self, dsl};
	let store = ctx.data::<DatabaseContext>()?;

	let mut connection = store.connection()?;

	Ok(namespace::table
		.filter(dsl::id.eq(namespace_id))
		.first::<Namespace>(&mut connection)?)
}

pub async fn acted_on_behalf_of<'a>(
	id: i32,
	ctx: &Context<'a>,
) -> async_graphql::Result<Vec<(Agent, Option<Role>)>> {
	use chronicle_persistence::schema::{
		agent as agentdsl,
		delegation::{self, dsl},
	};

	let store = ctx.data::<DatabaseContext>()?;

	let mut connection = store.connection()?;

	Ok(delegation::table
		.filter(dsl::delegate_id.eq(id))
		.inner_join(agentdsl::table.on(dsl::responsible_id.eq(agentdsl::id)))
		.order(agentdsl::external_id)
		.select((Agent::as_select(), dsl::role))
		.load::<(Agent, Role)>(&mut connection)?
		.into_iter()
		.map(|(a, r)| (a, if r.0.is_empty() { None } else { Some(r) }))
		.collect())
}

/// Return the entities an agent has attributed to it along with the roles in which they were
/// attributed
pub async fn attribution<'a>(
	id: i32,
	ctx: &Context<'a>,
) -> async_graphql::Result<Vec<(Entity, Option<Role>)>> {
	use chronicle_persistence::schema::{
		attribution::{self, dsl},
		entity as entity_dsl,
	};

	let store = ctx.data::<DatabaseContext>()?;

	let mut connection = store.connection()?;

	Ok(attribution::table
		.filter(dsl::agent_id.eq(id))
		.inner_join(entity_dsl::table.on(dsl::entity_id.eq(entity_dsl::id)))
		.order(entity_dsl::external_id)
		.select((Entity::as_select(), dsl::role))
		.load::<(Entity, Role)>(&mut connection)?
		.into_iter()
		.map(|(entity, role)| (entity, if role.0.is_empty() { None } else { Some(role) }))
		.collect())
}

pub async fn load_attribute<'a>(
	id: i32,
	external_id: &str,
	ctx: &Context<'a>,
) -> async_graphql::Result<Option<serde_json::Value>> {
	use chronicle_persistence::schema::agent_attribute;

	let store = ctx.data::<DatabaseContext>()?;

	let mut connection = store.connection()?;

	Ok(agent_attribute::table
		.filter(agent_attribute::agent_id.eq(id).and(agent_attribute::typename.eq(external_id)))
		.select(agent_attribute::value)
		.first::<String>(&mut connection)
		.optional()?
		.as_deref()
		.map(serde_json::from_str)
		.transpose()?)
}
