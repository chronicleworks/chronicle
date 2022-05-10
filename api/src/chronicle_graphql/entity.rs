use crate::chronicle_graphql::{Activity, Entity, Evidence, Store};
use async_graphql::Context;
use common::prov::operations::DerivationType;
use diesel::prelude::*;

use super::Namespace;

pub async fn typed_derivation<'a>(
    id: i32,
    ctx: &Context<'a>,
    typ: Option<DerivationType>,
) -> async_graphql::Result<Vec<Entity>> {
    use crate::persistence::schema::derivation::{self, dsl};
    use crate::persistence::schema::entity::{self as entitydsl};

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

pub async fn evidence<'a>(
    attachment_id: Option<i32>,
    ctx: &Context<'a>,
) -> async_graphql::Result<Option<Evidence>> {
    use crate::persistence::schema::attachment::{self, dsl};
    let store = ctx.data_unchecked::<Store>();

    let mut connection = store.pool.get()?;

    if let Some(attachment_id) = attachment_id {
        Ok(attachment::table
            .filter(dsl::id.eq(attachment_id))
            .first::<Evidence>(&mut connection)
            .optional()?)
    } else {
        Ok(None)
    }
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
    use crate::persistence::schema::derivation::{self, dsl};
    use crate::persistence::schema::entity::{self as entitydsl};

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
    typed_derivation(id, ctx, Some(DerivationType::PrimarySource)).await
}

pub async fn was_revision_of<'a>(id: i32, ctx: &Context<'a>) -> async_graphql::Result<Vec<Entity>> {
    typed_derivation(id, ctx, Some(DerivationType::Revision)).await
}
pub async fn was_quoted_from<'a>(id: i32, ctx: &Context<'a>) -> async_graphql::Result<Vec<Entity>> {
    typed_derivation(id, ctx, Some(DerivationType::Quotation)).await
}
