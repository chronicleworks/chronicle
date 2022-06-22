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

    // use chrono::{TimeZone, Utc};
    use common::{
        attributes::{Attribute, Attributes},
        commands::{
            ActivityCommand,
            AgentCommand,
            ApiCommand,
            ApiResponse,
            // KeyImport, KeyRegistration, NamespaceCommand,
        },
        ledger::InMemLedger,
        prov::{
            ActivityId, AgentId, ChronicleIri, ChronicleTransactionId, DomaintypeId, EntityId,
            ProvModel,
        },
    };

    use diesel::{
        r2d2::{ConnectionManager, Pool},
        SqliteConnection,
    };
    use tempfile::TempDir;
    use tracing::Level;
    use tracing_log::log::LevelFilter;
    use uuid::Uuid;

    use super::{CliModel, SubCommand};
    use crate::codegen::ChronicleDomainDef;
    // use assert_fs::prelude::*;

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
        let id = ChronicleIri::from(common::prov::AgentId::from_name("test_agent"));
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
        let id = ChronicleIri::from(common::prov::AgentId::from_name("test_agent"));
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
          ? name: testns
            uuid: 5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea
          : id:
              name: testns
              uuid: 5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea
            uuid: 5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea
            name: testns
        agents:
          ? - name: testns
              uuid: 5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea
            - test_agent
          : id: test_agent
            namespaceid:
              name: testns
              uuid: 5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea
            name: test_agent
            domaintypeid: ~
            attributes: {}
        activities: {}
        entities: {}
        identities:
          ? - name: testns
              uuid: 5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea
            - name: test_agent
              public_key: "[public]"
          : id:
              name: test_agent
              public_key: "[public]"
            namespaceid:
              name: testns
              uuid: 5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea
            public_key: "[public]"
        attachments: {}
        has_identity:
          ? - name: testns
              uuid: 5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea
            - test_agent
          : - name: testns
              uuid: 5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea
            - name: test_agent
              public_key: "[public]"
        had_identity: {}
        has_attachment: {}
        had_attachment: {}
        association: {}
        derivation: {}
        delegation: {}
        generation: {}
        useage: {}
        "###);
    }

    //     #[tokio::test]
    //     async fn agent_register_public_key() {
    //         let mut api = test_api().await;

    // //         let pk = r#"
    // // -----BEGIN PRIVATE KEY-----
    // // MIGEAgEAMBAGByqGSM49AgEGBSuBBAAKBG0wawIBAQQgCyEwIMMP6BdfMi7qyj9n
    // // CXfOgpTQqiEPHC7qOZl7wbGhRANCAAQZfbhU2MakiNSg7z7x/LDAbWZHj66eh6I3
    // // Fyz29vfeI2LG5PAmY/rKJsn/cEHHx+mdz1NB3vwzV/DJqj0NM+4s
    // // -----END PRIVATE KEY-----
    // // "#;

    //         let file = assert_fs::NamedTempFile::new("test.key").unwrap();
    //         file.write_str(
    //             r#"
    // -----BEGIN PRIVATE KEY-----
    // MIGEAgEAMBAGByqGSM49AgEGBSuBBAAKBG0wawIBAQQgCyEwIMMP6BdfMi7qyj9n
    // CXfOgpTQqiEPHC7qOZl7wbGhRANCAAQZfbhU2MakiNSg7z7x/LDAbWZHj66eh6I3
    // Fyz29vfeI2LG5PAmY/rKJsn/cEHHx+mdz1NB3vwzV/DJqj0NM+4s
    // -----END PRIVATE KEY-----
    // "#,
    //         ).unwrap();
    //         let path = file.path().to_string_lossy();
    //         let id = ChronicleIri::from(common::prov::AgentId::from_name("testagent"));
    //         let command_line = format!(r#"chronicle test-agent register-key --namespace testns {id} -k {path} "#);

    //         // api.dispatch(ApiCommand::NameSpace(NamespaceCommand::Create {
    //         //     name: "testns".into(),
    //         // }))
    //         // .await
    //         // .unwrap();

    //         let cmd = get_api_cmd(&command_line);
    //         api.dispatch(cmd).await.unwrap().unwrap();
    //         insta::assert_yaml_snapshot!(api.1);

    //         // api.dispatch(ApiCommand::Agent(AgentCommand::RegisterKey {
    //         //     id: AgentId::from_name("testagent"),
    //         //     namespace: "testns".into(),
    //         //     registration: KeyRegistration::ImportSigning(KeyImport::FromPEMBuffer {
    //         //         buffer: pk.as_bytes().into(),
    //         //     }),
    //         // }))
    //         // .await
    //         // .unwrap();

    //         // insta::assert_yaml_snapshot!(api.1, {
    //         //     ".*.publickey" => "[public]"
    //         // }, @r###"
    //         // ---
    //         // namespaces:
    //         //   ? name: testns
    //         //     uuid: 5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea
    //         //   : id:
    //         //       name: testns
    //         //       uuid: 5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea
    //         //     uuid: 5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea
    //         //     name: testns
    //         // agents:
    //         //   ? - name: testns
    //         //       uuid: 5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea
    //         //     - testagent
    //         //   : id: testagent
    //         //     namespaceid:
    //         //       name: testns
    //         //       uuid: 5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea
    //         //     name: testagent
    //         //     domaintypeid: ~
    //         //     attributes: {}
    //         // activities: {}
    //         // entities: {}
    //         // identities:
    //         //   ? - name: testns
    //         //       uuid: 5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea
    //         //     - name: testagent
    //         //       public_key: 02197db854d8c6a488d4a0ef3ef1fcb0c06d66478fae9e87a237172cf6f6f7de23
    //         //   : id:
    //         //       name: testagent
    //         //       public_key: 02197db854d8c6a488d4a0ef3ef1fcb0c06d66478fae9e87a237172cf6f6f7de23
    //         //     namespaceid:
    //         //       name: testns
    //         //       uuid: 5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea
    //         //     public_key: 02197db854d8c6a488d4a0ef3ef1fcb0c06d66478fae9e87a237172cf6f6f7de23
    //         // attachments: {}
    //         // has_identity:
    //         //   ? - name: testns
    //         //       uuid: 5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea
    //         //     - testagent
    //         //   : - name: testns
    //         //       uuid: 5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea
    //         //     - name: testagent
    //         //       public_key: 02197db854d8c6a488d4a0ef3ef1fcb0c06d66478fae9e87a237172cf6f6f7de23
    //         // had_identity: {}
    //         // has_attachment: {}
    //         // had_attachment: {}
    //         // association: {}
    //         // derivation: {}
    //         // delegation: {}
    //         // generation: {}
    //         // useage: {}
    //         // "###);
    //     }

    #[tokio::test]
    async fn agent_use() {
        let mut api = test_api().await;

        // note, if you don't supply all three types of attribute this won't run
        let command_line = r#"chronicle test-agent define testagent --namespace testns --test-string-attr "test" --test-bool-attr true --test-int-attr 23 "#;

        let cmd = get_api_cmd(command_line);
        api.dispatch(cmd).await.unwrap();

        let id = AgentId::from_name("testagent");

        let command_line = format!(r#"chronicle test-agent use --namespace testns {id} "#);
        let cmd = get_api_cmd(&command_line);
        api.dispatch(cmd).await.unwrap();

        let id = ActivityId::from_name("testactivity");
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
              "startTime": "2014-07-08T09:10:11+00:00",
              "value": {},
              "wasAssociatedWith": [
                "chronicle:agent:testagent"
              ]
            },
            {
              "@id": "chronicle:agent:testagent",
              "@type": "prov:Agent",
              "label": "testagent",
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
        let id = ChronicleIri::from(common::prov::EntityId::from_name("test_entity"));
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

        let generated_entity_id = EntityId::from_name("testgeneratedentity");
        let used_entity_id = EntityId::from_name("testusedentity");

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

        let generated_entity_id = EntityId::from_name("testgeneratedentity");
        let used_entity_id = EntityId::from_name("testusedentity");

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

        let generated_entity_id = EntityId::from_name("testgeneratedentity");
        let used_entity_id = EntityId::from_name("testusedentity");

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

        let generated_entity_id = EntityId::from_name("testgeneratedentity");
        let used_entity_id = EntityId::from_name("testusedentity");

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
    async fn activity_define_id() {
        let id = ChronicleIri::from(common::prov::ActivityId::from_name("test_activity"));
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

        api.dispatch(ApiCommand::Agent(AgentCommand::Create {
            name: "testagent".into(),
            namespace: "testns".into(),
            attributes: Attributes {
                typ: Some(DomaintypeId::from_name("test")),
                attributes: [(
                    "test".to_owned(),
                    Attribute {
                        typ: "test".to_owned(),
                        value: serde_json::Value::String("test".to_owned()),
                    },
                )]
                .into_iter()
                .collect(),
            },
        }))
        .await
        .unwrap();

        api.dispatch(ApiCommand::Agent(AgentCommand::UseInContext {
            id: AgentId::from_name("testagent"),
            namespace: "testns".into(),
        }))
        .await
        .unwrap();

        let id = ChronicleIri::from(ActivityId::from_name("testactivity"));
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
              "startTime": "2014-07-08T09:10:11+00:00",
              "value": {},
              "wasAssociatedWith": [
                "chronicle:agent:testagent"
              ]
            },
            {
              "@id": "chronicle:agent:testagent",
              "@type": "prov:Agent",
              "label": "testagent",
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
    async fn activity_end() {
        let mut api = test_api().await;

        api.dispatch(ApiCommand::Agent(AgentCommand::Create {
            name: "testagent".into(),
            namespace: "testns".into(),
            attributes: Attributes {
                typ: Some(DomaintypeId::from_name("test")),
                attributes: [(
                    "test".to_owned(),
                    Attribute {
                        typ: "test".to_owned(),
                        value: serde_json::Value::String("test".to_owned()),
                    },
                )]
                .into_iter()
                .collect(),
            },
        }))
        .await
        .unwrap();

        api.dispatch(ApiCommand::Agent(AgentCommand::UseInContext {
            id: AgentId::from_name("testagent"),
            namespace: "testns".into(),
        }))
        .await
        .unwrap();

        api.dispatch(ApiCommand::Activity(ActivityCommand::Start {
            id: ActivityId::from_name("testactivity"),
            namespace: "testns".into(),
            time: Some(chrono::TimeZone::ymd(&chrono::Utc, 2014, 7, 8).and_hms(9, 10, 11)),
            agent: None,
        }))
        .await
        .unwrap();

        // Should end the last opened activity
        let id = ActivityId::from_name("testactivity");
        let command_line = format!(
            r#"chronicle test-activity end --namespace testns --time 2014-07-09T09:10:12Z {id} "#
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
              "endTime": "2014-07-09T09:10:12+00:00",
              "label": "testactivity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "startTime": "2014-07-09T09:10:12+00:00",
              "value": {},
              "wasAssociatedWith": [
                "chronicle:agent:testagent"
              ]
            },
            {
              "@id": "chronicle:agent:testagent",
              "@type": "prov:Agent",
              "label": "testagent",
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
    async fn activity_generate() {
        let mut api = test_api().await;

        api.dispatch(ApiCommand::Activity(ActivityCommand::Create {
            name: "testactivity".into(),
            namespace: "testns".into(),
            attributes: Attributes {
                typ: Some(DomaintypeId::from_name("test")),
                attributes: [(
                    "test".to_owned(),
                    Attribute {
                        typ: "test".to_owned(),
                        value: serde_json::Value::String("test".to_owned()),
                    },
                )]
                .into_iter()
                .collect(),
            },
        }))
        .await
        .unwrap();

        let activity_id = ActivityId::from_name("testactivity");
        let entity_id = EntityId::from_name("testentity");
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
              "@type": "prov:Activity",
              "label": "testactivity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {}
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

        api.dispatch(ApiCommand::Agent(AgentCommand::Create {
            name: "testagent".into(),
            namespace: "testns".into(),
            attributes: common::attributes::Attributes {
                typ: Some(DomaintypeId::from_name("test")),
                attributes: [(
                    "test".to_owned(),
                    Attribute {
                        typ: "test".to_owned(),
                        value: serde_json::Value::String("test".to_owned()),
                    },
                )]
                .into_iter()
                .collect(),
            },
        }))
        .await
        .unwrap();

        api.dispatch(ApiCommand::Agent(AgentCommand::UseInContext {
            id: AgentId::from_name("testagent"),
            namespace: "testns".into(),
        }))
        .await
        .unwrap();

        api.dispatch(ApiCommand::Activity(ActivityCommand::Create {
            name: "testactivity".into(),
            namespace: "testns".into(),
            attributes: Attributes {
                typ: Some(DomaintypeId::from_name("test")),
                attributes: [(
                    "test".to_owned(),
                    Attribute {
                        typ: "test".to_owned(),
                        value: serde_json::Value::String("test".to_owned()),
                    },
                )]
                .into_iter()
                .collect(),
            },
        }))
        .await
        .unwrap();

        let activity_id = ActivityId::from_name("testactivity");
        let entity_id = EntityId::from_name("testentity");
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
              "@type": "prov:Activity",
              "label": "testactivity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "used": [
                "chronicle:entity:testentity"
              ],
              "value": {}
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

        for i in 0..=99 {
            api.dispatch(ApiCommand::Activity(ActivityCommand::Create {
                name: format!("testactivity{}", i).into(),
                namespace: "testns".into(),
                attributes: Attributes {
                    typ: Some(DomaintypeId::from_name("test")),
                    attributes: [(
                        "test".to_owned(),
                        Attribute {
                            typ: "test".to_owned(),
                            value: serde_json::Value::String("test".to_owned()),
                        },
                    )]
                    .into_iter()
                    .collect(),
                },
            }))
            .await
            .unwrap();
        }

        // all three attribute types are required
        let command_line = r#"chronicle test-activity define testactivity100 --test-string-attr "test" --test-bool-attr false --test-int-attr 23 --namespace testns "#;
        let cmd = get_api_cmd(command_line);

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
              "@id": "chronicle:activity:testactivity100",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:testActivity"
              ],
              "label": "testactivity100",
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
