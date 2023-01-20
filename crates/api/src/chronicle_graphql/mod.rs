use async_graphql::{
    extensions::OpenTelemetry,
    http::{playground_source, GraphQLPlaygroundConfig},
    scalar, Context, Enum, Error, ErrorExtensions, Object, ObjectType, Schema, SimpleObject,
    Subscription, SubscriptionType,
};
use async_graphql_poem::{GraphQL, GraphQLBatchRequest, GraphQLBatchResponse, GraphQLSubscription};
use chrono::{DateTime, NaiveDateTime, Utc};
use common::{
    identity::AuthId,
    ledger::{SubmissionError, SubmissionStage},
    prov::{to_json_ld::ToJson, ChronicleTransactionId, ProvModel},
};
use custom_error::custom_error;
use derivative::*;
use diesel::{
    prelude::*,
    r2d2::{ConnectionManager, Pool},
    PgConnection, Queryable,
};
use futures::Stream;
use poem::{
    get, handler,
    http::{HeaderValue, StatusCode},
    listener::TcpListener,
    post,
    web::{
        headers::authorization::{Bearer, Credentials},
        Html,
    },
    Endpoint, EndpointExt, IntoResponse, Route, Server,
};
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::sync::broadcast::error::RecvError;
use tracing::{error, trace_span, Instrument};
use url::Url;

use self::authorization::JwtChecker;
use crate::ApiDispatch;

#[macro_use]
pub mod activity;
pub mod agent;
mod authorization;
mod cursor_query;
pub mod entity;
pub mod mutation;
pub mod query;

#[derive(Default, Queryable, Selectable, SimpleObject)]
#[diesel(table_name = crate::persistence::schema::agent)]
pub struct Agent {
    pub id: i32,
    pub external_id: String,
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

#[derive(Default, Queryable, Selectable, SimpleObject)]
#[diesel(table_name = crate::persistence::schema::activity)]
pub struct Activity {
    pub id: i32,
    pub external_id: String,
    pub namespace_id: i32,
    pub domaintype: Option<String>,
    pub started: Option<NaiveDateTime>,
    pub ended: Option<NaiveDateTime>,
}

#[derive(Queryable, Selectable, SimpleObject)]
#[diesel(table_name = crate::persistence::schema::entity)]
pub struct Entity {
    pub id: i32,
    pub external_id: String,
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
    external_id: String,
}

#[Object]
impl Namespace {
    async fn external_id(&self) -> &str {
        &self.external_id
    }

    async fn uuid(&self) -> &str {
        &self.uuid
    }
}

#[derive(Default, Queryable)]
pub struct Submission {
    context: String,
    tx_id: String,
}

#[Object]
impl Submission {
    async fn context(&self) -> &str {
        &self.context
    }

