//! Primative mutation operations that are not in terms of particular domain types

use std::sync::Arc;

use async_graphql::{Context, Upload, ID};
use chrono::{DateTime, Utc};
use common::{
    attributes::Attributes,
    commands::{
        ActivityCommand, AgentCommand, ApiCommand, ApiResponse, EntityCommand, KeyRegistration,
        PathOrFile,
    },
    prov::{operations::DerivationType, AgentId, EntityId},
};

use crate::ApiDispatch;

use super::Submission;
pub async fn transaction_context<'a>(
    res: ApiResponse,
    _ctx: &Context<'a>,
) -> async_graphql::Result<Submission> {
    match res {
        ApiResponse::Submission {
            subject,
            correlation_id,
            ..
        } => Ok(Submission {
            context: subject.to_string(),
            correlation_id: correlation_id.to_string(),
        }),
        _ => unreachable!(),
    }
}

async fn derivation<'a>(
    ctx: &Context<'a>,
    namespace: Option<String>,
    generated_entity: ID,
    used_entity: ID,
    derivation: Option<DerivationType>,
) -> async_graphql::Result<Submission> {
    let api = ctx.data_unchecked::<ApiDispatch>();

    let namespace = namespace.unwrap_or_else(|| "default".into());

    let used_entity = EntityId::new(&**used_entity);
    let generated_entity = EntityId::new(&**generated_entity);

    let res = api
        .dispatch(ApiCommand::Entity(EntityCommand::Derive {
            name: generated_entity.decompose().to_string(),
            namespace,
            activity: None,
            used_entity: used_entity.decompose().to_string(),
            derivation,
        }))
        .await?;

    transaction_context(res, ctx).await
}

pub async fn agent<'a>(
    ctx: &Context<'a>,
    name: String,
    namespace: Option<String>,
    attributes: Attributes,
) -> async_graphql::Result<Submission> {
    let api = ctx.data_unchecked::<ApiDispatch>();

    let namespace = namespace.unwrap_or_else(|| "default".to_owned());

    let res = api
        .dispatch(ApiCommand::Agent(AgentCommand::Create {
            name,
            namespace: namespace.clone(),
            attributes,
        }))
        .await?;

    transaction_context(res, ctx).await
}

pub async fn activity<'a>(
    ctx: &Context<'a>,
    name: String,
    namespace: Option<String>,
    attributes: Attributes,
) -> async_graphql::Result<Submission> {
    let api = ctx.data_unchecked::<ApiDispatch>();

    let namespace = namespace.unwrap_or_else(|| "default".to_owned());

    let res = api
        .dispatch(ApiCommand::Activity(ActivityCommand::Create {
            name,
            namespace: namespace.clone(),
            attributes,
        }))
        .await?;

    transaction_context(res, ctx).await
}

pub async fn entity<'a>(
    ctx: &Context<'a>,
    name: String,
    namespace: Option<String>,
    attributes: Attributes,
) -> async_graphql::Result<Submission> {
    let api = ctx.data_unchecked::<ApiDispatch>();

    let namespace = namespace.unwrap_or_else(|| "default".to_owned());

    let res = api
        .dispatch(ApiCommand::Entity(EntityCommand::Create {
            name,
            namespace: namespace.clone(),
            attributes,
        }))
        .await?;

    transaction_context(res, ctx).await
}
pub async fn acted_on_behalf_of<'a>(
    ctx: &Context<'a>,
    namespace: Option<String>,
    responsible: ID,
    delegate: ID,
) -> async_graphql::Result<Submission> {
    let api = ctx.data_unchecked::<ApiDispatch>();

    let namespace = namespace.unwrap_or_else(|| "default".to_owned());

    let responsible_id = AgentId::new(&**responsible);
    let delegate_id = AgentId::new(&**delegate);

    let res = api
        .dispatch(ApiCommand::Agent(AgentCommand::Delegate {
            name: responsible_id.decompose().to_string(),
            delegate: delegate_id.decompose().to_string(),
            activity: None,
            namespace: namespace.clone(),
        }))
        .await?;

    transaction_context(res, ctx).await
}

pub async fn was_derived_from<'a>(
    ctx: &Context<'a>,
    namespace: Option<String>,
    generated_entity: ID,
    used_entity: ID,
) -> async_graphql::Result<Submission> {
    derivation(ctx, namespace, generated_entity, used_entity, None).await
}

