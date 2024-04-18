mod cli;
pub mod opa;
use api::{
	chronicle_graphql::{
		ChronicleApiServer, ChronicleGraphQl, EndpointSecurityConfiguration, JwksUri, SecurityConf,
		UserInfoUri,
	},
	commands::ApiResponse,
	Api, ApiDispatch, ApiError, StoreError, UuidGen,
};
use async_graphql::ObjectType;
use chronicle_persistence::database::{get_connection_with_retry, DatabaseConnector};
use common::{
	opa::{
		std::{load_bytes_from_stdin, load_bytes_from_url},
		PolicyAddress,
	},
	prov::json_ld::ToJson,
};
#[cfg(feature = "devmode")]
use embedded_substrate::EmbeddedSubstrate;
use futures::{Future, FutureExt, StreamExt};
#[cfg(not(feature = "devmode"))]
use protocol_substrate_chronicle::ChronicleSubstrateClient;

use chronicle_signing::{
	chronicle_secret_names, ChronicleSecretsOptions, ChronicleSigning, BATCHER_NAMESPACE,
	CHRONICLE_NAMESPACE,
};
use clap::{ArgMatches, Command};
use clap_complete::{generate, Generator, Shell};
pub use cli::*;
use common::{
	identity::AuthId,
	ledger::SubmissionStage,
	opa::{std::ExecutorContext, OpaSettings},
	prov::{operations::ChronicleOperation, NamespaceId},
};

use std::io::IsTerminal;
use tracing::{debug, error, info, instrument, warn};
use user_error::UFE;

use diesel::{
	r2d2::{ConnectionManager, Pool},
	PgConnection,
};

use chronicle_telemetry::{self, ConsoleLogging};
use url::Url;

use std::{
	collections::{BTreeSet, HashMap},
	io::{self},
	net::{SocketAddr, ToSocketAddrs},
	str::FromStr,
};

use crate::codegen::ChronicleDomainDef;

use self::opa::opa_executor_from_embedded_policy;

#[cfg(not(feature = "devmode"))]
fn validator_address(options: &ArgMatches) -> Result<Vec<SocketAddr>, CliError> {
	Ok(options
		.value_of("validator")
		.map(str::to_string)
		.ok_or(CliError::MissingArgument { arg: "validator".to_owned() })
		.and_then(|s| Url::parse(&s).map_err(CliError::from))
		.map(|u| u.socket_addrs(|| Some(4004)))
		.map_err(CliError::from)??)
}

#[allow(dead_code)]
#[cfg(not(feature = "devmode"))]
async fn ledger(
	options: &ArgMatches,
) -> Result<ChronicleSubstrateClient<protocol_substrate::PolkadotConfig>, CliError> {
	let url = options
		.value_of("validator")
		.map(str::to_string)
		.ok_or_else(|| CliError::MissingArgument { arg: "validator".to_owned() })?;

	let url = Url::parse(&url).map_err(CliError::from)?;

	let addrs = url.socket_addrs(|| Some(9944)).map_err(CliError::from)?;

	let client = ChronicleSubstrateClient::<protocol_substrate::PolkadotConfig>::connect(
		addrs[0].to_string(),
	)
	.await?;

	Ok(client)
}

#[allow(dead_code)]
#[cfg(feature = "devmode")]
async fn in_mem_ledger(
	_options: &ArgMatches,
) -> Result<std::sync::Arc<EmbeddedSubstrate>, ApiError> {
	embedded_substrate::shared_dev_node_rpc_on_arbitrary_port()
		.await
		.map_err(|e| ApiError::EmbeddedSubstrate(e.into()))
}

#[derive(Debug, Clone)]
struct UniqueUuid;

impl UuidGen for UniqueUuid {}

type ConnectionPool = Pool<ConnectionManager<PgConnection>>;

struct RemoteDatabaseConnector {
	db_uri: String,
}

#[async_trait::async_trait]
impl DatabaseConnector<(), StoreError> for RemoteDatabaseConnector {
	async fn try_connect(&self) -> Result<((), Pool<ConnectionManager<PgConnection>>), StoreError> {
		use diesel::Connection;
		PgConnection::establish(&self.db_uri)?;
		Ok(((), Pool::builder().build(ConnectionManager::<PgConnection>::new(&self.db_uri))?))
	}

