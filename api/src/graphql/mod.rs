use std::{convert::Infallible, net::SocketAddr, time::Duration};

use async_graphql::{
    extensions::Tracing,
    http::{playground_source, GraphQLPlaygroundConfig},
    Context, Error, ErrorExtensions, Object, Schema, Subscription, ID,
};
use async_graphql_extension_apollo_tracing::{ApolloTracing, ApolloTracingDataExt, HTTPMethod};
use async_graphql_warp::{graphql_subscription, GraphQLBadRequest, GraphQLResponse};
use chrono::{DateTime, Utc};
use common::{
    commands::{AgentCommand, ApiCommand},
    prov::vocab::Chronicle,
};
use custom_error::custom_error;
use derivative::*;
use diesel::{prelude::*, r2d2::Pool};
use diesel::{r2d2::ConnectionManager, Connection, Queryable, SqliteConnection};
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

#[derive(Default)]
pub struct Activity {
    pub id: i32,
    pub namespace: String,
    pub name: String,
    pub domaintypeid: Option<String>,
    pub started: Option<DateTime<Utc>>,
    pub ended: Option<DateTime<Utc>>,
}

pub enum Entity {
    Unsigned {
        id: i32,
        namespaceid: ID,
        name: ID,
        domaintypeid: Option<ID>,
    },
    Signed {
        id: i32,
        namespaceid: ID,
        name: ID,
        domaintypeid: Option<ID>,
        signature: String,
        locator: Option<String>,
        signature_time: DateTime<Utc>,
    },
}

/*
    pub was_associated_with: HashMap<(NamespaceId, ActivityId), HashSet<(NamespaceId, AgentId)>>,
    pub was_attributed_to: HashMap<(NamespaceId, EntityId), HashSet<(NamespaceId, AgentId)>>,
    pub was_generated_by: HashMap<(NamespaceId, EntityId), HashSet<(NamespaceId, ActivityId)>>,
    pub used: HashMap<(NamespaceId, ActivityId), HashSet<(NamespaceId, EntityId)>>,
*/

#[Object]
impl Agent {
    async fn namespace(&self) -> &str {
        &self.namespace
    }

    async fn name(&self) -> &str {
        &self.namespace
    }
}

#[Object]
impl Activity {
    async fn namespace(&self) -> &str {
        &self.namespace
    }

    async fn name(&self) -> &str {
        &self.namespace
    }

    async fn was_associated_with<'a>(&self, _ctx: &Context<'a>) -> Vec<Agent> {
        todo!()
    }
}

#[Object]
impl Entity {
    async fn name(&self) -> &ID {
        match self {
            Entity::Signed { name, .. } | Entity::Unsigned { name, .. } => name,
        }
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
        _types: Vec<String>,
        _from_inclusive: Option<DateTime<Utc>>,
        _end_exclusive: Option<DateTime<Utc>>,
    ) -> async_graphql::Result<Vec<Activity>> {
        let store = ctx.data_unchecked::<Store>();

        let _connection = store.pool.get()?;

        Ok(vec![])
    }
}

struct Mutation;

#[Object]
impl Mutation {
    pub async fn create_agent<'a>(
        &self,
        ctx: &Context<'a>,
        name: String,
        namespace: Option<String>,
        typ: String,
    ) -> async_graphql::Result<String> {
        let api = ctx.data_unchecked::<ApiDispatch>();

        let id = Chronicle::agent(&name);

        let namespace = namespace.unwrap_or("default".to_owned());

        let _res = api
            .dispatch(ApiCommand::Agent(AgentCommand::Create {
                name,
                namespace,
                domaintype: Some(typ),
            }))
            .await;

        Ok(id.to_string())
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
