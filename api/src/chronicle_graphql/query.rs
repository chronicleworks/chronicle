use async_graphql::{
    connection::{Connection, EmptyFields},
    Context, ID,
};

use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use common::prov::{AgentId, DomaintypeId, EntityId, NamePart};
use diesel::prelude::*;
use tracing::instrument;

use crate::{
    chronicle_graphql::{Activity, Store},
    persistence::schema::generation,
};

use super::{Agent, Entity};

#[allow(clippy::too_many_arguments)]
#[instrument(skip(ctx))]
pub async fn activity_timeline<'a>(
    ctx: &Context<'a>,
    activity_types: Vec<DomaintypeId>,
    for_entity: Vec<EntityId>,
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
    namespace: Option<ID>,
    after: Option<String>,
    before: Option<String>,
    first: Option<i32>,
    last: Option<i32>,
) -> async_graphql::Result<Connection<i32, Activity, EmptyFields, EmptyFields>> {
    use crate::persistence::schema::{activity, entity, namespace::dsl as nsdsl, useage};

    let store = ctx.data_unchecked::<Store>();

    let mut connection = store.pool.get()?;
    let ns = namespace.unwrap_or_else(|| "default".into());

    // Default from and to to the maximum possible time range
    let from = from.or_else(|| {
        Some(DateTime::<Utc>::from_utc(
            NaiveDateTime::new(
                NaiveDate::from_ymd(1582, 10, 16),
                NaiveTime::from_hms(0, 0, 0),
            ),
            Utc,
        ))
    });

    let to = to.or_else(|| Some(Utc::now()));

    gql_cursor!(
        after,
        before,
        first,
        last,
        activity::table
            .left_join(useage::table.on(useage::activity_id.eq(activity::id)))
            .left_join(generation::table.on(generation::activity_id.eq(activity::id)))
            .left_join(
                entity::table.on(entity::id
                    .eq(useage::entity_id)
                    .or(entity::id.eq(generation::generated_entity_id))),
            )
            .inner_join(nsdsl::namespace.on(activity::namespace_id.eq(nsdsl::id)))
            .filter(
                entity::name.eq_any(
                    for_entity
                        .iter()
                        .map(|x| x.name_part().clone())
                        .collect::<Vec<_>>(),
                ),
            )
            .filter(
                activity::domaintype.eq_any(
                    activity_types
                        .iter()
                        .map(|x| x.name_part().clone())
                        .collect::<Vec<_>>(),
                ),
            )
            .filter(nsdsl::name.eq(&**ns))
            .filter(activity::started.ge(from.map(|x| x.naive_utc())))
            .filter(activity::ended.le(to.map(|x| x.naive_utc()))),
        activity::started.asc(),
        Activity,
        connection
    )
}

#[allow(clippy::too_many_arguments)]
pub async fn agents_by_type<'a>(
    ctx: &Context<'a>,
    typ: Option<DomaintypeId>,
    namespace: Option<String>,
    after: Option<String>,
    before: Option<String>,
    first: Option<i32>,
    last: Option<i32>,
) -> async_graphql::Result<Connection<i32, Agent, EmptyFields, EmptyFields>> {
    use crate::persistence::schema::{
        agent::{self},
        namespace::dsl as nsdsl,
    };

    let store = ctx.data_unchecked::<Store>();

    let mut connection = store.pool.get()?;
    let ns = namespace.unwrap_or_else(|| "default".into());

    gql_cursor!(
        after,
        before,
        first,
        last,
        agent::table.inner_join(nsdsl::namespace).filter(
            nsdsl::name
                .eq(ns.as_str())
                .and(agent::domaintype.eq(typ.as_ref().map(|x| x.name_part().to_owned())))
        ),
        agent::name.asc(),
        Agent,
        connection
    )
}

pub async fn agent_by_id<'a>(
    ctx: &Context<'a>,
    id: AgentId,
    namespace: Option<String>,
) -> async_graphql::Result<Option<Agent>> {
    use crate::persistence::schema::{
        agent::{self, dsl},
        namespace::dsl as nsdsl,
    };

    let store = ctx.data_unchecked::<Store>();

    let ns = namespace.unwrap_or_else(|| "default".into());
    let mut connection = store.pool.get()?;

    Ok(agent::table
        .inner_join(nsdsl::namespace)
        .filter(dsl::name.eq(id.name_part()).and(nsdsl::name.eq(&ns)))
        .select(Agent::as_select())
        .first::<Agent>(&mut connection)
        .optional()?)
}

pub async fn entity_by_id<'a>(
    ctx: &Context<'a>,
    id: EntityId,
    namespace: Option<String>,
) -> async_graphql::Result<Option<Entity>> {
    use crate::persistence::schema::{
        entity::{self, dsl},
        namespace::dsl as nsdsl,
    };

    let store = ctx.data_unchecked::<Store>();
    let ns = namespace.unwrap_or_else(|| "default".into());
    let mut connection = store.pool.get()?;

    Ok(entity::table
        .inner_join(nsdsl::namespace)
        .filter(dsl::name.eq(id.name_part()).and(nsdsl::name.eq(&ns)))
        .select(Entity::as_select())
        .first::<Entity>(&mut connection)
        .optional()?)
}