	fn should_retry(&self, error: &StoreError) -> bool {
		matches!(error, StoreError::DbConnection(diesel::ConnectionError::BadConnection(_)))
	}
}

#[instrument(skip(db_uri))] //Do not log db_uri, as it can contain passwords
async fn pool_remote(db_uri: impl ToString) -> Result<ConnectionPool, ApiError> {
	let (_, pool) =
		get_connection_with_retry(RemoteDatabaseConnector { db_uri: db_uri.to_string() }).await?;
	Ok(pool)
}

#[instrument(skip_all)]
pub async fn arrow_api_server(
	domain: &ChronicleDomainDef,
	api: &ApiDispatch,
	pool: &ConnectionPool,
	addresses: Option<Vec<SocketAddr>>,
	security_conf: EndpointSecurityConfiguration,
	record_batch_size: usize,
	operation_batch_size: usize,
) -> Result<Option<impl Future<Output = Result<(), ApiError>> + Send>, ApiError> {
	tracing::info!(
		addresses = ?addresses,
		allow_anonymous = ?security_conf.allow_anonymous,
		jwt_must_claim = ?security_conf.must_claim,
		record_batch_size,
		operation_batch_size,
		"Starting arrow flight with the provided configuration"
	);

	match addresses {
		Some(addresses) => chronicle_arrow::run_flight_service(
			domain,
			pool,
			api,
			security_conf,
			&addresses,
			record_batch_size,
		)
		.await
		.map_err(|e| ApiError::ArrowService(e.into()))
		.map(|_| Some(futures::future::ready(Ok(())))),
		None => Ok(None),
	}
}

pub async fn graphql_api_server<Query, Mutation>(
	api: &ApiDispatch,
	pool: &ConnectionPool,
	gql: ChronicleGraphQl<Query, Mutation>,
	graphql_interface: Option<Vec<SocketAddr>>,
	security_conf: &SecurityConf,
	serve_graphql: bool,
	serve_data: bool,
) -> Result<Option<impl Future<Output = Result<(), ApiError>> + Send>, ApiError>
where
	Query: ObjectType + Copy + Send + 'static,
	Mutation: ObjectType + Copy + Send + 'static,
{
	if let Some(addresses) = graphql_interface {
		gql.serve_api(
			pool.clone(),
			api.clone(),
			addresses,
			security_conf,
			serve_graphql,
			serve_data,
		)
		.await?;
		Ok(Some(futures::future::ready(Ok(()))))
	} else {
		Ok(None)
	}
}

#[allow(dead_code)]
fn namespace_bindings(options: &ArgMatches) -> Vec<NamespaceId> {
	options
		.values_of("namespace-bindings")
		.map(|values| {
			values
				.map(|value| {
					let (id, uuid) = value.split_once(':').unwrap();

					let uuid = uuid::Uuid::parse_str(uuid).unwrap();
					NamespaceId::from_external_id(id, uuid)
				})
				.collect()
		})
		.unwrap_or_default()
}

fn vault_secrets_options(options: &ArgMatches) -> Result<ChronicleSecretsOptions, CliError> {
	let vault_url = options
		.value_of("vault-url")
		.ok_or_else(|| CliError::missing_argument("vault-url"))?;
	let token = options
		.value_of("vault-token")
		.ok_or_else(|| CliError::missing_argument("vault-token"))?;
	let mount_path = options
		.value_of("vault-mount-path")
		.ok_or_else(|| CliError::missing_argument("vault-mount-path"))?;
	Ok(ChronicleSecretsOptions::stored_in_vault(&Url::parse(vault_url)?, token, mount_path))
}

