use async_graphql::{
	extensions::OpenTelemetry,
	http::{playground_source, GraphQLPlaygroundConfig, ALL_WEBSOCKET_PROTOCOLS},
	scalar, Context, Enum, Error, ErrorExtensions, Object, ObjectType, Schema, ServerError,
	SimpleObject, Subscription, SubscriptionType,
};
use async_graphql_poem::{
	GraphQL, GraphQLBatchRequest, GraphQLBatchResponse, GraphQLProtocol, GraphQLSubscription,
	GraphQLWebSocket,
};
use chrono::NaiveDateTime;
use common::{
	identity::{AuthId, IdentityError, JwtClaims, OpaData, SignedIdentity},
	ledger::{SubmissionError, SubmissionStage},
	opa::{ExecutorContext, OpaExecutorError},
	prov::{
		to_json_ld::ToJson, ChronicleIri, ChronicleTransactionId, ExternalId, ExternalIdPart,
		ProvModel,
	},
};
use derivative::*;
use diesel::{
	prelude::*,
	r2d2::{ConnectionManager, Pool},
	PgConnection, Queryable,
};
use futures::Stream;
use lazy_static::lazy_static;
use poem::{
	get, handler,
	http::{HeaderValue, StatusCode},
	listener::{Listener, TcpListener},
	post,
	web::{
		headers::authorization::{Bearer, Credentials},
		Html,
	},
	Endpoint, IntoResponse, Route, Server,
};
use r2d2::PooledConnection;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
	collections::{BTreeSet, HashMap},
	fmt::Display,
	net::SocketAddr,
	str::FromStr,
	sync::Arc,
};
use thiserror::Error;
use tokio::sync::{broadcast::error::RecvError, Semaphore};
use tracing::{debug, error, instrument, warn};
use url::Url;

use self::authorization::TokenChecker;
use crate::{ApiDispatch, ApiError, StoreError};

#[macro_use]
pub mod activity;
pub mod agent;
mod authorization;
mod cursor_query;
pub mod entity;
pub mod mutation;
pub mod query;

pub type AuthorizationError = authorization::Error;

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
/// # `chronicle:Identity`
///
/// Represents a cryptographic identity for an agent, supporting a single current
/// signing identity via `chronicle:hasIdentity` and historical identities via
/// `chronicle:hadIdentity`.
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
}

#[derive(Default, Queryable)]
pub struct Namespace {
	_id: i32,
	uuid: String,
	external_id: String,
}

#[Object]
/// # `chronicle:Namespace`
///
/// An IRI containing an external id and uuid part, used for disambiguation.
/// In order to work on the same namespace discrete Chronicle instances must share
/// the uuid part.
impl Namespace {
	async fn external_id(&self) -> &str {
		&self.external_id
	}

	async fn uuid(&self) -> &str {
		&self.uuid
	}
}

#[derive(Queryable, SimpleObject)]
/// # `Submission`
///
/// ## Fields
///
/// * `context` - the activity, agent, or entity to which the operation relates
///
/// * `submission_result` - result type of an operation
///
/// * `tx_id` - transaction id for a submitted operation; returns `null` if `submission_result`
/// is `SubmissionResult::AlreadyRecorded`
pub struct Submission {
	context: String,
	submission_result: SubmissionResult,
	tx_id: Option<String>,
}

#[derive(Enum, PartialEq, Eq, Clone, Copy)]
/// # `SubmissionResult` result types
///
/// ## Variants
///
/// * `Submission` - operation has been submitted
/// * `AlreadyRecorded` - operation will not result in data changes and has not been submitted
pub enum SubmissionResult {
	Submission,
	AlreadyRecorded,
}

impl Submission {
	pub fn from_submission(subject: &ChronicleIri, tx_id: &ChronicleTransactionId) -> Self {
		Submission {
			context: subject.to_string(),
			submission_result: SubmissionResult::Submission,
			tx_id: Some(tx_id.to_string()),
		}
	}

