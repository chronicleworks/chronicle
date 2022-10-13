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
use common::{commands::ApiResponse, signing::DirectoryStoredKeys};

use tracing::{error, info, instrument};
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
    telemetry::telemetry(
        matches
            .get_one::<String>("instrument")
            .and_then(|s| Url::parse(&*s).ok()),
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
        ledger::InMemLedger,
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

        TestDispatch(dispatch, ProvModel::default())
    }

    fn get_api_cmd(command_line: &str) -> ApiCommand {
        let cli = test_cli_model();
        let matches = cli
            .as_cmd()
            .get_matches_from(command_line.split_whitespace());
        cli.matches(&matches).unwrap().unwrap()
    }

    async fn parse_and_execute(command_line: &str, cli: CliModel) -> ProvModel {
        let mut api = test_api().await;

        let matches = cli
            .as_cmd()
            .get_matches_from(command_line.split_whitespace());

        let cmd = cli.matches(&matches).unwrap().unwrap();

        api.dispatch(cmd).await.unwrap().unwrap().0
    }

    // Sort @graph by //@id, as objects are unordered
    fn sort_graph(mut v: serde_json::Value) -> serde_json::Value {
        let mut ok = false;
        if let Some(v) = v.pointer_mut("/@graph") {
            ok = true;
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
        assert!(ok);
        v
    }

    async fn sort_prov_model(prov_model: ProvModel) -> serde_json::Value {
        let v: serde_json::Value =
            serde_json::from_str(&prov_model.to_json().compact().await.unwrap().to_string())
                .unwrap();
        sort_graph(v)
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
        let command_line = r#"chronicle test-agent define test_agent --test-bool-attr false --test-string-attr "test" --test-int-attr 23 --namespace testns "#;
        let prov_model = parse_and_execute(command_line, test_cli_model()).await;
        let sorted = sort_prov_model(prov_model).await;
        insta::assert_snapshot!(serde_json::to_string_pretty(&sorted).unwrap(), @r###"
        {
          "@context": {
            "@version": 1.1,
            "actedOnBehalfOf": {
              "@container": "@set",
              "@id": "prov:actedOnBehalfOf",
              "@type": "@id"
            },
            "activity": {
              "@id": "prov:activity",
              "@type": "@id"
            },
            "agent": {
              "@id": "prov:agent",
              "@type": "@id"
            },
            "attachment": {
              "@id": "chronicle:hasAttachment",
              "@type": "@id"
            },
            "chronicle": "http://blockchaintp.com/chronicle/ns#",
            "endTime": {
              "@id": "prov:endedAtTime"
            },
            "entity": {
              "@id": "prov:entity",
              "@type": "@id"
            },
            "hadPrimarySource": {
              "@container": "@set",
              "@id": "prov:hadPrimarySource",
              "@type": "@id"
            },
            "identity": {
              "@id": "chronicle:hasIdentity",
              "@type": "@id"
            },
            "label": {
              "@id": "rdfs:label"
            },
            "namespace": {
              "@id": "chronicle:hasNamespace",
              "@type": "@id"
            },
            "previousAttachments": {
              "@container": "@set",
              "@id": "chronicle:hadAttachment",
              "@type": "@id"
            },
            "previousIdentities": {
              "@container": "@set",
              "@id": "chronicle:hadIdentity",
              "@type": "@id"
            },
            "prov": "http://www.w3.org/ns/prov#",
            "provext": "https://openprovenance.org/ns/provext#",
            "publicKey": {
              "@id": "chronicle:hasPublicKey"
            },
            "rdf": "http://www.w3.org/1999/02/22-rdf-syntax-ns#",
            "rdfs": "http://www.w3.org/2000/01/rdf-schema#",
            "signature": {
              "@id": "chronicle:entitySignature"
            },
            "signedAtTime": {
              "@id": "chronicle:signedAtTime"
            },
            "source": {
              "@id": "chronicle:entityLocator"
            },
            "startTime": {
              "@id": "prov:startedAtTime"
            },
            "used": {
              "@container": "@set",
              "@id": "prov:used",
              "@type": "@id"
            },
            "value": {
              "@id": "chronicle:value",
              "@type": "@json"
            },
            "wasAssociatedWith": {
              "@container": "@set",
              "@id": "prov:wasAssociatedWith",
              "@type": "@id"
            },
            "wasDerivedFrom": {
              "@container": "@set",
              "@id": "prov:wasDerivedFrom",
              "@type": "@id"
            },
            "wasGeneratedBy": {
              "@container": "@set",
              "@id": "prov:wasGeneratedBy",
              "@type": "@id"
            },
            "wasQuotedFrom": {
              "@container": "@set",
              "@id": "prov:wasQuotedFrom",
              "@type": "@id"
            },
            "wasRevisionOf": {
              "@container": "@set",
              "@id": "prov:wasRevisionOf",
              "@type": "@id"
            },
            "xsd": "http://www.w3.org/2001/XMLSchema#"
          },
          "@graph": [
            {
              "@id": "chronicle:agent:test%5Fagent",
              "@type": [
                "prov:Agent",
                "chronicle:domaintype:testAgent"
              ],
              "label": "test_agent",
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
              "label": "testns"
            }
          ]
        }
        "###);
    }

    #[tokio::test]
    async fn agent_define_id() {
        let id = ChronicleIri::from(common::prov::AgentId::from_external_id("test_agent"));
        let command_line = format!(
            r#"chronicle test-agent define --test-bool-attr false --test-string-attr "test" --test-int-attr 23 --namespace testns --id {id} "#
        );
        let prov_model = parse_and_execute(&command_line, test_cli_model()).await;
        let sorted = sort_prov_model(prov_model).await;
        insta::assert_snapshot!(serde_json::to_string_pretty(&sorted).unwrap(), @r###"
        {
          "@context": {
            "@version": 1.1,
            "actedOnBehalfOf": {
              "@container": "@set",
              "@id": "prov:actedOnBehalfOf",
              "@type": "@id"
            },
            "activity": {
              "@id": "prov:activity",
              "@type": "@id"
            },
            "agent": {
              "@id": "prov:agent",
              "@type": "@id"
            },
            "attachment": {
              "@id": "chronicle:hasAttachment",
              "@type": "@id"
            },
            "chronicle": "http://blockchaintp.com/chronicle/ns#",
            "endTime": {
              "@id": "prov:endedAtTime"
            },
            "entity": {
              "@id": "prov:entity",
              "@type": "@id"
            },
            "hadPrimarySource": {
              "@container": "@set",
              "@id": "prov:hadPrimarySource",
              "@type": "@id"
            },
            "identity": {
              "@id": "chronicle:hasIdentity",
              "@type": "@id"
            },
            "label": {
              "@id": "rdfs:label"
            },
            "namespace": {
              "@id": "chronicle:hasNamespace",
              "@type": "@id"
            },
            "previousAttachments": {
              "@container": "@set",
              "@id": "chronicle:hadAttachment",
              "@type": "@id"
            },
            "previousIdentities": {
              "@container": "@set",
              "@id": "chronicle:hadIdentity",
              "@type": "@id"
            },
            "prov": "http://www.w3.org/ns/prov#",
            "provext": "https://openprovenance.org/ns/provext#",
            "publicKey": {
              "@id": "chronicle:hasPublicKey"
            },
            "rdf": "http://www.w3.org/1999/02/22-rdf-syntax-ns#",
            "rdfs": "http://www.w3.org/2000/01/rdf-schema#",
            "signature": {
              "@id": "chronicle:entitySignature"
            },
            "signedAtTime": {
              "@id": "chronicle:signedAtTime"
            },
            "source": {
              "@id": "chronicle:entityLocator"
            },
            "startTime": {
              "@id": "prov:startedAtTime"
            },
            "used": {
              "@container": "@set",
              "@id": "prov:used",
              "@type": "@id"
            },
            "value": {
              "@id": "chronicle:value",
              "@type": "@json"
            },
            "wasAssociatedWith": {
              "@container": "@set",
              "@id": "prov:wasAssociatedWith",
              "@type": "@id"
            },
            "wasDerivedFrom": {
              "@container": "@set",
              "@id": "prov:wasDerivedFrom",
              "@type": "@id"
            },
            "wasGeneratedBy": {
              "@container": "@set",
              "@id": "prov:wasGeneratedBy",
              "@type": "@id"
            },
            "wasQuotedFrom": {
              "@container": "@set",
              "@id": "prov:wasQuotedFrom",
              "@type": "@id"
            },
            "wasRevisionOf": {
              "@container": "@set",
              "@id": "prov:wasRevisionOf",
              "@type": "@id"
            },
            "xsd": "http://www.w3.org/2001/XMLSchema#"
          },
          "@graph": [
            {
              "@id": "chronicle:agent:test%5Fagent",
              "@type": [
                "prov:Agent",
                "chronicle:domaintype:testAgent"
              ],
              "label": "test_agent",
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
              "label": "testns"
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
            format!(r#"chronicle test-agent register-key --namespace testns {id} -g "#);
        let cmd = get_api_cmd(&command_line);
        api.dispatch(cmd).await.unwrap().unwrap();
        insta::assert_yaml_snapshot!(api.1, {
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
        "###);
    }

    #[tokio::test]
    async fn agent_use() {
        let mut api = test_api().await;

        // note, if you don't supply all three types of attribute this won't run
        let command_line = r#"chronicle test-agent define testagent --namespace testns --test-string-attr "test" --test-bool-attr true --test-int-attr 23 "#;

        let cmd = get_api_cmd(command_line);
        api.dispatch(cmd).await.unwrap();

        let id = AgentId::from_external_id("testagent");

        let command_line = format!(r#"chronicle test-agent use --namespace testns {id} "#);
        let cmd = get_api_cmd(&command_line);
        api.dispatch(cmd).await.unwrap();

        let id = ActivityId::from_external_id("testactivity");
        let command_line = format!(
            r#"chronicle test-activity start {id} --namespace testns --time 2014-07-08T09:10:11Z "#
        );
        let cmd = get_api_cmd(&command_line);

        let (prov_model, _) = api.dispatch(cmd).await.unwrap().unwrap();

        let v: serde_json::Value =
            serde_json::from_str(&prov_model.to_json().compact().await.unwrap().to_string())
                .unwrap();
        let sorted = sort_graph(v);
        insta::assert_snapshot!(serde_json::to_string_pretty(&sorted).unwrap(), @r###"
        {
          "@context": {
            "@version": 1.1,
            "actedOnBehalfOf": {
              "@container": "@set",
              "@id": "prov:actedOnBehalfOf",
              "@type": "@id"
            },
            "activity": {
              "@id": "prov:activity",
              "@type": "@id"
            },
            "agent": {
              "@id": "prov:agent",
              "@type": "@id"
            },
            "attachment": {
              "@id": "chronicle:hasAttachment",
              "@type": "@id"
            },
            "chronicle": "http://blockchaintp.com/chronicle/ns#",
            "endTime": {
              "@id": "prov:endedAtTime"
            },
            "entity": {
              "@id": "prov:entity",
              "@type": "@id"
            },
            "hadPrimarySource": {
              "@container": "@set",
              "@id": "prov:hadPrimarySource",
              "@type": "@id"
            },
            "identity": {
              "@id": "chronicle:hasIdentity",
              "@type": "@id"
            },
            "label": {
              "@id": "rdfs:label"
            },
            "namespace": {
              "@id": "chronicle:hasNamespace",
              "@type": "@id"
            },
            "previousAttachments": {
              "@container": "@set",
              "@id": "chronicle:hadAttachment",
              "@type": "@id"
            },
            "previousIdentities": {
              "@container": "@set",
              "@id": "chronicle:hadIdentity",
              "@type": "@id"
            },
            "prov": "http://www.w3.org/ns/prov#",
            "provext": "https://openprovenance.org/ns/provext#",
            "publicKey": {
              "@id": "chronicle:hasPublicKey"
            },
            "rdf": "http://www.w3.org/1999/02/22-rdf-syntax-ns#",
            "rdfs": "http://www.w3.org/2000/01/rdf-schema#",
            "signature": {
              "@id": "chronicle:entitySignature"
            },
            "signedAtTime": {
              "@id": "chronicle:signedAtTime"
            },
            "source": {
              "@id": "chronicle:entityLocator"
            },
            "startTime": {
              "@id": "prov:startedAtTime"
            },
            "used": {
              "@container": "@set",
              "@id": "prov:used",
              "@type": "@id"
            },
            "value": {
              "@id": "chronicle:value",
              "@type": "@json"
            },
            "wasAssociatedWith": {
              "@container": "@set",
              "@id": "prov:wasAssociatedWith",
              "@type": "@id"
            },
            "wasDerivedFrom": {
              "@container": "@set",
              "@id": "prov:wasDerivedFrom",
              "@type": "@id"
            },
            "wasGeneratedBy": {
              "@container": "@set",
              "@id": "prov:wasGeneratedBy",
              "@type": "@id"
            },
            "wasQuotedFrom": {
              "@container": "@set",
              "@id": "prov:wasQuotedFrom",
              "@type": "@id"
            },
            "wasRevisionOf": {
              "@container": "@set",
              "@id": "prov:wasRevisionOf",
              "@type": "@id"
            },
            "xsd": "http://www.w3.org/2001/XMLSchema#"
          },
          "@graph": [
            {
              "@id": "chronicle:activity:testactivity",
              "@type": "prov:Activity",
              "label": "testactivity",
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
              "@id": "chronicle:agent:testagent",
              "@type": [
                "prov:Agent",
                "chronicle:domaintype:testAgent"
              ],
              "label": "testagent",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": true,
                "TestInt": 23,
                "TestString": "test"
              }
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
              "label": "testns"
            }
          ]
        }
        "###);
    }

    #[tokio::test]
    async fn entity_define() {
        let command_line = r#"chronicle test-entity define test_entity --test-bool-attr false --test-string-attr "test" --test-int-attr 23 --namespace testns "#;
        let prov_model = parse_and_execute(command_line, test_cli_model());
        let v: serde_json::Value = serde_json::from_str(
            &prov_model
                .await
                .to_json()
                .compact()
                .await
                .unwrap()
                .to_string(),
        )
        .unwrap();
        let sorted = sort_graph(v);
        insta::assert_snapshot!(serde_json::to_string_pretty(&sorted).unwrap(), @r###"
        {
          "@context": {
            "@version": 1.1,
            "actedOnBehalfOf": {
              "@container": "@set",
              "@id": "prov:actedOnBehalfOf",
              "@type": "@id"
            },
            "activity": {
              "@id": "prov:activity",
              "@type": "@id"
            },
            "agent": {
              "@id": "prov:agent",
              "@type": "@id"
            },
            "attachment": {
              "@id": "chronicle:hasAttachment",
              "@type": "@id"
            },
            "chronicle": "http://blockchaintp.com/chronicle/ns#",
            "endTime": {
              "@id": "prov:endedAtTime"
            },
            "entity": {
              "@id": "prov:entity",
              "@type": "@id"
            },
            "hadPrimarySource": {
              "@container": "@set",
              "@id": "prov:hadPrimarySource",
              "@type": "@id"
            },
            "identity": {
              "@id": "chronicle:hasIdentity",
              "@type": "@id"
            },
            "label": {
              "@id": "rdfs:label"
            },
            "namespace": {
              "@id": "chronicle:hasNamespace",
              "@type": "@id"
            },
            "previousAttachments": {
              "@container": "@set",
              "@id": "chronicle:hadAttachment",
              "@type": "@id"
            },
            "previousIdentities": {
              "@container": "@set",
              "@id": "chronicle:hadIdentity",
              "@type": "@id"
            },
            "prov": "http://www.w3.org/ns/prov#",
            "provext": "https://openprovenance.org/ns/provext#",
            "publicKey": {
              "@id": "chronicle:hasPublicKey"
            },
            "rdf": "http://www.w3.org/1999/02/22-rdf-syntax-ns#",
            "rdfs": "http://www.w3.org/2000/01/rdf-schema#",
            "signature": {
              "@id": "chronicle:entitySignature"
            },
            "signedAtTime": {
              "@id": "chronicle:signedAtTime"
            },
            "source": {
              "@id": "chronicle:entityLocator"
            },
            "startTime": {
              "@id": "prov:startedAtTime"
            },
            "used": {
              "@container": "@set",
              "@id": "prov:used",
              "@type": "@id"
            },
            "value": {
              "@id": "chronicle:value",
              "@type": "@json"
            },
            "wasAssociatedWith": {
              "@container": "@set",
              "@id": "prov:wasAssociatedWith",
              "@type": "@id"
            },
            "wasDerivedFrom": {
              "@container": "@set",
              "@id": "prov:wasDerivedFrom",
              "@type": "@id"
            },
            "wasGeneratedBy": {
              "@container": "@set",
              "@id": "prov:wasGeneratedBy",
              "@type": "@id"
            },
            "wasQuotedFrom": {
              "@container": "@set",
              "@id": "prov:wasQuotedFrom",
              "@type": "@id"
            },
            "wasRevisionOf": {
              "@container": "@set",
              "@id": "prov:wasRevisionOf",
              "@type": "@id"
            },
            "xsd": "http://www.w3.org/2001/XMLSchema#"
          },
          "@graph": [
            {
              "@id": "chronicle:entity:test%5Fentity",
              "@type": [
                "prov:Entity",
                "chronicle:domaintype:testEntity"
              ],
              "label": "test_entity",
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
              "label": "testns"
            }
          ]
        }
        "###);
    }

    #[tokio::test]
    async fn entity_define_id() {
        let id = ChronicleIri::from(common::prov::EntityId::from_external_id("test_entity"));
        let command_line = format!(
            r#"chronicle test-entity define --test-bool-attr false --test-string-attr "test" --test-int-attr 23 --namespace testns --id {id} "#
        );
        let prov_model = parse_and_execute(&command_line, test_cli_model());
        let v: serde_json::Value = serde_json::from_str(
            &prov_model
                .await
                .to_json()
                .compact()
                .await
                .unwrap()
                .to_string(),
        )
        .unwrap();
        let sorted = sort_graph(v);
        insta::assert_snapshot!(serde_json::to_string_pretty(&sorted).unwrap(), @r###"
        {
          "@context": {
            "@version": 1.1,
            "actedOnBehalfOf": {
              "@container": "@set",
              "@id": "prov:actedOnBehalfOf",
              "@type": "@id"
            },
            "activity": {
              "@id": "prov:activity",
              "@type": "@id"
            },
            "agent": {
              "@id": "prov:agent",
              "@type": "@id"
            },
            "attachment": {
              "@id": "chronicle:hasAttachment",
              "@type": "@id"
            },
            "chronicle": "http://blockchaintp.com/chronicle/ns#",
            "endTime": {
              "@id": "prov:endedAtTime"
            },
            "entity": {
              "@id": "prov:entity",
              "@type": "@id"
            },
            "hadPrimarySource": {
              "@container": "@set",
              "@id": "prov:hadPrimarySource",
              "@type": "@id"
            },
            "identity": {
              "@id": "chronicle:hasIdentity",
              "@type": "@id"
            },
            "label": {
              "@id": "rdfs:label"
            },
            "namespace": {
              "@id": "chronicle:hasNamespace",
              "@type": "@id"
            },
            "previousAttachments": {
              "@container": "@set",
              "@id": "chronicle:hadAttachment",
              "@type": "@id"
            },
            "previousIdentities": {
              "@container": "@set",
              "@id": "chronicle:hadIdentity",
              "@type": "@id"
            },
            "prov": "http://www.w3.org/ns/prov#",
            "provext": "https://openprovenance.org/ns/provext#",
            "publicKey": {
              "@id": "chronicle:hasPublicKey"
            },
            "rdf": "http://www.w3.org/1999/02/22-rdf-syntax-ns#",
            "rdfs": "http://www.w3.org/2000/01/rdf-schema#",
            "signature": {
              "@id": "chronicle:entitySignature"
            },
            "signedAtTime": {
              "@id": "chronicle:signedAtTime"
            },
            "source": {
              "@id": "chronicle:entityLocator"
            },
            "startTime": {
              "@id": "prov:startedAtTime"
            },
            "used": {
              "@container": "@set",
              "@id": "prov:used",
              "@type": "@id"
            },
            "value": {
              "@id": "chronicle:value",
              "@type": "@json"
            },
            "wasAssociatedWith": {
              "@container": "@set",
              "@id": "prov:wasAssociatedWith",
              "@type": "@id"
            },
            "wasDerivedFrom": {
              "@container": "@set",
              "@id": "prov:wasDerivedFrom",
              "@type": "@id"
            },
            "wasGeneratedBy": {
              "@container": "@set",
              "@id": "prov:wasGeneratedBy",
              "@type": "@id"
            },
            "wasQuotedFrom": {
              "@container": "@set",
              "@id": "prov:wasQuotedFrom",
              "@type": "@id"
            },
            "wasRevisionOf": {
              "@container": "@set",
              "@id": "prov:wasRevisionOf",
              "@type": "@id"
            },
            "xsd": "http://www.w3.org/2001/XMLSchema#"
          },
          "@graph": [
            {
              "@id": "chronicle:entity:test%5Fentity",
              "@type": [
                "prov:Entity",
                "chronicle:domaintype:testEntity"
              ],
              "label": "test_entity",
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
              "label": "testns"
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
            r#"chronicle test-entity derive {generated_entity_id} {used_entity_id} --namespace testns "#
        );
        let cmd = get_api_cmd(&command_line);
        let (prov_model, _) = api.dispatch(cmd).await.unwrap().unwrap();

        let v: serde_json::Value =
            serde_json::from_str(&prov_model.to_json().compact().await.unwrap().to_string())
                .unwrap();
        let sorted = sort_graph(v);
        insta::assert_snapshot!(serde_json::to_string_pretty(&sorted).unwrap(), @r###"
        {
          "@context": {
            "@version": 1.1,
            "actedOnBehalfOf": {
              "@container": "@set",
              "@id": "prov:actedOnBehalfOf",
              "@type": "@id"
            },
            "activity": {
              "@id": "prov:activity",
              "@type": "@id"
            },
            "agent": {
              "@id": "prov:agent",
              "@type": "@id"
            },
            "attachment": {
              "@id": "chronicle:hasAttachment",
              "@type": "@id"
            },
            "chronicle": "http://blockchaintp.com/chronicle/ns#",
            "endTime": {
              "@id": "prov:endedAtTime"
            },
            "entity": {
              "@id": "prov:entity",
              "@type": "@id"
            },
            "hadPrimarySource": {
              "@container": "@set",
              "@id": "prov:hadPrimarySource",
              "@type": "@id"
            },
            "identity": {
              "@id": "chronicle:hasIdentity",
              "@type": "@id"
            },
            "label": {
              "@id": "rdfs:label"
            },
            "namespace": {
              "@id": "chronicle:hasNamespace",
              "@type": "@id"
            },
            "previousAttachments": {
              "@container": "@set",
              "@id": "chronicle:hadAttachment",
              "@type": "@id"
            },
            "previousIdentities": {
              "@container": "@set",
              "@id": "chronicle:hadIdentity",
              "@type": "@id"
            },
            "prov": "http://www.w3.org/ns/prov#",
            "provext": "https://openprovenance.org/ns/provext#",
            "publicKey": {
              "@id": "chronicle:hasPublicKey"
            },
            "rdf": "http://www.w3.org/1999/02/22-rdf-syntax-ns#",
            "rdfs": "http://www.w3.org/2000/01/rdf-schema#",
            "signature": {
              "@id": "chronicle:entitySignature"
            },
            "signedAtTime": {
              "@id": "chronicle:signedAtTime"
            },
            "source": {
              "@id": "chronicle:entityLocator"
            },
            "startTime": {
              "@id": "prov:startedAtTime"
            },
            "used": {
              "@container": "@set",
              "@id": "prov:used",
              "@type": "@id"
            },
            "value": {
              "@id": "chronicle:value",
              "@type": "@json"
            },
            "wasAssociatedWith": {
              "@container": "@set",
              "@id": "prov:wasAssociatedWith",
              "@type": "@id"
            },
            "wasDerivedFrom": {
              "@container": "@set",
              "@id": "prov:wasDerivedFrom",
              "@type": "@id"
            },
            "wasGeneratedBy": {
              "@container": "@set",
              "@id": "prov:wasGeneratedBy",
              "@type": "@id"
            },
            "wasQuotedFrom": {
              "@container": "@set",
              "@id": "prov:wasQuotedFrom",
              "@type": "@id"
            },
            "wasRevisionOf": {
              "@container": "@set",
              "@id": "prov:wasRevisionOf",
              "@type": "@id"
            },
            "xsd": "http://www.w3.org/2001/XMLSchema#"
          },
          "@graph": [
            {
              "@id": "chronicle:entity:testgeneratedentity",
              "@type": "prov:Entity",
              "label": "testgeneratedentity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {},
              "wasDerivedFrom": [
                "chronicle:entity:testusedentity"
              ]
            },
            {
              "@id": "chronicle:entity:testusedentity",
              "@type": "prov:Entity",
              "label": "testusedentity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {}
            },
            {
              "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "@type": "chronicle:Namespace",
              "label": "testns"
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
            r#"chronicle test-entity derive {generated_entity_id} {used_entity_id} --namespace testns --subtype primary-source "#
        );
        let cmd = get_api_cmd(&command_line);
        let (prov_model, _) = api.dispatch(cmd).await.unwrap().unwrap();

        let v: serde_json::Value =
            serde_json::from_str(&prov_model.to_json().compact().await.unwrap().to_string())
                .unwrap();
        let sorted = sort_graph(v);
        insta::assert_snapshot!(serde_json::to_string_pretty(&sorted).unwrap(), @r###"
        {
          "@context": {
            "@version": 1.1,
            "actedOnBehalfOf": {
              "@container": "@set",
              "@id": "prov:actedOnBehalfOf",
              "@type": "@id"
            },
            "activity": {
              "@id": "prov:activity",
              "@type": "@id"
            },
            "agent": {
              "@id": "prov:agent",
              "@type": "@id"
            },
            "attachment": {
              "@id": "chronicle:hasAttachment",
              "@type": "@id"
            },
            "chronicle": "http://blockchaintp.com/chronicle/ns#",
            "endTime": {
              "@id": "prov:endedAtTime"
            },
            "entity": {
              "@id": "prov:entity",
              "@type": "@id"
            },
            "hadPrimarySource": {
              "@container": "@set",
              "@id": "prov:hadPrimarySource",
              "@type": "@id"
            },
            "identity": {
              "@id": "chronicle:hasIdentity",
              "@type": "@id"
            },
            "label": {
              "@id": "rdfs:label"
            },
            "namespace": {
              "@id": "chronicle:hasNamespace",
              "@type": "@id"
            },
            "previousAttachments": {
              "@container": "@set",
              "@id": "chronicle:hadAttachment",
              "@type": "@id"
            },
            "previousIdentities": {
              "@container": "@set",
              "@id": "chronicle:hadIdentity",
              "@type": "@id"
            },
            "prov": "http://www.w3.org/ns/prov#",
            "provext": "https://openprovenance.org/ns/provext#",
            "publicKey": {
              "@id": "chronicle:hasPublicKey"
            },
            "rdf": "http://www.w3.org/1999/02/22-rdf-syntax-ns#",
            "rdfs": "http://www.w3.org/2000/01/rdf-schema#",
            "signature": {
              "@id": "chronicle:entitySignature"
            },
            "signedAtTime": {
              "@id": "chronicle:signedAtTime"
            },
            "source": {
              "@id": "chronicle:entityLocator"
            },
            "startTime": {
              "@id": "prov:startedAtTime"
            },
            "used": {
              "@container": "@set",
              "@id": "prov:used",
              "@type": "@id"
            },
            "value": {
              "@id": "chronicle:value",
              "@type": "@json"
            },
            "wasAssociatedWith": {
              "@container": "@set",
              "@id": "prov:wasAssociatedWith",
              "@type": "@id"
            },
            "wasDerivedFrom": {
              "@container": "@set",
              "@id": "prov:wasDerivedFrom",
              "@type": "@id"
            },
            "wasGeneratedBy": {
              "@container": "@set",
              "@id": "prov:wasGeneratedBy",
              "@type": "@id"
            },
            "wasQuotedFrom": {
              "@container": "@set",
              "@id": "prov:wasQuotedFrom",
              "@type": "@id"
            },
            "wasRevisionOf": {
              "@container": "@set",
              "@id": "prov:wasRevisionOf",
              "@type": "@id"
            },
            "xsd": "http://www.w3.org/2001/XMLSchema#"
          },
          "@graph": [
            {
              "@id": "chronicle:entity:testgeneratedentity",
              "@type": "prov:Entity",
              "hadPrimarySource": [
                "chronicle:entity:testusedentity"
              ],
              "label": "testgeneratedentity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {}
            },
            {
              "@id": "chronicle:entity:testusedentity",
              "@type": "prov:Entity",
              "label": "testusedentity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {}
            },
            {
              "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "@type": "chronicle:Namespace",
              "label": "testns"
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
            r#"chronicle test-entity derive {generated_entity_id} {used_entity_id} --namespace testns --subtype revision "#
        );
        let cmd = get_api_cmd(&command_line);
        let (prov_model, _) = api.dispatch(cmd).await.unwrap().unwrap();

        let v: serde_json::Value =
            serde_json::from_str(&prov_model.to_json().compact().await.unwrap().to_string())
                .unwrap();
        let sorted = sort_graph(v);
        insta::assert_snapshot!(serde_json::to_string_pretty(&sorted).unwrap(), @r###"
        {
          "@context": {
            "@version": 1.1,
            "actedOnBehalfOf": {
              "@container": "@set",
              "@id": "prov:actedOnBehalfOf",
              "@type": "@id"
            },
            "activity": {
              "@id": "prov:activity",
              "@type": "@id"
            },
            "agent": {
              "@id": "prov:agent",
              "@type": "@id"
            },
            "attachment": {
              "@id": "chronicle:hasAttachment",
              "@type": "@id"
            },
            "chronicle": "http://blockchaintp.com/chronicle/ns#",
            "endTime": {
              "@id": "prov:endedAtTime"
            },
            "entity": {
              "@id": "prov:entity",
              "@type": "@id"
            },
            "hadPrimarySource": {
              "@container": "@set",
              "@id": "prov:hadPrimarySource",
              "@type": "@id"
            },
            "identity": {
              "@id": "chronicle:hasIdentity",
              "@type": "@id"
            },
            "label": {
              "@id": "rdfs:label"
            },
            "namespace": {
              "@id": "chronicle:hasNamespace",
              "@type": "@id"
            },
            "previousAttachments": {
              "@container": "@set",
              "@id": "chronicle:hadAttachment",
              "@type": "@id"
            },
            "previousIdentities": {
              "@container": "@set",
              "@id": "chronicle:hadIdentity",
              "@type": "@id"
            },
            "prov": "http://www.w3.org/ns/prov#",
            "provext": "https://openprovenance.org/ns/provext#",
            "publicKey": {
              "@id": "chronicle:hasPublicKey"
            },
            "rdf": "http://www.w3.org/1999/02/22-rdf-syntax-ns#",
            "rdfs": "http://www.w3.org/2000/01/rdf-schema#",
            "signature": {
              "@id": "chronicle:entitySignature"
            },
            "signedAtTime": {
              "@id": "chronicle:signedAtTime"
            },
            "source": {
              "@id": "chronicle:entityLocator"
            },
            "startTime": {
              "@id": "prov:startedAtTime"
            },
            "used": {
              "@container": "@set",
              "@id": "prov:used",
              "@type": "@id"
            },
            "value": {
              "@id": "chronicle:value",
              "@type": "@json"
            },
            "wasAssociatedWith": {
              "@container": "@set",
              "@id": "prov:wasAssociatedWith",
              "@type": "@id"
            },
            "wasDerivedFrom": {
              "@container": "@set",
              "@id": "prov:wasDerivedFrom",
              "@type": "@id"
            },
            "wasGeneratedBy": {
              "@container": "@set",
              "@id": "prov:wasGeneratedBy",
              "@type": "@id"
            },
            "wasQuotedFrom": {
              "@container": "@set",
              "@id": "prov:wasQuotedFrom",
              "@type": "@id"
            },
            "wasRevisionOf": {
              "@container": "@set",
              "@id": "prov:wasRevisionOf",
              "@type": "@id"
            },
            "xsd": "http://www.w3.org/2001/XMLSchema#"
          },
          "@graph": [
            {
              "@id": "chronicle:entity:testgeneratedentity",
              "@type": "prov:Entity",
              "label": "testgeneratedentity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {},
              "wasRevisionOf": [
                "chronicle:entity:testusedentity"
              ]
            },
            {
              "@id": "chronicle:entity:testusedentity",
              "@type": "prov:Entity",
              "label": "testusedentity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {}
            },
            {
              "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "@type": "chronicle:Namespace",
              "label": "testns"
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
            r#"chronicle test-entity derive {generated_entity_id} {used_entity_id} --namespace testns --subtype quotation "#
        );
        let cmd = get_api_cmd(&command_line);
        let (prov_model, _) = api.dispatch(cmd).await.unwrap().unwrap();

        let v: serde_json::Value =
            serde_json::from_str(&prov_model.to_json().compact().await.unwrap().to_string())
                .unwrap();
        let sorted = sort_graph(v);
        insta::assert_snapshot!(serde_json::to_string_pretty(&sorted).unwrap(), @r###"
        {
          "@context": {
            "@version": 1.1,
            "actedOnBehalfOf": {
              "@container": "@set",
              "@id": "prov:actedOnBehalfOf",
              "@type": "@id"
            },
            "activity": {
              "@id": "prov:activity",
              "@type": "@id"
            },
            "agent": {
              "@id": "prov:agent",
              "@type": "@id"
            },
            "attachment": {
              "@id": "chronicle:hasAttachment",
              "@type": "@id"
            },
            "chronicle": "http://blockchaintp.com/chronicle/ns#",
            "endTime": {
              "@id": "prov:endedAtTime"
            },
            "entity": {
              "@id": "prov:entity",
              "@type": "@id"
            },
            "hadPrimarySource": {
              "@container": "@set",
              "@id": "prov:hadPrimarySource",
              "@type": "@id"
            },
            "identity": {
              "@id": "chronicle:hasIdentity",
              "@type": "@id"
            },
            "label": {
              "@id": "rdfs:label"
            },
            "namespace": {
              "@id": "chronicle:hasNamespace",
              "@type": "@id"
            },
            "previousAttachments": {
              "@container": "@set",
              "@id": "chronicle:hadAttachment",
              "@type": "@id"
            },
            "previousIdentities": {
              "@container": "@set",
              "@id": "chronicle:hadIdentity",
              "@type": "@id"
            },
            "prov": "http://www.w3.org/ns/prov#",
            "provext": "https://openprovenance.org/ns/provext#",
            "publicKey": {
              "@id": "chronicle:hasPublicKey"
            },
            "rdf": "http://www.w3.org/1999/02/22-rdf-syntax-ns#",
            "rdfs": "http://www.w3.org/2000/01/rdf-schema#",
            "signature": {
              "@id": "chronicle:entitySignature"
            },
            "signedAtTime": {
              "@id": "chronicle:signedAtTime"
            },
            "source": {
              "@id": "chronicle:entityLocator"
            },
            "startTime": {
              "@id": "prov:startedAtTime"
            },
            "used": {
              "@container": "@set",
              "@id": "prov:used",
              "@type": "@id"
            },
            "value": {
              "@id": "chronicle:value",
              "@type": "@json"
            },
            "wasAssociatedWith": {
              "@container": "@set",
              "@id": "prov:wasAssociatedWith",
              "@type": "@id"
            },
            "wasDerivedFrom": {
              "@container": "@set",
              "@id": "prov:wasDerivedFrom",
              "@type": "@id"
            },
            "wasGeneratedBy": {
              "@container": "@set",
              "@id": "prov:wasGeneratedBy",
              "@type": "@id"
            },
            "wasQuotedFrom": {
              "@container": "@set",
              "@id": "prov:wasQuotedFrom",
              "@type": "@id"
            },
            "wasRevisionOf": {
              "@container": "@set",
              "@id": "prov:wasRevisionOf",
              "@type": "@id"
            },
            "xsd": "http://www.w3.org/2001/XMLSchema#"
          },
          "@graph": [
            {
              "@id": "chronicle:entity:testgeneratedentity",
              "@type": "prov:Entity",
              "label": "testgeneratedentity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {},
              "wasQuotedFrom": [
                "chronicle:entity:testusedentity"
              ]
            },
            {
              "@id": "chronicle:entity:testusedentity",
              "@type": "prov:Entity",
              "label": "testusedentity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {}
            },
            {
              "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "@type": "chronicle:Namespace",
              "label": "testns"
            }
          ]
        }
        "###);
    }

    #[tokio::test]
    async fn activity_define() {
        let command_line = r#"chronicle test-activity define test_activity --test-bool-attr false --test-string-attr "test" --test-int-attr 23 --namespace testns "#;
        let prov_model = parse_and_execute(command_line, test_cli_model());
        let v: serde_json::Value = serde_json::from_str(
            &prov_model
                .await
                .to_json()
                .compact()
                .await
                .unwrap()
                .to_string(),
        )
        .unwrap();
        let sorted = sort_graph(v);
        insta::assert_snapshot!(serde_json::to_string_pretty(&sorted).unwrap(), @r###"
        {
          "@context": {
            "@version": 1.1,
            "actedOnBehalfOf": {
              "@container": "@set",
              "@id": "prov:actedOnBehalfOf",
              "@type": "@id"
            },
            "activity": {
              "@id": "prov:activity",
              "@type": "@id"
            },
            "agent": {
              "@id": "prov:agent",
              "@type": "@id"
            },
            "attachment": {
              "@id": "chronicle:hasAttachment",
              "@type": "@id"
            },
            "chronicle": "http://blockchaintp.com/chronicle/ns#",
            "endTime": {
              "@id": "prov:endedAtTime"
            },
            "entity": {
              "@id": "prov:entity",
              "@type": "@id"
            },
            "hadPrimarySource": {
              "@container": "@set",
              "@id": "prov:hadPrimarySource",
              "@type": "@id"
            },
            "identity": {
              "@id": "chronicle:hasIdentity",
              "@type": "@id"
            },
            "label": {
              "@id": "rdfs:label"
            },
            "namespace": {
              "@id": "chronicle:hasNamespace",
              "@type": "@id"
            },
            "previousAttachments": {
              "@container": "@set",
              "@id": "chronicle:hadAttachment",
              "@type": "@id"
            },
            "previousIdentities": {
              "@container": "@set",
              "@id": "chronicle:hadIdentity",
              "@type": "@id"
            },
            "prov": "http://www.w3.org/ns/prov#",
            "provext": "https://openprovenance.org/ns/provext#",
            "publicKey": {
              "@id": "chronicle:hasPublicKey"
            },
            "rdf": "http://www.w3.org/1999/02/22-rdf-syntax-ns#",
            "rdfs": "http://www.w3.org/2000/01/rdf-schema#",
            "signature": {
              "@id": "chronicle:entitySignature"
            },
            "signedAtTime": {
              "@id": "chronicle:signedAtTime"
            },
            "source": {
              "@id": "chronicle:entityLocator"
            },
            "startTime": {
              "@id": "prov:startedAtTime"
            },
            "used": {
              "@container": "@set",
              "@id": "prov:used",
              "@type": "@id"
            },
            "value": {
              "@id": "chronicle:value",
              "@type": "@json"
            },
            "wasAssociatedWith": {
              "@container": "@set",
              "@id": "prov:wasAssociatedWith",
              "@type": "@id"
            },
            "wasDerivedFrom": {
              "@container": "@set",
              "@id": "prov:wasDerivedFrom",
              "@type": "@id"
            },
            "wasGeneratedBy": {
              "@container": "@set",
              "@id": "prov:wasGeneratedBy",
              "@type": "@id"
            },
            "wasQuotedFrom": {
              "@container": "@set",
              "@id": "prov:wasQuotedFrom",
              "@type": "@id"
            },
            "wasRevisionOf": {
              "@container": "@set",
              "@id": "prov:wasRevisionOf",
              "@type": "@id"
            },
            "xsd": "http://www.w3.org/2001/XMLSchema#"
          },
          "@graph": [
            {
              "@id": "chronicle:activity:test%5Factivity",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "test_activity",
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
              "label": "testns"
            }
          ]
        }
        "###);
    }

    #[tokio::test]
    async fn activity_define_id() {
        let id = ChronicleIri::from(common::prov::ActivityId::from_external_id("test_activity"));
        let command_line = format!(
            r#"chronicle test-activity define --test-bool-attr false --test-string-attr "test" --test-int-attr 23 --namespace testns --id {id} "#
        );
        let prov_model = parse_and_execute(&command_line, test_cli_model());
        let v: serde_json::Value = serde_json::from_str(
            &prov_model
                .await
                .to_json()
                .compact()
                .await
                .unwrap()
                .to_string(),
        )
        .unwrap();
        let sorted = sort_graph(v);
        insta::assert_snapshot!(serde_json::to_string_pretty(&sorted).unwrap(), @r###"
        {
          "@context": {
            "@version": 1.1,
            "actedOnBehalfOf": {
              "@container": "@set",
              "@id": "prov:actedOnBehalfOf",
              "@type": "@id"
            },
            "activity": {
              "@id": "prov:activity",
              "@type": "@id"
            },
            "agent": {
              "@id": "prov:agent",
              "@type": "@id"
            },
            "attachment": {
              "@id": "chronicle:hasAttachment",
              "@type": "@id"
            },
            "chronicle": "http://blockchaintp.com/chronicle/ns#",
            "endTime": {
              "@id": "prov:endedAtTime"
            },
            "entity": {
              "@id": "prov:entity",
              "@type": "@id"
            },
            "hadPrimarySource": {
              "@container": "@set",
              "@id": "prov:hadPrimarySource",
              "@type": "@id"
            },
            "identity": {
              "@id": "chronicle:hasIdentity",
              "@type": "@id"
            },
            "label": {
              "@id": "rdfs:label"
            },
            "namespace": {
              "@id": "chronicle:hasNamespace",
              "@type": "@id"
            },
            "previousAttachments": {
              "@container": "@set",
              "@id": "chronicle:hadAttachment",
              "@type": "@id"
            },
            "previousIdentities": {
              "@container": "@set",
              "@id": "chronicle:hadIdentity",
              "@type": "@id"
            },
            "prov": "http://www.w3.org/ns/prov#",
            "provext": "https://openprovenance.org/ns/provext#",
            "publicKey": {
              "@id": "chronicle:hasPublicKey"
            },
            "rdf": "http://www.w3.org/1999/02/22-rdf-syntax-ns#",
            "rdfs": "http://www.w3.org/2000/01/rdf-schema#",
            "signature": {
              "@id": "chronicle:entitySignature"
            },
            "signedAtTime": {
              "@id": "chronicle:signedAtTime"
            },
            "source": {
              "@id": "chronicle:entityLocator"
            },
            "startTime": {
              "@id": "prov:startedAtTime"
            },
            "used": {
              "@container": "@set",
              "@id": "prov:used",
              "@type": "@id"
            },
            "value": {
              "@id": "chronicle:value",
              "@type": "@json"
            },
            "wasAssociatedWith": {
              "@container": "@set",
              "@id": "prov:wasAssociatedWith",
              "@type": "@id"
            },
            "wasDerivedFrom": {
              "@container": "@set",
              "@id": "prov:wasDerivedFrom",
              "@type": "@id"
            },
            "wasGeneratedBy": {
              "@container": "@set",
              "@id": "prov:wasGeneratedBy",
              "@type": "@id"
            },
            "wasQuotedFrom": {
              "@container": "@set",
              "@id": "prov:wasQuotedFrom",
              "@type": "@id"
            },
            "wasRevisionOf": {
              "@container": "@set",
              "@id": "prov:wasRevisionOf",
              "@type": "@id"
            },
            "xsd": "http://www.w3.org/2001/XMLSchema#"
          },
          "@graph": [
            {
              "@id": "chronicle:activity:test%5Factivity",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "test_activity",
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
              "label": "testns"
            }
          ]
        }
        "###);
    }

    #[tokio::test]
    async fn activity_start() {
        let mut api = test_api().await;

        let command_line = r#"chronicle test-agent define testagent --namespace testns --test-string-attr "test" --test-bool-attr true --test-int-attr 40 "#;
        let cmd = get_api_cmd(command_line);
        api.dispatch(cmd).await.unwrap();

        let id = ChronicleIri::from(AgentId::from_external_id("testagent"));
        let command_line = format!(r#"chronicle test-agent use --namespace testns {id} "#);
        let cmd = get_api_cmd(&command_line);
        api.dispatch(cmd).await.unwrap();

        let id = ChronicleIri::from(ActivityId::from_external_id("testactivity"));
        let command_line = format!(
            r#"chronicle test-activity start {id} --namespace testns --time 2014-07-08T09:10:11Z "#
        );
        let cmd = get_api_cmd(&command_line);
        let sorted = sort_prov_model(api.dispatch(cmd).await.unwrap().unwrap().0);
        insta::assert_snapshot!(serde_json::to_string_pretty(&sorted.await).unwrap(), @r###"
        {
          "@context": {
            "@version": 1.1,
            "actedOnBehalfOf": {
              "@container": "@set",
              "@id": "prov:actedOnBehalfOf",
              "@type": "@id"
            },
            "activity": {
              "@id": "prov:activity",
              "@type": "@id"
            },
            "agent": {
              "@id": "prov:agent",
              "@type": "@id"
            },
            "attachment": {
              "@id": "chronicle:hasAttachment",
              "@type": "@id"
            },
            "chronicle": "http://blockchaintp.com/chronicle/ns#",
            "endTime": {
              "@id": "prov:endedAtTime"
            },
            "entity": {
              "@id": "prov:entity",
              "@type": "@id"
            },
            "hadPrimarySource": {
              "@container": "@set",
              "@id": "prov:hadPrimarySource",
              "@type": "@id"
            },
            "identity": {
              "@id": "chronicle:hasIdentity",
              "@type": "@id"
            },
            "label": {
              "@id": "rdfs:label"
            },
            "namespace": {
              "@id": "chronicle:hasNamespace",
              "@type": "@id"
            },
            "previousAttachments": {
              "@container": "@set",
              "@id": "chronicle:hadAttachment",
              "@type": "@id"
            },
            "previousIdentities": {
              "@container": "@set",
              "@id": "chronicle:hadIdentity",
              "@type": "@id"
            },
            "prov": "http://www.w3.org/ns/prov#",
            "provext": "https://openprovenance.org/ns/provext#",
            "publicKey": {
              "@id": "chronicle:hasPublicKey"
            },
            "rdf": "http://www.w3.org/1999/02/22-rdf-syntax-ns#",
            "rdfs": "http://www.w3.org/2000/01/rdf-schema#",
            "signature": {
              "@id": "chronicle:entitySignature"
            },
            "signedAtTime": {
              "@id": "chronicle:signedAtTime"
            },
            "source": {
              "@id": "chronicle:entityLocator"
            },
            "startTime": {
              "@id": "prov:startedAtTime"
            },
            "used": {
              "@container": "@set",
              "@id": "prov:used",
              "@type": "@id"
            },
            "value": {
              "@id": "chronicle:value",
              "@type": "@json"
            },
            "wasAssociatedWith": {
              "@container": "@set",
              "@id": "prov:wasAssociatedWith",
              "@type": "@id"
            },
            "wasDerivedFrom": {
              "@container": "@set",
              "@id": "prov:wasDerivedFrom",
              "@type": "@id"
            },
            "wasGeneratedBy": {
              "@container": "@set",
              "@id": "prov:wasGeneratedBy",
              "@type": "@id"
            },
            "wasQuotedFrom": {
              "@container": "@set",
              "@id": "prov:wasQuotedFrom",
              "@type": "@id"
            },
            "wasRevisionOf": {
              "@container": "@set",
              "@id": "prov:wasRevisionOf",
              "@type": "@id"
            },
            "xsd": "http://www.w3.org/2001/XMLSchema#"
          },
          "@graph": [
            {
              "@id": "chronicle:activity:testactivity",
              "@type": "prov:Activity",
              "label": "testactivity",
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
              "@id": "chronicle:agent:testagent",
              "@type": [
                "prov:Agent",
                "chronicle:domaintype:testAgent"
              ],
              "label": "testagent",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": true,
                "TestInt": 40,
                "TestString": "test"
              }
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
              "label": "testns"
            }
          ]
        }
        "###);
    }

    #[tokio::test]
    async fn activity_end() {
        let mut api = test_api().await;

        let command_line = r#"chronicle test-agent define testagent --namespace testns --test-string-attr "test" --test-bool-attr true --test-int-attr 40 "#;
        let cmd = get_api_cmd(command_line);
        api.dispatch(cmd).await.unwrap();

        let id = ChronicleIri::from(AgentId::from_external_id("testagent"));
        let command_line = format!(r#"chronicle test-agent use --namespace testns {id} "#);
        let cmd = get_api_cmd(&command_line);
        api.dispatch(cmd).await.unwrap();

        let id = ChronicleIri::from(ActivityId::from_external_id("testactivity"));
        let command_line = format!(
            r#"chronicle test-activity start {id} --namespace testns --time 2014-07-08T09:10:11Z "#
        );
        let cmd = get_api_cmd(&command_line);
        api.dispatch(cmd).await.unwrap();

        // Should end the last opened activity
        let id = ActivityId::from_external_id("testactivity");
        let command_line = format!(
            r#"chronicle test-activity end --namespace testns --time 2014-08-09T09:10:12Z {id} "#
        );
        let cmd = get_api_cmd(&command_line);
        let sorted = sort_prov_model(api.dispatch(cmd).await.unwrap().unwrap().0);
        insta::assert_snapshot!(serde_json::to_string_pretty(&sorted.await).unwrap(), @r###"
        {
          "@context": {
            "@version": 1.1,
            "actedOnBehalfOf": {
              "@container": "@set",
              "@id": "prov:actedOnBehalfOf",
              "@type": "@id"
            },
            "activity": {
              "@id": "prov:activity",
              "@type": "@id"
            },
            "agent": {
              "@id": "prov:agent",
              "@type": "@id"
            },
            "attachment": {
              "@id": "chronicle:hasAttachment",
              "@type": "@id"
            },
            "chronicle": "http://blockchaintp.com/chronicle/ns#",
            "endTime": {
              "@id": "prov:endedAtTime"
            },
            "entity": {
              "@id": "prov:entity",
              "@type": "@id"
            },
            "hadPrimarySource": {
              "@container": "@set",
              "@id": "prov:hadPrimarySource",
              "@type": "@id"
            },
            "identity": {
              "@id": "chronicle:hasIdentity",
              "@type": "@id"
            },
            "label": {
              "@id": "rdfs:label"
            },
            "namespace": {
              "@id": "chronicle:hasNamespace",
              "@type": "@id"
            },
            "previousAttachments": {
              "@container": "@set",
              "@id": "chronicle:hadAttachment",
              "@type": "@id"
            },
            "previousIdentities": {
              "@container": "@set",
              "@id": "chronicle:hadIdentity",
              "@type": "@id"
            },
            "prov": "http://www.w3.org/ns/prov#",
            "provext": "https://openprovenance.org/ns/provext#",
            "publicKey": {
              "@id": "chronicle:hasPublicKey"
            },
            "rdf": "http://www.w3.org/1999/02/22-rdf-syntax-ns#",
            "rdfs": "http://www.w3.org/2000/01/rdf-schema#",
            "signature": {
              "@id": "chronicle:entitySignature"
            },
            "signedAtTime": {
              "@id": "chronicle:signedAtTime"
            },
            "source": {
              "@id": "chronicle:entityLocator"
            },
            "startTime": {
              "@id": "prov:startedAtTime"
            },
            "used": {
              "@container": "@set",
              "@id": "prov:used",
              "@type": "@id"
            },
            "value": {
              "@id": "chronicle:value",
              "@type": "@json"
            },
            "wasAssociatedWith": {
              "@container": "@set",
              "@id": "prov:wasAssociatedWith",
              "@type": "@id"
            },
            "wasDerivedFrom": {
              "@container": "@set",
              "@id": "prov:wasDerivedFrom",
              "@type": "@id"
            },
            "wasGeneratedBy": {
              "@container": "@set",
              "@id": "prov:wasGeneratedBy",
              "@type": "@id"
            },
            "wasQuotedFrom": {
              "@container": "@set",
              "@id": "prov:wasQuotedFrom",
              "@type": "@id"
            },
            "wasRevisionOf": {
              "@container": "@set",
              "@id": "prov:wasRevisionOf",
              "@type": "@id"
            },
            "xsd": "http://www.w3.org/2001/XMLSchema#"
          },
          "@graph": [
            {
              "@id": "chronicle:activity:testactivity",
              "@type": "prov:Activity",
              "label": "testactivity",
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
              "@id": "chronicle:agent:testagent",
              "@type": [
                "prov:Agent",
                "chronicle:domaintype:testAgent"
              ],
              "label": "testagent",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": true,
                "TestInt": 40,
                "TestString": "test"
              }
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
              "label": "testns"
            }
          ]
        }
        "###);
    }

    #[tokio::test]
    async fn activity_generate() {
        let mut api = test_api().await;

        let command_line = r#"chronicle test-activity define testactivity --namespace testns --test-string-attr "test" --test-bool-attr true --test-int-attr 40 "#;
        let cmd = get_api_cmd(command_line);
        api.dispatch(cmd).await.unwrap();

        let activity_id = ActivityId::from_external_id("testactivity");
        let entity_id = EntityId::from_external_id("testentity");
        let command_line = format!(
            r#"chronicle test-activity generate --namespace testns {entity_id} {activity_id} "#
        );
        let cmd = get_api_cmd(&command_line);

        let sorted = sort_prov_model(api.dispatch(cmd).await.unwrap().unwrap().0);
        insta::assert_snapshot!(serde_json::to_string_pretty(&sorted.await).unwrap(), @r###"
        {
          "@context": {
            "@version": 1.1,
            "actedOnBehalfOf": {
              "@container": "@set",
              "@id": "prov:actedOnBehalfOf",
              "@type": "@id"
            },
            "activity": {
              "@id": "prov:activity",
              "@type": "@id"
            },
            "agent": {
              "@id": "prov:agent",
              "@type": "@id"
            },
            "attachment": {
              "@id": "chronicle:hasAttachment",
              "@type": "@id"
            },
            "chronicle": "http://blockchaintp.com/chronicle/ns#",
            "endTime": {
              "@id": "prov:endedAtTime"
            },
            "entity": {
              "@id": "prov:entity",
              "@type": "@id"
            },
            "hadPrimarySource": {
              "@container": "@set",
              "@id": "prov:hadPrimarySource",
              "@type": "@id"
            },
            "identity": {
              "@id": "chronicle:hasIdentity",
              "@type": "@id"
            },
            "label": {
              "@id": "rdfs:label"
            },
            "namespace": {
              "@id": "chronicle:hasNamespace",
              "@type": "@id"
            },
            "previousAttachments": {
              "@container": "@set",
              "@id": "chronicle:hadAttachment",
              "@type": "@id"
            },
            "previousIdentities": {
              "@container": "@set",
              "@id": "chronicle:hadIdentity",
              "@type": "@id"
            },
            "prov": "http://www.w3.org/ns/prov#",
            "provext": "https://openprovenance.org/ns/provext#",
            "publicKey": {
              "@id": "chronicle:hasPublicKey"
            },
            "rdf": "http://www.w3.org/1999/02/22-rdf-syntax-ns#",
            "rdfs": "http://www.w3.org/2000/01/rdf-schema#",
            "signature": {
              "@id": "chronicle:entitySignature"
            },
            "signedAtTime": {
              "@id": "chronicle:signedAtTime"
            },
            "source": {
              "@id": "chronicle:entityLocator"
            },
            "startTime": {
              "@id": "prov:startedAtTime"
            },
            "used": {
              "@container": "@set",
              "@id": "prov:used",
              "@type": "@id"
            },
            "value": {
              "@id": "chronicle:value",
              "@type": "@json"
            },
            "wasAssociatedWith": {
              "@container": "@set",
              "@id": "prov:wasAssociatedWith",
              "@type": "@id"
            },
            "wasDerivedFrom": {
              "@container": "@set",
              "@id": "prov:wasDerivedFrom",
              "@type": "@id"
            },
            "wasGeneratedBy": {
              "@container": "@set",
              "@id": "prov:wasGeneratedBy",
              "@type": "@id"
            },
            "wasQuotedFrom": {
              "@container": "@set",
              "@id": "prov:wasQuotedFrom",
              "@type": "@id"
            },
            "wasRevisionOf": {
              "@container": "@set",
              "@id": "prov:wasRevisionOf",
              "@type": "@id"
            },
            "xsd": "http://www.w3.org/2001/XMLSchema#"
          },
          "@graph": [
            {
              "@id": "chronicle:activity:testactivity",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": true,
                "TestInt": 40,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:entity:testentity",
              "@type": "prov:Entity",
              "label": "testentity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {},
              "wasGeneratedBy": [
                "chronicle:activity:testactivity"
              ]
            },
            {
              "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "@type": "chronicle:Namespace",
              "label": "testns"
            }
          ]
        }
        "###);
    }

    #[tokio::test]
    async fn activity_use() {
        let mut api = test_api().await;

        let command_line = r#"chronicle test-agent define testagent --namespace testns --test-string-attr "test" --test-bool-attr true --test-int-attr 40 "#;
        let cmd = get_api_cmd(command_line);
        api.dispatch(cmd).await.unwrap();

        let id = ChronicleIri::from(AgentId::from_external_id("testagent"));
        let command_line = format!(r#"chronicle test-agent use --namespace testns {id} "#);
        let cmd = get_api_cmd(&command_line);
        api.dispatch(cmd).await.unwrap();

        let command_line = r#"chronicle test-activity define testactivity --namespace testns --test-string-attr "test" --test-bool-attr true --test-int-attr 40 "#;
        let cmd = get_api_cmd(command_line);
        api.dispatch(cmd).await.unwrap();

        let activity_id = ActivityId::from_external_id("testactivity");
        let entity_id = EntityId::from_external_id("testentity");
        let command_line =
            format!(r#"chronicle test-activity use --namespace testns {entity_id} {activity_id} "#);

        let cmd = get_api_cmd(&command_line);

        let sorted = sort_prov_model(api.dispatch(cmd).await.unwrap().unwrap().0);
        insta::assert_snapshot!(serde_json::to_string_pretty(&sorted.await).unwrap(), @r###"
        {
          "@context": {
            "@version": 1.1,
            "actedOnBehalfOf": {
              "@container": "@set",
              "@id": "prov:actedOnBehalfOf",
              "@type": "@id"
            },
            "activity": {
              "@id": "prov:activity",
              "@type": "@id"
            },
            "agent": {
              "@id": "prov:agent",
              "@type": "@id"
            },
            "attachment": {
              "@id": "chronicle:hasAttachment",
              "@type": "@id"
            },
            "chronicle": "http://blockchaintp.com/chronicle/ns#",
            "endTime": {
              "@id": "prov:endedAtTime"
            },
            "entity": {
              "@id": "prov:entity",
              "@type": "@id"
            },
            "hadPrimarySource": {
              "@container": "@set",
              "@id": "prov:hadPrimarySource",
              "@type": "@id"
            },
            "identity": {
              "@id": "chronicle:hasIdentity",
              "@type": "@id"
            },
            "label": {
              "@id": "rdfs:label"
            },
            "namespace": {
              "@id": "chronicle:hasNamespace",
              "@type": "@id"
            },
            "previousAttachments": {
              "@container": "@set",
              "@id": "chronicle:hadAttachment",
              "@type": "@id"
            },
            "previousIdentities": {
              "@container": "@set",
              "@id": "chronicle:hadIdentity",
              "@type": "@id"
            },
            "prov": "http://www.w3.org/ns/prov#",
            "provext": "https://openprovenance.org/ns/provext#",
            "publicKey": {
              "@id": "chronicle:hasPublicKey"
            },
            "rdf": "http://www.w3.org/1999/02/22-rdf-syntax-ns#",
            "rdfs": "http://www.w3.org/2000/01/rdf-schema#",
            "signature": {
              "@id": "chronicle:entitySignature"
            },
            "signedAtTime": {
              "@id": "chronicle:signedAtTime"
            },
            "source": {
              "@id": "chronicle:entityLocator"
            },
            "startTime": {
              "@id": "prov:startedAtTime"
            },
            "used": {
              "@container": "@set",
              "@id": "prov:used",
              "@type": "@id"
            },
            "value": {
              "@id": "chronicle:value",
              "@type": "@json"
            },
            "wasAssociatedWith": {
              "@container": "@set",
              "@id": "prov:wasAssociatedWith",
              "@type": "@id"
            },
            "wasDerivedFrom": {
              "@container": "@set",
              "@id": "prov:wasDerivedFrom",
              "@type": "@id"
            },
            "wasGeneratedBy": {
              "@container": "@set",
              "@id": "prov:wasGeneratedBy",
              "@type": "@id"
            },
            "wasQuotedFrom": {
              "@container": "@set",
              "@id": "prov:wasQuotedFrom",
              "@type": "@id"
            },
            "wasRevisionOf": {
              "@container": "@set",
              "@id": "prov:wasRevisionOf",
              "@type": "@id"
            },
            "xsd": "http://www.w3.org/2001/XMLSchema#"
          },
          "@graph": [
            {
              "@id": "chronicle:activity:testactivity",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity",
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
              "@id": "chronicle:agent:testagent",
              "@type": [
                "prov:Agent",
                "chronicle:domaintype:testAgent"
              ],
              "label": "testagent",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": true,
                "TestInt": 40,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:entity:testentity",
              "@type": "prov:Entity",
              "label": "testentity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {}
            },
            {
              "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "@type": "chronicle:Namespace",
              "label": "testns"
            }
          ]
        }
        "###);
    }

    #[tokio::test]
    async fn many_activities() {
        let mut api = test_api().await;

        for i in 0..100 {
            let command_line = format!(
                r#"chronicle test-activity define testactivity{i} --namespace testns --test-string-attr "test" --test-bool-attr false --test-int-attr 23 "#
            );
            let cmd = get_api_cmd(&command_line);
            api.dispatch(cmd).await.unwrap();
        }

        let sorted = sort_prov_model(api.1);
        insta::assert_snapshot!(serde_json::to_string_pretty(&sorted.await).unwrap(), @r###"
        {
          "@context": {
            "@version": 1.1,
            "actedOnBehalfOf": {
              "@container": "@set",
              "@id": "prov:actedOnBehalfOf",
              "@type": "@id"
            },
            "activity": {
              "@id": "prov:activity",
              "@type": "@id"
            },
            "agent": {
              "@id": "prov:agent",
              "@type": "@id"
            },
            "attachment": {
              "@id": "chronicle:hasAttachment",
              "@type": "@id"
            },
            "chronicle": "http://blockchaintp.com/chronicle/ns#",
            "endTime": {
              "@id": "prov:endedAtTime"
            },
            "entity": {
              "@id": "prov:entity",
              "@type": "@id"
            },
            "hadPrimarySource": {
              "@container": "@set",
              "@id": "prov:hadPrimarySource",
              "@type": "@id"
            },
            "identity": {
              "@id": "chronicle:hasIdentity",
              "@type": "@id"
            },
            "label": {
              "@id": "rdfs:label"
            },
            "namespace": {
              "@id": "chronicle:hasNamespace",
              "@type": "@id"
            },
            "previousAttachments": {
              "@container": "@set",
              "@id": "chronicle:hadAttachment",
              "@type": "@id"
            },
            "previousIdentities": {
              "@container": "@set",
              "@id": "chronicle:hadIdentity",
              "@type": "@id"
            },
            "prov": "http://www.w3.org/ns/prov#",
            "provext": "https://openprovenance.org/ns/provext#",
            "publicKey": {
              "@id": "chronicle:hasPublicKey"
            },
            "rdf": "http://www.w3.org/1999/02/22-rdf-syntax-ns#",
            "rdfs": "http://www.w3.org/2000/01/rdf-schema#",
            "signature": {
              "@id": "chronicle:entitySignature"
            },
            "signedAtTime": {
              "@id": "chronicle:signedAtTime"
            },
            "source": {
              "@id": "chronicle:entityLocator"
            },
            "startTime": {
              "@id": "prov:startedAtTime"
            },
            "used": {
              "@container": "@set",
              "@id": "prov:used",
              "@type": "@id"
            },
            "value": {
              "@id": "chronicle:value",
              "@type": "@json"
            },
            "wasAssociatedWith": {
              "@container": "@set",
              "@id": "prov:wasAssociatedWith",
              "@type": "@id"
            },
            "wasDerivedFrom": {
              "@container": "@set",
              "@id": "prov:wasDerivedFrom",
              "@type": "@id"
            },
            "wasGeneratedBy": {
              "@container": "@set",
              "@id": "prov:wasGeneratedBy",
              "@type": "@id"
            },
            "wasQuotedFrom": {
              "@container": "@set",
              "@id": "prov:wasQuotedFrom",
              "@type": "@id"
            },
            "wasRevisionOf": {
              "@container": "@set",
              "@id": "prov:wasRevisionOf",
              "@type": "@id"
            },
            "xsd": "http://www.w3.org/2001/XMLSchema#"
          },
          "@graph": [
            {
              "@id": "chronicle:activity:testactivity0",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity0",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity1",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity1",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity10",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity10",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity11",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity11",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity12",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity12",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity13",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity13",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity14",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity14",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity15",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity15",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity16",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity16",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity17",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity17",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity18",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity18",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity19",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity19",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity2",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity2",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity20",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity20",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity21",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity21",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity22",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity22",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity23",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity23",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity24",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity24",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity25",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity25",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity26",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity26",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity27",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity27",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity28",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity28",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity29",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity29",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity3",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity3",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity30",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity30",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity31",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity31",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity32",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity32",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity33",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity33",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity34",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity34",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity35",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity35",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity36",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity36",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity37",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity37",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity38",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity38",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity39",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity39",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity4",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity4",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity40",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity40",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity41",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity41",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity42",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity42",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity43",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity43",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity44",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity44",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity45",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity45",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity46",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity46",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity47",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity47",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity48",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity48",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity49",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity49",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity5",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity5",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity50",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity50",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity51",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity51",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity52",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity52",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity53",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity53",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity54",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity54",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity55",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity55",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity56",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity56",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity57",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity57",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity58",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity58",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity59",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity59",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity6",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity6",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity60",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity60",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity61",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity61",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity62",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity62",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity63",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity63",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity64",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity64",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity65",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity65",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity66",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity66",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity67",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity67",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity68",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity68",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity69",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity69",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity7",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity7",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity70",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity70",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity71",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity71",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity72",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity72",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity73",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity73",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity74",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity74",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity75",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity75",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity76",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity76",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity77",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity77",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity78",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity78",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity79",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity79",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity8",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity8",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity80",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity80",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity81",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity81",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity82",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity82",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity83",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity83",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity84",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity84",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity85",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity85",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity86",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity86",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity87",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity87",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity88",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity88",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity89",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity89",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity9",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity9",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity90",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity90",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity91",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity91",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity92",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity92",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity93",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity93",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity94",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity94",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity95",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity95",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity96",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity96",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity97",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity97",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity98",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity98",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "TestBool": false,
                "TestInt": 23,
                "TestString": "test"
              }
            },
            {
              "@id": "chronicle:activity:testactivity99",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity99",
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
              "label": "testns"
            }
          ]
        }
        "###);
    }
}
