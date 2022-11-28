use super::{Agent, Identity, Namespace, Store};
use async_graphql::Context;
use common::prov::Role;
use diesel::prelude::*;

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
) -> async_graphql::Result<Vec<(Agent, Option<Role>)>> {
    use crate::persistence::schema::{
        agent as agentdsl,
        delegation::{self, dsl},
    };

    let store = ctx.data_unchecked::<Store>();

    let mut connection = store.pool.get()?;

    Ok(delegation::table
        .filter(dsl::responsible_id.eq(id))
        .inner_join(agentdsl::table.on(dsl::delegate_id.eq(agentdsl::id)))
        .order(agentdsl::external_id)
        .select((Agent::as_select(), dsl::role))
        .load::<(Agent, Role)>(&mut connection)?
        .into_iter()
        .map(|(a, r)| (a, if r.0.is_empty() { None } else { Some(r) }))
        .collect())
}

pub async fn load_attribute<'a>(
    id: i32,
    external_id: &str,
    ctx: &Context<'a>,
) -> async_graphql::Result<Option<serde_json::Value>> {
    use crate::persistence::schema::agent_attribute;

    let store = ctx.data_unchecked::<Store>();

    let mut connection = store.pool.get()?;

    Ok(agent_attribute::table
        .filter(
            agent_attribute::agent_id
                .eq(id)
                .and(agent_attribute::typename.eq(external_id)),
        )
        .select(agent_attribute::value)
        .first::<String>(&mut connection)
        .optional()?
        .as_deref()
        .map(serde_json::from_str)
        .transpose()?)
}
