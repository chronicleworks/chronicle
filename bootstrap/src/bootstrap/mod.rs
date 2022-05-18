mod cli;
mod config;
pub mod telemetry;

use api::{
    chronicle_graphql::{ChronicleGraphQl, ChronicleGraphQlServer},
    Api, ApiDispatch, ApiError, ConnectionOptions, UuidGen,
};
use async_graphql::ObjectType;
use clap::{ArgMatches, Command};
use clap_complete::{generate, Generator, Shell};
pub use cli::*;
use common::{
    attributes::Attributes,
    commands::{
        ActivityCommand, AgentCommand, ApiCommand, ApiResponse, EntityCommand, KeyImport,
        KeyRegistration, NamespaceCommand, PathOrFile, QueryCommand,
    },
    prov::{ActivityId, AgentId, CompactionError, DomaintypeId, EntityId},
};
use custom_error::custom_error;
use tokio::sync::broadcast::error::RecvError;
use tracing::{error, instrument};
use user_error::UFE;

use common::signing::SignerError;
use config::*;
use diesel::{
    r2d2::{ConnectionManager, Pool},
    SqliteConnection,
};

use sawtooth_protocol::{events::StateDelta, messaging::SawtoothSubmitter};
use url::Url;

use std::{
    io,
    net::SocketAddr,
    path::{Path, PathBuf},
    time::Duration,
};

#[allow(dead_code)]
fn submitter(config: &Config, options: &ArgMatches) -> Result<SawtoothSubmitter, SignerError> {
    Ok(SawtoothSubmitter::new(
        &options
            .value_of("sawtooth")
            .map(Url::parse)
            .unwrap_or_else(|| Ok(config.validator.address.clone()))?,
        &common::signing::DirectoryStoredKeys::new(&config.secrets.path)?.chronicle_signing()?,
    ))
}

#[allow(dead_code)]
fn state_delta(config: &Config, options: &ArgMatches) -> Result<StateDelta, SignerError> {
    Ok(StateDelta::new(
        &options
            .value_of("sawtooth")
            .map(Url::parse)
            .unwrap_or_else(|| Ok(config.validator.address.clone()))?,
        &common::signing::DirectoryStoredKeys::new(&config.secrets.path)?.chronicle_signing()?,
    ))
}

#[cfg(feature = "inmem")]
fn ledger() -> Result<common::ledger::InMemLedger, std::convert::Infallible> {
    Ok(common::ledger::InMemLedger::new())
}

#[derive(Debug, Clone)]
struct UniqueUuid;

impl UuidGen for UniqueUuid {}

type ConnectionPool = Pool<ConnectionManager<SqliteConnection>>;
fn pool(config: &Config) -> Result<ConnectionPool, ApiError> {
    Ok(Pool::builder()
        .connection_customizer(Box::new(ConnectionOptions {
            enable_wal: true,
            enable_foreign_keys: true,
            busy_timeout: Some(Duration::from_secs(2)),
        }))
        .build(ConnectionManager::<SqliteConnection>::new(
            &*Path::join(&config.store.path, &PathBuf::from("db.sqlite")).to_string_lossy(),
        ))?)
}

fn graphql_addr(options: &ArgMatches) -> Result<Option<SocketAddr>, ApiError> {
    if !options.is_present("gql") {
        Ok(None)
    } else if let Some(addr) = options.value_of("gql-interface") {
        Ok(Some(addr.parse()?))
    } else {
        Ok(None)
    }
}

pub async fn graphql_server<Query, Mutation>(
    api: &ApiDispatch,
    pool: &ConnectionPool,
    gql: ChronicleGraphQl<Query, Mutation>,
    options: &ArgMatches,
    open: bool,
) -> Result<(), ApiError>
where
    Query: ObjectType + Copy,
    Mutation: ObjectType + Copy,
{
    if let Some(addr) = graphql_addr(options)? {
        gql.serve_graphql(pool.clone(), api.clone(), addr, open)
            .await
    }

    Ok(())
}

#[cfg(not(feature = "inmem"))]
pub async fn api(
    pool: &ConnectionPool,
    options: &ArgMatches,
    config: &Config,
) -> Result<ApiDispatch, ApiError> {
    let submitter = submitter(config, options)?;
    let state = state_delta(config, options)?;

    Api::new(
        pool.clone(),
        submitter,
        state,
        &config.secrets.path,
        UniqueUuid,
    )
    .await
}

