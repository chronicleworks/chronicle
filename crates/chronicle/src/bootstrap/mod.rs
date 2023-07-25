mod cli;
mod config;
mod opa;

#[cfg(feature = "inmem")]
use api::inmem::EmbeddedChronicleTp;
use api::{
    chronicle_graphql::{
        ChronicleApiServer, ChronicleGraphQl, Endpoints, JwksUri, SecurityConf, UserInfoUri,
    },
    Api, ApiDispatch, ApiError, StoreError, UuidGen,
};
use async_graphql::{async_trait, ObjectType};
#[cfg(not(feature = "inmem"))]
use chronicle_protocol::{
    address::{FAMILY, VERSION},
    ChronicleLedger,
};
use clap::{ArgMatches, Command};
use clap_complete::{generate, Generator, Shell};
pub use cli::*;
use common::{
    commands::ApiResponse,
    database::{get_connection_with_retry, DatabaseConnector},
    identity::AuthId,
    import::{load_bytes_from_stdin, load_bytes_from_url},
    ledger::SubmissionStage,
    opa::ExecutorContext,
    prov::{operations::ChronicleOperation, to_json_ld::ToJson, ExpandedJson, NamespaceId},
    signing::DirectoryStoredKeys,
};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use std::io::IsTerminal;
use tracing::{debug, error, info, instrument};
use user_error::UFE;

use config::*;
use diesel::{
    r2d2::{ConnectionManager, Pool},
    PgConnection,
};

use chronicle_telemetry::{self, ConsoleLogging};
use url::Url;

use std::{
    collections::{BTreeSet, HashMap},
    io,
    net::{SocketAddr, ToSocketAddrs},
    str::FromStr,
};

use crate::codegen::ChronicleDomainDef;

use self::opa::opa_executor_from_embedded_policy;

#[cfg(not(feature = "inmem"))]
fn sawtooth_address(config: &Config, options: &ArgMatches) -> Result<Vec<SocketAddr>, CliError> {
    Ok(options
        .value_of("sawtooth")
        .map(str::to_string)
        .or_else(|| Some(config.validator.address.clone().to_string()))
        .ok_or(CliError::MissingArgument {
            arg: "sawtooth".to_owned(),
        })
        .and_then(|s| Url::parse(&s).map_err(CliError::from))
        .map(|u| u.socket_addrs(|| Some(4004)))
        .map_err(CliError::from)??)
}

#[allow(dead_code)]
#[cfg(not(feature = "inmem"))]
fn ledger(config: &Config, options: &ArgMatches) -> Result<ChronicleLedger, CliError> {
    use async_stl_client::zmq_client::{
        HighestBlockValidatorSelector, ZmqRequestResponseSawtoothChannel,
    };

    Ok(ChronicleLedger::new(
        ZmqRequestResponseSawtoothChannel::new(
            "inmem",
            &sawtooth_address(config, options)?,
            HighestBlockValidatorSelector,
        )?
        .retrying(),
        FAMILY,
        VERSION,
    ))
}

#[allow(dead_code)]
fn in_mem_ledger(
    _config: &Config,
    _options: &ArgMatches,
) -> Result<crate::api::inmem::EmbeddedChronicleTp, ApiError> {
    Ok(crate::api::inmem::EmbeddedChronicleTp::new()?)
}

#[cfg(feature = "inmem")]
#[allow(dead_code)]
fn ledger() -> Result<EmbeddedChronicleTp, ApiError> {
    Ok(EmbeddedChronicleTp::new()?)
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
        Ok((
            (),
            Pool::builder().build(ConnectionManager::<PgConnection>::new(&self.db_uri))?,
        ))
    }

    fn should_retry(&self, error: &StoreError) -> bool {
        matches!(
            error,
            StoreError::DbConnection(diesel::ConnectionError::BadConnection(_))
        )
    }
}

#[instrument(skip(db_uri))] //Do not log db_uri, as it can contain passwords
async fn pool_remote(db_uri: impl ToString) -> Result<ConnectionPool, ApiError> {
    let (_, pool) = get_connection_with_retry(RemoteDatabaseConnector {
        db_uri: db_uri.to_string(),
    })
    .await?;
    Ok(pool)
}

pub async fn api_server<Query, Mutation>(
    api: &ApiDispatch,
    pool: &ConnectionPool,
    gql: ChronicleGraphQl<Query, Mutation>,
    interface: Option<Vec<SocketAddr>>,
    security_conf: SecurityConf,
    endpoints: Endpoints,
    metrics_handle: Option<PrometheusHandle>,
) -> Result<(), ApiError>
where
    Query: ObjectType + Copy,
    Mutation: ObjectType + Copy,
{
    if let Some(addresses) = interface {
        gql.serve_api(
            pool.clone(),
            api.clone(),
            addresses,
            security_conf,
            endpoints,
            metrics_handle,
        )
        .await?
    }

    Ok(())
}