#[cfg(not(feature = "devmode"))]
async fn chronicle_signing(options: &ArgMatches) -> Result<ChronicleSigning, CliError> {
	// Determine batcher configuration

	use std::path::PathBuf;
	let batcher_options = match (
		options.get_one::<PathBuf>("batcher-key-from-path"),
		options.get_flag("batcher-key-from-vault"),
		options.get_flag("batcher-key-generated"),
	) {
		(Some(path), _, _) => ChronicleSecretsOptions::stored_at_path(path),
		(_, true, _) => vault_secrets_options(options)?,
		(_, _, true) => ChronicleSecretsOptions::generate_in_memory(),
		_ => unreachable!("CLI should always set batcher key"),
	};

	let chronicle_options = match (
		options.get_one::<PathBuf>("chronicle-key-from-path"),
		options.get_flag("chronicle-key-from-vault"),
		options.get_flag("chronicle-key-generated"),
	) {
		(Some(path), _, _) => ChronicleSecretsOptions::stored_at_path(path),
		(_, true, _) => vault_secrets_options(options)?,
		(_, _, true) => ChronicleSecretsOptions::generate_in_memory(),
		_ => unreachable!("CLI should always set chronicle key"),
	};

	Ok(ChronicleSigning::new(
		chronicle_secret_names(),
		vec![
			(CHRONICLE_NAMESPACE.to_string(), chronicle_options),
			(BATCHER_NAMESPACE.to_string(), batcher_options),
		],
	)
	.await?)
}

#[cfg(feature = "devmode")]
async fn chronicle_signing(_options: &ArgMatches) -> Result<ChronicleSigning, CliError> {
	Ok(ChronicleSigning::new(
		chronicle_secret_names(),
		vec![
			(CHRONICLE_NAMESPACE.to_string(), ChronicleSecretsOptions::generate_in_memory()),
			(BATCHER_NAMESPACE.to_string(), ChronicleSecretsOptions::generate_in_memory()),
		],
	)
	.await?)
}

#[cfg(not(feature = "devmode"))]
pub async fn api(
	pool: &ConnectionPool,
	options: &ArgMatches,
	policy_address: Option<PolicyAddress>,
	liveness_check_interval: Option<u64>,
) -> Result<ApiDispatch, CliError> {
	let ledger = ledger(options).await?;

	Ok(Api::new(
		pool.clone(),
		ledger,
		UniqueUuid,
		chronicle_signing(options).await?,
		namespace_bindings(options),
		policy_address,
		liveness_check_interval,
	)
	.await?)
}

#[cfg(feature = "devmode")]
pub async fn api(
	pool: &ConnectionPool,
	options: &ArgMatches,
	remote_opa: Option<PolicyAddress>,
	liveness_check_interval: Option<u64>,
) -> Result<api::ApiDispatch, CliError> {
	use protocol_substrate::PolkadotConfig;

	let embedded_tp = in_mem_ledger(options).await?;

	Ok(Api::new(
		pool.clone(),
		embedded_tp.connect_chronicle::<PolkadotConfig>().await?,
		UniqueUuid,
		chronicle_signing(options).await?,
		vec![],
		remote_opa,
		liveness_check_interval,
	)
	.await?)
}

fn construct_db_uri(matches: &ArgMatches) -> String {
	fn encode(string: &str) -> String {
		use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
		utf8_percent_encode(string, NON_ALPHANUMERIC).to_string()
	}

	let password = match std::env::var("PGPASSWORD") {
		Ok(password) => {
			debug!("PGPASSWORD is set, using for DB connection");
			format!(":{}", encode(password.as_str()))
		},
		Err(_) => {
			debug!("PGPASSWORD is not set, omitting for DB connection");
			String::new()
		},
	};

	format!(
		"postgresql://{}{}@{}:{}/{}",
		encode(
			matches
				.value_of("database-username")
				.expect("CLI should always set database user")
		),
		password,
		encode(matches.value_of("database-host").expect("CLI should always set database host")),
		encode(matches.value_of("database-port").expect("CLI should always set database port")),
		encode(matches.value_of("database-name").expect("CLI should always set database name"))
	)
}

#[derive(Debug, Clone)]
pub enum ConfiguredOpa {
	Embedded(ExecutorContext),
	Remote(ExecutorContext, OpaSettings),
	Url(ExecutorContext),
}

impl ConfiguredOpa {
	pub fn context(&self) -> &ExecutorContext {
		match self {
			ConfiguredOpa::Embedded(context) => context,
			ConfiguredOpa::Remote(context, _) => context,
			ConfiguredOpa::Url(context) => context,
		}
	}

