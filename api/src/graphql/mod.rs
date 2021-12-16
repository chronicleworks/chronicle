use std::{convert::Infallible, net::SocketAddr, sync::Arc, time::Duration};

use async_graphql::{
    extensions::Tracing,
    http::{playground_source, GraphQLPlaygroundConfig},
    Context, Error, ErrorExtensions, Object, Schema, Subscription, Upload,
};
use async_graphql_extension_apollo_tracing::{ApolloTracing, ApolloTracingDataExt, HTTPMethod};
use async_graphql_warp::{graphql_subscription, GraphQLBadRequest, GraphQLResponse};
use chrono::{DateTime, NaiveDateTime, Utc};
use common::{
    commands::{
        ActivityCommand, AgentCommand, ApiCommand, ApiResponse, EntityCommand, KeyRegistration,
        PathOrFile,
    },
    prov::{ActivityId, AgentId, EntityId},
};
use custom_error::custom_error;
use derivative::*;
use diesel::{prelude::*, r2d2::Pool};
use diesel::{r2d2::ConnectionManager, Queryable, SqliteConnection};
use futures::Stream;
use tracing::{debug, instrument};
use warp::{
    hyper::{Response, StatusCode},
    Filter, Rejection,
};

use crate::ApiDispatch;

#[derive(Default, Queryable)]
pub struct Agent {
    pub id: i32,
    pub name: String,
    pub namespace: String,
    pub domaintype: Option<String>,
    pub publickey: Option<String>,
    pub current: i32,
}

#[derive(Default, Queryable)]
pub struct Activity {
    pub id: i32,
    pub name: String,
    pub namespace: String,
    pub domaintype: Option<String>,
    pub started: Option<NaiveDateTime>,
    pub ended: Option<NaiveDateTime>,
}

#[derive(Default, Queryable)]
pub struct Entity {
    id: i32,
    name: String,
    namespace: String,
    domaintype: Option<String>,
    signature_time: Option<NaiveDateTime>,
    signature: Option<String>,
    locator: Option<String>,
}

#[Object]
impl Agent {
    async fn namespace(&self) -> &str {
        &self.namespace
    }

    async fn name(&self) -> &str {
        &self.name
    }

    async fn public_key(&self) -> Option<&str> {
        self.publickey.as_deref()
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
    async fn namespace(&self) -> &str {
        &self.namespace
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
        use crate::persistence::schema::wasassociatedwith::{self, dsl};

        let store = ctx.data_unchecked::<Store>();

        let mut connection = store.pool.get()?;

        let res = wasassociatedwith::table
            .filter(dsl::activity.eq(self.id))
            .inner_join(crate::persistence::schema::agent::table)
            .load::<((i32, i32), Agent)>(&mut connection)?;

        Ok(res.into_iter().map(|(_, x)| x).collect())
    }

    async fn used<'a>(&self, ctx: &Context<'a>) -> async_graphql::Result<Vec<Entity>> {
        use crate::persistence::schema::used::{self, dsl};

        let store = ctx.data_unchecked::<Store>();

        let mut connection = store.pool.get()?;

        let res = used::table
            .filter(dsl::activity.eq(self.id))
            .inner_join(crate::persistence::schema::entity::table)
            .load::<((i32, i32), Entity)>(&mut connection)?;

        Ok(res.into_iter().map(|(_, x)| x).collect())
    }
}

#[Object]
impl Entity {
    async fn namespace(&self) -> &str {
        &self.namespace
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

    async fn signature_time(&self) -> Option<DateTime<Utc>> {
        self.signature_time.map(|x| DateTime::from_utc(x, Utc))
    }

    async fn signature(&self) -> Option<&str> {
        self.signature.as_deref()
    }

    async fn locator(&self) -> Option<&str> {
        self.locator.as_deref()
    }

    async fn was_attributed_to<'a>(
        &self,
        ctx: &Context<'a>,
    ) -> async_graphql::Result<Vec<Activity>> {
        use crate::persistence::schema::wasgeneratedby::{self, dsl};

        let store = ctx.data_unchecked::<Store>();

        let mut connection = store.pool.get()?;

        let res = wasgeneratedby::table
            .filter(dsl::entity.eq(self.id))
            .inner_join(crate::persistence::schema::activity::table)
            .load::<((i32, i32), Activity)>(&mut connection)?;

        Ok(res.into_iter().map(|(_, x)| x).collect())
    }

    async fn was_generated_by<'a>(
        &self,
        ctx: &Context<'a>,
    ) -> async_graphql::Result<Vec<Activity>> {
        use crate::persistence::schema::wasgeneratedby::{self, dsl};

        let store = ctx.data_unchecked::<Store>();

        let mut connection = store.pool.get()?;

        let res = wasgeneratedby::table
            .filter(dsl::entity.eq(self.id))
            .inner_join(crate::persistence::schema::activity::table)
            .load::<((i32, i32), Activity)>(&mut connection)?;

        Ok(res.into_iter().map(|(_, x)| x).collect())
    }
}

