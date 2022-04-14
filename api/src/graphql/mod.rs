use async_graphql::{
    connection::{query, Connection, EmptyFields},
    extensions::OpenTelemetry,
    Context, Error, ErrorExtensions, Object, Schema, Subscription, Upload, ID,
};
use async_graphql_warp::{graphql_subscription, GraphQLBadRequest};
use chrono::{DateTime, NaiveDateTime, Utc};
use common::{
    commands::{
        ActivityCommand, AgentCommand, ApiCommand, ApiResponse, EntityCommand, KeyRegistration,
        PathOrFile,
    },
    prov::{vocab::Chronicle, AgentId, DerivationType, EntityId},
};
use custom_error::custom_error;
use derivative::*;
use diesel::{
    prelude::*,
    r2d2::{ConnectionManager, Pool},
    Queryable, SqliteConnection,
};
use futures::Stream;
use std::{convert::Infallible, net::SocketAddr, sync::Arc, time::Duration};
use tokio::sync::broadcast::error::RecvError;
use tracing::instrument;
use warp::{hyper::StatusCode, Filter, Rejection};

use crate::ApiDispatch;
#[macro_use]
mod cursor_query;

#[derive(Default, Queryable, Selectable)]
#[diesel(table_name = crate::persistence::schema::agent)]
pub struct Agent {
    pub id: i32,
    pub name: String,
    pub namespace_id: i32,
    pub domaintype: Option<String>,
    pub current: i32,
    pub identity_id: Option<i32>,
}

#[derive(Default, Queryable, Selectable)]
#[diesel(table_name = crate::persistence::schema::identity)]
pub struct Identity {
    pub id: i32,
    pub namespace_id: i32,
    pub public_key: String,
}

#[Object]
impl Identity {
    async fn public_key(&self) -> &str {
        &self.public_key
    }
}

#[derive(Default, Queryable, Selectable)]
#[diesel(table_name = crate::persistence::schema::activity)]
pub struct Activity {
    pub id: i32,
    pub name: String,
    pub namespace_id: i32,
    pub domaintype: Option<String>,
    pub started: Option<NaiveDateTime>,
    pub ended: Option<NaiveDateTime>,
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = crate::persistence::schema::entity)]
pub struct Entity {
    id: i32,
    name: String,
    namespace_id: i32,
    domaintype: Option<String>,
    attachment_id: Option<i32>,
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = crate::persistence::schema::attachment)]
#[allow(dead_code)]
pub struct Attachment {
    id: i32,
    namespace_id: i32,
    signature_time: NaiveDateTime,
    signature: String,
    signer_id: i32,
    locator: Option<String>,
}

#[Object]
impl Attachment {
    async fn signature_time(&self) -> DateTime<Utc> {
        DateTime::from_utc(self.signature_time, Utc)
    }

    async fn signature(&self) -> &str {
        &self.signature
    }

    async fn locator(&self) -> Option<&str> {
        self.locator.as_deref()
    }
}

#[derive(Default, Queryable)]
pub struct Namespace {
    _id: i32,
    uuid: String,
    name: String,
}

#[Object]
impl Namespace {
    async fn name(&self) -> &str {
        &self.name
    }

    async fn uuid(&self) -> &str {
        &self.uuid
    }
}

#[derive(Default, Queryable)]
pub struct Submission {
    context: String,
    correlation_id: String,
}

#[Object]
impl Submission {
    async fn context(&self) -> &str {
        &self.context
    }

    async fn correlation_id(&self) -> &str {
        &self.correlation_id
    }
}

#[Object]
impl Agent {
    async fn id(&self) -> ID {
        ID::from(Chronicle::agent(&*self.name).to_string())
    }