	pub fn from_already_recorded(subject: &ChronicleIri) -> Self {
		Submission {
			context: subject.to_string(),
			submission_result: SubmissionResult::AlreadyRecorded,
			tx_id: None,
		}
	}
}

/// # `TimelineOrder`
///
/// Specify the order in which multiple results of query data are returned
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
pub struct CommitIdentity {
	identity: String,
	signature: String,
	verifying_key: String,
}

impl From<SignedIdentity> for CommitIdentity {
	fn from(identity: SignedIdentity) -> Self {
		CommitIdentity {
			identity: identity.identity,
			signature: identity.signature.map(|x| hex::encode(&*x)).unwrap_or_default(),
			verifying_key: identity
				.verifying_key
				.map(|x| hex::encode(x.to_bytes()))
				.unwrap_or_default(),
		}
	}
}

#[derive(SimpleObject)]
pub struct CommitNotification {
	pub stage: Stage,
	pub tx_id: String,
	pub error: Option<String>,
	pub delta: Option<Delta>,
	pub id: Option<CommitIdentity>,
}

impl CommitNotification {
	pub fn from_submission(tx_id: &ChronicleTransactionId) -> Self {
		CommitNotification {
			stage: Stage::Submit,
			tx_id: tx_id.to_string(),
			error: None,
			delta: None,
			id: None,
		}
	}

	pub fn from_submission_failed(e: &SubmissionError) -> Self {
		CommitNotification {
			stage: Stage::Submit,
			tx_id: e.tx_id().to_string(),
			error: Some(e.to_string()),
			delta: None,
			id: None,
		}
	}

	pub fn from_contradiction(
		tx_id: &ChronicleTransactionId,
		contradiction: &str,
		id: SignedIdentity,
	) -> Self {
		CommitNotification {
			stage: Stage::Commit,
			tx_id: tx_id.to_string(),
			error: Some(contradiction.to_string()),
			delta: None,
			id: Some(id.into()),
		}
	}

	pub async fn from_committed(
		tx_id: &ChronicleTransactionId,
		delta: Box<ProvModel>,
		id: SignedIdentity,
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
			id: Some(id.into()),
		})
	}
}

pub struct Subscription;

#[Subscription]
/// GraphQL subscription[^note] to notify clients when a Chronicle operation has been sent to a
/// backend ledger and when that operation has been applied to both the ledger and Chronicle.
///
/// [^note](https://graphql.org/blog/subscriptions-in-graphql-and-relay/)
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
					Ok(SubmissionStage::Committed(commit, id)) => {
					  let notify = CommitNotification::from_committed(&commit.tx_id, commit.delta, *id).await;
					  if let Ok(notify) = notify {
						yield notify;
					  } else {
						error!("Failed to convert commit to notification: {:?}", notify.err());
					  }
					}
					Ok(SubmissionStage::NotCommitted((commit,contradiction, id))) =>
					  yield CommitNotification::from_contradiction(&commit, &contradiction.to_string(), *id),
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

