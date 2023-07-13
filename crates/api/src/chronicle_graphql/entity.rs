use super::{Activity, Agent, Entity, Namespace, Store};
use async_graphql::Context;
use common::prov::{operations::DerivationType, Role};
use diesel::prelude::*;

async fn typed_derivation<'a>(
    id: i32,
    ctx: &Context<'a>,
    typ: DerivationType,
) -> async_graphql::Result<Vec<Entity>> {
    use crate::persistence::schema::{
        derivation::{self, dsl},
        entity as entitydsl,
    };

    let store = ctx.data_unchecked::<Store>();

    let mut connection = store.pool.get()?;

    let res = derivation::table
        .filter(dsl::generated_entity_id.eq(id).and(dsl::typ.eq(typ)))
        .inner_join(entitydsl::table.on(dsl::used_entity_id.eq(entitydsl::id)))
        .select(Entity::as_select())
        .load::<Entity>(&mut connection)?;

    Ok(res)
}

pub async fn namespace<'a>(
    namespace_id: i32,
    ctx: &Context<'a>,
) -> async_graphql::Result<Namespace> {
    use crate::persistence::schema::namespace::{self, dsl};

    let store = ctx.data_unchecked::<Store>();

    let mut connection = store.pool.get()?;

    Ok(namespace::table
        .filter(dsl::id.eq(namespace_id))
        .first::<Namespace>(&mut connection)?)
}

/// Return the agents to which an entity was attributed along with the roles in which it was attributed
pub async fn was_attributed_to<'a>(
    id: i32,
    ctx: &Context<'a>,
) -> async_graphql::Result<Vec<(Agent, Option<Role>)>> {
    use crate::persistence::schema::{agent, attribution};

    let store = ctx.data_unchecked::<Store>();
    let mut connection = store.pool.get()?;

    let res = attribution::table
        .filter(attribution::dsl::entity_id.eq(id))
        .inner_join(agent::table)
        .order(agent::external_id)
        .select((Agent::as_select(), attribution::role))
        .load::<(Agent, Role)>(&mut connection)?
        .into_iter()
        .map(|(agent, role)| {
            let role = if role.0.is_empty() { None } else { Some(role) };
            (agent, role)
        })
        .collect();

    Ok(res)
}

pub async fn was_generated_by<'a>(
    id: i32,
    ctx: &Context<'a>,
) -> async_graphql::Result<Vec<Activity>> {
    use crate::persistence::schema::generation::{self, dsl};

    let store = ctx.data_unchecked::<Store>();

    let mut connection = store.pool.get()?;

    let res = generation::table
        .filter(dsl::generated_entity_id.eq(id))
        .inner_join(crate::persistence::schema::activity::table)
        .select(Activity::as_select())
        .load::<Activity>(&mut connection)?;

    Ok(res)
}

pub async fn was_derived_from<'a>(
    id: i32,
    ctx: &Context<'a>,
) -> async_graphql::Result<Vec<Entity>> {
    use crate::persistence::schema::{
        derivation::{self, dsl},
        entity as entitydsl,
    };

    let store = ctx.data_unchecked::<Store>();

    let mut connection = store.pool.get()?;

    let res = derivation::table
        .filter(dsl::generated_entity_id.eq(id))
        .inner_join(entitydsl::table.on(dsl::used_entity_id.eq(entitydsl::id)))
        .select(Entity::as_select())
        .load::<Entity>(&mut connection)?;

    Ok(res)
}

pub async fn had_primary_source<'a>(
    id: i32,
    ctx: &Context<'a>,
) -> async_graphql::Result<Vec<Entity>> {
    typed_derivation(id, ctx, DerivationType::PrimarySource).await
}

pub async fn was_revision_of<'a>(id: i32, ctx: &Context<'a>) -> async_graphql::Result<Vec<Entity>> {
    typed_derivation(id, ctx, DerivationType::Revision).await
}
pub async fn was_quoted_from<'a>(id: i32, ctx: &Context<'a>) -> async_graphql::Result<Vec<Entity>> {
    typed_derivation(id, ctx, DerivationType::Quotation).await
}

pub async fn load_attribute<'a>(
    id: i32,
    external_id: &str,
    ctx: &Context<'a>,
) -> async_graphql::Result<Option<serde_json::Value>> {
    use crate::persistence::schema::entity_attribute;

    let store = ctx.data_unchecked::<Store>();

    let mut connection = store.pool.get()?;

    Ok(entity_attribute::table
        .filter(
            entity_attribute::entity_id
                .eq(id)
                .and(entity_attribute::typename.eq(external_id)),
        )
        .select(entity_attribute::value)
        .first::<String>(&mut connection)
        .optional()?
        .as_deref()
        .map(serde_json::from_str)
        .transpose()?)
}