    async fn namespace<'a>(&self, ctx: &Context<'a>) -> async_graphql::Result<Namespace> {
        use crate::persistence::schema::namespace::{self, dsl};
        let store = ctx.data_unchecked::<Store>();

        let mut connection = store.pool.get()?;

        Ok(namespace::table
            .filter(dsl::id.eq(self.namespace_id))
            .first::<Namespace>(&mut connection)?)
    }

    async fn name(&self) -> &str {
        &self.name
    }

    async fn identity<'a>(&self, ctx: &Context<'a>) -> async_graphql::Result<Option<Identity>> {
        use crate::persistence::schema::identity::{self, dsl};
        let store = ctx.data_unchecked::<Store>();

        let mut connection = store.pool.get()?;

        if let Some(identity_id) = self.identity_id {
            Ok(identity::table
                .filter(dsl::id.eq(identity_id))
                .first::<Identity>(&mut connection)
                .optional()?)
        } else {
            Ok(None)
        }
    }

    async fn acted_on_behalf_of<'a>(&self, ctx: &Context<'a>) -> async_graphql::Result<Vec<Agent>> {
        use crate::persistence::schema::agent as agentdsl;
        use crate::persistence::schema::delegation::{self, dsl};

        let store = ctx.data_unchecked::<Store>();

        let mut connection = store.pool.get()?;

        Ok(delegation::table
            .filter(dsl::responsible_id.eq(self.id))
            .order(dsl::offset)
            .inner_join(agentdsl::table.on(dsl::delegate_id.eq(agentdsl::id)))
            .select(Agent::as_select())
            .load::<Agent>(&mut connection)?)
    }

    #[graphql(name = "type")]
    async fn typ(&self) -> &str {
        if let Some(ref typ) = self.domaintype {
            typ
        } else {
            "agent"
        }
    }
}

#[Object]
impl Activity {
    async fn namespace<'a>(&self, ctx: &Context<'a>) -> async_graphql::Result<Namespace> {
        use crate::persistence::schema::namespace::{self, dsl};
        let store = ctx.data_unchecked::<Store>();

        let mut connection = store.pool.get()?;

        Ok(namespace::table
            .filter(dsl::id.eq(self.namespace_id))
            .first::<Namespace>(&mut connection)?)
    }

    async fn name(&self) -> &str {
        &self.name
    }

    async fn started(&self) -> Option<DateTime<Utc>> {
        self.started.map(|x| DateTime::from_utc(x, Utc))
    }

    async fn ended(&self) -> Option<DateTime<Utc>> {
        self.ended.map(|x| DateTime::from_utc(x, Utc))
    }

    #[graphql(name = "type")]
    async fn typ(&self) -> &str {
        if let Some(ref typ) = self.domaintype {
            typ
        } else {
            "activity"
        }
    }

    async fn was_associated_with<'a>(
        &self,
        ctx: &Context<'a>,
    ) -> async_graphql::Result<Vec<Agent>> {
        use crate::persistence::schema::association::{self, dsl};

        let store = ctx.data_unchecked::<Store>();

        let mut connection = store.pool.get()?;

        let res = association::table
            .filter(dsl::activity_id.eq(self.id))
            .order(dsl::offset)
            .inner_join(crate::persistence::schema::agent::table)
            .select(Agent::as_select())
            .load::<Agent>(&mut connection)?;

        Ok(res)
    }

    async fn used<'a>(&self, ctx: &Context<'a>) -> async_graphql::Result<Vec<Entity>> {
        use crate::persistence::schema::useage::{self, dsl};

        let store = ctx.data_unchecked::<Store>();

        let mut connection = store.pool.get()?;

        let res = useage::table
            .filter(dsl::activity_id.eq(self.id))
            .order(dsl::offset)
            .inner_join(crate::persistence::schema::entity::table)
            .select(Entity::as_select())
            .load::<Entity>(&mut connection)?;

        Ok(res)
    }
}

