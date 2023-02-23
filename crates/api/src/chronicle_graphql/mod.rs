use async_graphql::{
    extensions::OpenTelemetry, http::ALL_WEBSOCKET_PROTOCOLS, scalar, Context, Enum, Error,
    ErrorExtensions, Object, ObjectType, Schema, SimpleObject, Subscription, SubscriptionType,
};
use async_graphql_poem::{
    GraphQL, GraphQLBatchRequest, GraphQLBatchResponse, GraphQLProtocol, GraphQLSubscription,
    GraphQLWebSocket,
};
use chrono::{DateTime, NaiveDateTime, Utc};
use common::{
    identity::{AuthId, JwtClaims, OpaData},
    ledger::{SubmissionError, SubmissionStage},
    opa::ExecutorContext,
    prov::{to_json_ld::ToJson, ChronicleTransactionId, ProvModel},
};
use derivative::*;
use diesel::{
    prelude::*,
    r2d2::{ConnectionManager, Pool},
    PgConnection, Queryable,
};
use futures::Stream;
use poem::{
    get,
    http::{HeaderValue, StatusCode},
    listener::TcpListener,
    post,
    web::headers::authorization::{Bearer, Credentials},
    Endpoint, Route, Server,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use thiserror::Error;
use tokio::sync::broadcast::error::RecvError;
use tracing::{error, instrument};
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

#[derive(Error, Debug)]
pub enum GraphQlError {
    #[error("Database operation failed: {0}")]
    Db(#[from] diesel::result::Error),

    #[error("Connection pool error: {0}")]
    R2d2(#[from] r2d2::Error),

    #[error("Database connection failed: {0}")]
    DbConnection(#[from] diesel::ConnectionError),

    #[error("API: {0}")]
    Api(#[from] crate::ApiError),

    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),
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
                    e.set(format!("reason {i}"), reason);
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

pub struct SecurityConf {
    pub jwks_uri: Option<Url>,
    pub userinfo_uri: Option<Url>,
    pub id_pointer: Option<String>,
    pub jwt_must_claim: HashMap<String, String>,
    pub allow_anonymous: bool,
    pub opa: ExecutorContext,
}

#[async_trait::async_trait]
pub trait ChronicleGraphQlServer {
    async fn serve_graphql(
        &self,
        pool: Pool<ConnectionManager<PgConnection>>,
        api: ApiDispatch,
        address: SocketAddr,
        security_conf: SecurityConf,
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

fn check_required_claim(must_value: &str, actual_value: &serde_json::Value) -> bool {
    match actual_value {
        serde_json::Value::String(actual_value) => must_value == actual_value,
        serde_json::Value::Array(actual_values) => actual_values
            .iter()
            .any(|actual_value| check_required_claim(must_value, actual_value)),
        _ => false,
    }
}

#[instrument(level = "debug", ret(Debug))]
fn check_required_claims(
    must_claim: &HashMap<String, String>,
    actual_claims: &serde_json::Map<String, serde_json::Value>,
) -> bool {
    for (name, value) in must_claim {
        if let Some(json) = actual_claims.get(name) {
            if !check_required_claim(value, json) {
                return false;
            }
        } else {
            return false;
        }
    }
    true
}

struct AuthorizationEndpointQuery<Q, M, S> {
    checker: JwtChecker,
    must_claim: HashMap<String, String>,
    allow_anonymous: bool,
    schema: Schema<Q, M, S>,
}

impl<Q, M, S> AuthorizationEndpointQuery<Q, M, S>
where
    Q: ObjectType + 'static,
    M: ObjectType + 'static,
    S: SubscriptionType + 'static,
{
    #[instrument(level = "debug", skip(self, prepare_req), ret(Debug))]
    async fn respond(
        &self,
        req: poem::Request,
        prepare_req: impl FnOnce(GraphQLBatchRequest) -> async_graphql::BatchRequest,
    ) -> poem::Result<poem::Response> {
        use poem::{FromRequest, IntoResponse};
        let (req, mut body) = req.split();
        let req = prepare_req(GraphQLBatchRequest::from_request(&req, &mut body).await?);
        Ok(GraphQLBatchResponse(self.schema.execute_batch(req).await).into_response())
    }
}

#[poem::async_trait]
impl<Q, M, S> Endpoint for AuthorizationEndpointQuery<Q, M, S>
where
    Q: ObjectType + 'static,
    M: ObjectType + 'static,
    S: SubscriptionType + 'static,
{
    type Output = poem::Response;

    async fn call(&self, req: poem::Request) -> poem::Result<Self::Output> {
        if let Some(authorization) = req.header("Authorization") {
            if let Ok(authorization) = HeaderValue::from_str(authorization) {
                let bearer_token_maybe: Option<Bearer> = Credentials::decode(&authorization);
                if let Some(bearer_token) = bearer_token_maybe {
                    if let Ok(claims) = self.checker.verify_jwt(bearer_token.token()).await {
                        if check_required_claims(&self.must_claim, &claims) {
                            return self.respond(req, |req| req.0.data(JwtClaims(claims))).await;
                        }
                    }
                }
            }
            tracing::warn!(
                "rejected authorization from {}: {:?}",
                req.remote_addr(),
                authorization
            );
            Err(poem::error::Error::from_string(
                "Authorization header present but without a satisfactory bearer token",
                StatusCode::UNAUTHORIZED,
            ))
        } else if self.allow_anonymous {
            tracing::debug!("anonymous access from {}", req.remote_addr());
            self.respond(req, |req| req.0).await
        } else {
            tracing::warn!("rejected anonymous access from {}", req.remote_addr());
            Err(poem::error::Error::from_string(
                "required Authorization header not present",
                StatusCode::UNAUTHORIZED,
            ))
        }
    }
}

struct AuthorizationEndpointSubscription<Q, M, S> {
    checker: JwtChecker,
    must_claim: HashMap<String, String>,
    allow_anonymous: bool,
    schema: Schema<Q, M, S>,
}

impl<Q, M, S> AuthorizationEndpointSubscription<Q, M, S>
where
    Q: ObjectType + 'static,
    M: ObjectType + 'static,
    S: SubscriptionType + 'static,
{
    #[instrument(level = "debug", skip(self), ret(Debug))]
    async fn respond(
        &self,
        req: poem::Request,
        data: async_graphql::Data,
    ) -> poem::Result<poem::Response> {
        use poem::{FromRequest, IntoResponse};
        let (req, mut body) = req.split();
        let websocket = poem::web::websocket::WebSocket::from_request(&req, &mut body).await?;
        let protocol = GraphQLProtocol::from_request(&req, &mut body).await?;
        let schema = self.schema.clone();
        Ok(websocket
            .protocols(ALL_WEBSOCKET_PROTOCOLS)
            .on_upgrade(move |stream| {
                GraphQLWebSocket::new(stream, schema, protocol)
                    .with_data(data)
                    .serve()
            })
            .into_response())
    }
}

#[poem::async_trait]
impl<Q, M, S> Endpoint for AuthorizationEndpointSubscription<Q, M, S>
where
    Q: ObjectType + 'static,
    M: ObjectType + 'static,
    S: SubscriptionType + 'static,
{
    type Output = poem::Response;

    async fn call(&self, req: poem::Request) -> poem::Result<Self::Output> {
        if let Some(authorization) = req.header("Authorization") {
            if let Ok(authorization) = HeaderValue::from_str(authorization) {
                let bearer_token_maybe: Option<Bearer> = Credentials::decode(&authorization);
                if let Some(bearer_token) = bearer_token_maybe {
                    if let Ok(claims) = self.checker.verify_jwt(bearer_token.token()).await {
                        if check_required_claims(&self.must_claim, &claims) {
                            let mut data = async_graphql::Data::default();
                            data.insert(JwtClaims(claims));
                            return self.respond(req, data).await;
                        }
                    }
                }
            }
            tracing::warn!(
                "rejected authorization from {}: {:?}",
                req.remote_addr(),
                authorization
            );
            Err(poem::error::Error::from_string(
                "Authorization header present but without a satisfactory bearer token",
                StatusCode::UNAUTHORIZED,
            ))
        } else if self.allow_anonymous {
            self.respond(req, async_graphql::Data::default()).await
        } else {
            tracing::warn!("rejected anonymous access from {}", req.remote_addr());
            Err(poem::error::Error::from_string(
                "required Authorization header not present",
                StatusCode::UNAUTHORIZED,
            ))
        }
    }
}

#[derive(Clone, Debug)]
pub struct AuthFromJwt {
    json_pointer: String,
}

impl AuthFromJwt {
    #[instrument(level = "debug", ret(Debug))]
    fn identity(&self, claims: &JwtClaims) -> Option<AuthId> {
        AuthId::from_jwt_claims(claims, &self.json_pointer).ok()
    }
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
            if let Some(chronicle_id) = self.identity(claims) {
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

#[derive(Clone, Debug)]
pub struct OpaCheck {
    pub claim_parser: Option<AuthFromJwt>,
}

#[async_trait::async_trait]
impl async_graphql::extensions::Extension for OpaCheck {
    #[instrument(level = "debug", skip_all, ret(Debug))]
    async fn resolve(
        &self,
        ctx: &async_graphql::extensions::ExtensionContext<'_>,
        info: async_graphql::extensions::ResolveInfo<'_>,
        next: async_graphql::extensions::NextResolve<'_>,
    ) -> async_graphql::ServerResult<Option<async_graphql::Value>> {
        use async_graphql::ServerError;
        use serde_json::Value;
        if let Some(opa_executor) = ctx.data_opt::<ExecutorContext>() {
            // If unable to get an external_id from the JwtClaims or no claims found,
            // identity will be `Anonymous`
            let identity = match (ctx.data_opt::<JwtClaims>(), &self.claim_parser) {
                (Some(claims), Some(parser)) => {
                    parser.identity(claims).unwrap_or(AuthId::anonymous())
                }
                _ => AuthId::anonymous(),
            };

            // Create OPA context data for the user identity
            let opa_data = OpaData::graphql(
                &identity,
                &Value::String(info.parent_type.to_string()),
                &Value::Array(
                    info.path_node
                        .to_string_vec()
                        .into_iter()
                        .map(Value::String)
                        .collect(),
                ),
            );

            // Execute OPA check
            match opa_executor.evaluate(&identity, &opa_data).await {
                Ok(()) => next.run(ctx, info).await,
                Err(error) => {
                    tracing::warn!(
                        "{error}: attempt to violate policy rules by identity: {identity}, in context: {:#?}",
                        opa_data
                    );
                    Err(ServerError::new("violation of policy rules", None))
                }
            }
        } else {
            Err(ServerError::new("cannot check policy rules", None))
        }
    }
}

#[async_trait::async_trait]
impl async_graphql::extensions::ExtensionFactory for OpaCheck {
    fn create(&self) -> Arc<dyn async_graphql::extensions::Extension> {
        Arc::new(OpaCheck {
            claim_parser: self.claim_parser.clone(),
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
        sec: SecurityConf,
    ) {
        let claim_parser = sec
            .id_pointer
            .map(|json_pointer| AuthFromJwt { json_pointer });
        let mut schema = Schema::build(self.query, self.mutation, Subscription)
            .extension(OpenTelemetry::new(opentelemetry::global::tracer(
                "chronicle-api-gql",
            )))
            .extension(OpaCheck {
                claim_parser: claim_parser.clone(),
            });
        if let Some(claim_parser) = claim_parser {
            schema = schema.extension(claim_parser);
        }
        let schema = schema
            .data(Store::new(pool.clone()))
            .data(api)
            .data(sec.opa)
            .data(AuthId::anonymous())
            .finish();

        let app = match sec.jwks_uri {
            Some(jwks_uri) => {
                const CACHE_EXPIRY_SECONDS: u32 = 100;
                tracing::debug!("API endpoint authentication uses {}", jwks_uri);
                Route::new()
                    .at(
                        "/",
                        post(AuthorizationEndpointQuery {
                            checker: JwtChecker::new(
                                &jwks_uri,
                                sec.userinfo_uri.as_ref(),
                                CACHE_EXPIRY_SECONDS,
                            ),
                            must_claim: sec.jwt_must_claim.clone(),
                            allow_anonymous: sec.allow_anonymous,
                            schema: schema.clone(),
                        }),
                    )
                    .at(
                        "/ws",
                        get(AuthorizationEndpointSubscription {
                            checker: JwtChecker::new(
                                &jwks_uri,
                                sec.userinfo_uri.as_ref(),
                                CACHE_EXPIRY_SECONDS,
                            ),
                            must_claim: sec.jwt_must_claim,
                            allow_anonymous: sec.allow_anonymous,
                            schema,
                        }),
                    )
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
