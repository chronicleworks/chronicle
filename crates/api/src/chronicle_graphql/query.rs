use async_graphql::{
    connection::{Connection, EmptyFields, query},
    Context, ID,
};
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use diesel::{debug_query, pg::Pg, prelude::*};
use tracing::{debug, instrument};

use chronicle_persistence::{
    cursor::Cursorize,
    queryable::{Activity, Agent, Entity},
    schema::generation,
};
use common::{prov::{ActivityId, AgentId, DomaintypeId, EntityId, ExternalIdPart}};

use crate::chronicle_graphql::DatabaseContext;

use super::{cursor_project::project_to_nodes, GraphQlError, TimelineOrder};

#[allow(clippy::too_many_arguments)]
#[instrument(skip(ctx))]
pub async fn activity_timeline<'a>(
    ctx: &Context<'a>,
    activity_types: Option<Vec<DomaintypeId>>,
    for_agent: Option<Vec<AgentId>>,
    for_entity: Option<Vec<EntityId>>,
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
    order: Option<TimelineOrder>,
    namespace: Option<ID>,
    after: Option<String>,
    before: Option<String>,
    first: Option<i32>,
    last: Option<i32>,
) -> async_graphql::Result<Connection<i32, Activity, EmptyFields, EmptyFields>> {
    use chronicle_persistence::schema::{
        activity, agent, association, delegation, entity, namespace::dsl as nsdsl, usage,
        wasinformedby,
    };

    let store = ctx.data::<DatabaseContext>()?;

    let mut connection = store.connection()?;
    let ns = namespace.unwrap_or_else(|| "default".into());

    // Default from and to to the maximum possible time range
    let from = from.or_else(|| {
        Some(
            Utc.from_utc_datetime(
                &NaiveDate::from_ymd_opt(1582, 10, 16)
                    .expect("Invalid date")
                    .and_hms_opt(0, 0, 0)
                    .expect("Invalid time"),
            ),
        )
    });

    let to = to.or_else(|| Some(Utc::now()));

    let mut sql_query = activity::table
        .left_join(wasinformedby::table.on(wasinformedby::activity_id.eq(activity::id)))
        .left_join(usage::table.on(usage::activity_id.eq(activity::id)))
        .left_join(generation::table.on(generation::activity_id.eq(activity::id)))
        .left_join(association::table.on(association::activity_id.eq(activity::id)))
        .left_join(
            delegation::table.on(delegation::activity_id.nullable().eq(activity::id.nullable())),
        )
        .left_join(
            entity::table.on(entity::id
                .eq(usage::entity_id)
                .or(entity::id.eq(generation::generated_entity_id))),
        )
        .left_join(
            agent::table.on(agent::id
                .eq(association::agent_id)
                .or(agent::id.eq(delegation::delegate_id))
                .or(agent::id.eq(delegation::responsible_id))),
        )
        .inner_join(nsdsl::namespace.on(activity::namespace_id.eq(nsdsl::id)))
        .filter(nsdsl::external_id.eq(&**ns))
        .filter(activity::started.ge(from.map(|x| x.naive_utc())))
        .filter(activity::ended.le(to.map(|x| x.naive_utc())))
        .distinct()
        .select(Activity::as_select())
        .into_boxed();

    if let Some(for_entity) = for_entity {
        if !for_entity.is_empty() {
            sql_query = sql_query.filter(entity::external_id.eq_any(
                for_entity.iter().map(|x| x.external_id_part().clone()).collect::<Vec<_>>(),
            ))
        }
    }

    if let Some(for_agent) = for_agent {
        if !for_agent.is_empty() {
            sql_query =
                sql_query.filter(agent::external_id.eq_any(
                    for_agent.iter().map(|x| x.external_id_part().clone()).collect::<Vec<_>>(),
                ))
        }
    }

    if let Some(activity_types) = activity_types {
        if !activity_types.is_empty() {
            sql_query = sql_query.filter(activity::domaintype.eq_any(
                activity_types.iter().map(|x| x.external_id_part().clone()).collect::<Vec<_>>(),
            ));
        }
    }

    if order.unwrap_or(TimelineOrder::NewestFirst) == TimelineOrder::NewestFirst {
        sql_query = sql_query.order_by(activity::started.desc());
    } else {
        sql_query = sql_query.order_by(activity::started.asc());
    };

    query(after, before, first, last, |after, before, first, last| async move {
        debug!("Cursor query {}", debug_query::<Pg, _>(&sql_query).to_string());
        let rx = sql_query.cursor(after, before, first, last);

        let start = rx.start;
        let limit = rx.limit;

        let rx = rx.load::<(Activity, i64)>(&mut connection)?;

        Ok::<_, GraphQlError>(project_to_nodes(rx, start, limit))
    })
        .await
}

#[allow(clippy::too_many_arguments)]
pub async fn entities_by_type<'a>(
    ctx: &Context<'a>,
    typ: Option<DomaintypeId>,
    namespace: Option<ID>,
    after: Option<String>,
    before: Option<String>,
    first: Option<i32>,
    last: Option<i32>,
) -> async_graphql::Result<Connection<i32, Entity, EmptyFields, EmptyFields>> {
    use chronicle_persistence::schema::{entity, namespace::dsl as nsdsl};

    let store = ctx.data::<DatabaseContext>()?;

    let mut connection = store.connection()?;
    let ns = namespace.unwrap_or_else(|| "default".into());

    let sql_query = entity::table
        .inner_join(nsdsl::namespace)
        .filter(
            nsdsl::external_id
                .eq(&**ns)
                .and(entity::domaintype.eq(typ.as_ref().map(|x| x.external_id_part().to_owned()))),
        )
        .select(Entity::as_select())
        .order_by(entity::external_id.asc());

    query(after, before, first, last, |after, before, first, last| async move {
        debug!("Cursor query {}", debug_query::<Pg, _>(&sql_query).to_string());
        let rx = sql_query.cursor(after, before, first, last);

        let start = rx.start;
        let limit = rx.limit;

        let rx = rx.load::<(Entity, i64)>(&mut connection)?;

        Ok::<_, GraphQlError>(project_to_nodes(rx, start, limit))
    })
        .await
}

