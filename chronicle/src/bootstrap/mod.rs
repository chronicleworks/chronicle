mod cli;
mod config;

use api::{
    chronicle_graphql::{ChronicleGraphQl, ChronicleGraphQlServer},
    Api, ApiDispatch, ApiError, ConnectionOptions, UuidGen,
};
use async_graphql::ObjectType;
use clap::{ArgMatches, Command};
use clap_complete::{generate, Generator, Shell};
pub use cli::*;
use common::{commands::ApiResponse, ledger::SubmissionStage, signing::DirectoryStoredKeys};

use tracing::{debug, error, info, instrument};
use user_error::UFE;

use common::signing::SignerError;
use config::*;
use diesel::{
    r2d2::{ConnectionManager, Pool},
    SqliteConnection,
};

use sawtooth_protocol::{events::StateDelta, messaging::SawtoothSubmitter};
use telemetry::{self, ConsoleLogging};
use url::Url;

use common::prov::to_json_ld::ToJson;
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
            .map(|s| Url::parse(s))
            .unwrap_or_else(|| Ok(config.validator.address.clone()))?,
        &common::signing::DirectoryStoredKeys::new(&config.secrets.path)?.chronicle_signing()?,
    ))
}

#[allow(dead_code)]
fn state_delta(config: &Config, options: &ArgMatches) -> Result<StateDelta, SignerError> {
    Ok(StateDelta::new(
        &options
            .get_one::<String>("sawtooth")
            .map(|s| Url::parse(s))
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
        config.namespace_bindings.clone(),
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
        config.namespace_bindings.clone(),
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
                            eprintln!("Transaction rejected by chronicle: {} {}", err, err.tx_id());
                            break;
                        }
                    }
                    SubmissionStage::Committed(Ok(commit)) => {
                        if commit.tx_id == tx_id {
                            debug!("Transaction committed: {}", commit.tx_id);
                        }
                        println!("{}", subject);
                    }
                    SubmissionStage::Committed(Err((id, contradiction))) => {
                        if id == tx_id {
                            eprintln!("Transaction rejected by ledger: {} {}", id, contradiction);
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
    telemetry::telemetry(
        matches
            .get_one::<String>("instrument")
            .and_then(|s| Url::parse(s).ok()),
        if matches.contains_id("console-logging") {
            match matches.get_one::<String>("console-logging") {
                Some(level) => match level.as_str() {
                    "pretty" => ConsoleLogging::Pretty,
                    "json" => ConsoleLogging::Json,
                    _ => ConsoleLogging::Off,
                },
                _ => ConsoleLogging::Off,
            }
        } else if matches.subcommand_name() == Some("serve-graphql") {
            ConsoleLogging::Pretty
        } else {
            ConsoleLogging::Off
        },
    );

    if matches.subcommand_matches("verify-keystore").is_some() {
        let config = handle_config_and_init(&domain.into()).unwrap();
        let store = DirectoryStoredKeys::new(&config.secrets.path).unwrap();
        info!(keystore=?store);

        if store.chronicle_signing().is_err() {
            info!("Generating new chronicle key");
            store.generate_chronicle().unwrap();
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
    use std::collections::HashMap;

    use api::{Api, ApiDispatch, ApiError, ConnectionOptions, UuidGen};
    use common::{
        commands::{ApiCommand, ApiResponse},
        ledger::{InMemLedger, SubmissionStage},
        prov::{
            to_json_ld::ToJson, ActivityId, AgentId, ChronicleIri, ChronicleTransactionId,
            EntityId, ProvModel,
        },
    };

    use diesel::{
        r2d2::{ConnectionManager, Pool},
        SqliteConnection,
    };

    use tempfile::TempDir;
    use uuid::Uuid;

    use super::{CliModel, SubCommand};
    use crate::codegen::ChronicleDomainDef;

    #[derive(Clone)]
    struct TestDispatch(ApiDispatch);

    impl TestDispatch {
        pub async fn dispatch(
            &mut self,
            command: ApiCommand,
        ) -> Result<Option<(Box<ProvModel>, ChronicleTransactionId)>, ApiError> {
            // We can sort of get final on chain state here by using a map of subject to model
            if let ApiResponse::Submission { .. } = self.0.dispatch(command).await? {
                loop {
                    let submission = self.0.notify_commit.subscribe().recv().await.unwrap();

                    if let SubmissionStage::Committed(Ok(commit)) = submission {
                        break Ok(Some((commit.delta, commit.tx_id)));
                    }
                    if let SubmissionStage::Committed(Err((_, contradiction))) = submission {
                        panic!("Contradiction: {}", contradiction);
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

    async fn test_api() -> TestDispatch {
        telemetry::telemetry(None, telemetry::ConsoleLogging::Pretty);

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

        let dispatch = Api::new(
            pool,
            ledger,
            reader,
            &secretpath.into_path(),
            SameUuid,
            HashMap::default(),
        )
        .await
        .unwrap();

        TestDispatch(dispatch)
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
        let command_line = r#"chronicle test-agent-agent define test_agent --test-bool-attr false --test-string-attr "test" --test-int-attr 23 --namespace testns "#;

        insta::assert_snapshot!(
          serde_json::to_string_pretty(
          &parse_and_execute(command_line, test_cli_model()).await.to_json().compact_stable_order().await.unwrap()
        ).unwrap() , @r###"
        {
          "@context": "https://blockchaintp.com/chr/1.0/c.jsonld",
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
          "@context": "https://blockchaintp.com/chr/1.0/c.jsonld",
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
    async fn agent_register_key() {
        let mut api = test_api().await;
        let id = ChronicleIri::from(common::prov::AgentId::from_external_id("test_agent"));
        let command_line =
            format!(r#"chronicle test-agent-agent register-key --namespace testns {id} -g "#);
        let cmd = get_api_cmd(&command_line);
        let delta = api.dispatch(cmd).await.unwrap().unwrap();
        insta::assert_yaml_snapshot!(delta.0, {
            ".*.*.public_key" => "[public]",
            ".*.*.*.public_key" => "[public]"
        }, @r###"
        ---
        namespaces:
          ? external_id: testns
            uuid: 5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea
          : id:
              external_id: testns
              uuid: 5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea
            uuid: 5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea
            external_id: testns
        agents:
          ? - external_id: testns
              uuid: 5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea
            - test_agent
          : id: test_agent
            namespaceid:
              external_id: testns
              uuid: 5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea
            external_id: test_agent
            domaintypeid: ~
            attributes: {}
        activities: {}
        entities: {}
        identities:
          ? - external_id: testns
              uuid: 5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea
            - external_id: test_agent
              public_key: "[public]"
          : id:
              external_id: test_agent
              public_key: "[public]"
            namespaceid:
              external_id: testns
              uuid: 5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea
            public_key: "[public]"
        attachments: {}
        has_identity:
          ? - external_id: testns
              uuid: 5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea
            - test_agent
          : - external_id: testns
              uuid: 5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea
            - external_id: test_agent
              public_key: "[public]"
        had_identity: {}
        has_evidence: {}
        had_attachment: {}
        association: {}
        derivation: {}
        delegation: {}
        generation: {}
        usage: {}
        was_informed_by: {}
        generated: {}
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
          &api.dispatch(cmd).await.unwrap().unwrap().0.to_json().compact_stable_order().await.unwrap()
        ).unwrap() , @r###"
        {
          "@context": "https://blockchaintp.com/chr/1.0/c.jsonld",
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

        api.dispatch(cmd).await.unwrap();

        let id = ActivityId::from_external_id("testactivity");
        let command_line = format!(
            r#"chronicle test-activity-activity start {id} --namespace testns --time 2014-07-08T09:10:11Z "#
        );
        let cmd = get_api_cmd(&command_line);

        insta::assert_snapshot!(
          serde_json::to_string_pretty(
          &api.dispatch(cmd).await.unwrap().unwrap().0.to_json().compact_stable_order().await.unwrap()
        ).unwrap() , @r###"
        {
          "@context": "https://blockchaintp.com/chr/1.0/c.jsonld",
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
          "@context": "https://blockchaintp.com/chr/1.0/c.jsonld",
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
          "@context": "https://blockchaintp.com/chr/1.0/c.jsonld",
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
          &api.dispatch(cmd).await.unwrap().unwrap().0.to_json().compact_stable_order().await.unwrap()
        ).unwrap() , @r###"
        {
          "@context": "https://blockchaintp.com/chr/1.0/c.jsonld",
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
          &api.dispatch(cmd).await.unwrap().unwrap().0.to_json().compact_stable_order().await.unwrap()
        ).unwrap() , @r###"
        {
          "@context": "https://blockchaintp.com/chr/1.0/c.jsonld",
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
          &api.dispatch(cmd).await.unwrap().unwrap().0.to_json().compact_stable_order().await.unwrap()
        ).unwrap() , @r###"
        {
          "@context": "https://blockchaintp.com/chr/1.0/c.jsonld",
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
          &api.dispatch(cmd).await.unwrap().unwrap().0.to_json().compact_stable_order().await.unwrap()
        ).unwrap() , @r###"
        {
          "@context": "https://blockchaintp.com/chr/1.0/c.jsonld",
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
          "@context": "https://blockchaintp.com/chr/1.0/c.jsonld",
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
          "@context": "https://blockchaintp.com/chr/1.0/c.jsonld",
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
        api.dispatch(cmd).await.unwrap();

        let id = ChronicleIri::from(AgentId::from_external_id("testagent"));
        let command_line = format!(r#"chronicle test-agent-agent use --namespace testns {id} "#);
        let cmd = get_api_cmd(&command_line);
        api.dispatch(cmd).await.unwrap();

        let id = ChronicleIri::from(ActivityId::from_external_id("testactivity"));
        let command_line = format!(
            r#"chronicle test-activity-activity start {id} --namespace testns --time 2014-07-08T09:10:11Z "#
        );
        let cmd = get_api_cmd(&command_line);

        insta::assert_snapshot!(
          serde_json::to_string_pretty(
          &api.dispatch(cmd).await.unwrap().unwrap().0.to_json().compact_stable_order().await.unwrap()
        ).unwrap() , @r###"
        {
          "@context": "https://blockchaintp.com/chr/1.0/c.jsonld",
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
        api.dispatch(cmd).await.unwrap();

        let id = ChronicleIri::from(AgentId::from_external_id("testagent"));
        let command_line = format!(r#"chronicle test-agent-agent use --namespace testns {id} "#);
        let cmd = get_api_cmd(&command_line);
        api.dispatch(cmd).await.unwrap();

        let id = ChronicleIri::from(ActivityId::from_external_id("testactivity"));
        let command_line = format!(
            r#"chronicle test-activity-activity start {id} --namespace testns --time 2014-07-08T09:10:11Z "#
        );
        let cmd = get_api_cmd(&command_line);
        api.dispatch(cmd).await.unwrap();

        // Should end the last opened activity
        let id = ActivityId::from_external_id("testactivity");
        let command_line = format!(
            r#"chronicle test-activity-activity end --namespace testns --time 2014-08-09T09:10:12Z {id} "#
        );
        let cmd = get_api_cmd(&command_line);

        insta::assert_snapshot!(
          serde_json::to_string_pretty(
          &api.dispatch(cmd).await.unwrap().unwrap().0.to_json().compact_stable_order().await.unwrap()
        ).unwrap() , @r###"
        {
          "@context": "https://blockchaintp.com/chr/1.0/c.jsonld",
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
        api.dispatch(cmd).await.unwrap();

        let activity_id = ActivityId::from_external_id("testactivity");
        let entity_id = EntityId::from_external_id("testentity");
        let command_line = format!(
            r#"chronicle test-activity-activity generate --namespace testns {entity_id} {activity_id} "#
        );
        let cmd = get_api_cmd(&command_line);

        insta::assert_snapshot!(
          serde_json::to_string_pretty(
          &api.dispatch(cmd).await.unwrap().unwrap().0.to_json().compact_stable_order().await.unwrap()
        ).unwrap() , @r###"
        {
          "@context": "https://blockchaintp.com/chr/1.0/c.jsonld",
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
        api.dispatch(cmd).await.unwrap();

        let id = ChronicleIri::from(AgentId::from_external_id("testagent"));
        let command_line = format!(r#"chronicle test-agent-agent use --namespace testns {id} "#);
        let cmd = get_api_cmd(&command_line);
        api.dispatch(cmd).await.unwrap();

        let command_line = r#"chronicle test-activity-activity define testactivity --namespace testns --test-string-attr "test" --test-bool-attr true --test-int-attr 40 "#;
        let cmd = get_api_cmd(command_line);
        api.dispatch(cmd).await.unwrap();

        let activity_id = ActivityId::from_external_id("testactivity");
        let entity_id = EntityId::from_external_id("testentity");
        let command_line = format!(
            r#"chronicle test-activity-activity use --namespace testns {entity_id} {activity_id} "#
        );

        let cmd = get_api_cmd(&command_line);

        insta::assert_snapshot!(
          serde_json::to_string_pretty(
          &api.dispatch(cmd).await.unwrap().unwrap().0.to_json().compact_stable_order().await.unwrap()
        ).unwrap() , @r###"
        {
          "@context": "https://blockchaintp.com/chr/1.0/c.jsonld",
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
