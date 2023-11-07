mod mockchain;
mod stubstrate;
use crate::substitutes::stubstrate::Stubstrate;
use api::{
	commands::{ApiCommand, ApiResponse},
	Api, ApiDispatch, ApiError, UuidGen,
};
use common::{
	identity::AuthId,
	prov::{ChronicleTransactionId, ProvModel},
};

use uuid::Uuid;

use chronicle_signing::{
	chronicle_secret_names, ChronicleSecretsOptions, ChronicleSigning, BATCHER_NAMESPACE,
	CHRONICLE_NAMESPACE,
};

use diesel::{r2d2::ConnectionManager, Connection, PgConnection};
use r2d2::Pool;
use testcontainers::{images::postgres::Postgres, Container};

use lazy_static::lazy_static;
use testcontainers::clients;

lazy_static! {
	static ref CLIENT: clients::Cli = clients::Cli::default();
}

pub struct TemporaryDatabase<'a> {
	db_uris: Vec<String>,
	container: Container<'a, Postgres>,
}

impl<'a> Drop for TemporaryDatabase<'a> {
	#[tracing::instrument(skip(self))]
	fn drop(&mut self) {
		self.container.stop();
	}
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
				format!("postgresql://postgres@127.0.0.1:{}/", container.get_host_port_ipv4(PORT)),
				format!("postgresql://postgres@{}:{}/", container.get_bridge_ip_address(), PORT),
			],
			container,
		}
	}
}

pub struct TestDispatch<'a> {
	api: ApiDispatch,
	_db: TemporaryDatabase<'a>,
	_substrate: Stubstrate,
}

impl<'a> TestDispatch<'a> {
	pub async fn dispatch(
		&mut self,
		command: ApiCommand,
		identity: AuthId,
	) -> Result<Option<(Box<ProvModel>, ChronicleTransactionId)>, ApiError> {
		// We can sort of get final on chain state here by using a map of subject to model
		match self.api.dispatch(command, identity).await? {
			ApiResponse::Submission { .. } | ApiResponse::ImportSubmitted { .. } => {
				// Recv until we get a commit notification
				loop {
					let commit = self.api.notify_commit.subscribe().recv().await.unwrap();
					match commit {
						common::ledger::SubmissionStage::Submitted(Ok(_)) => continue,
						common::ledger::SubmissionStage::Committed(commit, _id) =>
							return Ok(Some((commit.delta, commit.tx_id))),
						common::ledger::SubmissionStage::Submitted(Err(e)) => panic!("{e:?}"),
						common::ledger::SubmissionStage::NotCommitted((_, tx, _id)) => {
							panic!("{tx:?}")
						},
					}
				}
			},
			ApiResponse::AlreadyRecorded { subject: _, prov } =>
				Ok(Some((prov, ChronicleTransactionId::default()))),
			_ => Ok(None),
		}
	}
}

#[derive(Debug, Clone)]
struct SameUuid;

impl UuidGen for SameUuid {
	fn uuid() -> Uuid {
		Uuid::parse_str("5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea").unwrap()
	}
}

pub async fn embed_substrate() -> Stubstrate {
	stubstrate::Stubstrate::new()
}

pub async fn test_api<'a>() -> TestDispatch<'a> {
	chronicle_telemetry::telemetry(None, chronicle_telemetry::ConsoleLogging::Pretty);

	let secrets = ChronicleSigning::new(
		chronicle_secret_names(),
		vec![
			(CHRONICLE_NAMESPACE.to_string(), ChronicleSecretsOptions::generate_in_memory()),
			(BATCHER_NAMESPACE.to_string(), ChronicleSecretsOptions::generate_in_memory()),
		],
	)
	.await
	.unwrap();

	let embed_substrate = embed_substrate().await;
	let database = TemporaryDatabase::default();
	let pool = database.connection_pool().unwrap();

	let liveness_check_interval = None;

	let dispatch = Api::new(
		pool,
		embed_substrate.clone(),
		SameUuid,
		secrets,
		vec![],
		None,
		liveness_check_interval,
	)
	.await
	.unwrap();

	TestDispatch {
		api: dispatch,
		_db: database, // share the lifetime
		_substrate: embed_substrate,
	}
}