#[allow(clippy::too_many_arguments)]
pub async fn activities_by_type<'a>(
    ctx: &Context<'a>,
    typ: Option<DomaintypeId>,
    namespace: Option<ID>,
    after: Option<String>,
    before: Option<String>,
    first: Option<i32>,
    last: Option<i32>,
) -> async_graphql::Result<Connection<i32, Activity, EmptyFields, EmptyFields>> {
    use chronicle_persistence::schema::{activity, namespace::dsl as nsdsl};

    let store = ctx.data::<DatabaseContext>()?;

    let mut connection = store.connection()?;
    let ns = namespace.unwrap_or_else(|| "default".into());

    let sql_query =
        activity::table
            .inner_join(nsdsl::namespace)
            .filter(nsdsl::external_id.eq(&**ns).and(
                activity::domaintype.eq(typ.as_ref().map(|x| x.external_id_part().to_owned())),
            ))
            .select(Activity::as_select())
            .order_by(activity::external_id.asc());

    query(after, before, first, last, |after, before, first, last| async move {
        debug!("Cursor query {}", debug_query::<Pg, _>(&sql_query).to_string());
        let rx = sql_query.cursor(after, before, first, last);

        let start = rx.start;
        let limit = rx.limit;

        let rx = rx.load::<(Activity, i64)>(&mut connection)?;

        Ok::<_, GraphQlError>(project_to_nodes(rx, start, limit))
    })
        .await
}

#[allow(clippy::too_many_arguments)]
pub async fn agents_by_type<'a>(
    ctx: &Context<'a>,
    typ: Option<DomaintypeId>,
    namespace: Option<ID>,
    after: Option<String>,
    before: Option<String>,
    first: Option<i32>,
    last: Option<i32>,
) -> async_graphql::Result<Connection<i32, Agent, EmptyFields, EmptyFields>> {
    use chronicle_persistence::schema::{agent, namespace::dsl as nsdsl};

    let store = ctx.data::<DatabaseContext>()?;

    let mut connection = store.connection()?;
    let ns = namespace.unwrap_or_else(|| "default".into());

    let sql_query = agent::table
        .inner_join(nsdsl::namespace)
        .filter(
            nsdsl::external_id
                .eq(&**ns)
                .and(agent::domaintype.eq(typ.as_ref().map(|x| x.external_id_part().to_owned()))),
        )
        .select(Agent::as_select())
        .order_by(agent::external_id.asc());

    query(after, before, first, last, |after, before, first, last| async move {
        debug!("Cursor query {}", debug_query::<Pg, _>(&sql_query).to_string());
        let rx = sql_query.cursor(after, before, first, last);

        let start = rx.start;
        let limit = rx.limit;

        let rx = rx.load::<(Agent, i64)>(&mut connection)?;

        Ok::<_, GraphQlError>(project_to_nodes(rx, start, limit))
    })
        .await
}

pub async fn agent_by_id<'a>(
    ctx: &Context<'a>,
    id: AgentId,
    namespace: Option<String>,
) -> async_graphql::Result<Option<Agent>> {
    use chronicle_persistence::schema::{
        agent::{self, dsl},
        namespace::dsl as nsdsl,
    };

    let store = ctx.data::<DatabaseContext>()?;

    let ns = namespace.unwrap_or_else(|| "default".into());
    let mut connection = store.connection()?;

    Ok(agent::table
        .inner_join(nsdsl::namespace)
        .filter(dsl::external_id.eq(id.external_id_part()).and(nsdsl::external_id.eq(&ns)))
        .select(Agent::as_select())
        .first::<Agent>(&mut connection)
        .optional()?)
}

pub async fn activity_by_id<'a>(
    ctx: &Context<'a>,
    id: ActivityId,
    namespace: Option<String>,
) -> async_graphql::Result<Option<Activity>> {
    use chronicle_persistence::schema::{
        activity::{self, dsl},
        namespace::dsl as nsdsl,
    };

    let store = ctx.data::<DatabaseContext>()?;

    let ns = namespace.unwrap_or_else(|| "default".into());
    let mut connection = store.connection()?;

    Ok(activity::table
        .inner_join(nsdsl::namespace)
        .filter(dsl::external_id.eq(id.external_id_part()).and(nsdsl::external_id.eq(&ns)))
        .select(Activity::as_select())
        .first::<Activity>(&mut connection)
        .optional()?)
}

pub async fn entity_by_id<'a>(
    ctx: &Context<'a>,
    id: EntityId,
    namespace: Option<String>,
) -> async_graphql::Result<Option<Entity>> {
    use chronicle_persistence::schema::{
        entity::{self, dsl},
        namespace::dsl as nsdsl,
    };

    let store = ctx.data::<DatabaseContext>()?;
    let ns = namespace.unwrap_or_else(|| "default".into());
    let mut connection = store.connection()?;

    Ok(entity::table
        .inner_join(nsdsl::namespace)
        .filter(dsl::external_id.eq(id.external_id_part()).and(nsdsl::external_id.eq(&ns)))
        .select(Entity::as_select())
        .first::<Entity>(&mut connection)
        .optional()?)
}