impl Entity {
    async fn typed_derivation<'a>(
        &self,
        ctx: &Context<'a>,
        typ: Option<DerivationType>,
    ) -> async_graphql::Result<Vec<Entity>> {
        use crate::persistence::schema::derivation::{self, dsl};
        use crate::persistence::schema::entity::{self as entitydsl};

        let store = ctx.data_unchecked::<Store>();

        let mut connection = store.pool.get()?;

        let res = derivation::table
            .filter(dsl::generated_entity_id.eq(self.id).and(dsl::typ.eq(typ)))
            .inner_join(entitydsl::table.on(dsl::used_entity_id.eq(entitydsl::id)))
            .select(Entity::as_select())
            .load::<Entity>(&mut connection)?;

        Ok(res)
    }
}

#[Object]
impl Entity {
    async fn id(&self) -> ID {
        ID::from(Chronicle::entity(&*self.name).to_string())
    }
    async fn namespace<'a>(&self, ctx: &Context<'a>) -> async_graphql::Result<Namespace> {
        use crate::persistence::schema::namespace::{self, dsl};
        let store = ctx.data_unchecked::<Store>();

        let mut connection = store.pool.get()?;

        Ok(namespace::table
            .filter(dsl::id.eq(self.namespace_id))
            .first::<Namespace>(&mut connection)?)
    }

    async fn name(&self) -> &str {
        &self.name
    }

    #[graphql(name = "type")]
    async fn typ(&self) -> &str {
        if let Some(ref typ) = self.domaintype {
            typ
        } else {
            "entity"
        }
    }

    async fn attachment<'a>(&self, ctx: &Context<'a>) -> async_graphql::Result<Option<Attachment>> {
        use crate::persistence::schema::attachment::{self, dsl};
        let store = ctx.data_unchecked::<Store>();

        let mut connection = store.pool.get()?;

        if let Some(attachment_id) = self.attachment_id {
            Ok(attachment::table
                .filter(dsl::id.eq(attachment_id))
                .first::<Attachment>(&mut connection)
                .optional()?)
        } else {
            Ok(None)
        }
    }
    async fn was_generated_by<'a>(
        &self,
        ctx: &Context<'a>,
    ) -> async_graphql::Result<Vec<Activity>> {
        use crate::persistence::schema::generation::{self, dsl};

        let store = ctx.data_unchecked::<Store>();

        let mut connection = store.pool.get()?;

        let res = generation::table
            .filter(dsl::generated_entity_id.eq(self.id))
            .inner_join(crate::persistence::schema::activity::table)
            .select(Activity::as_select())
            .load::<Activity>(&mut connection)?;

        Ok(res)
    }

    async fn was_derived_from<'a>(&self, ctx: &Context<'a>) -> async_graphql::Result<Vec<Entity>> {
        use crate::persistence::schema::derivation::{self, dsl};
        use crate::persistence::schema::entity::{self as entitydsl};

        let store = ctx.data_unchecked::<Store>();

        let mut connection = store.pool.get()?;

        let res = derivation::table
            .filter(dsl::generated_entity_id.eq(self.id))
            .inner_join(entitydsl::table.on(dsl::used_entity_id.eq(entitydsl::id)))
            .select(Entity::as_select())
            .load::<Entity>(&mut connection)?;

        Ok(res)
    }

    async fn had_primary_source<'a>(
        &self,
        ctx: &Context<'a>,
    ) -> async_graphql::Result<Vec<Entity>> {
        self.typed_derivation(ctx, Some(DerivationType::PrimarySource))
            .await
    }

    async fn was_revision_of<'a>(&self, ctx: &Context<'a>) -> async_graphql::Result<Vec<Entity>> {
        self.typed_derivation(ctx, Some(DerivationType::Revision))
            .await
    }
    async fn was_quoted_from<'a>(&self, ctx: &Context<'a>) -> async_graphql::Result<Vec<Entity>> {
        self.typed_derivation(ctx, Some(DerivationType::Quotation))
            .await
    }
}

