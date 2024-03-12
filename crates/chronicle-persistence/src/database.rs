use diesel::{r2d2::ConnectionManager, PgConnection};

use diesel::r2d2::Pool;
use std::{fmt::Display, time::Duration};

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