pub async fn was_revision_of<'a>(
    ctx: &Context<'a>,
    namespace: Option<String>,
    generated_entity: ID,
    used_entity: ID,
) -> async_graphql::Result<Submission> {
    derivation(
        ctx,
        namespace,
        generated_entity,
        used_entity,
        Some(DerivationType::Revision),
    )
    .await
}
pub async fn had_primary_source<'a>(
    ctx: &Context<'a>,
    namespace: Option<String>,
    generated_entity: ID,
    used_entity: ID,
) -> async_graphql::Result<Submission> {
    derivation(
        ctx,
        namespace,
        generated_entity,
        used_entity,
        Some(DerivationType::PrimarySource),
    )
    .await
}
pub async fn was_quoted_from<'a>(
    ctx: &Context<'a>,
    namespace: Option<String>,
    generated_entity: ID,
    used_entity: ID,
) -> async_graphql::Result<Submission> {
    derivation(
        ctx,
        namespace,
        generated_entity,
        used_entity,
        Some(DerivationType::Quotation),
    )
    .await
}

pub async fn generate_key<'a>(
    ctx: &Context<'a>,
    name: String,
    namespace: Option<String>,
) -> async_graphql::Result<Submission> {
    let api = ctx.data_unchecked::<ApiDispatch>();

    let namespace = namespace.unwrap_or_else(|| "default".to_owned());

    let res = api
        .dispatch(ApiCommand::Agent(AgentCommand::RegisterKey {
            name,
            namespace: namespace.clone(),
            registration: KeyRegistration::Generate,
        }))
        .await?;

    transaction_context(res, ctx).await
}

pub async fn start_activity<'a>(
    ctx: &Context<'a>,
    name: String,
    namespace: Option<String>,
    agent: String,
    time: Option<DateTime<Utc>>,
) -> async_graphql::Result<Submission> {
    let api = ctx.data_unchecked::<ApiDispatch>();

    let namespace = namespace.unwrap_or_else(|| "default".to_owned());

    let res = api
        .dispatch(ApiCommand::Activity(ActivityCommand::Start {
            name,
            namespace: namespace.clone(),
            time,
            agent: Some(agent),
        }))
        .await?;

    transaction_context(res, ctx).await
}

pub async fn end_activity<'a>(
    ctx: &Context<'a>,
    name: String,
    namespace: Option<String>,
    agent: String,
    time: Option<DateTime<Utc>>,
) -> async_graphql::Result<Submission> {
    let api = ctx.data_unchecked::<ApiDispatch>();

    let namespace = namespace.unwrap_or_else(|| "default".to_owned());

    let res = api
        .dispatch(ApiCommand::Activity(ActivityCommand::End {
            name: Some(name),
            namespace: namespace.clone(),
            time,
            agent: Some(agent),
        }))
        .await?;

    transaction_context(res, ctx).await
}

pub async fn used<'a>(
    ctx: &Context<'a>,
    activity: String,
    name: String,
    namespace: Option<String>,
) -> async_graphql::Result<Submission> {
    let api = ctx.data_unchecked::<ApiDispatch>();

    let namespace = namespace.unwrap_or_else(|| "default".to_owned());

    let res = api
        .dispatch(ApiCommand::Activity(ActivityCommand::Use {
            name,
            namespace: namespace.clone(),
            activity: Some(activity),
        }))
        .await?;

    transaction_context(res, ctx).await
}

pub async fn was_generated_by<'a>(
    ctx: &Context<'a>,
    activity: String,
    name: String,
    namespace: Option<String>,
) -> async_graphql::Result<Submission> {
    let api = ctx.data_unchecked::<ApiDispatch>();

    let namespace = namespace.unwrap_or_else(|| "default".to_owned());

    let res = api
        .dispatch(ApiCommand::Activity(ActivityCommand::Generate {
            name,
            namespace: namespace.clone(),
            activity: Some(activity),
        }))
        .await?;

    transaction_context(res, ctx).await
}

pub async fn has_attachment<'a>(
    ctx: &Context<'a>,
    name: String,
    namespace: Option<String>,
    attachment: Upload,
    on_behalf_of_agent: String,
    locator: String,
) -> async_graphql::Result<Submission> {
    let api = ctx.data_unchecked::<ApiDispatch>();

    let namespace = namespace.unwrap_or_else(|| "default".to_owned());

    let res = api
        .dispatch(ApiCommand::Entity(EntityCommand::Attach {
            name,
            namespace: namespace.clone(),
            agent: Some(on_behalf_of_agent),
            file: PathOrFile::File(Arc::new(Box::pin(attachment.value(ctx)?.into_async_read()))),
            locator: Some(locator),
        }))
        .await?;

    transaction_context(res, ctx).await
}
