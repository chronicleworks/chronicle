#![cfg_attr(feature = "strict", deny(warnings))]
#[macro_use]
extern crate serde_derive;

mod cli;
mod config;
mod telemetry;

use api::{Api, ApiDispatch, ApiError, UuidGen};
use clap::{ArgMatches, Command};
use clap_complete::{generate, Generator, Shell};
use cli::cli;

use common::{
    commands::{
        ActivityCommand, AgentCommand, ApiCommand, ApiResponse, EntityCommand, KeyImport,
        KeyRegistration, NamespaceCommand, PathOrFile, QueryCommand,
    },
    prov::CompactionError,
    signing::SignerError,
};
use config::*;
use custom_error::custom_error;
use futures::Future;
use sawtooth_protocol::{events::StateDelta, messaging::SawtoothSubmitter};
use tokio::sync::broadcast::error::RecvError;

use std::{
    io,
    path::{Path, PathBuf},
};

use tracing::{error, instrument};
use user_error::UFE;

#[allow(dead_code)]
fn submitter(config: &Config, options: &ArgMatches) -> Result<SawtoothSubmitter, SignerError> {
    use url::Url;

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
    use url::Url;

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

async fn api(
    options: &ArgMatches,
    config: &Config,
) -> Result<(ApiDispatch, impl Future<Output = ()>), ApiError> {
    #[cfg(not(feature = "inmem"))]
    {
        let submitter = submitter(config, options)?;
        let state = state_delta(config, options)?;

        Api::new(
            options.value_of("ui-interface").unwrap().parse()?,
            &*Path::join(&config.store.path, &PathBuf::from("db.sqlite")).to_string_lossy(),
            submitter,
            state,
            &config.secrets.path,
            UniqueUuid,
        )
        .await
    }
    #[cfg(feature = "inmem")]
    {
        let mut ledger = ledger()?;
        let state = ledger.reader();

        Ok(Api::new(
            options.value_of("ui-interface").unwrap().parse()?,
            &*Path::join(&config.store.path, &PathBuf::from("db.sqlite")).to_string_lossy(),
            ledger,
            state,
            &config.secrets.path,
            UniqueUuid,
        )
        .await?)
    }
}

fn domain_type(args: &ArgMatches) -> Option<String> {
    if !args.is_present("domaintype") {
        None
    } else {
        args.value_of("domaintype").map(|x| x.to_owned())
    }
}

#[instrument]
async fn api_exec(
    config: Config,
    options: &ArgMatches,
) -> Result<(ApiResponse, ApiDispatch), ApiError> {
    dotenv::dotenv().ok();

    let (api, ui) = api(options, &config).await?;
    let ret_api = api.clone();

    let execution = vec![
        options.subcommand_matches("namespace").and_then(|m| {
            m.subcommand_matches("create").map(|m| {
                api.dispatch(ApiCommand::NameSpace(NamespaceCommand::Create {
                    name: m.value_of("namespace").unwrap().to_owned(),
                }))
            })
        }),
        options.subcommand_matches("agent").and_then(|m| {
            vec![
                m.subcommand_matches("create").map(|m| {
                    api.dispatch(ApiCommand::Agent(AgentCommand::Create {
                        name: m.value_of("agent_name").unwrap().to_owned(),
                        namespace: m.value_of("namespace").unwrap().to_owned(),
                        domaintype: domain_type(m),
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
                        name: m.value_of("agent_name").unwrap().to_owned(),
                        namespace: m.value_of("namespace").unwrap().to_owned(),
                        registration,
                    }))
                }),
                m.subcommand_matches("use").map(|m| {
                    api.dispatch(ApiCommand::Agent(AgentCommand::Use {
                        name: m.value_of("agent_name").unwrap().to_owned(),
                        namespace: m.value_of("namespace").unwrap().to_owned(),
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
                        name: m.value_of("activity_name").unwrap().to_owned(),
                        namespace: m.value_of("namespace").unwrap().to_owned(),
                        domaintype: domain_type(m),
                    }))
                }),
                m.subcommand_matches("start").map(|m| {
                    api.dispatch(ApiCommand::Activity(ActivityCommand::Start {
                        name: m.value_of("activity_name").unwrap().to_owned(),
                        namespace: m.value_of("namespace").unwrap().to_owned(),
                        time: None,
                        agent: None,
                    }))
                }),
                m.subcommand_matches("end").map(|m| {
                    api.dispatch(ApiCommand::Activity(ActivityCommand::End {
                        name: m.value_of("activity_name").map(|x| x.to_owned()),
                        namespace: m.value_of("namespace").unwrap().to_owned(),
                        time: None,
                        agent: None,
                    }))
                }),
                m.subcommand_matches("use").map(|m| {
                    api.dispatch(ApiCommand::Activity(ActivityCommand::Use {
                        name: m.value_of("entity_name").unwrap().to_owned(),
                        namespace: m.value_of("namespace").unwrap().to_owned(),
                        activity: m.value_of("activity_name").map(|x| x.to_owned()),
                        domaintype: domain_type(m),
                    }))
                }),
                m.subcommand_matches("generate").map(|m| {
                    api.dispatch(ApiCommand::Activity(ActivityCommand::Generate {
                        name: m.value_of("entity_name").unwrap().to_owned(),
                        namespace: m.value_of("namespace").unwrap().to_owned(),
                        activity: m.value_of("activity_name").map(|x| x.to_owned()),
                        domaintype: domain_type(m),
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
                    name: m.value_of("entity_name").unwrap().to_owned(),
                    namespace: m.value_of("namespace").unwrap().to_owned(),
                    file: PathOrFile::Path(m.value_of_t::<PathBuf>("file").unwrap()),
                    locator: m.value_of("locator").map(|x| x.to_owned()),
                    agent: m.value_of("agent").map(|x| x.to_owned()),
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
        ui.await;
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

#[tokio::main]
async fn main() {
    let matches = cli().get_matches();

    if let Ok(generator) = matches.value_of_t::<Shell>("completions") {
        let mut app = cli();
        eprintln!("Generating completion file for {}...", generator);
        print_completions(generator, &mut app);
        std::process::exit(0);
    }

    if matches.is_present("instrument") {
        telemetry::tracing();
    }

    config_and_exec(&matches)
        .await
        .map_err(|e| {
            error!(?e, "Api error");
            e.into_ufe().print();
            std::process::exit(1);
        })
        .ok();

    std::process::exit(0);
}

async fn config_and_exec(matches: &ArgMatches) -> Result<(), CliError> {
    use colored_json::prelude::*;
    let config = handle_config_and_init(matches)?;
    let response = api_exec(config, matches).await?;

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
