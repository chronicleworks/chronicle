use async_graphql::{
    extensions::OpenTelemetry, Context, Error, ErrorExtensions, Object, ObjectType, Schema,
    Subscription,
};
use async_graphql_warp::{graphql_subscription, GraphQLBadRequest};
use chrono::{DateTime, NaiveDateTime, Utc};
use custom_error::custom_error;
use derivative::*;
use diesel::{
    prelude::*,
    r2d2::{ConnectionManager, Pool},
    Queryable, SqliteConnection,
};
use futures::Stream;
use std::{convert::Infallible, net::SocketAddr, time::Duration};
use tokio::sync::broadcast::error::RecvError;

use warp::{hyper::StatusCode, Filter, Rejection};

use crate::ApiDispatch;
#[macro_use]
mod cursor_query;
pub mod activity;
pub mod agent;
pub mod entity;
pub mod mutation;
pub mod query;

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
    pub id: i32,
    pub name: String,
    pub namespace_id: i32,
    pub domaintype: Option<String>,
    pub attachment_id: Option<i32>,
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = crate::persistence::schema::attachment)]
#[allow(dead_code)]
pub struct Evidence {
    id: i32,
    namespace_id: i32,
    signature_time: NaiveDateTime,
    signature: String,
    signer_id: i32,
    locator: Option<String>,
}

#[Object]
impl Evidence {
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

#[derive(Debug, Clone)]
pub struct ChronicleGraphQl<Query, Mutation>
where
    Query: ObjectType + 'static,
    Mutation: ObjectType + 'static,
{
    query: Query,
    mutation: Mutation,
}

#[async_trait::async_trait]
pub trait ChronicleGraphQlServer {
    async fn serve_graphql(
        &self,
        pool: Pool<ConnectionManager<SqliteConnection>>,
        api: ApiDispatch,
        address: SocketAddr,
        open: bool,
    );
}

impl<Query, Mutation> ChronicleGraphQl<Query, Mutation>
where
    Query: ObjectType + Copy,
    Mutation: ObjectType + Copy,
{
    pub fn new(query: Query, mutation: Mutation) -> Self {
        Self { query, mutation }
    }

    pub fn exportable_schema(&self) -> String
    where
        Query: ObjectType + Copy,
        Mutation: ObjectType + Copy,
    {
        let schema = Schema::build(self.query, self.mutation, Subscription).finish();

        schema.sdl()
    }
}

#[async_trait::async_trait]
impl<Query, Mutation> ChronicleGraphQlServer for ChronicleGraphQl<Query, Mutation>
where
    Query: ObjectType + Copy,
    Mutation: ObjectType + Copy,
{
    async fn serve_graphql(
        &self,
        pool: Pool<ConnectionManager<SqliteConnection>>,
        api: ApiDispatch,
        address: SocketAddr,
        open: bool,
    ) {
        let schema = Schema::build(self.query, self.mutation, Subscription)
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
}
