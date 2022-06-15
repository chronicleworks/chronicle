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
use common::commands::ApiResponse;

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

use crate::codegen::ChronicleDomainDef;

#[allow(dead_code)]
fn submitter(config: &Config, options: &ArgMatches) -> Result<SawtoothSubmitter, SignerError> {
    Ok(SawtoothSubmitter::new(
        &options
            .get_one::<String>("sawtooth")
            .map(|s| Url::parse(&*s))
            .unwrap_or_else(|| Ok(config.validator.address.clone()))?,
        &common::signing::DirectoryStoredKeys::new(&config.secrets.path)?.chronicle_signing()?,
    ))
}

#[allow(dead_code)]
fn state_delta(config: &Config, options: &ArgMatches) -> Result<StateDelta, SignerError> {
    Ok(StateDelta::new(
        &options
            .get_one::<String>("sawtooth")
            .map(|s| Url::parse(&*s))
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
    if let Some(addr) = options.get_one::<String>("interface") {
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
    dotenv::dotenv().ok();

    let matches = cli.as_cmd().get_matches();
    let pool = pool(&config)?;
    let api = api(&pool, &matches, &config).await?;
    let ret_api = api.clone();

    let api = api.clone();

    if let Some(matches) = matches.subcommand_matches("serve-graphql") {
        graphql_server(&api, &pool, gql, matches, matches.contains_id("open")).await?;

        Ok((ApiResponse::Unit, ret_api))
    } else if let Some(cmd) = cli.matches(&matches)? {
        Ok((api.dispatch(cmd).await?, ret_api))
    } else {
        Ok((ApiResponse::Unit, ret_api))
    }
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

pub async fn bootstrap<Query, Mutation>(
    domain: ChronicleDomainDef,
    gql: ChronicleGraphQl<Query, Mutation>,
) where
    Query: ObjectType + 'static + Copy,
    Mutation: ObjectType + 'static + Copy,
{
    let matches = cli(domain.clone()).as_cmd().get_matches();

    if let Some(generator) = matches.subcommand_matches("completions") {
        let shell = generator.get_one::<Shell>("shell").unwrap();
        print_completions(shell.to_owned(), &mut cli(domain.clone()).as_cmd());
        std::process::exit(0);
    }

    if matches.subcommand_matches("export-schema").is_some() {
        print!("{}", gql.exportable_schema());
        std::process::exit(0);
    }

    if matches.contains_id("console-logging") {
        telemetry::console_logging();
    }

    if matches.contains_id("instrument") {
        telemetry::telemetry(
            Url::parse(&*matches.get_one::<String>("instrument").unwrap()).unwrap(),
        );
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
    use api::{Api, ApiDispatch, ApiError, ConnectionOptions, UuidGen};

    use common::{
        commands::{ApiCommand, ApiResponse},
        ledger::InMemLedger,
        prov::{ChronicleTransactionId, ProvModel},
    };

    use diesel::{
        r2d2::{ConnectionManager, Pool},
        SqliteConnection,
    };
    use tempfile::TempDir;
    use tracing::Level;
    use tracing_log::log::LevelFilter;
    use uuid::Uuid;

    use crate::codegen::ChronicleDomainDef;

    use super::{CliModel, SubCommand};

    #[derive(Clone)]
    struct TestDispatch(ApiDispatch, ProvModel);

    impl TestDispatch {
        pub async fn dispatch(
            &mut self,
            command: ApiCommand,
        ) -> Result<Option<(ProvModel, ChronicleTransactionId)>, ApiError> {
            // We can sort of get final on chain state here by using a map of subject to model
            if let ApiResponse::Submission { prov, .. } = self.0.dispatch(command).await? {
                self.1.merge(*prov);

                Ok(Some(self.0.notify_commit.subscribe().recv().await.unwrap()))
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

    async fn test_api() -> TestDispatch {
        tracing_log::LogTracer::init_with_filter(LevelFilter::Trace).ok();
        tracing_subscriber::fmt()
            .pretty()
            .with_max_level(Level::TRACE)
            .try_init()
            .ok();

        let secretpath = TempDir::new().unwrap();
        // We need to use a real file for sqlite, as in mem either re-creates between
        // macos temp dir permissions don't work with sqlite
        std::fs::create_dir("./sqlite_test").ok();
        let dbid = Uuid::new_v4();
        let mut ledger = InMemLedger::new();
        let reader = ledger.reader();

        let pool = Pool::builder()
            .connection_customizer(Box::new(ConnectionOptions {
                enable_wal: true,
                enable_foreign_keys: true,
                busy_timeout: Some(std::time::Duration::from_secs(2)),
            }))
            .build(ConnectionManager::<SqliteConnection>::new(&*format!(
                "./sqlite_test/db{}.sqlite",
                dbid
            )))
            .unwrap();

        let dispatch = Api::new(pool, ledger, reader, &secretpath.into_path(), SameUuid)
            .await
            .unwrap();

        TestDispatch(dispatch, ProvModel::default())
    }

    macro_rules! assert_json_ld {
        ($x:expr) => {
            let mut v: serde_json::Value =
                serde_json::from_str(&*$x.await.to_json().compact().await.unwrap().to_string())
                    .unwrap();

            // Sort @graph by //@id, as objects are unordered
            if let Some(v) = v.pointer_mut("/@graph") {
                v.as_array_mut().unwrap().sort_by(|l, r| {
                    l.as_object()
                        .unwrap()
                        .get("@id")
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .cmp(r.as_object().unwrap().get("@id").unwrap().as_str().unwrap())
                });
            }

            insta::assert_snapshot!(serde_json::to_string_pretty(&v).unwrap());
        };
    }

    async fn parse_and_execute(command_line: &str, cli: CliModel) -> ProvModel {
        let mut api = test_api().await;

        let matches = cli
            .as_cmd()
            .get_matches_from(command_line.split_whitespace());

        let cmd = cli.matches(&matches).unwrap().unwrap();

        api.dispatch(cmd).await.unwrap().unwrap().0
    }

    fn test_cli_model() -> CliModel {
        CliModel::from(
            ChronicleDomainDef::build("test")
                .with_attribute_type("testString", crate::PrimitiveType::String)
                .unwrap()
                .with_attribute_type("testBool", crate::PrimitiveType::Bool)
                .unwrap()
                .with_attribute_type("testInt", crate::PrimitiveType::Int)
                .unwrap()
                .with_activity("testActivity", |b| {
                    b.with_attribute("testString")
                        .unwrap()
                        .with_attribute("testBool")
                        .unwrap()
                        .with_attribute("testInt")
                })
                .unwrap()
                .with_agent("testAgent", |b| {
                    b.with_attribute("testString")
                        .unwrap()
                        .with_attribute("testBool")
                        .unwrap()
                        .with_attribute("testInt")
                })
                .unwrap()
                .with_entity("testEntity", |b| {
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
        assert_json_ld!(parse_and_execute(
            r#"chronicle test-agent define test_agent --test-bool-attr false --test-string-attr "test" --test-int-attr 23 "#,
            test_cli_model()
        ));
    }
}
