use diesel::{r2d2::ConnectionManager, Connection, PgConnection};
use lazy_static::lazy_static;
use r2d2::Pool;
use std::{fmt::Display, time::Duration};
use testcontainers::{clients::Cli, images::postgres::Postgres, Container};

lazy_static! {
    static ref CLIENT: Cli = Cli::default();
}

pub struct TemporaryDatabase<'a> {
    db_uris: Vec<String>,
    _container: Container<'a, Postgres>,
}

impl<'a> TemporaryDatabase<'a> {
    pub fn connection_pool(&self) -> Result<Pool<ConnectionManager<PgConnection>>, r2d2::Error> {
        let db_uri = self
            .db_uris
            .iter()
            .find(|db_uri| PgConnection::establish(db_uri).is_ok())
            .expect("cannot establish connection");
        Pool::builder().build(ConnectionManager::<PgConnection>::new(db_uri))
    }
}

impl<'a> Default for TemporaryDatabase<'a> {
    fn default() -> Self {
        let container = CLIENT.run(Postgres::default());
        const PORT: u16 = 5432;
        Self {
            db_uris: vec![
                format!(
                    "postgresql://postgres@127.0.0.1:{}/",
                    container.get_host_port_ipv4(PORT)
                ),
                format!(
                    "postgresql://postgres@{}:{}/",
                    container.get_bridge_ip_address(),
                    PORT
                ),
            ],
            _container: container,
        }
    }
}

#[async_trait::async_trait]
pub trait DatabaseConnector<X, E> {
    async fn try_connect(&self) -> Result<(X, Pool<ConnectionManager<PgConnection>>), E>;
    fn should_retry(&self, error: &E) -> bool;
}

pub async fn get_connection_with_retry<X, E: Display>(
    connector: impl DatabaseConnector<X, E>,
) -> Result<(X, Pool<ConnectionManager<PgConnection>>), E> {
    let mut i = 1;
    let mut j = 1;
    loop {
        let connection = connector.try_connect().await;
        if let Err(source) = &connection {
            tracing::warn!("database connection failed: {source}");
            if i < 20 && connector.should_retry(source) {
                tracing::info!("waiting to retry database connection...");
                std::thread::sleep(Duration::from_secs(i));
                (i, j) = (i + j, i);
                continue;
            }
        }
        return connection;
    }
}
