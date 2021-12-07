use std::{
    convert::Infallible,
    net::SocketAddr,
    time::Duration,
};

use async_graphql::{
    http::{playground_source, GraphQLPlaygroundConfig},
    Context, EmptyMutation, EmptySubscription, Error, ErrorExtensions, Object, Schema, ID,
};
use async_graphql_warp::{graphql_subscription, GraphQLBadRequest, GraphQLResponse};
use chrono::{DateTime, Utc};
use custom_error::custom_error;
use derivative::*;
use diesel::{
    prelude::*,
    r2d2::{Pool},
};
use diesel::{r2d2::ConnectionManager, Connection, Queryable, SqliteConnection};
use tracing::{debug, instrument};
use warp::{
    hyper::{Response, StatusCode},
    Filter, Rejection,
};

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
    pub namespace: ID,
    pub name: ID,
    pub domaintypeid: Option<ID>,
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
}

#[Object]
impl Entity {
    async fn name(&self) -> &ID {
        match self {
            Entity::Signed { name, .. } | Entity::Unsigned { name, .. } => name,
        }
    }
}

#[derive(Default)]
pub struct QueryRoot;

custom_error! {pub GraphQlError
    Db{source: diesel::result::Error}                           = "Database operation failed",
    DbConnection{source: diesel::ConnectionError}               = "Database connection failed",
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
    pub fn new(
        connection: Pool<ConnectionManager<SqliteConnection>>,
    ) -> Result<Self, GraphQlError> {
        Ok(Store { pool: connection })
    }
}

#[Object]
impl QueryRoot {
    async fn agent<'a>(
        &self,
        ctx: &Context<'a>,
        name: String,
        namespace: String,
    ) -> async_graphql::Result<Option<Agent>> {
        use crate::persistence::schema::agent::{self, dsl};

        let store = ctx.data_unchecked::<Store>();

        let mut connection = store.pool.get().unwrap();

        Ok(agent::table
            .filter(dsl::name.eq(name).and(dsl::namespace.eq(namespace)))
            .first::<Agent>(&mut connection)
            .optional()?)
    }
}

#[instrument]
pub async fn serve_graphql(
    pool: Pool<ConnectionManager<SqliteConnection>>,
    address: SocketAddr,
    open: bool,
) -> Result<(), GraphQlError> {
    let schema = Schema::build(QueryRoot, EmptyMutation, EmptySubscription)
        .data(Store::new(pool.clone())?)
        .finish();

    let graphql_post = async_graphql_warp::graphql(schema.clone()).and_then(
        |(schema, request): (
            Schema<QueryRoot, EmptyMutation, EmptySubscription>,
            async_graphql::Request,
        )| async move {
            Ok::<_, Infallible>(GraphQLResponse::from(schema.execute(request).await))
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
    Ok(())
}
