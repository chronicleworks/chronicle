use crate::chronicle_graphql::{Agent, Store};
use async_graphql::Context;
use diesel::prelude::*;

use super::{Entity, Namespace};

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
) -> async_graphql::Result<Vec<Agent>> {
    use crate::persistence::schema::association::{self, dsl};

    let store = ctx.data_unchecked::<Store>();

    let mut connection = store.pool.get()?;

    let res = association::table
        .filter(dsl::activity_id.eq(id))
        .order(dsl::offset)
        .inner_join(crate::persistence::schema::agent::table)
        .select(Agent::as_select())
        .load::<Agent>(&mut connection)?;

    Ok(res)
}

pub async fn used<'a>(id: i32, ctx: &Context<'a>) -> async_graphql::Result<Vec<Entity>> {
    use crate::persistence::schema::useage::{self, dsl};

    let store = ctx.data_unchecked::<Store>();

    let mut connection = store.pool.get()?;

    let res = useage::table
        .filter(dsl::activity_id.eq(id))
        .order(dsl::offset)
        .inner_join(crate::persistence::schema::entity::table)
        .select(Entity::as_select())
        .load::<Entity>(&mut connection)?;

    Ok(res)
}