custom_error! {pub GraphQlError
    Db{source: diesel::result::Error}                           = "Database operation failed",
    R2d2{source: r2d2::Error }                                  = "Connection pool error",
    DbConnection{source: diesel::ConnectionError}               = "Database connection failed",
    Api{source: crate::ApiError}                                = "API",
}

impl GraphQlError {
    fn error_sources(
        mut source: Option<&(dyn std::error::Error + 'static)>,
    ) -> Option<Vec<String>> {
        /* Check if we have any sources to derive reasons from */
        if source.is_some() {
            /* Add all the error sources to a list of reasons for the error */
            let mut reasons = Vec::new();
            while let Some(error) = source {
                reasons.push(error.to_string());
                source = error.source();
            }
            Some(reasons)
        } else {
            None
        }
    }
}

impl ErrorExtensions for GraphQlError {
    // lets define our base extensions
    fn extend(&self) -> Error {
        Error::new(self.to_string()).extend_with(|_err, e| {
            if let Some(reasons) = Self::error_sources(custom_error::Error::source(&self)) {
                let mut i = 1;
                for reason in reasons {
                    e.set(format!("reason {}", i), reason);
                    i += 1;
                }
            }
        })
    }
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct Store {
    #[derivative(Debug = "ignore")]
    pub pool: Pool<ConnectionManager<SqliteConnection>>,
}

impl Store {
    pub fn new(pool: Pool<ConnectionManager<SqliteConnection>>) -> Self {
        Store { pool }
    }
}

#[derive(Default)]
pub struct Query;

#[Object]
impl Query {
    #[allow(clippy::too_many_arguments)]
    async fn agents_by_type<'a>(
        &self,
        ctx: &Context<'a>,
        typ: ID,
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
            agent::table
                .inner_join(nsdsl::namespace)
                .filter(nsdsl::name.eq(&**ns).and(agent::domaintype.eq(&**typ))),
            agent::name.asc(),
            Agent,
            connection
        )
    }
    async fn agent_by_id<'a>(
        &self,
        ctx: &Context<'a>,
        id: ID,
        namespace: Option<String>,
    ) -> async_graphql::Result<Option<Agent>> {
        use crate::persistence::schema::{
            agent::{self, dsl},
            namespace::dsl as nsdsl,
        };

        let store = ctx.data_unchecked::<Store>();

        let ns = namespace.unwrap_or_else(|| "default".into());
        let mut connection = store.pool.get()?;
        let name = AgentId::new(&**id);

        Ok(agent::table
            .inner_join(nsdsl::namespace)
            .filter(dsl::name.eq(name.decompose()).and(nsdsl::name.eq(&ns)))
            .select(Agent::as_select())
            .first::<Agent>(&mut connection)
            .optional()?)
    }

    async fn entity_by_id<'a>(
        &self,
        ctx: &Context<'a>,
        id: ID,
        namespace: Option<String>,
    ) -> async_graphql::Result<Option<Entity>> {
        use crate::persistence::schema::{
            entity::{self, dsl},
            namespace::dsl as nsdsl,
        };

        let store = ctx.data_unchecked::<Store>();
        let ns = namespace.unwrap_or_else(|| "default".into());
        let mut connection = store.pool.get()?;
        let name = EntityId::new(&**id);

        Ok(entity::table
            .inner_join(nsdsl::namespace)
            .filter(dsl::name.eq(name.decompose()).and(nsdsl::name.eq(&ns)))
            .select(Entity::as_select())
            .first::<Entity>(&mut connection)
            .optional()?)
    }
}

struct Mutation;

async fn transaction_context<'a>(
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

/// Mutation methods that are not exposed as graphql
impl Mutation {
    async fn derivation<'a>(
        &self,
        ctx: &Context<'a>,
        namespace: Option<String>,
        generated_entity: ID,
        used_entity: ID,
        typ: Option<DerivationType>,
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
                typ,
            }))
            .await?;

        transaction_context(res, ctx).await
    }
}

