use async_graphql::{
    connection::{Connection, EmptyFields},
    Context, ID,
};

use chrono::{DateTime, Utc};
use common::prov::{AgentId, DomaintypeId, EntityId, NamePart};
use diesel::prelude::*;

use crate::chronicle_graphql::Store;

use super::{Agent, Entity};

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
                .eq(&**ns)
                .and(agent::domaintype.eq(typ.map(|x| x.name_part().to_owned())))
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