custom_error! {pub GraphQlError
    Db{source: diesel::result::Error}                           = "Database operation failed",
    DbConnection{source: diesel::ConnectionError}               = "Database connection failed",
    Api{source: crate::ApiError}                                = "API",
}

impl ErrorExtensions for GraphQlError {
    // lets define our base extensions
    fn extend(&self) -> Error {
        Error::new(format!("{}", self)).extend_with(|_err, _e| ())
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
    async fn agent<'a>(
        &self,
        ctx: &Context<'a>,
        name: String,
        namespace: String,
    ) -> async_graphql::Result<Option<Agent>> {
        use crate::persistence::schema::agent::{self, dsl};

        let store = ctx.data_unchecked::<Store>();

        let mut connection = store.pool.get()?;

        Ok(agent::table
            .filter(dsl::name.eq(name).and(dsl::namespace.eq(namespace)))
            .first::<Agent>(&mut connection)
            .optional()?)
    }

    async fn activities_by_time<'a>(
        &self,
        ctx: &Context<'a>,
        types: Vec<String>,
        from_inclusive: Option<DateTime<Utc>>,
        end_exclusive: Option<DateTime<Utc>>,
    ) -> async_graphql::Result<Vec<Activity>> {
        use crate::persistence::schema::activity;
        let store = ctx.data_unchecked::<Store>();

        let mut connection = store.pool.get()?;
        let mut query = activity::table.into_boxed();

        if let Some(start) = from_inclusive {
            query = query.filter(activity::started.gt(start.naive_utc()));
        }

        if let Some(end) = end_exclusive {
            query = query.filter(activity::started.lt(end.naive_utc()));
        }

        for t in types {
            query = query.or_filter(activity::domaintype.eq(t))
        }

        Ok(query.load::<Activity>(&mut connection)?)
    }
}

struct Mutation;

async fn agent_context<'a>(
    namespace: &str,
    res: ApiResponse,
    ctx: &Context<'a>,
) -> async_graphql::Result<Agent> {
    match res {
        ApiResponse::Prov(id, _) => {
            use crate::persistence::schema::agent::{self, dsl};

            let store = ctx.data_unchecked::<Store>();

            let mut connection = store.pool.get()?;

            Ok(agent::table
                .filter(
                    dsl::name
                        .eq(AgentId::from(id).decompose())
                        .and(dsl::namespace.eq(namespace)),
                )
                .first::<Agent>(&mut connection)?)
        }
        _ => unreachable!(),
    }
}

async fn activity_context<'a>(
    namespace: &str,
    res: ApiResponse,
    ctx: &Context<'a>,
) -> async_graphql::Result<Activity> {
    match res {
        ApiResponse::Prov(id, _) => {
            use crate::persistence::schema::activity::{self, dsl};

            let store = ctx.data_unchecked::<Store>();

            let mut connection = store.pool.get()?;

            Ok(activity::table
                .filter(
                    dsl::name
                        .eq(ActivityId::from(id).decompose())
                        .and(dsl::namespace.eq(namespace)),
                )
                .first::<Activity>(&mut connection)?)
        }
        _ => unreachable!(),
    }
}

async fn entity_context<'a>(
    namespace: &str,
    res: ApiResponse,
    ctx: &Context<'a>,
) -> async_graphql::Result<Entity> {
    match res {
        ApiResponse::Prov(id, _) => {
            use crate::persistence::schema::entity::{self, dsl};

            let store = ctx.data_unchecked::<Store>();

            let mut connection = store.pool.get()?;

            Ok(entity::table
                .filter(
                    dsl::name
                        .eq(EntityId::from(id).decompose())
                        .and(dsl::namespace.eq(namespace)),
                )
                .first::<Entity>(&mut connection)?)
        }
        _ => unreachable!(),
    }
}