#[cfg(feature = "inmem")]
pub async fn api(
    pool: &ConnectionPool,
    _options: &ArgMatches,
    config: &Config,
) -> Result<api::ApiDispatch, ApiError> {
    let mut ledger = ledger()?;
    let state = ledger.reader();

    Api::new(
        pool.clone(),
        ledger,
        state,
        &config.secrets.path,
        UniqueUuid,
    )
    .await
}

fn domain_type(args: &ArgMatches) -> Option<DomaintypeId> {
    if !args.is_present("domaintype") {
        None
    } else {
        args.value_of("domaintype").map(DomaintypeId::from_name)
    }
}

#[instrument(skip(gql))]
async fn execute_arguments<Query, Mutation>(
    gql: ChronicleGraphQl<Query, Mutation>,
    config: Config,
    options: &ArgMatches,
) -> Result<(ApiResponse, ApiDispatch), ApiError>
where
    Query: ObjectType + Copy,
    Mutation: ObjectType + Copy,
{
    dotenv::dotenv().ok();

    let pool = pool(&config)?;
    let api = api(&pool, options, &config).await?;
    let ret_api = api.clone();

    let execution = vec![
        options.subcommand_matches("namespace").and_then(|m| {
            m.subcommand_matches("create").map(|m| {
                api.dispatch(ApiCommand::NameSpace(NamespaceCommand::Create {
                    name: m.value_of("namespace").unwrap().into(),
                }))
            })
        }),
        options.subcommand_matches("agent").and_then(|m| {
            vec![
                m.subcommand_matches("create").map(|m| {
                    api.dispatch(ApiCommand::Agent(AgentCommand::Create {
                        name: m.value_of("agent_name").unwrap().into(),
                        namespace: m.value_of("namespace").unwrap().into(),
                        attributes: Attributes::type_only(domain_type(m)),
                    }))
                }),
                m.subcommand_matches("register-key").map(|m| {
                    let registration = {
                        if m.is_present("generate") {
                            KeyRegistration::Generate
                        } else if m.is_present("privatekey") {
                            KeyRegistration::ImportSigning(KeyImport::FromPath {
                                path: m.value_of_t::<PathBuf>("privatekey").unwrap(),
                            })
                        } else {
                            KeyRegistration::ImportVerifying(KeyImport::FromPath {
                                path: m.value_of_t::<PathBuf>("privatekey").unwrap(),
                            })
                        }
                    };

                    api.dispatch(ApiCommand::Agent(AgentCommand::RegisterKey {
                        id: AgentId::from_name(m.value_of("agent_name").unwrap()),
                        namespace: m.value_of("namespace").unwrap().into(),
                        registration,
                    }))
                }),
                m.subcommand_matches("use").map(|m| {
                    api.dispatch(ApiCommand::Agent(AgentCommand::UseInContext {
                        id: AgentId::from_name(m.value_of("agent_name").unwrap()),
                        namespace: m.value_of("namespace").unwrap().into(),
                    }))
                }),
            ]
            .into_iter()
            .flatten()
            .next()
        }),
        options.subcommand_matches("activity").and_then(|m| {
            vec![
                m.subcommand_matches("create").map(|m| {
                    api.dispatch(ApiCommand::Activity(ActivityCommand::Create {
                        name: m.value_of("activity_name").unwrap().into(),
                        namespace: m.value_of("namespace").unwrap().into(),
                        attributes: Attributes::type_only(domain_type(m)),
                    }))
                }),
                m.subcommand_matches("start").map(|m| {
                    api.dispatch(ApiCommand::Activity(ActivityCommand::Start {
                        id: ActivityId::from_name(m.value_of("activity_name").unwrap()),
                        namespace: m.value_of("namespace").unwrap().into(),
                        time: None,
                        agent: None,
                    }))
                }),
                m.subcommand_matches("end").map(|m| {
                    api.dispatch(ApiCommand::Activity(ActivityCommand::End {
                        id: m.value_of("activity_name").map(ActivityId::from_name),
                        namespace: m.value_of("namespace").unwrap().into(),
                        time: None,
                        agent: None,
                    }))
                }),
                m.subcommand_matches("use").map(|m| {
                    api.dispatch(ApiCommand::Activity(ActivityCommand::Use {
                        id: EntityId::from_name(m.value_of("entity_name").unwrap()),
                        namespace: m.value_of("namespace").unwrap().into(),
                        activity: m.value_of("activity_name").map(ActivityId::from_name),
                    }))
                }),
                m.subcommand_matches("generate").map(|m| {
                    api.dispatch(ApiCommand::Activity(ActivityCommand::Generate {
                        id: EntityId::from_name(m.value_of("entity_name").unwrap()),
                        namespace: m.value_of("namespace").unwrap().into(),
                        activity: m.value_of("activity_name").map(ActivityId::from_name),
                    }))
                }),
            ]
            .into_iter()
            .flatten()
            .next()
        }),
        options.subcommand_matches("entity").and_then(|m| {
            vec![m.subcommand_matches("attach").map(|m| {
                api.dispatch(ApiCommand::Entity(EntityCommand::Attach {
                    id: EntityId::from_name(m.value_of("entity_name").unwrap()),
                    namespace: m.value_of("namespace").unwrap().into(),
                    file: PathOrFile::Path(m.value_of_t::<PathBuf>("file").unwrap()),
                    locator: m.value_of("locator").map(|x| x.to_owned()),
                    agent: m.value_of("agent").map(AgentId::from_name),
                }))
            })]
            .into_iter()
            .flatten()
            .next()
        }),
        options.subcommand_matches("export").map(|m| {
            api.dispatch(ApiCommand::Query(QueryCommand {
                namespace: m.value_of("namespace").unwrap().to_owned(),
            }))
        }),
    ]
    .into_iter()
    .flatten()
    .next();

    // If we actually execute a command, then do not run the api
    if let Some(execution) = execution {
        let exresult = execution.await;

        Ok((exresult?, ret_api))
    } else {
        graphql_server(&api, &pool, gql, options, options.is_present("open")).await?;

        Ok((ApiResponse::Unit, ret_api))
    }
}

