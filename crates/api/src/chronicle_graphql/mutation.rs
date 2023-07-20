//! Primitive mutation operations that are not in terms of particular domain types

use async_graphql::Context;
use chrono::{DateTime, Utc};
use common::{
    attributes::Attributes,
    commands::{ActivityCommand, AgentCommand, ApiCommand, ApiResponse, EntityCommand},
    identity::AuthId,
    prov::{operations::DerivationType, ActivityId, AgentId, EntityId, Role},
};

use crate::ApiDispatch;

use super::Submission;
async fn transaction_context<'a>(
    res: ApiResponse,
    _ctx: &Context<'a>,
) -> async_graphql::Result<Submission> {
    match res {
        ApiResponse::Submission { subject, tx_id, .. } => {
            Ok(Submission::from_submission(&subject, &tx_id))
        }
        ApiResponse::AlreadyRecorded { subject, .. } => {
            Ok(Submission::from_already_recorded(&subject))
        }
        _ => unreachable!(),
    }
}

async fn derivation<'a>(
    ctx: &Context<'a>,
    namespace: Option<String>,
    generated_entity: EntityId,
    used_entity: EntityId,
    derivation: DerivationType,
) -> async_graphql::Result<Submission> {
    let api = ctx.data_unchecked::<ApiDispatch>();

    let identity = ctx.data_unchecked::<AuthId>().to_owned();

    let namespace = namespace.unwrap_or_else(|| "default".into()).into();

    let res = api
        .dispatch(
            ApiCommand::Entity(EntityCommand::Derive {
                id: generated_entity,
                namespace,
                activity: None,
                used_entity,
                derivation,
            }),
            identity,
        )
        .await?;

    transaction_context(res, ctx).await
}

pub async fn agent<'a>(
    ctx: &Context<'a>,
    external_id: String,
    namespace: Option<String>,
    attributes: Attributes,
) -> async_graphql::Result<Submission> {
    let api = ctx.data_unchecked::<ApiDispatch>();

    let identity = ctx.data_unchecked::<AuthId>().to_owned();

    let namespace = namespace.unwrap_or_else(|| "default".to_owned());

    let res = api
        .dispatch(
            ApiCommand::Agent(AgentCommand::Create {
                external_id: external_id.into(),
                namespace: namespace.into(),
                attributes,
            }),
            identity,
        )
        .await?;

    transaction_context(res, ctx).await
}

pub async fn activity<'a>(
    ctx: &Context<'a>,
    external_id: String,
    namespace: Option<String>,
    attributes: Attributes,
) -> async_graphql::Result<Submission> {
    let api = ctx.data_unchecked::<ApiDispatch>();

    let identity = ctx.data_unchecked::<AuthId>().to_owned();

    let namespace = namespace.unwrap_or_else(|| "default".to_owned());

    let res = api
        .dispatch(
            ApiCommand::Activity(ActivityCommand::Create {
                external_id: external_id.into(),
                namespace: namespace.into(),
                attributes,
            }),
            identity,
        )
        .await?;

    transaction_context(res, ctx).await
}

pub async fn entity<'a>(
    ctx: &Context<'a>,
    external_id: String,
    namespace: Option<String>,
    attributes: Attributes,
) -> async_graphql::Result<Submission> {
    let api = ctx.data_unchecked::<ApiDispatch>();

    let identity = ctx.data_unchecked::<AuthId>().to_owned();

    let namespace = namespace.unwrap_or_else(|| "default".to_owned());

    let res = api
        .dispatch(
            ApiCommand::Entity(EntityCommand::Create {
                external_id: external_id.into(),
                namespace: namespace.into(),
                attributes,
            }),
            identity,
        )
        .await?;

    transaction_context(res, ctx).await
}