	pub fn remote_settings(&self) -> Option<PolicyAddress> {
		match self {
			ConfiguredOpa::Embedded(_) => None,
			ConfiguredOpa::Remote(_, settings) => Some(settings.policy_address),
			ConfiguredOpa::Url(_) => None,
		}
	}
}

/// If embedded-opa-policy is set, we will use the embedded policy, otherwise we
/// attempt to load it from sawtooth settings. If we are running in development mode,
/// then always use embedded policy
#[cfg(feature = "devmode")]
#[allow(unused_variables)]
async fn configure_opa(options: &ArgMatches) -> Result<ConfiguredOpa, CliError> {
	let (default_policy_name, entrypoint) =
		("allow_transactions", "allow_transactions.allowed_users");
	let opa = opa_executor_from_embedded_policy(default_policy_name, entrypoint).await?;
	Ok(ConfiguredOpa::Embedded(opa))
}

// Check if the `embedded-opa-policy` flag is present in the CLI options.
// If it is, this means the user wants to use an embedded OPA policy.
// We then define the default policy name and entrypoint to be used,
// and attempt to load the OPA executor with this embedded policy.
// A warning is logged to indicate that Chronicle is operating in an insecure mode
// with an embedded default OPA policy.
// If the `embedded-opa-policy` flag is not present, we then check if the `opa-bundle-address`
// is provided. If it is, this means the user wants to load the OPA policy from a specified URL.
// We extract the policy name and entrypoint from the CLI options and attempt to load the OPA
// executor from the provided URL. A log is recorded to indicate that Chronicle is operating
// with an OPA policy loaded from a URL.
// If neither `embedded-opa-policy` nor `opa-bundle-address` is provided, we attempt to load the
// OPA executor from the substrate state. This involves connecting to the substrate client using
// the validator address provided in the CLI options and loading the OPA executor and settings
// from there. If settings are found, a log is recorded to indicate that Chronicle is operating
// in a secure mode with an on-chain OPA policy. Otherwise, a warning is logged to indicate an
// insecure mode of operation, and an attempt is made to load the OPA executor with an embedded
// default policy.
#[cfg(not(feature = "devmode"))]
#[instrument(skip(options))]
async fn configure_opa(options: &ArgMatches) -> Result<ConfiguredOpa, CliError> {
	if options.is_present("embedded-opa-policy") {
		let (default_policy_name, entrypoint) =
			("allow_transactions", "allow_transactions.allowed_users");
		let opa = opa_executor_from_embedded_policy(default_policy_name, entrypoint).await?;
		tracing::warn!(
			"Chronicle operating in an insecure mode with an embedded default OPA policy"
		);
		Ok(ConfiguredOpa::Embedded(opa))
	} else if let Some(url) = options.value_of("opa-bundle-address") {
		let (policy_name, entrypoint) = (
			options.value_of("opa-policy-name").unwrap(),
			options.value_of("opa-policy-entrypoint").unwrap(),
		);
		let opa = self::opa::opa_executor_from_url(url, policy_name, entrypoint).await?;
		tracing::info!("Chronicle operating with OPA policy from URL");

		Ok(ConfiguredOpa::Url(opa))
	} else {
		let (opa, settings) = self::opa::opa_executor_from_substrate_state(
			&ChronicleSubstrateClient::connect_socket_addr(validator_address(options)?[0]).await?,
			&protocol_substrate_opa::OpaSubstrateClient::connect_socket_addr(
				validator_address(options)?[0],
			)
			.await?,
		)
		.await?;

		if let Some(settings) = settings {
			tracing::info!(use_on_chain_opa= ?settings, "Chronicle operating in secure mode with on chain OPA policy");
			Ok(ConfiguredOpa::Remote(opa, settings))
		} else {
			tracing::warn!(
				"Chronicle operating in an insecure mode with an embedded default OPA policy"
			);
			tracing::warn!(use_on_chain_opa= ?settings, "Chronicle operating in secure mode with on chain OPA policy");
			let (default_policy_name, entrypoint) =
				("allow_transactions", "allow_transactions.allowed_users");
			let opa = opa_executor_from_embedded_policy(default_policy_name, entrypoint).await?;

			Ok(ConfiguredOpa::Embedded(opa))
		}
	}
}

