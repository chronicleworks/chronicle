use pg_embed::{
    pg_enums::{Architecture, PgAuthMethod},
    pg_fetch::{PgFetchSettings, PG_V13},
    postgres::*,
};
use resolve_path::PathResolveExt;
use std::path::PathBuf;
use std::time::Duration;
use tracing::info;
use url::Url;

pub struct EmbeddedPostgres {
    pub address: Url,
    _pg: pg_embed::postgres::PgEmbed,
}

// Macos M1 has no pg image, but can run pg under rosetta, arm64 linux cannot
// currently, but has an image
#[tracing(ret(Info))]
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

impl EmbeddedPostgres {
    pub async fn new(config_file: &str) -> Self {
        let port = portpicker::pick_unused_port().expect("No free port");

        let mut database_dir = PathBuf::from(config_file);

        database_dir = database_dir
            .try_resolve()
            .expect("Invalid config path")
            .to_path_buf();

        database_dir.pop();
        database_dir.push("store/");

        std::fs::create_dir_all(&database_dir).expect("Failed to create database directory");

        let pg_settings = PgSettings {
            // Where to store the postgresql database
            database_dir,
            port: port as _,
            user: "postgres".to_string(),
            password: "postgres".to_string(),
            auth_method: PgAuthMethod::Plain,
            persistent: true,
            timeout: Some(Duration::from_secs(15)),
            migration_dir: None,
        };

        let fetch_settings = PgFetchSettings {
            version: PG_V13,
            architecture: arch(),
            ..Default::default()
        };

        let mut pg = PgEmbed::new(pg_settings, fetch_settings).await.unwrap();

        info!("Starting embedded postgresql");
        pg.setup().await.unwrap();
        info!("Embedded postgresql started");
        pg.init_db().await.unwrap();
        info!("Embedded postgresql initialised");
        pg.start_db().await.unwrap();

        if !pg.database_exists("chronicle").await.unwrap() {
            pg.create_database("chronicle").await.unwrap();
        }

        Self {
            address: Url::parse(&*pg.full_db_uri("chronicle")).unwrap(),
            _pg: pg,
        }
    }
}
