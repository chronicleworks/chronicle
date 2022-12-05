use diesel::{r2d2::ConnectionManager, PgConnection};
use pg_embed::{
    self,
    pg_enums::PgAuthMethod,
    pg_types::PgResult,
    postgres::{self, PgEmbed},
};
use r2d2::Pool;
use std::time::Duration;
use temp_dir::TempDir;

pub struct Database {
    _embedded: PgEmbed,
    _location: TempDir,
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
    panic!("database directory path is {:?}", settings.database_dir);
}