/// If `--liveness-check` is set, we use either the interval in seconds provided or the default of
/// 1800. Otherwise, we use `None` to disable the depth charge.
fn configure_depth_charge(matches: &ArgMatches) -> Option<u64> {
	if let Some(serve_api_matches) = matches.subcommand_matches("serve-api") {
		if let Some(interval) = serve_api_matches.value_of("liveness-check") {
			let parsed_interval = interval.parse::<u64>().unwrap_or_else(|e| {
				warn!("Failed to parse '--liveness-check' value: {e}");
				1800
			});

			if parsed_interval == 1800 {
				debug!("Using default liveness health check interval value: 1800");
			} else {
				debug!("Using custom liveness health check interval value: {parsed_interval}");
			}
			return Some(parsed_interval);
		}
	}
	debug!("Liveness health check disabled");
	None
}

#[instrument(skip(gql, cli))]
async fn execute_subcommand<Query, Mutation>(
	gql: ChronicleGraphQl<Query, Mutation>,
	domain: &ChronicleDomainDef,
	cli: CliModel,
) -> Result<(ApiResponse, ApiDispatch), CliError>
where
	Query: ObjectType + Copy,
	Mutation: ObjectType + Copy,
{
	dotenvy::dotenv().ok();

	let matches = cli.as_cmd().get_matches();

	let pool = pool_remote(&construct_db_uri(&matches)).await?;

	let opa = configure_opa(&matches).await?;

	let liveness_check_interval = configure_depth_charge(&matches);

	let api = api(&pool, &matches, opa.remote_settings(), liveness_check_interval).await?;
	let ret_api = api.clone();

	if let Some(matches) = matches.subcommand_matches("serve-api") {
		let interface = match matches.get_many::<String>("interface") {
			Some(interface_args) => {
				let mut addrs = Vec::new();
				for interface_arg in interface_args {
					addrs.extend(interface_arg.to_socket_addrs()?);
				}
				Some(addrs)
			},
			None => None,
		};

		let arrow_interface = match matches.get_many::<String>("arrow-interface") {
			Some(interface_args) => {
				let mut addrs = Vec::new();
				for interface_arg in interface_args {
					addrs.extend(interface_arg.to_socket_addrs()?);
				}
				Some(addrs)
			},
			None => None,
		};

		let jwks_uri = if let Some(uri) = matches.value_of("jwks-address") {
			Some(JwksUri::new(Url::from_str(uri)?))
		} else {
			None
		};

		let userinfo_uri = if let Some(uri) = matches.value_of("userinfo-address") {
			Some(UserInfoUri::new(Url::from_str(uri)?))
		} else {
			None
		};

		let allow_anonymous = !matches.is_present("require-auth");

		let id_claims = matches.get_many::<String>("id-claims").map(|id_claims| {
			let mut id_keys = BTreeSet::new();
			for id_claim in id_claims {
				id_keys.extend(id_claim.split_whitespace().map(|s| s.to_string()));
			}
			id_keys
		});

		let mut jwt_must_claim: HashMap<String, String> = HashMap::new();
		for (name, value) in std::env::vars() {
			if let Some(name) = name.strip_prefix("JWT_MUST_CLAIM_") {
				jwt_must_claim.insert(name.to_lowercase(), value);
			}
		}
		if let Some(mut claims) = matches.get_many::<String>("jwt-must-claim") {
			while let (Some(name), Some(value)) = (claims.next(), claims.next()) {
				jwt_must_claim.insert(name.clone(), value.clone());
			}
		}

		let endpoints: Vec<String> =
			matches.get_many("offer-endpoints").unwrap().map(String::clone).collect();

		let security_conf = SecurityConf::new(
			jwks_uri,
			userinfo_uri,
			id_claims,
			jwt_must_claim.clone(),
			allow_anonymous,
			opa.context().clone(),
		);

		let arrow = arrow_api_server(
			domain,
			&api,
			&pool,
			arrow_interface,
			security_conf.as_endpoint_conf(30),
			1000,
			100,
		);

		let serve_graphql = endpoints.contains(&"graphql".to_string());
		let serve_data = endpoints.contains(&"data".to_string());

		let gql = graphql_api_server(
			&api,
			&pool,
			gql,
			interface,
			&security_conf,
			serve_graphql,
			serve_data,
		);

		tokio::task::spawn(async move {
			use async_signals::Signals;

			let mut signals = Signals::new(vec![libc::SIGHUP, libc::SIGINT]).unwrap();

			signals.next().await;
			chronicle_arrow::trigger_shutdown();
			api::chronicle_graphql::trigger_shutdown();
		});

		let (gql_result, arrow_result) = tokio::join!(gql, arrow);

		if let Err(e) = gql_result {
			return Err(e.into());
		}
		if let Err(e) = arrow_result {
			return Err(e.into());
		}

		Ok((ApiResponse::Unit, ret_api))
	} else if let Some(matches) = matches.subcommand_matches("import") {
		let data = if let Some(url) = matches.value_of("url") {
			let data = load_bytes_from_url(url).await?;
			info!("Loaded import data from {:?}", url);
			data
		} else {
			if std::io::stdin().is_terminal() {
				eprintln!("Attempting to import data from standard input, press Ctrl-D to finish.");
			}
			info!("Attempting to read import data from stdin...");
			let data = load_bytes_from_stdin()?;
			info!("Loaded {} bytes of import data from stdin", data.len());
			data
		};

		let data = std::str::from_utf8(&data)?;

		if data.trim().is_empty() {
			eprintln!("Import data is empty, nothing to import");
			return Ok((ApiResponse::Unit, ret_api));
		}

		let json_array = serde_json::from_str::<Vec<serde_json::Value>>(data)?;

		let mut operations = Vec::new();
		for value in json_array.into_iter() {
			let op = ChronicleOperation::from_json(&value)
				.await
				.expect("Failed to parse imported JSON-LD to ChronicleOperation");
			operations.push(op);
		}

		info!("Loading import data complete");

		let identity = AuthId::chronicle();

		let response = api.handle_import_command(identity, operations).await?;

		Ok((response, ret_api))
	} else if let Some(cmd) = cli.matches(&matches)? {
		let identity = AuthId::chronicle();
		Ok((api.dispatch(cmd, identity).await?, ret_api))
	} else {
		Ok((ApiResponse::Unit, ret_api))
	}
}

