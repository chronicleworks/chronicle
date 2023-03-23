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

async fn get_embedded_db_connection_one_try(
) -> PgResult<(Database, Pool<ConnectionManager<PgConnection>>)> {
    let temp_dir = TempDir::new().unwrap();
    TEMP_DIRS.lock().await.push(temp_dir.clone());
    let settings = postgres::PgSettings {
        database_dir: temp_dir.path().to_path_buf(),
        port: portpicker::pick_unused_port().unwrap(),
        user: "chronicle".to_string(),
        password: "please".to_string(),
        auth_method: PgAuthMethod::MD5,
        persistent: false,
        timeout: Some(Duration::from_secs(50)),
        migration_dir: None,
    };

    let mut database = PgEmbed::new(settings, pg_fetch_settings()).await?;
    database.setup().await?;
    database.start_db().await?;
    let db_name = format!("chronicle-{}", Uuid::new_v4());
    database.create_database(db_name.as_str()).await?;
    let db_uri = database.full_db_uri(&db_name);
    let pool = Pool::builder()
        .build(ConnectionManager::<PgConnection>::new(db_uri))
        .unwrap();
    Ok((
        Database {
            _embedded: database,
            _location: temp_dir,
        },
        pool,
    ))
}

pub async fn get_embedded_db_connection(
) -> PgResult<(Database, Pool<ConnectionManager<PgConnection>>)> {
    get_connection_with_retry(EmbeddedDatabaseConnector).await
}

pub struct EmbeddedDatabaseConnector;

#[async_trait::async_trait]
impl DatabaseConnector<Database, PgEmbedError> for EmbeddedDatabaseConnector {
    async fn try_connect(
        &self,
    ) -> Result<(Database, Pool<ConnectionManager<PgConnection>>), PgEmbedError> {
        get_embedded_db_connection_one_try().await
    }

    fn should_retry(&self, error: &PgEmbedError) -> bool {
        vec![
            PgEmbedErrorType::PgCleanUpFailure,
            PgEmbedErrorType::PgStartFailure,
            PgEmbedErrorType::UnpackFailure,
        ]
        .contains(&error.error_type)
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
