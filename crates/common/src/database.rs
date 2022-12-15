use diesel::{r2d2::ConnectionManager, PgConnection};
use pg_embed::{
    self,
    pg_enums::{Architecture, OperationSystem, PgAuthMethod},
    pg_fetch::PgFetchSettings,
    pg_types::PgResult,
    postgres::{self, PgEmbed},
};
use r2d2::Pool;
use std::time::Duration;
use temp_dir::TempDir;
use uuid::Uuid;

pub struct Database {
    _embedded: PgEmbed,
    _location: TempDir,
}

#[cfg(target_os = "macos")]
pub fn pg_fetch_settings() -> PgFetchSettings {
    use pg_embed::pg_fetch::PG_V13;

    PgFetchSettings {
        host: "https://repo1.maven.org".to_string(),
        operating_system: OperationSystem::default(),
        architecture: Architecture::Amd64,
        version: PG_V13,
    }
}

#[cfg(target_os = "linux")]
pub fn pg_fetch_settings() -> PgFetchSettings {
    PgFetchSettings::default()
}

pub async fn get_embedded_db_connection(
) -> PgResult<(Database, Pool<ConnectionManager<PgConnection>>)> {
    let temp_dir = TempDir::new().unwrap();
    let settings = postgres::PgSettings {
        database_dir: temp_dir.path().to_path_buf(),
        port: portpicker::pick_unused_port().unwrap() as i16,
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