#[Object]
impl Mutation {
    pub async fn agent<'a>(
        &self,
        ctx: &Context<'a>,
        name: String,
        namespace: Option<String>,
        typ: Option<String>,
    ) -> async_graphql::Result<Submission> {
        let api = ctx.data_unchecked::<ApiDispatch>();

        let namespace = namespace.unwrap_or_else(|| "default".to_owned());

        let res = api
            .dispatch(ApiCommand::Agent(AgentCommand::Create {
                name,
                namespace: namespace.clone(),
                domaintype: typ,
            }))
            .await?;

        transaction_context(res, ctx).await
    }

    pub async fn activity<'a>(
        &self,
        ctx: &Context<'a>,
        name: String,
        namespace: Option<String>,
        typ: Option<String>,
    ) -> async_graphql::Result<Submission> {
        let api = ctx.data_unchecked::<ApiDispatch>();

        let namespace = namespace.unwrap_or_else(|| "default".to_owned());

        let res = api
            .dispatch(ApiCommand::Activity(ActivityCommand::Create {
                name,
                namespace: namespace.clone(),
                domaintype: typ,
            }))
            .await?;

        transaction_context(res, ctx).await
    }

    pub async fn acted_on_behalf_of<'a>(
        &self,
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
        &self,
        ctx: &Context<'a>,
        namespace: Option<String>,
        generated_entity: ID,
        used_entity: ID,
    ) -> async_graphql::Result<Submission> {
        self.derivation(ctx, namespace, generated_entity, used_entity, None)
            .await
    }

    pub async fn was_revision_of<'a>(
        &self,
        ctx: &Context<'a>,
        namespace: Option<String>,
        generated_entity: ID,
        used_entity: ID,
    ) -> async_graphql::Result<Submission> {
        self.derivation(
            ctx,
            namespace,
            generated_entity,
            used_entity,
            Some(DerivationType::Revision),
        )
        .await
    }
    pub async fn had_primary_source<'a>(
        &self,
        ctx: &Context<'a>,
        namespace: Option<String>,
        generated_entity: ID,
        used_entity: ID,
    ) -> async_graphql::Result<Submission> {
        self.derivation(
            ctx,
            namespace,
            generated_entity,
            used_entity,
            Some(DerivationType::PrimarySource),
        )
        .await
    }
    pub async fn was_quoted_from<'a>(
        &self,
        ctx: &Context<'a>,
        namespace: Option<String>,
        generated_entity: ID,
        used_entity: ID,
    ) -> async_graphql::Result<Submission> {
        self.derivation(
            ctx,
            namespace,
            generated_entity,
            used_entity,
            Some(DerivationType::Quotation),
        )
        .await
    }

    pub async fn generate_key<'a>(
        &self,
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
        &self,
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
        &self,
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
        &self,
        ctx: &Context<'a>,
        activity: String,
        name: String,
        namespace: Option<String>,
        typ: Option<String>,
    ) -> async_graphql::Result<Submission> {
        let api = ctx.data_unchecked::<ApiDispatch>();

        let namespace = namespace.unwrap_or_else(|| "default".to_owned());

        let res = api
            .dispatch(ApiCommand::Activity(ActivityCommand::Use {
                name,
                namespace: namespace.clone(),
                domaintype: typ,
                activity: Some(activity),
            }))
            .await?;

        transaction_context(res, ctx).await
    }

    pub async fn was_generated_by<'a>(
        &self,
        ctx: &Context<'a>,
        activity: String,
        name: String,
        namespace: Option<String>,
        typ: Option<String>,
    ) -> async_graphql::Result<Submission> {
        let api = ctx.data_unchecked::<ApiDispatch>();

        let namespace = namespace.unwrap_or_else(|| "default".to_owned());

        let res = api
            .dispatch(ApiCommand::Activity(ActivityCommand::Generate {
                name,
                namespace: namespace.clone(),
                domaintype: typ,
                activity: Some(activity),
            }))
            .await?;

        transaction_context(res, ctx).await
    }

    pub async fn has_attachment<'a>(
        &self,
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
                file: PathOrFile::File(Arc::new(Box::pin(
                    attachment.value(ctx)?.into_async_read(),
                ))),
                locator: Some(locator),
            }))
            .await?;

        transaction_context(res, ctx).await
    }
}