#[cfg(not(feature = "inmem"))]
pub async fn api(
    pool: &ConnectionPool,
    options: &ArgMatches,
    config: &Config,
    policy_name: Option<String>,
    depth_charge_interval: Option<u64>,
) -> Result<ApiDispatch, CliError> {
    let ledger = ledger(config, options)?;

    Ok(Api::new(
        pool.clone(),
        ledger,
        &config.secrets.path,
        UniqueUuid,
        config.namespace_bindings.clone(),
        policy_name,
        depth_charge_interval,
    )
    .await?)
}

#[cfg(feature = "inmem")]
pub async fn api(
    pool: &ConnectionPool,
    _options: &ArgMatches,
    config: &Config,
    remote_opa: Option<String>,
    depth_charge_interval: Option<u64>,
) -> Result<api::ApiDispatch, ApiError> {
    let embedded_tp = in_mem_ledger(config, _options)?;

    Api::new(
        pool.clone(),
        embedded_tp.ledger,
        &config.secrets.path,
        UniqueUuid,
        config.namespace_bindings.clone(),
        remote_opa,
        depth_charge_interval,
    )
    .await
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
        }
        Err(_) => {
            debug!("PGPASSWORD is not set, omitting for DB connection");
            String::new()
        }
    };

    format!(
        "postgresql://{}{}@{}:{}/{}",
        encode(
            matches
                .value_of("database-username")
                .expect("CLI should always set database user")
        ),
        password,
        encode(
            matches
                .value_of("database-host")
                .expect("CLI should always set database host")
        ),
        encode(
            matches
                .value_of("database-port")
                .expect("CLI should always set database port")
        ),
        encode(
            matches
                .value_of("database-name")
                .expect("CLI should always set database name")
        )
    )
}

#[derive(Debug, Clone)]
pub enum ConfiguredOpa {
    Embedded(ExecutorContext),
    Remote(ExecutorContext, chronicle_protocol::settings::OpaSettings),
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

    pub fn remote_settings(&self) -> Option<String> {
        match self {
            ConfiguredOpa::Embedded(_) => None,
            ConfiguredOpa::Remote(_, settings) => Some(settings.policy_name.clone()),
            ConfiguredOpa::Url(_) => None,
        }
    }
}

/// If embedded-opa-policy is set, we will use the embedded policy, otherwise we
/// attempt to load it from sawtooth settings. If we are running in inmem mode,
/// then always use embedded policy
#[cfg(feature = "inmem")]
#[allow(unused_variables)]
async fn configure_opa(config: &Config, options: &ArgMatches) -> Result<ConfiguredOpa, CliError> {
    let (default_policy_name, entrypoint) =
        ("allow_transactions", "allow_transactions.allowed_users");
    let opa = opa_executor_from_embedded_policy(default_policy_name, entrypoint).await?;
    Ok(ConfiguredOpa::Embedded(opa))
}

#[cfg(not(feature = "inmem"))]
#[instrument(skip(config, options))]
async fn configure_opa(config: &Config, options: &ArgMatches) -> Result<ConfiguredOpa, CliError> {
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
        let (opa, settings) =
            self::opa::opa_executor_from_sawtooth_settings(&sawtooth_address(config, options)?)
                .await?;
        tracing::info!(use_on_chain_opa= ?settings, "Chronicle operating in secure mode with on chain OPA policy");

        Ok(ConfiguredOpa::Remote(opa, settings))
    }
}

/// If health metrics are enabled, we use either the interval in seconds provided or the default of 1800.
/// Otherwise, we return `None` for the handle **and** for the interval value in order to disable the health
/// metrics depth charge transactions as well as the `/metrics` endpoint.
fn configure_health_metrics(
    matches: &ArgMatches,
) -> Result<(Option<PrometheusHandle>, Option<u64>), CliError> {
    if let Some(serve_api_matches) = matches.subcommand_matches("serve-api") {
        if serve_api_matches.is_present("enable-health-metrics") {
            debug!("Health metrics enabled");

            let interval = serve_api_matches
                .value_of("health-metrics-interval")
                .unwrap_or("1800")
                .parse::<u64>()?;

            debug!(
                "Using {} health metrics interval value: {}",
                if interval == 1800 {
                    "default"
                } else {
                    "custom"
                },
                interval
            );
            let handle = PrometheusBuilder::new().install_recorder()?;

            debug!("Health metrics Prometheus exporter installed successfully");
            return Ok((Some(handle), Some(interval)));
        } else {
            debug!("Health metrics disabled");
            if serve_api_matches.is_present("health-metrics-interval") {
                debug!(
                    "Health metrics interval value provided but health metrics disabled, ignoring"
                );
            }
        }
    }
    Ok((None, None))
}