#[handler]
async fn gql_playground() -> impl IntoResponse {
	Html(playground_source(GraphQLPlaygroundConfig::new("/").subscription_endpoint("/ws")))
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

#[derive(Clone)]
pub struct JwksUri {
	uri: Url,
}

impl JwksUri {
	pub fn new(uri: Url) -> Self {
		Self { uri }
	}

	// not ToString to prevent accidental disclosure of credentials
	pub fn full_uri(&self) -> String {
		self.uri.to_string()
	}
}

impl core::fmt::Debug for JwksUri {
	fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> Result<(), core::fmt::Error> {
		write!(
			fmt,
			r#"JwksUri {{ uri: Url {{ scheme: {:?}, cannot_be_a_base: {:?}, username: {:?}, password: ***SECRET***, host: {:?}, port: {:?}, path: {:?}, query: {:?}, fragment: {:?} }} }}"#,
			self.uri.scheme(),
			self.uri.cannot_be_a_base(),
			self.uri.username(),
			self.uri.host(),
			self.uri.port(),
			self.uri.path(),
			self.uri.query(),
			self.uri.fragment(),
		)?;
		Ok(())
	}
}

#[derive(Clone)]
pub struct UserInfoUri {
	uri: Url,
}

impl UserInfoUri {
	pub fn new(uri: Url) -> Self {
		Self { uri }
	}

	// not ToString to prevent accidental disclosure of credentials
	pub fn full_uri(&self) -> String {
		self.uri.to_string()
	}
}

impl core::fmt::Debug for UserInfoUri {
	fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> Result<(), core::fmt::Error> {
		write!(
			fmt,
			r#"UserInfoUri {{ uri: Url {{ scheme: {:?}, cannot_be_a_base: {:?}, username: {:?}, password: ***SECRET***, host: {:?}, port: {:?}, path: {:?}, query: {:?}, fragment: {:?} }} }}"#,
			self.uri.scheme(),
			self.uri.cannot_be_a_base(),
			self.uri.username(),
			self.uri.host(),
			self.uri.port(),
			self.uri.path(),
			self.uri.query(),
			self.uri.fragment(),
		)?;
		Ok(())
	}
}

pub struct SecurityConf {
	jwks_uri: Option<JwksUri>,
	userinfo_uri: Option<UserInfoUri>,
	id_claims: Option<BTreeSet<String>>,
	jwt_must_claim: HashMap<String, String>,
	allow_anonymous: bool,
	opa: ExecutorContext,
}

impl SecurityConf {
	pub fn new(
		jwks_uri: Option<JwksUri>,
		userinfo_uri: Option<UserInfoUri>,
		id_claims: Option<BTreeSet<String>>,
		jwt_must_claim: HashMap<String, String>,
		allow_anonymous: bool,
		opa: ExecutorContext,
	) -> Self {
		Self { jwks_uri, userinfo_uri, id_claims, jwt_must_claim, allow_anonymous, opa }
	}
}

#[async_trait::async_trait]
pub trait ChronicleApiServer {
	async fn serve_api(
		&self,
		pool: Pool<ConnectionManager<PgConnection>>,
		api: ApiDispatch,
		addresses: Vec<SocketAddr>,
		security_conf: SecurityConf,
		serve_graphql: bool,
		serve_data: bool,
	) -> Result<(), ApiError>;
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

#[instrument(level = "trace", ret(Debug))]
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

async fn check_claims(
	secconf: &EndpointSecurityConfiguration,
	req: &poem::Request,
) -> Result<Option<JwtClaims>, poem::Error> {
	if let Some(authorization) = req.header("Authorization") {
		if let Ok(authorization) = HeaderValue::from_str(authorization) {
			let bearer_token_maybe: Option<Bearer> = Credentials::decode(&authorization);
			if let Some(bearer_token) = bearer_token_maybe {
				if let Ok(claims) = secconf.checker.verify_token(bearer_token.token()).await {
					if check_required_claims(&secconf.must_claim, &claims) {
						return Ok(Some(JwtClaims(claims)));
					}
				}
			}
		}
		tracing::trace!("rejected authorization from {}: {:?}", req.remote_addr(), authorization);
		Err(poem::error::Error::from_string(
			"Authorization header present but without a satisfactory bearer token",
			StatusCode::UNAUTHORIZED,
		))
	} else if secconf.allow_anonymous {
		tracing::trace!("anonymous access from {}", req.remote_addr());
		Ok(None)
	} else {
		tracing::trace!("rejected anonymous access from {}", req.remote_addr());
		Err(poem::error::Error::from_string(
			"required Authorization header not present",
			StatusCode::UNAUTHORIZED,
		))
	}
}

async fn execute_opa_check(
	opa_executor: &ExecutorContext,
	claim_parser: &Option<AuthFromJwt>,
	claims: Option<&JwtClaims>,
	construct_data: impl FnOnce(&AuthId) -> OpaData,
) -> Result<(), OpaExecutorError> {
	// If unable to get an external_id from the JwtClaims or no claims found,
	// identity will be `Anonymous`
	let identity = match (claims, claim_parser) {
		(Some(claims), Some(parser)) => parser.identity(claims).unwrap_or(AuthId::anonymous()),
		_ => AuthId::anonymous(),
	};

	// Create OPA context data for the user identity
	let opa_data = construct_data(&identity);

	// Execute OPA check
	match opa_executor.evaluate(&identity, &opa_data).await {
		Err(error) => {
			tracing::warn!(
                        "{error}: attempt to violate policy rules by identity: {identity}, in context: {:#?}",
                        opa_data
                    );
			Err(error)
		},
		ok => ok,
	}
}

struct EndpointSecurityConfiguration {
	checker: TokenChecker,
	must_claim: HashMap<String, String>,
	allow_anonymous: bool,
}

impl EndpointSecurityConfiguration {
	fn new(
		checker: TokenChecker,
		must_claim: HashMap<String, String>,
		allow_anonymous: bool,
	) -> Self {
		Self { checker, must_claim, allow_anonymous }
	}

	async fn check_status(&self) -> Result<(), AuthorizationError> {
		self.checker.check_status().await
	}
}

struct QueryEndpoint<Q, M, S> {
	secconf: EndpointSecurityConfiguration,
	schema: Schema<Q, M, S>,
}

impl<Q, M, S> QueryEndpoint<Q, M, S>
where
	Q: ObjectType + 'static,
	M: ObjectType + 'static,
	S: SubscriptionType + 'static,
{
	#[instrument(level = "debug", skip_all, ret(Debug))]
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
impl<Q, M, S> Endpoint for QueryEndpoint<Q, M, S>
where
	Q: ObjectType + 'static,
	M: ObjectType + 'static,
	S: SubscriptionType + 'static,
{
	type Output = poem::Response;

	async fn call(&self, req: poem::Request) -> poem::Result<Self::Output> {
		let checked_claims = check_claims(&self.secconf, &req).await?;
		self.respond(req, |api_req| {
			if let Some(claims) = checked_claims {
				api_req.0.data(claims)
			} else {
				api_req.0
			}
		})
		.await
	}
}

struct SubscriptionEndpoint<Q, M, S> {
	secconf: EndpointSecurityConfiguration,
	schema: Schema<Q, M, S>,
}

impl<Q, M, S> SubscriptionEndpoint<Q, M, S>
where
	Q: ObjectType + 'static,
	M: ObjectType + 'static,
	S: SubscriptionType + 'static,
{
	#[instrument(level = "trace", skip(self, req), ret(Debug))]
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
				GraphQLWebSocket::new(stream, schema, protocol).with_data(data).serve()
			})
			.into_response())
	}
}