custom_error! {pub CliError
    Api{source: api::ApiError}                  = "Api error",
    Keys{source: SignerError}                   = "Key storage",
    FileSystem{source: std::io::Error}          = "Cannot locate configuration file",
    ConfigInvalid{source: toml::de::Error}      = "Invalid configuration file",
    InvalidPath                                 = "Invalid path", //TODO - the path, you know how annoying this is
    Ld{source: CompactionError}                 = "Invalid Json LD",
    CommitNoticiationStream {source: RecvError} = "Failure in commit notification stream"
}

impl UFE for CliError {}

async fn config_and_exec<Query, Mutation>(
    gql: ChronicleGraphQl<Query, Mutation>,
    matches: &ArgMatches,
) -> Result<(), CliError>
where
    Query: ObjectType + Copy,
    Mutation: ObjectType + Copy,
{
    use colored_json::prelude::*;
    let config = handle_config_and_init(matches)?;
    let response = execute_arguments(gql, config, matches).await?;

    match response {
        (
            ApiResponse::Submission {
                subject,
                prov: _,
                correlation_id,
            },
            api,
        ) => {
            // For commands that have initiated a ledger operation, wait for the matching result
            let mut tx_notifications = api.notify_commit.subscribe();

            loop {
                let (_prov, incoming_correlation_id) =
                    tx_notifications.recv().await.map_err(CliError::from)?;
                if correlation_id == incoming_correlation_id {
                    println!("{}", subject);
                    break;
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
    };
    Ok(())
}

fn print_completions<G: Generator>(gen: G, app: &mut Command) {
    generate(gen, app, app.get_name().to_string(), &mut io::stdout());
}

pub async fn bootstrap<Query, Mutation>(gql: ChronicleGraphQl<Query, Mutation>)
where
    Query: ObjectType + 'static + Copy,
    Mutation: ObjectType + 'static + Copy,
{
    let matches = cli().get_matches();

    if let Ok(generator) = matches.value_of_t::<Shell>("completions") {
        let mut app = cli();
        eprintln!("Generating completion file for {}...", generator);
        print_completions(generator, &mut app);
        std::process::exit(0);
    }

    if matches.is_present("export-schema") {
        print!("{}", gql.exportable_schema());
        std::process::exit(0);
    }

    if matches.is_present("console-logging") {
        telemetry::console_logging();
    }

    if matches.is_present("instrument") {
        telemetry::telemetry(
            Url::parse(&*matches.value_of_t::<String>("instrument").unwrap()).unwrap(),
        );
    }

    config_and_exec(gql, &matches)
        .await
        .map_err(|e| {
            error!(?e, "Api error");
            e.into_ufe().print();
            std::process::exit(1);
        })
        .ok();

    std::process::exit(0);
}
