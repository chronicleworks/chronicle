use diesel::{r2d2::ConnectionManager, PgConnection};
use pg_embed::{
    self,
    pg_enums::PgAuthMethod,
    pg_fetch::PgFetchSettings,
    pg_types::PgResult,
    postgres::{self, PgEmbed},
};
use r2d2::Pool;
use std::time::Duration;
use tempfile::TempDir;
use uuid::Uuid;

pub async fn get_embedded_db_connection(
) -> PgResult<(PgEmbed, Pool<ConnectionManager<PgConnection>>)> {
    let settings = postgres::PgSettings {
        database_dir: TempDir::new().unwrap().into_path(),
        port: portpicker::pick_unused_port().unwrap() as i16,
        user: "chronicle".to_string(),
        password: "please".to_string(),
        auth_method: PgAuthMethod::MD5,
        persistent: false,
        timeout: Some(Duration::from_secs(50)),
        migration_dir: None,
    };
    let mut database = PgEmbed::new(settings, PgFetchSettings::default()).await?;
    database.setup().await?;
    database.start_db().await?;
    let db_name = format!("chronicle-{}", Uuid::new_v4());
    database.create_database(db_name.as_str()).await?;
    let db_uri = database.full_db_uri(&db_name);
    let pool = Pool::builder()
        .build(ConnectionManager::<PgConnection>::new(db_uri))
        .unwrap();
    Ok((database, pool))
}