#[poem::async_trait]
impl<Q, M, S> Endpoint for SubscriptionEndpoint<Q, M, S>
where
	Q: ObjectType + 'static,
	M: ObjectType + 'static,
	S: SubscriptionType + 'static,
{
	type Output = poem::Response;

	async fn call(&self, req: poem::Request) -> poem::Result<Self::Output> {
		let checked_claims = check_claims(&self.secconf, &req).await?;
		self.respond(
			req,
			if let Some(claims) = checked_claims {
				let mut data = async_graphql::Data::default();
				data.insert(claims);
				data
			} else {
				async_graphql::Data::default()
			},
		)
		.await
	}
}

struct IriEndpoint {
	secconf: Option<EndpointSecurityConfiguration>,
	store: super::persistence::Store,
	opa_executor: ExecutorContext,
	claim_parser: Option<AuthFromJwt>,
}

impl IriEndpoint {
	async fn response_for_query<ID: Display + ExternalIdPart, X: ToJson>(
		&self,
		claims: Option<&JwtClaims>,
		prov_type: &str,
		id: &ID,
		ns: &ExternalId,
		retrieve: impl FnOnce(
			PooledConnection<ConnectionManager<PgConnection>>,
			&ID,
			&ExternalId,
		) -> Result<X, StoreError>,
	) -> poem::Result<poem::Response> {
		match execute_opa_check(&self.opa_executor, &self.claim_parser, claims, |identity| {
			OpaData::operation(
				identity,
				&json!("ReadData"),
				&json!({
						"type": prov_type,
						"id": id.external_id_part(),
						"namespace": ns
				}),
			)
		})
		.await
		{
			Ok(()) => match self.store.connection() {
				Ok(connection) => match retrieve(connection, id, ns) {
					Ok(data) => match data.to_json().compact().await {
						Ok(mut json) => {
							use serde_json::Value;
							if let Value::Object(mut map) = json {
								map.insert(
									"@context".to_string(),
									Value::String("/context".to_string()),
								);
								json = Value::Object(map);
							}
							Ok(IntoResponse::into_response(poem::web::Json(json)))
						},
						Err(error) => {
							tracing::error!("JSON failed compaction: {error}");
							Ok(poem::Response::builder()
								.status(StatusCode::INTERNAL_SERVER_ERROR)
								.body("failed to compact JSON response"))
						},
					},
					Err(StoreError::Db(diesel::result::Error::NotFound))
					| Err(StoreError::RecordNotFound) => {
						tracing::debug!("not found: {prov_type} {} in {ns}", id.external_id_part());
						Ok(poem::Response::builder()
							.status(StatusCode::NOT_FOUND)
							.body(format!("the specified {prov_type} does not exist")))
					},
					Err(error) => {
						tracing::error!("failed to retrieve from database: {error}");
						Ok(poem::Response::builder()
							.status(StatusCode::INTERNAL_SERVER_ERROR)
							.body("failed to fetch from backend storage"))
					},
				},
				Err(error) => {
					tracing::error!("failed to connect to database: {error}");
					Ok(poem::Response::builder()
						.status(StatusCode::INTERNAL_SERVER_ERROR)
						.body("failed to access backend storage"))
				},
			},
			Err(_) => Ok(poem::Response::builder()
				.status(StatusCode::FORBIDDEN)
				.body("violation of policy rules")),
		}
	}