#[instrument(skip(gql, cli))]
async fn execute_subcommand<Query, Mutation>(
    gql: ChronicleGraphQl<Query, Mutation>,
    config: Config,
    cli: CliModel,
) -> Result<(ApiResponse, ApiDispatch), CliError>
where
    Query: ObjectType + Copy,
    Mutation: ObjectType + Copy,
{
    dotenvy::dotenv().ok();

    let matches = cli.as_cmd().get_matches();

    let pool = pool_remote(&construct_db_uri(&matches)).await?;

    let opa = configure_opa(&config, &matches).await?;

    let (health_metrics_handle, depth_charge_interval) = configure_health_metrics(&matches)?;

    let api = api(
        &pool,
        &matches,
        &config,
        opa.remote_settings(),
        depth_charge_interval,
    )
    .await?;
    let ret_api = api.clone();

    if let Some(matches) = matches.subcommand_matches("serve-api") {
        let interface = match matches.get_many::<String>("interface") {
            Some(interface_args) => {
                let mut addrs = Vec::new();
                for interface_arg in interface_args {
                    addrs.extend(interface_arg.to_socket_addrs()?);
                }
                Some(addrs)
            }
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

        let endpoints: Vec<String> = matches
            .get_many("offer-endpoints")
            .unwrap()
            .map(String::clone)
            .collect();

        if endpoints.contains(&"health".to_string()) && health_metrics_handle.is_none() {
            return Err(CliError::HealthEndpointConfigError);
        }
        if endpoints.contains(&"metrics".to_string()) && health_metrics_handle.is_none() {
            return Err(CliError::MetricsEndpointConfigError);
        }

        api_server(
            &api,
            &pool,
            gql,
            interface,
            SecurityConf::new(
                jwks_uri,
                userinfo_uri,
                id_claims,
                jwt_must_claim,
                allow_anonymous,
                opa.context().clone(),
            ),
            Endpoints::new(
                endpoints.contains(&"graphql".to_string()),
                endpoints.contains(&"data".to_string()),
                endpoints.contains(&"health".to_string()),
                endpoints.contains(&"metrics".to_string()),
            ),
            health_metrics_handle,
        )
        .await?;

        Ok((ApiResponse::Unit, ret_api))
    } else if let Some(matches) = matches.subcommand_matches("import") {
        let namespace = get_namespace(matches);

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
            let op = ChronicleOperation::from_json(ExpandedJson(value))
                .await
                .expect("Failed to parse imported JSON-LD to ChronicleOperation");
            // Only import operations for the specified namespace
            if op.namespace() == &namespace {
                operations.push(op);
            }
        }

        info!("Loading import data complete");

        let identity = AuthId::chronicle();
        info!("Importing data as root to Chronicle namespace: {namespace}");

        let response = api
            .handle_import_command(identity, namespace, operations)
            .await?;

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
    model: CliModel,
) -> Result<(), CliError>
where
    Query: ObjectType + Copy,
    Mutation: ObjectType + Copy,
{
    use colored_json::prelude::*;
    let config = handle_config_and_init(&model)?;

    let response = execute_subcommand(gql, config, model).await?;

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
        let shell = generator
            .get_one::<String>("shell")
            .unwrap()
            .parse::<Shell>()
            .unwrap();
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
            .map(|s| Url::parse(s).expect("cannot parse instrument as URI: {s}")),
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

    if matches.subcommand_matches("verify-keystore").is_some() {
        let config = handle_config_and_init(&domain.into())
            .expect("failed to initialize from domain definition");
        let store = DirectoryStoredKeys::new(config.secrets.path)
            .expect("failed to create key store at {config.secrets.path}");
        info!(keystore=?store);

        let retrieve_signer = common::signing::directory_signing_key;
        if store.chronicle_signing(retrieve_signer).is_err() {
            info!("Generating new chronicle key");
            store
                .generate_chronicle()
                .expect("failed to create key in {store.base}");
        }

        std::process::exit(0);
    }

    config_and_exec(gql, domain.into())
        .await
        .map_err(|e| {
            error!(?e, "Api error");
            e.into_ufe().print();
            std::process::exit(1);
        })
        .ok();

    std::process::exit(0);
}

/// We can only sensibly test subcommand parsing for the CLI's PROV actions,
/// configuration + server execution would get a little tricky in the context of a unit test.
#[cfg(test)]
pub mod test {
    use api::{inmem::EmbeddedChronicleTp, Api, ApiDispatch, ApiError, UuidGen};
    use async_stl_client::prost::Message;
    use common::{
        commands::{ApiCommand, ApiResponse},
        database::TemporaryDatabase,
        identity::AuthId,
        k256::sha2::{Digest, Sha256},
        ledger::SubmissionStage,
        prov::{
            to_json_ld::ToJson, ActivityId, AgentId, ChronicleIri, ChronicleTransactionId,
            EntityId, ProvModel,
        },
        signing::DirectoryStoredKeys,
    };
    use opa_tp_protocol::state::{policy_address, policy_meta_address, PolicyMeta};
    use std::collections::HashMap;
    use tempfile::TempDir;
    use uuid::Uuid;

    use super::{CliModel, SubCommand};
    use crate::codegen::ChronicleDomainDef;

    struct TestDispatch<'a> {
        api: ApiDispatch,
        _db: TemporaryDatabase<'a>, // share lifetime
        _tp: EmbeddedChronicleTp,
    }

    impl TestDispatch<'_> {
        pub async fn dispatch(
            &mut self,
            command: ApiCommand,
            identity: AuthId,
        ) -> Result<Option<(Box<ProvModel>, ChronicleTransactionId)>, ApiError> {
            // We can sort of get final on chain state here by using a map of subject to model
            if let ApiResponse::Submission { .. } = self.api.dispatch(command, identity).await? {
                loop {
                    let submission = self
                        .api
                        .notify_commit
                        .subscribe()
                        .recv()
                        .await
                        .expect("failed to receive response to submission");

                    if let SubmissionStage::Committed(commit, _) = submission {
                        break Ok(Some((commit.delta, commit.tx_id)));
                    }
                    if let SubmissionStage::NotCommitted((_, contradiction, _)) = submission {
                        panic!("Contradiction: {contradiction}");
                    }
                }
            } else {
                Ok(None)
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

    async fn test_api<'a>() -> TestDispatch<'a> {
        chronicle_telemetry::telemetry(None, chronicle_telemetry::ConsoleLogging::Pretty);

        let secretpath = TempDir::new().unwrap().into_path();

        let keystore_path = secretpath.clone();
        let keystore = DirectoryStoredKeys::new(keystore_path).unwrap();
        keystore.generate_chronicle().unwrap();

        let buf = async_stl_client::messages::Setting {
            entries: vec![async_stl_client::messages::setting::Entry {
                key: "chronicle.opa.policy_name".to_string(),
                value: "allow_transactions".to_string(),
            }],
        }
        .encode_to_vec();
        let setting_id = (
            chronicle_protocol::settings::sawtooth_settings_address("chronicle.opa.policy_name"),
            buf,
        );
        let buf = async_stl_client::messages::Setting {
            entries: vec![async_stl_client::messages::setting::Entry {
                key: "chronicle.opa.entrypoint".to_string(),
                value: "allow_transactions.allowed_users".to_string(),
            }],
        }
        .encode_to_vec();

        let setting_entrypoint = (
            chronicle_protocol::settings::sawtooth_settings_address("chronicle.opa.entrypoint"),
            buf,
        );

        let d = env!("CARGO_MANIFEST_DIR").to_owned() + "/../../policies/bundle.tar.gz";
        let bin = std::fs::read(d).unwrap();

        let meta = PolicyMeta {
            id: "allow_transactions".to_string(),
            hash: hex::encode(Sha256::digest(&bin)),
            policy_address: policy_address("allow_transactions"),
        };

        let embedded_tp = EmbeddedChronicleTp::new_with_state(
            vec![
                setting_id,
                setting_entrypoint,
                (policy_address("allow_transactions"), bin),
                (
                    policy_meta_address("allow_transactions"),
                    serde_json::to_vec(&meta).unwrap(),
                ),
            ]
            .into_iter()
            .collect(),
        )
        .unwrap();

        let database = TemporaryDatabase::default();
        let pool = database.connection_pool().unwrap();

        let dispatch = Api::new(
            pool,
            embedded_tp.ledger.clone(),
            &secretpath,
            SameUuid,
            HashMap::default(),
            Some("allow_transactions".to_owned()),
            None,
        )
        .await
        .unwrap();

        TestDispatch {
            api: dispatch,
            _db: database,
            _tp: embedded_tp,
        }
    }

    fn get_api_cmd(command_line: &str) -> ApiCommand {
        let cli = test_cli_model();
        let matches = cli
            .as_cmd()
            .get_matches_from(command_line.split_whitespace());
        cli.matches(&matches).unwrap().unwrap()
    }

    async fn parse_and_execute(command_line: &str, cli: CliModel) -> Box<ProvModel> {
        let mut api = test_api().await;

        let matches = cli
            .as_cmd()
            .get_matches_from(command_line.split_whitespace());

        let cmd = cli.matches(&matches).unwrap().unwrap();

        let identity = AuthId::chronicle();

        api.dispatch(cmd, identity).await.unwrap().unwrap().0
    }

    fn test_cli_model() -> CliModel {
        CliModel::from(
            ChronicleDomainDef::build("test")
                .with_attribute_type("testString", None, crate::PrimitiveType::String)
                .unwrap()
                .with_attribute_type("testBool", None, crate::PrimitiveType::Bool)
                .unwrap()
                .with_attribute_type("testInt", None, crate::PrimitiveType::Int)
                .unwrap()
                .with_attribute_type("testJSON", None, crate::PrimitiveType::JSON)
                .unwrap()
                .with_activity("testActivity", None, |b| {
                    b.with_attribute("testString")
                        .unwrap()
                        .with_attribute("testBool")
                        .unwrap()
                        .with_attribute("testInt")
                })
                .unwrap()
                .with_agent("testAgent", None, |b| {
                    b.with_attribute("testString")
                        .unwrap()
                        .with_attribute("testBool")
                        .unwrap()
                        .with_attribute("testInt")
                })
                .unwrap()
                .with_entity("testEntity", None, |b| {
                    b.with_attribute("testString")
                        .unwrap()
                        .with_attribute("testBool")
                        .unwrap()
                        .with_attribute("testInt")
                })
                .unwrap()
                .build(),
        )
    }

    #[tokio::test]
    async fn agent_define() {
        let command_line = r#"chronicle test-agent-agent define test_agent --test-bool-attr false --test-string-attr "test" --test-int-attr 23 --namespace testns "#;

        insta::assert_snapshot!(
          serde_json::to_string_pretty(
          &parse_and_execute(command_line, test_cli_model()).await.to_json().compact_stable_order().await.unwrap()
        ).unwrap() , @r###"
        {
          "@context": "https://btp.works/chr/1.0/c.jsonld",
          "@graph": [
            {
              "@id": "chronicle:agent:test%5Fagent",
              "@type": [
                "prov:Agent",
                "chronicle:domaintype:testAgent"
              ],
              "externalId": "test_agent",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "@type": "chronicle:Namespace",
              "externalId": "testns"
            }
          ]
        }
        "###);
    }

    #[tokio::test]
    async fn agent_define_id() {
        let id = ChronicleIri::from(common::prov::AgentId::from_external_id("test_agent"));
        let command_line = format!(
            r#"chronicle test-agent-agent define --test-bool-attr false --test-string-attr "test" --test-int-attr 23 --namespace testns --id {id} "#
        );

        insta::assert_snapshot!(
          serde_json::to_string_pretty(
          &parse_and_execute(&command_line, test_cli_model()).await.to_json().compact_stable_order().await.unwrap()
        ).unwrap() , @r###"
        {
          "@context": "https://btp.works/chr/1.0/c.jsonld",
          "@graph": [
            {
              "@id": "chronicle:agent:test%5Fagent",
              "@type": [
                "prov:Agent",
                "chronicle:domaintype:testAgent"
              ],
              "externalId": "test_agent",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "@type": "chronicle:Namespace",
              "externalId": "testns"
            }
          ]
        }
        "###);
    }

    #[tokio::test]
    async fn agent_use() {
        let mut api = test_api().await;

        // note, if you don't supply all three types of attribute this won't run
        let command_line = r#"chronicle test-agent-agent define testagent --namespace testns --test-string-attr "test" --test-bool-attr true --test-int-attr 23 "#;

        let cmd = get_api_cmd(command_line);

        insta::assert_snapshot!(
          serde_json::to_string_pretty(
          &api.dispatch(cmd, AuthId::chronicle()).await.unwrap().unwrap().0.to_json().compact_stable_order().await.unwrap()
        ).unwrap() , @r###"
        {
          "@context": "https://btp.works/chr/1.0/c.jsonld",
          "@graph": [
            {
              "@id": "chronicle:agent:testagent",
              "@type": [
                "prov:Agent",
                "chronicle:domaintype:testAgent"
              ],
              "externalId": "testagent",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": true,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "@type": "chronicle:Namespace",
              "externalId": "testns"
            }
          ]
        }
        "###);

        let id = AgentId::from_external_id("testagent");

        let command_line = format!(r#"chronicle test-agent-agent use --namespace testns {id} "#);
        let cmd = get_api_cmd(&command_line);

        api.dispatch(cmd, AuthId::chronicle()).await.unwrap();

        let id = ActivityId::from_external_id("testactivity");
        let command_line = format!(
            r#"chronicle test-activity-activity start {id} --namespace testns --time 2014-07-08T09:10:11Z "#
        );
        let cmd = get_api_cmd(&command_line);

        insta::assert_snapshot!(
          serde_json::to_string_pretty(
          &api.dispatch(cmd, AuthId::chronicle()).await.unwrap().unwrap().0.to_json().compact_stable_order().await.unwrap()
        ).unwrap() , @r###"
        {
          "@context": "https://btp.works/chr/1.0/c.jsonld",
          "@graph": [
            {
              "@id": "chronicle:activity:testactivity",
              "@type": "prov:Activity",
              "externalId": "testactivity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "prov:qualifiedAssociation": {
                "@id": "chronicle:association:testagent:testactivity:role="
              },
              "startTime": "2014-07-08T09:10:11+00:00",
              "value": {},
              "wasAssociatedWith": [
                "chronicle:agent:testagent"
              ]
            },
            {
              "@id": "chronicle:association:testagent:testactivity:role=",
              "@type": "prov:Association",
              "agent": "chronicle:agent:testagent",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "prov:hadActivity": {
                "@id": "chronicle:activity:testactivity"
              }
            },
            {
              "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "@type": "chronicle:Namespace",
              "externalId": "testns"
            }
          ]
        }
        "###);
    }

    #[tokio::test]
    async fn entity_define() {
        let command_line = r#"chronicle test-entity-entity define test_entity --test-bool-attr false --test-string-attr "test" --test-int-attr 23 --namespace testns "#;
        let _delta = parse_and_execute(command_line, test_cli_model());

        insta::assert_snapshot!(
          serde_json::to_string_pretty(
          &parse_and_execute(command_line, test_cli_model()).await.to_json().compact_stable_order().await.unwrap()
        ).unwrap() , @r###"
        {
          "@context": "https://btp.works/chr/1.0/c.jsonld",
          "@graph": [
            {
              "@id": "chronicle:entity:test%5Fentity",
              "@type": [
                "prov:Entity",
                "chronicle:domaintype:testEntity"
              ],
              "externalId": "test_entity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "@type": "chronicle:Namespace",
              "externalId": "testns"
            }
          ]
        }
        "###);
    }

    #[tokio::test]
    async fn entity_define_id() {
        let id = ChronicleIri::from(common::prov::EntityId::from_external_id("test_entity"));
        let command_line = format!(
            r#"chronicle test-entity-entity define --test-bool-attr false --test-string-attr "test" --test-int-attr 23 --namespace testns --id {id} "#
        );

        insta::assert_snapshot!(
          serde_json::to_string_pretty(
          &parse_and_execute(&command_line, test_cli_model()).await.to_json().compact_stable_order().await.unwrap()
        ).unwrap() , @r###"
        {
          "@context": "https://btp.works/chr/1.0/c.jsonld",
          "@graph": [
            {
              "@id": "chronicle:entity:test%5Fentity",
              "@type": [
                "prov:Entity",
                "chronicle:domaintype:testEntity"
              ],
              "externalId": "test_entity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "@type": "chronicle:Namespace",
              "externalId": "testns"
            }
          ]
        }
        "###);
    }

    #[tokio::test]
    async fn entity_derive_abstract() {
        let mut api = test_api().await;

        let generated_entity_id = EntityId::from_external_id("testgeneratedentity");
        let used_entity_id = EntityId::from_external_id("testusedentity");

        let command_line = format!(
            r#"chronicle test-entity-entity derive {generated_entity_id} {used_entity_id} --namespace testns "#
        );
        let cmd = get_api_cmd(&command_line);

        insta::assert_snapshot!(
          serde_json::to_string_pretty(
          &api.dispatch(cmd, AuthId::chronicle()).await.unwrap().unwrap().0.to_json().compact_stable_order().await.unwrap()
        ).unwrap() , @r###"
        {
          "@context": "https://btp.works/chr/1.0/c.jsonld",
          "@graph": [
            {
              "@id": "chronicle:entity:testgeneratedentity",
              "@type": "prov:Entity",
              "externalId": "testgeneratedentity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {},
              "wasDerivedFrom": [
                "chronicle:entity:testusedentity"
              ]
            },
            {
              "@id": "chronicle:entity:testusedentity",
              "@type": "prov:Entity",
              "externalId": "testusedentity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {}
            },
            {
              "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "@type": "chronicle:Namespace",
              "externalId": "testns"
            }
          ]
        }
        "###);
    }

    #[tokio::test]
    async fn entity_derive_primary_source() {
        let mut api = test_api().await;

        let generated_entity_id = EntityId::from_external_id("testgeneratedentity");
        let used_entity_id = EntityId::from_external_id("testusedentity");

        let command_line = format!(
            r#"chronicle test-entity-entity derive {generated_entity_id} {used_entity_id} --namespace testns --subtype primary-source "#
        );
        let cmd = get_api_cmd(&command_line);

        insta::assert_snapshot!(
          serde_json::to_string_pretty(
          &api.dispatch(cmd, AuthId::chronicle()).await.unwrap().unwrap().0.to_json().compact_stable_order().await.unwrap()
        ).unwrap() , @r###"
        {
          "@context": "https://btp.works/chr/1.0/c.jsonld",
          "@graph": [
            {
              "@id": "chronicle:entity:testgeneratedentity",
              "@type": "prov:Entity",
              "externalId": "testgeneratedentity",
              "hadPrimarySource": [
                "chronicle:entity:testusedentity"
              ],
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {}
            },
            {
              "@id": "chronicle:entity:testusedentity",
              "@type": "prov:Entity",
              "externalId": "testusedentity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {}
            },
            {
              "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "@type": "chronicle:Namespace",
              "externalId": "testns"
            }
          ]
        }
        "###);
    }

    #[tokio::test]
    async fn entity_derive_revision() {
        let mut api = test_api().await;

        let generated_entity_id = EntityId::from_external_id("testgeneratedentity");
        let used_entity_id = EntityId::from_external_id("testusedentity");

        let command_line = format!(
            r#"chronicle test-entity-entity derive {generated_entity_id} {used_entity_id} --namespace testns --subtype revision "#
        );
        let cmd = get_api_cmd(&command_line);

        insta::assert_snapshot!(
          serde_json::to_string_pretty(
          &api.dispatch(cmd, AuthId::chronicle()).await.unwrap().unwrap().0.to_json().compact_stable_order().await.unwrap()
        ).unwrap() , @r###"
        {
          "@context": "https://btp.works/chr/1.0/c.jsonld",
          "@graph": [
            {
              "@id": "chronicle:entity:testgeneratedentity",
              "@type": "prov:Entity",
              "externalId": "testgeneratedentity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {},
              "wasRevisionOf": [
                "chronicle:entity:testusedentity"
              ]
            },
            {
              "@id": "chronicle:entity:testusedentity",
              "@type": "prov:Entity",
              "externalId": "testusedentity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {}
            },
            {
              "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "@type": "chronicle:Namespace",
              "externalId": "testns"
            }
          ]
        }
        "###);
    }

    #[tokio::test]
    async fn entity_derive_quotation() {
        let mut api = test_api().await;

        let generated_entity_id = EntityId::from_external_id("testgeneratedentity");
        let used_entity_id = EntityId::from_external_id("testusedentity");

        let command_line = format!(
            r#"chronicle test-entity-entity derive {generated_entity_id} {used_entity_id} --namespace testns --subtype quotation "#
        );
        let cmd = get_api_cmd(&command_line);

        insta::assert_snapshot!(
          serde_json::to_string_pretty(
          &api.dispatch(cmd, AuthId::chronicle()).await.unwrap().unwrap().0.to_json().compact_stable_order().await.unwrap()
        ).unwrap() , @r###"
        {
          "@context": "https://btp.works/chr/1.0/c.jsonld",
          "@graph": [
            {
              "@id": "chronicle:entity:testgeneratedentity",
              "@type": "prov:Entity",
              "externalId": "testgeneratedentity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {},
              "wasQuotedFrom": [
                "chronicle:entity:testusedentity"
              ]
            },
            {
              "@id": "chronicle:entity:testusedentity",
              "@type": "prov:Entity",
              "externalId": "testusedentity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {}
            },
            {
              "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "@type": "chronicle:Namespace",
              "externalId": "testns"
            }
          ]
        }
        "###);
    }

    #[tokio::test]
    async fn activity_define() {
        let command_line = r#"chronicle test-activity-activity define test_activity --test-bool-attr false --test-string-attr "test" --test-int-attr 23 --namespace testns "#;

        insta::assert_snapshot!(
          serde_json::to_string_pretty(
          &parse_and_execute(command_line, test_cli_model()).await.to_json().compact_stable_order().await.unwrap()
        ).unwrap() , @r###"
        {
          "@context": "https://btp.works/chr/1.0/c.jsonld",
          "@graph": [
            {
              "@id": "chronicle:activity:test%5Factivity",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "externalId": "test_activity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "@type": "chronicle:Namespace",
              "externalId": "testns"
            }
          ]
        }
        "###);
    }

    #[tokio::test]
    async fn activity_define_id() {
        let id = ChronicleIri::from(common::prov::ActivityId::from_external_id("test_activity"));
        let command_line = format!(
            r#"chronicle test-activity-activity define --test-bool-attr false --test-string-attr "test" --test-int-attr 23 --namespace testns --id {id} "#
        );

        insta::assert_snapshot!(
          serde_json::to_string_pretty(
          &parse_and_execute(&command_line, test_cli_model()).await.to_json().compact_stable_order().await.unwrap()
        ).unwrap() , @r###"
        {
          "@context": "https://btp.works/chr/1.0/c.jsonld",
          "@graph": [
            {
              "@id": "chronicle:activity:test%5Factivity",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "externalId": "test_activity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "@type": "chronicle:Namespace",
              "externalId": "testns"
            }
          ]
        }
        "###);
    }

    #[tokio::test]
    async fn activity_start() {
        let mut api = test_api().await;

        let command_line = r#"chronicle test-agent-agent define testagent --namespace testns --test-string-attr "test" --test-bool-attr true --test-int-attr 40 "#;
        let cmd = get_api_cmd(command_line);

        api.dispatch(cmd, AuthId::chronicle()).await.unwrap();

        let id = ChronicleIri::from(AgentId::from_external_id("testagent"));
        let command_line = format!(r#"chronicle test-agent-agent use --namespace testns {id} "#);
        let cmd = get_api_cmd(&command_line);
        api.dispatch(cmd, AuthId::chronicle()).await.unwrap();

        let id = ChronicleIri::from(ActivityId::from_external_id("testactivity"));
        let command_line = format!(
            r#"chronicle test-activity-activity start {id} --namespace testns --time 2014-07-08T09:10:11Z "#
        );
        let cmd = get_api_cmd(&command_line);

        insta::assert_snapshot!(
          serde_json::to_string_pretty(
          &api.dispatch(cmd, AuthId::chronicle()).await.unwrap().unwrap().0.to_json().compact_stable_order().await.unwrap()
        ).unwrap() , @r###"
        {
          "@context": "https://btp.works/chr/1.0/c.jsonld",
          "@graph": [
            {
              "@id": "chronicle:activity:testactivity",
              "@type": "prov:Activity",
              "externalId": "testactivity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "prov:qualifiedAssociation": {
                "@id": "chronicle:association:testagent:testactivity:role="
              },
              "startTime": "2014-07-08T09:10:11+00:00",
              "value": {},
              "wasAssociatedWith": [
                "chronicle:agent:testagent"
              ]
            },
            {
              "@id": "chronicle:association:testagent:testactivity:role=",
              "@type": "prov:Association",
              "agent": "chronicle:agent:testagent",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "prov:hadActivity": {
                "@id": "chronicle:activity:testactivity"
              }
            },
            {
              "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "@type": "chronicle:Namespace",
              "externalId": "testns"
            }
          ]
        }
        "###);
    }

    #[tokio::test]
    async fn activity_end() {
        let mut api = test_api().await;

        let command_line = r#"chronicle test-agent-agent define testagent --namespace testns --test-string-attr "test" --test-bool-attr true --test-int-attr 40 "#;
        let cmd = get_api_cmd(command_line);

        api.dispatch(cmd, AuthId::chronicle()).await.unwrap();

        let id = ChronicleIri::from(AgentId::from_external_id("testagent"));
        let command_line = format!(r#"chronicle test-agent-agent use --namespace testns {id} "#);
        let cmd = get_api_cmd(&command_line);
        api.dispatch(cmd, AuthId::chronicle()).await.unwrap();

        let id = ChronicleIri::from(ActivityId::from_external_id("testactivity"));
        let command_line = format!(
            r#"chronicle test-activity-activity start {id} --namespace testns --time 2014-07-08T09:10:11Z "#
        );
        let cmd = get_api_cmd(&command_line);
        api.dispatch(cmd, AuthId::chronicle()).await.unwrap();

        // Should end the last opened activity
        let id = ActivityId::from_external_id("testactivity");
        let command_line = format!(
            r#"chronicle test-activity-activity end --namespace testns --time 2014-08-09T09:10:12Z {id} "#
        );
        let cmd = get_api_cmd(&command_line);

        insta::assert_snapshot!(
          serde_json::to_string_pretty(
          &api.dispatch(cmd, AuthId::chronicle()).await.unwrap().unwrap().0.to_json().compact_stable_order().await.unwrap()
        ).unwrap() , @r###"
        {
          "@context": "https://btp.works/chr/1.0/c.jsonld",
          "@graph": [
            {
              "@id": "chronicle:activity:testactivity",
              "@type": "prov:Activity",
              "endTime": "2014-08-09T09:10:12+00:00",
              "externalId": "testactivity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "startTime": "2014-07-08T09:10:11+00:00",
              "value": {}
            },
            {
              "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "@type": "chronicle:Namespace",
              "externalId": "testns"
            }
          ]
        }
        "###);
    }

    #[tokio::test]
    async fn activity_generate() {
        let mut api = test_api().await;

        let command_line = r#"chronicle test-activity-activity define testactivity --namespace testns --test-string-attr "test" --test-bool-attr true --test-int-attr 40 "#;
        let cmd = get_api_cmd(command_line);

        api.dispatch(cmd, AuthId::chronicle()).await.unwrap();

        let activity_id = ActivityId::from_external_id("testactivity");
        let entity_id = EntityId::from_external_id("testentity");
        let command_line = format!(
            r#"chronicle test-activity-activity generate --namespace testns {entity_id} {activity_id} "#
        );
        let cmd = get_api_cmd(&command_line);

        insta::assert_snapshot!(
          serde_json::to_string_pretty(
          &api.dispatch(cmd, AuthId::chronicle()).await.unwrap().unwrap().0.to_json().compact_stable_order().await.unwrap()
        ).unwrap() , @r###"
        {
          "@context": "https://btp.works/chr/1.0/c.jsonld",
          "@graph": [
            {
              "@id": "chronicle:entity:testentity",
              "@type": "prov:Entity",
              "externalId": "testentity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {},
              "wasGeneratedBy": [
                "chronicle:activity:testactivity"
              ]
            },
            {
              "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "@type": "chronicle:Namespace",
              "externalId": "testns"
            }
          ]
        }
        "###);
    }

    #[tokio::test]
    async fn activity_use() {
        let mut api = test_api().await;

        let command_line = r#"chronicle test-agent-agent define testagent --namespace testns --test-string-attr "test" --test-bool-attr true --test-int-attr 40 "#;
        let cmd = get_api_cmd(command_line);

        api.dispatch(cmd, AuthId::chronicle()).await.unwrap();

        let id = ChronicleIri::from(AgentId::from_external_id("testagent"));
        let command_line = format!(r#"chronicle test-agent-agent use --namespace testns {id} "#);
        let cmd = get_api_cmd(&command_line);
        api.dispatch(cmd, AuthId::chronicle()).await.unwrap();

        let command_line = r#"chronicle test-activity-activity define testactivity --namespace testns --test-string-attr "test" --test-bool-attr true --test-int-attr 40 "#;
        let cmd = get_api_cmd(command_line);
        api.dispatch(cmd, AuthId::chronicle()).await.unwrap();

        let activity_id = ActivityId::from_external_id("testactivity");
        let entity_id = EntityId::from_external_id("testentity");
        let command_line = format!(
            r#"chronicle test-activity-activity use --namespace testns {entity_id} {activity_id} "#
        );

        let cmd = get_api_cmd(&command_line);

        insta::assert_snapshot!(
          serde_json::to_string_pretty(
          &api.dispatch(cmd, AuthId::chronicle()).await.unwrap().unwrap().0.to_json().compact_stable_order().await.unwrap()
        ).unwrap() , @r###"
        {
          "@context": "https://btp.works/chr/1.0/c.jsonld",
          "@graph": [
            {
              "@id": "chronicle:activity:testactivity",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "externalId": "testactivity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "used": [
                "chronicle:entity:testentity"
              ],
              "value": {
                "TestBool": true,
                "TestInt": 40,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:entity:testentity",
              "@type": "prov:Entity",
              "externalId": "testentity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {}
            },
            {
              "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "@type": "chronicle:Namespace",
              "externalId": "testns"
            }
          ]
        }
        "###);
    }
}