pub struct Subscription;

#[derive(Queryable)]
pub struct CommitNotification {
    correlation_id: String,
}

#[Object]
impl CommitNotification {
    pub async fn correlation_id(&self) -> &String {
        &self.correlation_id
    }
}

#[Subscription]
impl Subscription {
    async fn commit_notifications<'a>(
        &self,
        ctx: &Context<'a>,
    ) -> impl Stream<Item = CommitNotification> {
        let api = ctx.data_unchecked::<ApiDispatch>().clone();
        let mut rx = api.notify_commit.subscribe();
        async_stream::stream! {
            loop {
                match rx.recv().await {
                    Ok((_prov, correlation_id)) =>
                    yield CommitNotification {correlation_id: correlation_id.to_string()},
                    Err(RecvError::Lagged(_)) => {
                    }
                    Err(_) => break
                }
            }
        }
    }
}

pub fn exportable_schema() -> String {
    let schema = Schema::build(Query, Mutation, Subscription).finish();

    schema.federation_sdl()
}

#[instrument]
pub async fn serve_graphql(
    pool: Pool<ConnectionManager<SqliteConnection>>,
    api: ApiDispatch,
    address: SocketAddr,
    open: bool,
) {
    let schema = Schema::build(Query, Mutation, Subscription)
        .extension(OpenTelemetry::new(opentelemetry::global::tracer(
            "chronicle-api-gql",
        )))
        .data(Store::new(pool.clone()))
        .data(api)
        .finish();

    let graphql_post = async_graphql_warp::graphql(schema.clone()).and_then(
        |(schema, request): (
            Schema<Query, Mutation, Subscription>,
            async_graphql::Request,
        )| async move {
            Ok::<_, Infallible>(async_graphql_warp::GraphQLResponse::from(
                schema.execute(request).await,
            ))
        },
    );

    let routes =
        graphql_subscription(schema)
            .or(graphql_post)
            .recover(|err: Rejection| async move {
                if let Some(GraphQLBadRequest(err)) = err.find() {
                    return Ok::<_, Infallible>(warp::reply::with_status(
                        err.to_string(),
                        StatusCode::BAD_REQUEST,
                    ));
                }

                Ok(warp::reply::with_status(
                    "INTERNAL_SERVER_ERROR".to_string(),
                    StatusCode::INTERNAL_SERVER_ERROR,
                ))
            });

    if open {
        let allow_apollo_studio = warp::cors()
            .allow_methods(vec!["GET", "POST"])
            .allow_any_origin()
            .allow_headers(vec!["Content-Type"])
            .allow_credentials(true);

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(200)).await;
            open::that("https://studio.apollographql.com/sandbox/explorer/").ok();
        });

        warp::serve(routes.with(allow_apollo_studio))
            .run(address)
            .await;
    } else {
        warp::serve(routes).run(address).await;
    }
}

#[cfg(test)]
mod test {
    use async_graphql::{Request, Schema};
    use common::ledger::InMemLedger;
    use diesel::{r2d2::ConnectionManager, SqliteConnection};
    use r2d2::Pool;
    use std::time::Duration;
    use tempfile::TempDir;
    use tracing::Level;
    use uuid::Uuid;

    use crate::{persistence::ConnectionOptions, Api, UuidGen};

    use super::{Mutation, Query, Store, Subscription};

    #[derive(Debug, Clone)]
    struct SameUuid;

    impl UuidGen for SameUuid {
        fn uuid() -> Uuid {
            Uuid::parse_str("5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea").unwrap()
        }
    }

