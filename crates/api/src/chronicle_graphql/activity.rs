use std::collections::HashMap;

use async_graphql::Context;
use diesel::prelude::*;

use chronicle_persistence::queryable::{Activity, Agent, Entity, Namespace};
use common::prov::Role;

use crate::chronicle_graphql::DatabaseContext;

pub async fn namespace<'a>(
    namespaceid: i32,
    ctx: &Context<'a>,
) -> async_graphql::Result<Namespace> {
    use chronicle_persistence::schema::namespace::{self, dsl};
    let store = ctx.data::<DatabaseContext>()?;

    let mut connection = store.connection()?;

    Ok(namespace::table
        .filter(dsl::id.eq(namespaceid))
        .first::<Namespace>(&mut connection)?)
}

pub async fn was_associated_with<'a>(
    id: i32,
    ctx: &Context<'a>,
) -> async_graphql::Result<Vec<(Agent, Option<Role>, Option<Agent>, Option<Role>)>> {
    use chronicle_persistence::schema::{agent, association, delegation};

    #[derive(Queryable)]
    struct DelegationAgents {
        responsible_id: i32,
        delegate: Agent,
        role: String,
    }

    let store = ctx.data::<DatabaseContext>()?;
    let mut connection = store.connection()?;

    let delegation_entries = delegation::table
        .filter(delegation::dsl::activity_id.eq(id))
        .inner_join(agent::table.on(agent::id.eq(delegation::delegate_id)))
        .select((delegation::responsible_id, Agent::as_select(), delegation::role))
        .load::<DelegationAgents>(&mut connection)?
        .into_iter();

    let mut agent_reservoir = HashMap::new();
    let mut agent_delegations = HashMap::new();

    for delegation_entry in delegation_entries {
        let delegate_id = delegation_entry.delegate.id;
        agent_reservoir.insert(delegate_id, delegation_entry.delegate);
        agent_delegations.insert(
            delegation_entry.responsible_id,
            (
                delegate_id,
                if delegation_entry.role.is_empty() {
                    None
                } else {
                    Some(Role(delegation_entry.role))
                },
            ),
        );
    }

    let res = association::table
        .filter(association::dsl::activity_id.eq(id))
        .inner_join(chronicle_persistence::schema::agent::table)
        .order(chronicle_persistence::schema::agent::external_id)
        .select((Agent::as_select(), association::role))
        .load::<(Agent, Role)>(&mut connection)?
        .into_iter()
        .map(|(responsible_agent, responsible_role)| {
            let responsible_role =
                if responsible_role.0.is_empty() { None } else { Some(responsible_role) };
            let (delegate_agent, delegate_role): (Option<Agent>, Option<Role>) =
                match agent_delegations.get(&responsible_agent.id) {
                    Some((delegate_id, optional_role)) => {
                        let delegate = agent_reservoir.remove(delegate_id).unwrap_or_else(|| {
                            agent::table.find(delegate_id).first::<Agent>(&mut connection).unwrap()
                        });
                        let optional_role = optional_role.as_ref().cloned();
                        (Some(delegate), optional_role)
                    }
                    None => (None, None),
                };
            (responsible_agent, responsible_role, delegate_agent, delegate_role)
        })
        .collect();

    Ok(res)
}

pub async fn used<'a>(id: i32, ctx: &Context<'a>) -> async_graphql::Result<Vec<Entity>> {
    use chronicle_persistence::schema::usage::{self, dsl};

    let store = ctx.data::<DatabaseContext>()?;

    let mut connection = store.connection()?;

    let res = usage::table
        .filter(dsl::activity_id.eq(id))
        .inner_join(chronicle_persistence::schema::entity::table)
        .order(chronicle_persistence::schema::entity::external_id)
        .select(Entity::as_select())
        .load::<Entity>(&mut connection)?;

    Ok(res)
}

pub async fn was_informed_by<'a>(
    id: i32,
    ctx: &Context<'a>,
) -> async_graphql::Result<Vec<Activity>> {
    use chronicle_persistence::schema::wasinformedby::{self, dsl};

    let store = ctx.data::<DatabaseContext>()?;

    let mut connection = store.connection()?;

    let res = wasinformedby::table
        .filter(dsl::activity_id.eq(id))
        .inner_join(chronicle_persistence::schema::activity::table.on(
            wasinformedby::informing_activity_id.eq(chronicle_persistence::schema::activity::id),
        ))
        .order(chronicle_persistence::schema::activity::external_id)
        .select(Activity::as_select())
        .load::<Activity>(&mut connection)?;

    Ok(res)
}

pub async fn generated<'a>(id: i32, ctx: &Context<'a>) -> async_graphql::Result<Vec<Entity>> {
    use chronicle_persistence::schema::generation::{self, dsl};

    let store = ctx.data::<DatabaseContext>()?;

    let mut connection = store.connection()?;

    let res = generation::table
        .filter(dsl::activity_id.eq(id))
        .inner_join(chronicle_persistence::schema::entity::table)
        .select(Entity::as_select())
        .load::<Entity>(&mut connection)?;

    Ok(res)
}

pub async fn load_attribute<'a>(
    id: i32,
    external_id: &str,
    ctx: &Context<'a>,
) -> async_graphql::Result<Option<serde_json::Value>> {
    use chronicle_persistence::schema::activity_attribute;

    let store = ctx.data::<DatabaseContext>()?;

    let mut connection = store.connection()?;

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