	async fn parse_ns_iri_from_uri_path(
		&self,
		req: poem::Request,
	) -> poem::Result<Result<(ExternalId, ChronicleIri), poem::Response>> {
		use poem::{web::Path, FromRequest, Response};

		#[derive(Clone, Debug, Serialize, Deserialize)]
		struct NamespacedIri {
			ns: String,
			iri: String,
		}

		#[derive(Clone, Debug, Serialize, Deserialize)]
		struct Iri {
			iri: String,
		}

		impl From<Iri> for NamespacedIri {
			fn from(value: Iri) -> Self {
				NamespacedIri { ns: "default".to_string(), iri: value.iri }
			}
		}

		let (req, mut body) = req.split();

		let ns_iri: poem::Result<Path<NamespacedIri>> =
			FromRequest::from_request(&req, &mut body).await;

		let ns_iri: NamespacedIri = match ns_iri {
			Ok(Path(nsi)) => nsi,
			Err(_) => {
				let path: Path<Iri> = FromRequest::from_request(&req, &mut body).await?;
				path.0.into()
			},
		};

		match ChronicleIri::from_str(&ns_iri.iri) {
			Ok(iri) => Ok(Ok((ns_iri.ns.into(), iri))),
			Err(error) => {
				Ok(Err(Response::builder().status(StatusCode::NOT_FOUND).body(error.to_string())))
			},
		}
	}

	#[instrument(level = "trace", skip(self, req), ret(Debug))]
	async fn respond(
		&self,
		req: poem::Request,
		claims: Option<&JwtClaims>,
	) -> poem::Result<poem::Response> {
		match self.parse_ns_iri_from_uri_path(req).await? {
			Ok((ns, ChronicleIri::Activity(id))) => {
				self.response_for_query(claims, "activity", &id, &ns, |mut conn, id, ns| {
					self.store.prov_model_for_activity_id(&mut conn, id, ns)
				})
				.await
			},
			Ok((ns, ChronicleIri::Agent(id))) => {
				self.response_for_query(claims, "agent", &id, &ns, |mut conn, id, ns| {
					self.store.prov_model_for_agent_id(&mut conn, id, ns)
				})
				.await
			},
			Ok((ns, ChronicleIri::Entity(id))) => {
				self.response_for_query(claims, "entity", &id, &ns, |mut conn, id, ns| {
					self.store.prov_model_for_entity_id(&mut conn, id, ns)
				})
				.await
			},
			Ok(_) => Ok(poem::Response::builder()
				.status(StatusCode::NOT_FOUND)
				.body("may query only: activity, agent, entity")),
			Err(rsp) => Ok(rsp),
		}
	}
}