#[Object]
impl Mutation {
    pub async fn create_agent<'a>(
        &self,
        ctx: &Context<'a>,
        name: String,
        namespace: Option<String>,
        typ: Option<String>,
    ) -> async_graphql::Result<Agent> {
        let api = ctx.data_unchecked::<ApiDispatch>();

        let namespace = namespace.unwrap_or_else(|| "default".to_owned());

        let res = api
            .dispatch(ApiCommand::Agent(AgentCommand::Create {
                name,
                namespace: namespace.clone(),
                domaintype: typ,
            }))
            .await?;

        agent_context(&namespace, res, ctx).await
    }

    pub async fn create_activity<'a>(
        &self,
        ctx: &Context<'a>,
        name: String,
        namespace: Option<String>,
        typ: Option<String>,
    ) -> async_graphql::Result<Activity> {
        let api = ctx.data_unchecked::<ApiDispatch>();

        let namespace = namespace.unwrap_or_else(|| "default".to_owned());

        let res = api
            .dispatch(ApiCommand::Activity(ActivityCommand::Create {
                name,
                namespace: namespace.clone(),
                domaintype: typ,
            }))
            .await?;

        activity_context(&namespace, res, ctx).await
    }

    pub async fn generate_key<'a>(
        &self,
        ctx: &Context<'a>,
        name: String,
        namespace: Option<String>,
    ) -> async_graphql::Result<Agent> {
        let api = ctx.data_unchecked::<ApiDispatch>();

        let namespace = namespace.unwrap_or_else(|| "default".to_owned());

        let res = api
            .dispatch(ApiCommand::Agent(AgentCommand::RegisterKey {
                name,
                namespace: namespace.clone(),
                registration: KeyRegistration::Generate,
            }))
            .await?;

        agent_context(&namespace, res, ctx).await
    }

    pub async fn start_activity<'a>(
        &self,
        ctx: &Context<'a>,
        name: String,
        namespace: Option<String>,
        agent: String,
        time: Option<DateTime<Utc>>,
    ) -> async_graphql::Result<Activity> {
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

        activity_context(&namespace, res, ctx).await
    }

    pub async fn end_activity<'a>(
        &self,
        ctx: &Context<'a>,
        name: String,
        namespace: Option<String>,
        agent: String,
        time: Option<DateTime<Utc>>,
    ) -> async_graphql::Result<Activity> {
        let api = ctx.data_unchecked::<ApiDispatch>();

        let namespace = namespace.unwrap_or_else(|| "default".to_owned());

        let res = api
            .dispatch(ApiCommand::Activity(ActivityCommand::End {
                name: Some(name),
                namespace: Some(namespace.clone()),
                time,
                agent: Some(agent),
            }))
            .await?;

        activity_context(&namespace, res, ctx).await
    }

    pub async fn activity_use<'a>(
        &self,
        ctx: &Context<'a>,
        activity: String,
        name: String,
        namespace: Option<String>,
        typ: Option<String>,
    ) -> async_graphql::Result<Entity> {
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

        entity_context(&namespace, res, ctx).await
    }

    pub async fn activity_generate<'a>(
        &self,
        ctx: &Context<'a>,
        activity: String,
        name: String,
        namespace: Option<String>,
        typ: Option<String>,
    ) -> async_graphql::Result<Entity> {
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

        entity_context(&namespace, res, ctx).await
    }

    pub async fn entity_attach<'a>(
        &self,
        ctx: &Context<'a>,
        name: String,
        namespace: Option<String>,
        attachment: Upload,
        on_behalf_of_agent: String,
        locator: String,
    ) -> async_graphql::Result<Entity> {
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

        entity_context(&namespace, res, ctx).await
    }
}

pub struct Subscription;

#[Subscription]
impl Subscription {
    async fn interval(&self, #[graphql(default = 1)] n: i32) -> impl Stream<Item = i32> {
        let mut value = 0;
        async_stream::stream! {
            loop {
                tokio::time::sleep(Duration::from_secs(1)).await;
                value += n;
                yield value;
            }
        }
    }
}

#[instrument]
pub async fn serve_graphql(
    pool: Pool<ConnectionManager<SqliteConnection>>,
    api: ApiDispatch,
    address: SocketAddr,
    open: bool,
) {
    let schema = Schema::build(Query, Mutation, Subscription)
        .extension(ApolloTracing::new(
            "authorization_token".into(),
            "https://yourdomain.ltd".into(),
            "your_graph@variant".into(),
            "v1.0.0".into(),
            10,
        ))
        .extension(Tracing)
        .data(Store::new(pool.clone()))
        .data(api)
        .finish();

    async_graphql_extension_apollo_tracing::register::register(
        "authorization_token",
        &schema,
        "my-allocation-id",
        "variant",
        "1.0.0",
        "staging",
    )
    .await
    .ok();

    let graphql_post = async_graphql_warp::graphql(schema.clone()).and_then(
        |(schema, request): (
            Schema<Query, Mutation, Subscription>,
            async_graphql::Request,
        )| async move {
            Ok::<_, Infallible>(GraphQLResponse::from(
                schema
                    .execute(request.data(ApolloTracingDataExt {
                        userid: None,
                        path: Some("/".to_string()),
                        host: None,
                        method: Some(HTTPMethod::POST),
                        secure: Some(false),
                        protocol: Some("HTTP/1.1".to_string()),
                        status_code: Some(200),
                        client_name: None,
                        client_version: None,
                    }))
                    .await,
            ))
        },
    );

    let graphql_playground = warp::path::end().and(warp::get()).map(|| {
        Response::builder()
            .header("content-type", "text/html")
            .body(playground_source(
                GraphQLPlaygroundConfig::new("/").subscription_endpoint("/"),
            ))
    });

    let open_address = address;
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(200)).await;
        debug!(?open_address, "Open browser at");
        open::that(&format!("http://{}/", open_address)).ok();
    });

    let routes = graphql_subscription(schema)
        .or(graphql_playground)
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

    warp::serve(routes).run(address).await;
}