    async fn test_schema() -> Schema<Query, Mutation, Subscription> {
        tracing_log::LogTracer::init_with_filter(tracing::log::LevelFilter::Trace).ok();
        tracing_subscriber::fmt()
            .pretty()
            .with_max_level(Level::TRACE)
            .try_init()
            .ok();

        let secretpath = TempDir::new().unwrap();

        // We need to use a real file for sqlite, as in mem either re-creates between
        // macos temp dir permissions don't work with sqlite
        std::fs::create_dir("./sqlite_test").ok();
        let dbid = Uuid::new_v4();
        let mut ledger = InMemLedger::new();
        let reader = ledger.reader();

        let pool = Pool::builder()
            .connection_customizer(Box::new(ConnectionOptions {
                enable_wal: true,
                enable_foreign_keys: true,
                busy_timeout: Some(Duration::from_secs(2)),
            }))
            .build(ConnectionManager::<SqliteConnection>::new(&*format!(
                "./sqlite_test/db{}.sqlite",
                dbid
            )))
            .unwrap();

        let (dispatch, _ui) = Api::new(
            None,
            pool.clone(),
            ledger,
            reader,
            &secretpath.into_path(),
            SameUuid,
        )
        .await
        .unwrap();

        Schema::build(Query, Mutation, Subscription)
            .data(Store::new(pool))
            .data(dispatch)
            .finish()
    }

    #[tokio::test]
    async fn delegation() {
        let schema = test_schema().await;

        let created = schema
            .execute(Request::new(
                r#"
            mutation {
                actedOnBehalfOf(
                    responsible: "http://blockchaintp.com/chronicle/ns#agent:responsible",
                    delegate: "http://blockchaintp.com/chronicle/ns#agent:delegate",
                    ) {
                    context
                }
            }
        "#,
            ))
            .await;

        insta::assert_toml_snapshot!(created);

        tokio::time::sleep(Duration::from_millis(100)).await;

        let derived = schema
            .execute(Request::new(
                r#"
            query {
                agentById(id: "http://blockchaintp.com/chronicle/ns#agent:responsible") {
                    actedOnBehalfOf {
                        id
                    }
                }
            }
        "#,
            ))
            .await;
        insta::assert_toml_snapshot!(derived);
    }

    #[tokio::test]
    async fn untyped_derivation() {
        let schema = test_schema().await;

        let created = schema
            .execute(Request::new(
                r#"
            mutation {
                wasDerivedFrom(generatedEntity: "http://blockchaintp.com/chronicle/ns#entity:generated",
                               usedEntity: "http://blockchaintp.com/chronicle/ns#entity:used") {
                    context
                }
            }
        "#,
            ))
            .await;

        insta::assert_toml_snapshot!(created);

        tokio::time::sleep(Duration::from_millis(100)).await;
        let derived = schema
            .execute(Request::new(
                r#"
            query {
                entityById(id: "http://blockchaintp.com/chronicle/ns#entity:generated") {
                    wasDerivedFrom {
                        id
                    }
                }
            }
        "#,
            ))
            .await;
        insta::assert_toml_snapshot!(derived);
    }

    #[tokio::test]
    async fn primary_source() {
        let schema = test_schema().await;

        let created = schema
            .execute(Request::new(
                r#"
            mutation {
                hadPrimarySource(generatedEntity: "http://blockchaintp.com/chronicle/ns#entity:generated",
                               usedEntity: "http://blockchaintp.com/chronicle/ns#entity:used") {
                    context
                }
            }
        "#,
            ))
            .await;

        insta::assert_toml_snapshot!(created);

        tokio::time::sleep(Duration::from_millis(100)).await;
        let derived = schema
            .execute(Request::new(
                r#"
            query {
                entityById(id: "http://blockchaintp.com/chronicle/ns#entity:generated") {
                    wasDerivedFrom {
                        id
                    }
                    hadPrimarySource {
                        id
                    }
                }
            }
        "#,
            ))
            .await;
        insta::assert_toml_snapshot!(derived);
    }