pub async fn acted_on_behalf_of<'a>(
    ctx: &Context<'a>,
    namespace: Option<String>,
    responsible_id: AgentId,
    delegate_id: AgentId,
    activity_id: Option<ActivityId>,
    role: Option<Role>,
) -> async_graphql::Result<Submission> {
    let api = ctx.data_unchecked::<ApiDispatch>();

    let identity = ctx.data_unchecked::<AuthId>().to_owned();

    let namespace = namespace.unwrap_or_else(|| "default".to_owned()).into();

    let res = api
        .dispatch(
            ApiCommand::Agent(AgentCommand::Delegate {
                id: responsible_id,
                delegate: delegate_id,
                activity: activity_id,
                namespace,
                role,
            }),
            identity,
        )
        .await?;

    transaction_context(res, ctx).await
}

pub async fn was_derived_from<'a>(
    ctx: &Context<'a>,
    namespace: Option<String>,
    generated_entity: EntityId,
    used_entity: EntityId,
) -> async_graphql::Result<Submission> {
    derivation(
        ctx,
        namespace,
        generated_entity,
        used_entity,
        DerivationType::None,
    )
    .await
}

pub async fn was_revision_of<'a>(
    ctx: &Context<'a>,
    namespace: Option<String>,
    generated_entity: EntityId,
    used_entity: EntityId,
) -> async_graphql::Result<Submission> {
    derivation(
        ctx,
        namespace,
        generated_entity,
        used_entity,
        DerivationType::Revision,
    )
    .await
}
pub async fn had_primary_source<'a>(
    ctx: &Context<'a>,
    namespace: Option<String>,
    generated_entity: EntityId,
    used_entity: EntityId,
) -> async_graphql::Result<Submission> {
    derivation(
        ctx,
        namespace,
        generated_entity,
        used_entity,
        DerivationType::PrimarySource,
    )
    .await
}
pub async fn was_quoted_from<'a>(
    ctx: &Context<'a>,
    namespace: Option<String>,
    generated_entity: EntityId,
    used_entity: EntityId,
) -> async_graphql::Result<Submission> {
    derivation(
        ctx,
        namespace,
        generated_entity,
        used_entity,
        DerivationType::Quotation,
    )
    .await
}

pub async fn start_activity<'a>(
    ctx: &Context<'a>,
    id: ActivityId,
    namespace: Option<String>,
    agent: Option<AgentId>, // deprecated, slated for removal in CHRON-185
    time: Option<DateTime<Utc>>,
) -> async_graphql::Result<Submission> {
    let api = ctx.data_unchecked::<ApiDispatch>();

    let identity = ctx.data_unchecked::<AuthId>().to_owned();

    let namespace = namespace.unwrap_or_else(|| "default".to_owned()).into();

    let res = api
        .dispatch(
            ApiCommand::Activity(ActivityCommand::Start {
                id,
                namespace,
                time,
                agent,
            }),
            identity,
        )
        .await?;

    transaction_context(res, ctx).await
}

pub async fn end_activity<'a>(
    ctx: &Context<'a>,
    id: ActivityId,
    namespace: Option<String>,
    agent: Option<AgentId>, // deprecated, slated for removal in CHRON-185
    time: Option<DateTime<Utc>>,
) -> async_graphql::Result<Submission> {
    let api = ctx.data_unchecked::<ApiDispatch>();

    let identity = ctx.data_unchecked::<AuthId>().to_owned();

    let namespace = namespace.unwrap_or_else(|| "default".to_owned()).into();

    let res = api
        .dispatch(
            ApiCommand::Activity(ActivityCommand::End {
                id,
                namespace,
                time,
                agent,
            }),
            identity,
        )
        .await?;

    transaction_context(res, ctx).await
}