    async fn tx_id(&self) -> &str {
        &self.tx_id
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum TimelineOrder {
    NewestFirst,
    OldestFirst,
}

custom_error! {pub GraphQlError
    Db{source: diesel::result::Error}                           = "Database operation failed",
    R2d2{source: r2d2::Error }                                  = "Connection pool error",
    DbConnection{source: diesel::ConnectionError}               = "Database connection failed",
    Api{source: crate::ApiError}                                = "API",
    Io{source: std::io::Error}                                  = "I/O",
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
    pub pool: Pool<ConnectionManager<PgConnection>>,
}

impl Store {
    pub fn new(pool: Pool<ConnectionManager<PgConnection>>) -> Self {
        Store { pool }
    }
}

pub struct Commit {
    pub tx_id: String,
}

pub struct Rejection {
    pub commit: Commit,
    pub reason: String,
}

#[derive(Enum, PartialEq, Eq, Clone, Copy)]
pub enum Stage {
    Submit,
    Commit,
}

#[derive(Serialize, Deserialize)]
pub struct Delta(async_graphql::Value);
scalar!(Delta);

#[derive(SimpleObject)]
pub struct CommitNotification {
    pub stage: Stage,
    pub tx_id: String,
    pub error: Option<String>,
    pub delta: Option<Delta>,
}

impl CommitNotification {
    pub fn from_submission(tx_id: &ChronicleTransactionId) -> Self {
        CommitNotification {
            stage: Stage::Submit,
            tx_id: tx_id.to_string(),
            error: None,
            delta: None,
        }
    }

    pub fn from_submission_failed(e: &SubmissionError) -> Self {
        CommitNotification {
            stage: Stage::Submit,
            tx_id: e.tx_id().to_string(),
            error: Some(e.to_string()),
            delta: None,
        }
    }

    pub fn from_contradiction(tx_id: &ChronicleTransactionId, contradiction: &str) -> Self {
        CommitNotification {
            stage: Stage::Commit,
            tx_id: tx_id.to_string(),
            error: Some(contradiction.to_string()),
            delta: None,
        }
    }

    pub async fn from_committed(
        tx_id: &ChronicleTransactionId,
        delta: Box<ProvModel>,
    ) -> Result<Self, async_graphql::Error> {
        Ok(CommitNotification {
            stage: Stage::Commit,
            tx_id: tx_id.to_string(),
            error: None,
            delta: delta
                .to_json()
                .compact_stable_order()
                .await
                .ok()
                .map(async_graphql::Value::from_json)
                .transpose()?
                .map(Delta),
        })
    }
}

pub struct Subscription;

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
                    Ok(SubmissionStage::Submitted(Ok(submission))) =>
                      yield CommitNotification::from_submission(&submission),
                    Ok(SubmissionStage::Committed(Ok(commit))) => {
                      let notify = CommitNotification::from_committed(&commit.tx_id, commit.delta).await;
                      if let Ok(notify) = notify {
                        yield notify;
                      } else {
                        error!("Failed to convert commit to notification: {:?}", notify.err());
                      }
                    }
                    Ok(SubmissionStage::Committed(Err((commit,contradiction)))) =>
                      yield CommitNotification::from_contradiction(&commit, &*contradiction.to_string()),
                    Ok(SubmissionStage::Submitted(Err(e))) => {
                      error!("Failed to submit: {:?}", e);
                      yield CommitNotification::from_submission_failed(&e);
                    }
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
        pool: Pool<ConnectionManager<PgConnection>>,
        api: ApiDispatch,
        address: SocketAddr,
        open: bool,
        jwks_uri: Option<Url>,
        id_pointer: Option<String>,
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

#[handler]
async fn gql_playground() -> impl IntoResponse {
    Html(playground_source(
        GraphQLPlaygroundConfig::new("/").subscription_endpoint("/ws"),
    ))
}

#[derive(Clone, Debug)]
pub struct JwtClaims(pub serde_json::Map<String, serde_json::Value>);

struct AuthorizationEndpoint<Q, M, S> {
    checker: JwtChecker,
    schema: Schema<Q, M, S>,
}

#[poem::async_trait]
impl<Q, M, S> Endpoint for AuthorizationEndpoint<Q, M, S>
where
    Q: ObjectType + 'static,
    M: ObjectType + 'static,
    S: SubscriptionType + 'static,
{
    type Output = GraphQLBatchResponse;

    async fn call(&self, req: poem::Request) -> poem::Result<Self::Output> {
        if let Some(authorization) = req.header("Authorization") {
            if let Ok(authorization) = HeaderValue::from_str(authorization) {
                let bearer_token_maybe: Option<Bearer> = Credentials::decode(&authorization);
                if let Some(bearer_token) = bearer_token_maybe {
                    if let Ok(claims) = self.checker.verify_jwt(bearer_token.token()).await {
                        use poem::FromRequest;
                        let (req, mut body) = req.split();
                        let req = GraphQLBatchRequest::from_request(&req, &mut body).await?;
                        let req = req.0.data(JwtClaims(claims));
                        let span = trace_span!("Execute GraphQL request", request = ?req);
                        return Ok(GraphQLBatchResponse(
                            self.schema.execute_batch(req).instrument(span).await,
                        ));
                    }
                }
            }
            tracing::warn!("rejected authorization: {:?}", authorization);
        } else {
            tracing::info!("rejected anonymous access attempt");
        }
        Err(poem::error::Error::from_string(
            "Authorization header must provide valid bearer token",
            StatusCode::UNAUTHORIZED,
        ))
    }
}

struct AuthFromJwt {
    json_pointer: String,
}

#[async_trait::async_trait]
impl async_graphql::extensions::Extension for AuthFromJwt {
    async fn prepare_request(
        &self,
        ctx: &async_graphql::extensions::ExtensionContext<'_>,
        mut request: async_graphql::Request,
        next: async_graphql::extensions::NextPrepareRequest<'_>,
    ) -> async_graphql::ServerResult<async_graphql::Request> {
        if let Some(claims) = ctx.data_opt::<JwtClaims>() {
            use common::prov::AgentId;
            use serde_json::Value;

            if let Some(Value::String(external_id)) =
                Value::Object(claims.0.clone()).pointer(&self.json_pointer)
            {
                let chronicle_id = AuthId::agent(&AgentId::from_external_id(external_id));
                tracing::debug!(
                    "Chronicle identity for GraphQL request is {:?}",
                    chronicle_id
                );
                request = request.data(chronicle_id);
            }
        }
        next.run(ctx, request).await
    }
}

#[async_trait::async_trait]
impl async_graphql::extensions::ExtensionFactory for AuthFromJwt {
    fn create(&self) -> Arc<dyn async_graphql::extensions::Extension> {
        Arc::new(AuthFromJwt {
            json_pointer: self.json_pointer.clone(),
        })
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
        pool: Pool<ConnectionManager<PgConnection>>,
        api: ApiDispatch,
        address: SocketAddr,
        open: bool,
        jwks_uri: Option<Url>,
        id_pointer: Option<String>,
    ) {
        let mut schema = Schema::build(self.query, self.mutation, Subscription).extension(
            OpenTelemetry::new(opentelemetry::global::tracer("chronicle-api-gql")),
        );
        if let Some(id_pointer) = id_pointer {
            schema = schema.extension(AuthFromJwt {
                json_pointer: id_pointer,
            })
        };
        let schema = schema
            .data(Store::new(pool.clone()))
            .data(api)
            .data(AuthId::chronicle())
            .finish();

        if open {
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(200)).await;
                open::that(format!("http://{}", address)).ok();
            });
            let app = Route::new()
                .at("/", get(gql_playground).post(GraphQL::new(schema.clone())))
                .at("/ws", get(GraphQLSubscription::new(schema.clone())))
                .data(schema);

            Server::new(TcpListener::bind(address)).run(app).await.ok();
        } else {
            let app = match jwks_uri {
                Some(jwks_uri) => {
                    tracing::debug!("API endpoint authentication uses {}", jwks_uri);
                    Route::new()
                        .at(
                            "/",
                            post(AuthorizationEndpoint {
                                checker: JwtChecker::new(&jwks_uri),
                                schema: schema.clone(),
                            }),
                        )
                        .at("/ws", get(GraphQLSubscription::new(schema)))
                }
                None => {
                    tracing::warn!("API endpoint uses no authentication");
                    Route::new()
                        .at("/", post(GraphQL::new(schema.clone())))
                        .at("/ws", get(GraphQLSubscription::new(schema)))
                }
            };

            Server::new(TcpListener::bind(address)).run(app).await.ok();
        }
    }
}