    #[tokio::test]
    async fn revision() {
        let schema = test_schema().await;

        let created = schema
            .execute(Request::new(
                r#"
            mutation {
                wasRevisionOf(generatedEntity: "http://blockchaintp.com/chronicle/ns#entity:generated",
                            usedEntity: "http://blockchaintp.com/chronicle/ns#entity:used") {
                    context
                }
            }
        "#,
            ))
            .await;

        insta::assert_toml_snapshot!(created);

        tokio::time::sleep(Duration::from_millis(100)).await;
        let derived = schema
            .execute(Request::new(
                r#"
            query {
                entityById(id: "http://blockchaintp.com/chronicle/ns#entity:generated") {
                    wasDerivedFrom {
                        id
                    }
                    wasRevisionOf {
                        id
                    }
                }
            }
        "#,
            ))
            .await;
        insta::assert_toml_snapshot!(derived);
    }

    #[tokio::test]
    async fn quotation() {
        let schema = test_schema().await;

        let created = schema
            .execute(Request::new(
                r#"
            mutation {
                wasQuotedFrom(generatedEntity: "http://blockchaintp.com/chronicle/ns#entity:generated",
                            usedEntity: "http://blockchaintp.com/chronicle/ns#entity:used") {
                    context
                }
            }
        "#,
            ))
            .await;

        insta::assert_toml_snapshot!(created);

        tokio::time::sleep(Duration::from_millis(100)).await;
        let derived = schema
            .execute(Request::new(
                r#"
            query {
                entityById(id: "http://blockchaintp.com/chronicle/ns#entity:generated") {
                    wasDerivedFrom {
                        id
                    }
                    wasQuotedFrom {
                        id
                    }
                }
            }
        "#,
            ))
            .await;
        insta::assert_toml_snapshot!(derived);
    }

    #[tokio::test]
    async fn agent_can_be_created() {
        let schema = test_schema().await;

        let create = schema
            .execute(Request::new(
                r#"
            mutation {
                agent(name:"bobross", typ: "artist") {
                    context
                }
            }
        "#,
            ))
            .await;

        insta::assert_toml_snapshot!(create);
    }

    #[tokio::test]
    async fn query_agents_by_cursor() {
        let schema = test_schema().await;

        for i in 0..100 {
            schema
                .execute(Request::new(format!(
                    r#"
            mutation {{
                agent(name:"bobross{}", typ: "artist") {{
                    context
                }}
            }}
        "#,
                    i
                )))
                .await;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;

        let default_cursor = schema
            .execute(Request::new(
                r#"
                query {
                agentsByType(typ: "artist") {
                    pageInfo {
                        hasPreviousPage
                        hasNextPage
                        startCursor
                        endCursor
                    }
                    edges {
                        node {
                            id,
                            name
                        }
                        cursor
                    }
                }
                }
        "#,
            ))
            .await;

        insta::assert_json_snapshot!(default_cursor);

        let middle_cursor = schema
            .execute(Request::new(
                r#"
                query {
                agentsByType(typ: "artist", first: 20, after: "3") {
                    pageInfo {
                        hasPreviousPage
                        hasNextPage
                        startCursor
                        endCursor
                    }
                    edges {
                        node {
                            id,
                            name
                        }
                        cursor
                    }
                }
                }
        "#,
            ))
            .await;

        insta::assert_json_snapshot!(middle_cursor);

        let out_of_bound_cursor = schema
            .execute(Request::new(
                r#"
                query {
                agentsByType(typ: "artist", first: 20, after: "90") {
                    pageInfo {
                        hasPreviousPage
                        hasNextPage
                        startCursor
                        endCursor
                    }
                    edges {
                        node {
                            id,
                            name
                        }
                        cursor
                    }
                }
                }
        "#,
            ))
            .await;

        insta::assert_json_snapshot!(out_of_bound_cursor);
    }
}
