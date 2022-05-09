use async_graphql::Context;
use diesel::prelude::*;

use crate::graphql::{Identity, Namespace, Store};

use super::Agent;

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

pub async fn identity<'a>(
    identity_id: Option<i32>,
    ctx: &Context<'a>,
) -> async_graphql::Result<Option<Identity>> {
    use crate::persistence::schema::identity::{self, dsl};
    let store = ctx.data_unchecked::<Store>();

    let mut connection = store.pool.get()?;

    if let Some(identity_id) = identity_id {
        Ok(identity::table
            .filter(dsl::id.eq(identity_id))
            .first::<Identity>(&mut connection)
            .optional()?)
    } else {
        Ok(None)
    }
}

pub async fn acted_on_behalf_of<'a>(
    id: i32,
    ctx: &Context<'a>,
) -> async_graphql::Result<Vec<Agent>> {
    use crate::persistence::schema::agent as agentdsl;
    use crate::persistence::schema::delegation::{self, dsl};

    let store = ctx.data_unchecked::<Store>();

    let mut connection = store.pool.get()?;

    Ok(delegation::table
        .filter(dsl::responsible_id.eq(id))
        .order(dsl::offset)
        .inner_join(agentdsl::table.on(dsl::delegate_id.eq(agentdsl::id)))
        .select(Agent::as_select())
        .load::<Agent>(&mut connection)?)
}
