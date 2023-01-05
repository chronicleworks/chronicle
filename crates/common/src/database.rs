use diesel::{r2d2::ConnectionManager, PgConnection};
use lazy_static::lazy_static;
use pg_embed::{
    self,
    pg_enums::{Architecture, OperationSystem, PgAuthMethod},
    pg_fetch::{PgFetchSettings, PG_V13},
    pg_types::PgResult,
    postgres::{self, PgEmbed},
};
use r2d2::Pool;
use std::time::Duration;
use temp_dir::TempDir;
use tokio::sync::Mutex;
use uuid::Uuid;

pub struct Database {
    _embedded: PgEmbed,
    _location: TempDir,
}

lazy_static! {
    static ref TEMP_DIRS: Mutex<Vec<TempDir>> = Mutex::new(Vec::new());
}

fn arch() -> Architecture {
    if cfg!(target_os = "macos") {
        Architecture::Amd64
    } else if cfg!(target_arch = "aarch64") {
        Architecture::Arm64v8
    } else if cfg!(target_arch = "x86_64") {
        Architecture::Amd64
    } else if cfg!(target_arch = "x86") {
        Architecture::I386
    } else {
        panic!("Unsupported architecture");
    }
}

pub fn pg_fetch_settings() -> PgFetchSettings {
    PgFetchSettings {
        host: "https://repo1.maven.org".to_string(),
        operating_system: OperationSystem::default(),
        architecture: arch(),
        version: PG_V13,
    }
}

pub async fn get_embedded_db_connection(
) -> PgResult<(Database, Pool<ConnectionManager<PgConnection>>)> {
    let temp_dir = TempDir::new().unwrap();
    TEMP_DIRS.lock().await.push(temp_dir.clone());
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