#[poem::async_trait]
impl Endpoint for IriEndpoint {
	type Output = poem::Response;

	async fn call(&self, req: poem::Request) -> poem::Result<Self::Output> {
		let checked_claims = if let Some(secconf) = &self.secconf {
			check_claims(secconf, &req).await?
		} else {
			None
		};
		self.respond(req, checked_claims.as_ref()).await
	}
}

struct LdContextEndpoint;

#[poem::async_trait]
impl Endpoint for LdContextEndpoint {
	type Output = poem::Response;

	async fn call(&self, _req: poem::Request) -> poem::Result<Self::Output> {
		let context: &serde_json::Value = &common::context::PROV;
		Ok(IntoResponse::into_response(poem::web::Json(context)))
	}
}

#[derive(Clone, Debug)]
pub struct AuthFromJwt {
	id_claims: BTreeSet<String>,
	allow_anonymous: bool,
}

impl AuthFromJwt {
	#[instrument(level = "debug", ret(Debug))]
	fn identity(&self, claims: &JwtClaims) -> Result<AuthId, IdentityError> {
		AuthId::from_jwt_claims(claims, &self.id_claims)
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
			match self.identity(claims) {
				Ok(chronicle_id) => request = request.data(chronicle_id),
				Err(error) if self.allow_anonymous => {
					debug!("Identity could not be determined: {:?}", error)
				},
				Err(error) => {
					warn!(
						"Rejecting request because required identity could not be determined: {:?}",
						error
					);
					return Err(ServerError::new("Authorization header present but identity could not be determined from bearer token", None));
				},
			}
		}
		next.run(ctx, request).await
	}
}

#[async_trait::async_trait]
impl async_graphql::extensions::ExtensionFactory for AuthFromJwt {
	fn create(&self) -> Arc<dyn async_graphql::extensions::Extension> {
		Arc::new(AuthFromJwt {
			id_claims: self.id_claims.clone(),
			allow_anonymous: self.allow_anonymous,
		})
	}
}

#[derive(Clone, Debug)]
pub struct OpaCheck {
	pub claim_parser: Option<AuthFromJwt>,
}

#[async_trait::async_trait]
impl async_graphql::extensions::Extension for OpaCheck {
	#[instrument(level = "trace", skip_all, ret(Debug))]
	async fn resolve(
		&self,
		ctx: &async_graphql::extensions::ExtensionContext<'_>,
		info: async_graphql::extensions::ResolveInfo<'_>,
		next: async_graphql::extensions::NextResolve<'_>,
	) -> async_graphql::ServerResult<Option<async_graphql::Value>> {
		use async_graphql::ServerError;
		use serde_json::Value;
		if let Some(opa_executor) = ctx.data_opt::<ExecutorContext>() {
			match execute_opa_check(
				opa_executor,
				&self.claim_parser,
				ctx.data_opt::<JwtClaims>(),
				|identity| {
					OpaData::graphql(
						identity,
						&Value::String(info.parent_type.to_string()),
						&Value::Array(
							info.path_node.to_string_vec().into_iter().map(Value::String).collect(),
						),
					)
				},
			)
			.await
			{
				Ok(()) => next.run(ctx, info).await,
				Err(_) => Err(ServerError::new("violation of policy rules", None)),
			}
		} else {
			Err(ServerError::new("cannot check policy rules", None))
		}
	}
}

#[async_trait::async_trait]
impl async_graphql::extensions::ExtensionFactory for OpaCheck {
	fn create(&self) -> Arc<dyn async_graphql::extensions::Extension> {
		Arc::new(OpaCheck { claim_parser: self.claim_parser.clone() })
	}
}