fn get_namespace(matches: &ArgMatches) -> NamespaceId {
	let namespace_id = matches.value_of("namespace-id").unwrap();
	let namespace_uuid = matches.value_of("namespace-uuid").unwrap();
	let uuid = uuid::Uuid::try_parse(namespace_uuid)
		.unwrap_or_else(|_| panic!("cannot parse namespace UUID: {}", namespace_uuid));
	NamespaceId::from_external_id(namespace_id, uuid)
}

async fn config_and_exec<Query, Mutation>(
	gql: ChronicleGraphQl<Query, Mutation>,
	domain: &ChronicleDomainDef,
	model: CliModel,
) -> Result<(), CliError>
where
	Query: ObjectType + Copy,
	Mutation: ObjectType + Copy,
{
	use colored_json::prelude::*;

	let response = execute_subcommand(gql, domain, model).await?;

	match response {
        (
            ApiResponse::Submission {
                subject,
                prov: _,
                tx_id,
            },
            api,
        ) => {
            // For commands that have initiated a ledger operation, wait for the matching result
            let mut tx_notifications = api.notify_commit.subscribe();

            loop {
                let stage = tx_notifications.recv().await.map_err(CliError::from)?;

                match stage {
                    SubmissionStage::Submitted(Ok(id)) => {
                        if id == tx_id {
                            debug!("Transaction submitted: {}", id);
                        }
                    }
                    SubmissionStage::Submitted(Err(err)) => {
                        if err.tx_id() == &tx_id {
                            eprintln!("Transaction rejected by Chronicle: {} {}", err, err.tx_id());
                            break;
                        }
                    }
                    SubmissionStage::Committed(commit, _) => {
                        if commit.tx_id == tx_id {
                            debug!("Transaction committed: {}", commit.tx_id);
                        }
                        println!("{subject}");
                    }
                    SubmissionStage::NotCommitted((id, contradiction, _)) => {
                        if id == tx_id {
                            eprintln!("Transaction rejected: {id} {contradiction}");
                            break;
                        }
                    }
                }
            }
        }
        (ApiResponse::QueryReply { prov }, _) => {
            println!(
                "{}",
                prov.to_json()
                    .compact()
                    .await?
                    .to_string()
                    .to_colored_json_auto()
                    .unwrap()
            );
        }
        (ApiResponse::Unit, _api) => {}
        (ApiResponse::AlreadyRecorded { subject, prov }, _api) => {
            println!("Transaction will not result in any data changes: {subject}");
            println!(
                "{}",
                prov.to_json()
                    .compact()
                    .await?
                    .to_string()
                    .to_colored_json_auto()
                    .unwrap()
            );
        }
		(ApiResponse::AlreadyRecordedAll, _api) => {
			println!("Import will not result in any data changes");
        }
        (ApiResponse::ImportSubmitted { prov, tx_id }, api) => {
            let mut tx_notifications = api.notify_commit.subscribe();

            loop {
                let stage = tx_notifications.recv().await.map_err(CliError::from)?;

                match stage {
                    SubmissionStage::Submitted(Ok(id)) => {
                        if id == tx_id {
                            debug!("Import operations submitted: {}", id);
                        }
                    }
                    SubmissionStage::Submitted(Err(err)) => {
                        if err.tx_id() == &tx_id {
                            eprintln!(
                                "Import transaction rejected by Chronicle: {} {}",
                                err,
                                err.tx_id()
                            );
                            break;
                        }
                    }
                    SubmissionStage::Committed(commit, _) => {
                        if commit.tx_id == tx_id {
                            debug!("Import transaction committed: {}", commit.tx_id);
                            println!("Import complete");
                            println!(
                                "{}",
                                prov.to_json()
                                    .compact()
                                    .await?
                                    .to_string()
                                    .to_colored_json_auto()
                                    .unwrap()
                            );
                            // An import command generates a single transaction, so we can break here and exit
                            break;
                        }
                    }
                    SubmissionStage::NotCommitted((id, contradiction, _)) => {
                        if id == tx_id {
                            eprintln!("Transaction rejected by ledger: {id} {contradiction}");
                            break;
                        }
                    }
                }
            }
        }
        (ApiResponse::DepthChargeSubmitted { tx_id }, _) => error!(
            "DepthChargeSubmitted is an unexpected API response for transaction: {tx_id}. Depth charge not implemented."
        ),
    };
	Ok(())
}