pub async fn instant_activity<'a>(
    ctx: &Context<'a>,
    id: ActivityId,
    namespace: Option<String>,
    agent: Option<AgentId>, // deprecated, slated for removal in CHRON-185
    time: Option<DateTime<Utc>>,
) -> async_graphql::Result<Submission> {
    let api = ctx.data_unchecked::<ApiDispatch>();

    let identity = ctx.data_unchecked::<AuthId>().to_owned();

    let namespace = namespace.unwrap_or_else(|| "default".to_owned()).into();

    let res = api
        .dispatch(
            ApiCommand::Activity(ActivityCommand::Instant {
                id,
                namespace,
                time,
                agent,
            }),
            identity,
        )
        .await?;

    transaction_context(res, ctx).await
}

pub async fn was_associated_with<'a>(
    ctx: &Context<'a>,
    namespace: Option<String>,
    responsible: AgentId,
    activity: ActivityId,
    role: Option<Role>,
) -> async_graphql::Result<Submission> {
    let api = ctx.data_unchecked::<ApiDispatch>();

    let identity = ctx.data_unchecked::<AuthId>().to_owned();

    let namespace = namespace.unwrap_or_else(|| "default".to_owned()).into();

    let res = api
        .dispatch(
            ApiCommand::Activity(ActivityCommand::Associate {
                id: activity,
                responsible,
                role,
                namespace,
            }),
            identity,
        )
        .await?;

    transaction_context(res, ctx).await
}

pub async fn was_attributed_to<'a>(
    ctx: &Context<'a>,
    namespace: Option<String>,
    responsible: AgentId,
    id: EntityId,
    role: Option<Role>,
) -> async_graphql::Result<Submission> {
    let api = ctx.data_unchecked::<ApiDispatch>();

    let identity = ctx.data_unchecked::<AuthId>().to_owned();

    let namespace = namespace.unwrap_or_else(|| "default".to_owned()).into();

    let res = api
        .dispatch(
            ApiCommand::Entity(EntityCommand::Attribute {
                id,
                namespace,
                responsible,
                role,
            }),
            identity,
        )
        .await?;

    transaction_context(res, ctx).await
}

pub async fn used<'a>(
    ctx: &Context<'a>,
    activity: ActivityId,
    entity: EntityId,
    namespace: Option<String>,
) -> async_graphql::Result<Submission> {
    let api = ctx.data_unchecked::<ApiDispatch>();

    let identity = ctx.data_unchecked::<AuthId>().to_owned();

    let namespace = namespace.unwrap_or_else(|| "default".to_owned()).into();

    let res = api
        .dispatch(
            ApiCommand::Activity(ActivityCommand::Use {
                id: entity,
                namespace,
                activity,
            }),
            identity,
        )
        .await?;

    transaction_context(res, ctx).await
}

pub async fn was_informed_by<'a>(
    ctx: &Context<'a>,
    activity: ActivityId,
    informing_activity: ActivityId,
    namespace: Option<String>,
) -> async_graphql::Result<Submission> {
    let api = ctx.data_unchecked::<ApiDispatch>();

    let identity = ctx.data_unchecked::<AuthId>().to_owned();

    let namespace = namespace.unwrap_or_else(|| "default".to_owned()).into();

    let res = api
        .dispatch(
            ApiCommand::Activity(ActivityCommand::WasInformedBy {
                id: activity,
                namespace,
                informing_activity,
            }),
            identity,
        )
        .await?;

    transaction_context(res, ctx).await
}

pub async fn was_generated_by<'a>(
    ctx: &Context<'a>,
    activity: ActivityId,
    entity: EntityId,
    namespace: Option<String>,
) -> async_graphql::Result<Submission> {
    let api = ctx.data_unchecked::<ApiDispatch>();

    let identity = ctx.data_unchecked::<AuthId>().to_owned();

    let namespace = namespace.unwrap_or_else(|| "default".to_owned()).into();

    let res = api
        .dispatch(
            ApiCommand::Activity(ActivityCommand::Generate {
                id: entity,
                namespace,
                activity,
            }),
            identity,
        )
        .await?;

    transaction_context(res, ctx).await
}