lazy_static! {
	static ref SHUTDOWN_SIGNAL: Arc<Semaphore> = Arc::new(Semaphore::new(0));
}

fn trigger_shutdown() {
	SHUTDOWN_SIGNAL.add_permits(1);
}

async fn await_shutdown() {
	let _permit = SHUTDOWN_SIGNAL.acquire().await.unwrap();
}

#[async_trait::async_trait]
impl<Query, Mutation> ChronicleApiServer for ChronicleGraphQl<Query, Mutation>
where
	Query: ObjectType + Copy,
	Mutation: ObjectType + Copy,
{
	async fn serve_api(
		&self,
		pool: Pool<ConnectionManager<PgConnection>>,
		api: ApiDispatch,
		addresses: Vec<SocketAddr>,
		sec: SecurityConf,
		serve_graphql: bool,
		serve_data: bool,
	) -> Result<(), ApiError> {
		let claim_parser = sec
			.id_claims
			.map(|id_claims| AuthFromJwt { id_claims, allow_anonymous: sec.allow_anonymous });
		let mut schema = Schema::build(self.query, self.mutation, Subscription)
			.extension(OpenTelemetry::new(opentelemetry::global::tracer("chronicle-api-gql")))
			.extension(OpaCheck { claim_parser: claim_parser.clone() });
		if let Some(claim_parser) = &claim_parser {
			schema = schema.extension(claim_parser.clone());
		}
		let schema = schema
			.data(Store::new(pool.clone()))
			.data(api)
			.data(sec.opa.clone())
			.data(AuthId::anonymous())
			.finish();

		let iri_endpoint = |secconf| IriEndpoint {
			secconf,
			store: super::persistence::Store::new(pool.clone()).unwrap(),
			opa_executor: sec.opa.clone(),
			claim_parser: claim_parser.clone(),
		};

		let mut app = Route::new();

		match (&sec.jwks_uri, &sec.userinfo_uri) {
			(None, None) => {
				tracing::warn!("API endpoint uses no authentication");

				if serve_graphql {
					app = app
						.at("/", get(gql_playground).post(GraphQL::new(schema.clone())))
						.at("/ws", get(GraphQLSubscription::new(schema)))
				};
				if serve_data {
					app = app
						.at("/context", get(LdContextEndpoint))
						.at("/data/:iri", get(iri_endpoint(None)))
						.at("/data/:ns/:iri", get(iri_endpoint(None)))
				};
			},
			(jwks_uri, userinfo_uri) => {
				const CACHE_EXPIRY_SECONDS: u32 = 100;
				if let Some(uri) = jwks_uri {
					tracing::debug!(oidc_jwks_endpoint = ?uri);
				}
				if let Some(uri) = userinfo_uri {
					tracing::debug!(oidc_userinfo_endpoint = ?uri);
				}

				let secconf = || {
					EndpointSecurityConfiguration::new(
						TokenChecker::new(
							jwks_uri.as_ref(),
							userinfo_uri.as_ref(),
							CACHE_EXPIRY_SECONDS,
						),
						sec.jwt_must_claim.clone(),
						sec.allow_anonymous,
					)
				};

				secconf().check_status().await?;

				if serve_graphql {
					app = app
						.at("/", post(QueryEndpoint { secconf: secconf(), schema: schema.clone() }))
						.at("/ws", get(SubscriptionEndpoint { secconf: secconf(), schema }))
				};
				if serve_data {
					app = app
						.at("/context", get(LdContextEndpoint))
						.at("/data/:iri", get(iri_endpoint(Some(secconf()))))
						.at("/data/:ns/:iri", get(iri_endpoint(Some(secconf()))))
				};
			},
		}

		let listener = addresses
			.into_iter()
			.map(|address| TcpListener::bind(address).boxed())
			.reduce(|listener_1, listener_2| listener_1.combine(listener_2).boxed())
			.unwrap();

		Server::new(listener)
			.run_with_graceful_shutdown(app, await_shutdown(), None)
			.await?;

		Ok(())
	}
}