fn print_completions<G: Generator>(gen: G, app: &mut Command) {
	generate(gen, app, app.get_name().to_string(), &mut io::stdout());
}

pub async fn bootstrap<Query, Mutation>(
	domain: ChronicleDomainDef,
	gql: ChronicleGraphQl<Query, Mutation>,
) where
	Query: ObjectType + 'static + Copy,
	Mutation: ObjectType + 'static + Copy,
{
	let matches = cli(domain.clone()).as_cmd().get_matches();

	if let Some(generator) = matches.subcommand_matches("completions") {
		let shell = generator.get_one::<String>("shell").unwrap().parse::<Shell>().unwrap();
		print_completions(shell.to_owned(), &mut cli(domain.clone()).as_cmd());
		std::process::exit(0);
	}

	if matches.subcommand_matches("export-schema").is_some() {
		print!("{}", gql.exportable_schema());
		std::process::exit(0);
	}
	chronicle_telemetry::telemetry(
		matches
			.get_one::<String>("instrument")
			.map(|s| s.to_socket_addrs().expect("Could not parse as socketaddr").next().unwrap()),
		if matches.contains_id("console-logging") {
			match matches.get_one::<String>("console-logging") {
				Some(level) => match level.as_str() {
					"pretty" => ConsoleLogging::Pretty,
					"json" => ConsoleLogging::Json,
					_ => ConsoleLogging::Off,
				},
				_ => ConsoleLogging::Off,
			}
		} else if matches.subcommand_name() == Some("serve-api") {
			ConsoleLogging::Pretty
		} else {
			ConsoleLogging::Off
		},
	);

	config_and_exec(gql, &domain, domain.clone().into())
		.await
		.map_err(|e| {
			error!(?e, "Api error");
			e.into_ufe().print();
			std::process::exit(1);
		})
		.ok();

	std::process::exit(0);
}
