use super::{Activity, Agent, Entity, Namespace, Store};
use async_graphql::Context;
use common::prov::Role;
use diesel::prelude::*;

pub async fn namespace<'a>(
    namespaceid: i32,
    ctx: &Context<'a>,
) -> async_graphql::Result<Namespace> {
    use crate::persistence::schema::namespace::{self, dsl};
    let store = ctx.data_unchecked::<Store>();

    let mut connection = store.pool.get()?;

    Ok(namespace::table
        .filter(dsl::id.eq(namespaceid))
        .first::<Namespace>(&mut connection)?)
}

pub async fn was_associated_with<'a>(
    id: i32,
    ctx: &Context<'a>,
) -> async_graphql::Result<Vec<(Agent, Option<Role>)>> {
    use crate::persistence::schema::association::{self, dsl};

    let store = ctx.data_unchecked::<Store>();

    let mut connection = store.pool.get()?;

    let res = association::table
        .filter(dsl::activity_id.eq(id))
        .inner_join(crate::persistence::schema::agent::table)
        .order(crate::persistence::schema::agent::external_id)
        .select((Agent::as_select(), association::role))
        .load::<(Agent, Option<Role>)>(&mut connection)?;

    Ok(res)
}

pub async fn used<'a>(id: i32, ctx: &Context<'a>) -> async_graphql::Result<Vec<Entity>> {
    use crate::persistence::schema::usage::{self, dsl};

    let store = ctx.data_unchecked::<Store>();

    let mut connection = store.pool.get()?;

    let res = usage::table
        .filter(dsl::activity_id.eq(id))
        .inner_join(crate::persistence::schema::entity::table)
        .order(crate::persistence::schema::entity::external_id)
        .select(Entity::as_select())
        .load::<Entity>(&mut connection)?;

    Ok(res)
}

pub async fn was_informed_by<'a>(
    id: i32,
    ctx: &Context<'a>,
) -> async_graphql::Result<Vec<Activity>> {
    use crate::persistence::schema::wasinformedby::{self, dsl};

    let store = ctx.data_unchecked::<Store>();

    let mut connection = store.pool.get()?;

    let res =
        wasinformedby::table
            .filter(dsl::activity_id.eq(id))
            .inner_join(crate::persistence::schema::activity::table.on(
                wasinformedby::informing_activity_id.eq(crate::persistence::schema::activity::id),
            ))
            .order(crate::persistence::schema::activity::external_id)
            .select(Activity::as_select())
            .load::<Activity>(&mut connection)?;

    Ok(res)
}

pub async fn generated<'a>(id: i32, ctx: &Context<'a>) -> async_graphql::Result<Vec<Entity>> {
    use crate::persistence::schema::generation::{self, dsl};

    let store = ctx.data_unchecked::<Store>();

    let mut connection = store.pool.get()?;

    let res = generation::table
        .filter(dsl::activity_id.eq(id))
        .inner_join(crate::persistence::schema::entity::table)
        .select(Entity::as_select())
        .load::<Entity>(&mut connection)?;

    Ok(res)
}

pub async fn load_attribute<'a>(
    id: i32,
    external_id: &str,
    ctx: &Context<'a>,
) -> async_graphql::Result<Option<serde_json::Value>> {
    use crate::persistence::schema::activity_attribute;

    let store = ctx.data_unchecked::<Store>();

    let mut connection = store.pool.get()?;

    Ok(activity_attribute::table
        .filter(
            activity_attribute::activity_id
                .eq(id)
                .and(activity_attribute::typename.eq(external_id)),
        )
        .select(activity_attribute::value)
        .first::<String>(&mut connection)
        .optional()?
        .as_deref()
        .map(serde_json::from_str)
        .transpose()?)
}
