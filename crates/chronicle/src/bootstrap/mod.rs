mod cli;
mod config;

use api::{
    chronicle_graphql::{ChronicleGraphQl, ChronicleGraphQlServer, SecurityConf},
    Api, ApiDispatch, ApiError, StoreError, UuidGen,
};
use async_graphql::{async_trait, ObjectType};
use clap::{ArgMatches, Command};
use clap_complete::{generate, Generator, Shell};
pub use cli::*;
use common::{
    commands::ApiResponse,
    database::{get_connection_with_retry, Database, DatabaseConnector},
    identity::AuthId,
    ledger::SubmissionStage,
    opa::{CliPolicyLoader, ExecutorContext, PolicyLoader},
    prov::to_json_ld::ToJson,
    signing::{DirectoryStoredKeys, SignerError},
};
use tracing::{debug, error, info, instrument};
use user_error::UFE;

use config::*;
use diesel::{
    r2d2::{ConnectionManager, Pool},
    PgConnection,
};

use chronicle_telemetry::{self, ConsoleLogging};
use sawtooth_protocol::{events::StateDelta, messaging::SawtoothSubmitter};
use url::Url;

use std::{collections::HashMap, io, net::SocketAddr, str::FromStr};

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

trait SetRuleOptions {
    fn rule_addr(&mut self, options: &ArgMatches) -> Result<(), CliError>;
    fn rule_entrypoint(&mut self, options: &ArgMatches) -> Result<(), CliError>;
    fn set_addr_and_entrypoint(&mut self, options: &ArgMatches) -> Result<(), CliError> {
        self.rule_addr(options)?;
        self.rule_entrypoint(options)?;
        Ok(())
    }
}

impl SetRuleOptions for CliPolicyLoader {
    fn rule_addr(&mut self, options: &ArgMatches) -> Result<(), CliError> {
        if let Some(val) = options.get_one::<String>("opa-rule") {
            self.set_address(val);
            Ok(())
        } else {
            Err(CliError::MissingArgument {
                arg: "opa-rule".to_string(),
            })
        }
    }

    fn rule_entrypoint(&mut self, options: &ArgMatches) -> Result<(), CliError> {
        if let Some(val) = options.get_one::<String>("opa-entrypoint") {
            self.set_entrypoint(val);
            Ok(())
        } else {
            Err(CliError::MissingArgument {
                arg: "opa-entrypoint".to_string(),
            })
        }
    }
}

async fn opa_executor_from_embedded_policy(
    policy_name: &str,
    entrypoint: &str,
) -> Result<ExecutorContext, CliError> {
    tracing::warn!("insecure operating mode");
    let loader = CliPolicyLoader::from_embedded_policy(policy_name, entrypoint)?;
    Ok(ExecutorContext::from_loader(&loader)?)
}

#[cfg(feature = "inmem")]
fn ledger() -> Result<common::ledger::InMemLedger, std::convert::Infallible> {
    Ok(common::ledger::InMemLedger::new())
}

#[derive(Debug, Clone)]
struct UniqueUuid;

impl UuidGen for UniqueUuid {}

type ConnectionPool = Pool<ConnectionManager<PgConnection>>;

async fn pool_embedded() -> Result<(ConnectionPool, Option<Database>), ApiError> {
    let (database, pool) = common::database::get_embedded_db_connection()
        .await
        .map_err(|source| api::StoreError::EmbeddedDb(source.to_string()))?;
    Ok((pool, Some(database)))
}

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

async fn pool_remote(
    db_uri: impl ToString,
) -> Result<(ConnectionPool, Option<Database>), ApiError> {
    let (_, pool) = get_connection_with_retry(RemoteDatabaseConnector {
        db_uri: db_uri.to_string(),
    })
    .await?;
    Ok((pool, None::<Database>))
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
    security_conf: SecurityConf,
) -> Result<(), ApiError>
where
    Query: ObjectType + Copy,
    Mutation: ObjectType + Copy,
{
    if let Some(addr) = graphql_addr(options)? {
        gql.serve_graphql(pool.clone(), api.clone(), addr, security_conf)
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

async fn pool(matches: &ArgMatches) -> Result<(ConnectionPool, Option<Database>), ApiError> {
    let mut relevant_error = None;
    if !matches.is_present("embedded-database") {
        debug!("connecting to remote DB");
        match pool_remote(&construct_db_uri(matches)).await {
            success @ Ok(_) => return success,
            Err(error) => relevant_error = Some(error),
        }
    };
    if !matches.is_present("remote-database") {
        debug!("connecting to embedded DB");
        match pool_embedded().await {
            success @ Ok(_) => return success,
            Err(error) => {
                if relevant_error.is_none() {
                    relevant_error = Some(error)
                }
            }
        }
    };
    Err(relevant_error.unwrap())
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
    let (pool, _pool_scope) = pool(&matches).await?;
    let api = api(&pool, &matches, &config).await?;
    let ret_api = api.clone();

    let api = api.clone();

    if let Some(matches) = matches.subcommand_matches("serve-graphql") {
        let jwks_uri = if let Some(uri) = matches.value_of("jwks-address") {
            Some(Url::from_str(uri)?)
        } else {
            None
        };

        let userinfo_uri = if let Some(uri) = matches.value_of("userinfo-address") {
            Some(Url::from_str(uri)?)
        } else {
            None
        };

        let allow_anonymous = !matches.is_present("require-auth");

        // CliModel defaults to looking for "/sub"
        let id_pointer = matches
            .value_of("id-pointer")
            .map(|id_pointer| id_pointer.to_string());

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

        let (default_policy_name, entrypoint) =
            ("allow_transactions", "allow_transactions.allowed_users");
        let opa = opa_executor_from_embedded_policy(default_policy_name, entrypoint).await?;

        graphql_server(
            &api,
            &pool,
            gql,
            matches,
            SecurityConf::new(
                jwks_uri,
                userinfo_uri,
                id_pointer,
                jwt_must_claim,
                allow_anonymous,
                opa,
            ),
        )
        .await?;

        Ok((ApiResponse::Unit, ret_api))
    } else if let Some(cmd) = cli.matches(&matches)? {
        let identity = AuthId::chronicle();

        Ok((api.dispatch(cmd, identity).await?, ret_api))
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
                        println!("{subject}");
                    }
                    SubmissionStage::Committed(Err((id, contradiction))) => {
                        if id == tx_id {
                            eprintln!("Transaction rejected by ledger: {id} {contradiction}");
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
        let store = DirectoryStoredKeys::new(config.secrets.path).unwrap();
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
    use api::{Api, ApiDispatch, ApiError, UuidGen};
    use common::{
        commands::{ApiCommand, ApiResponse},
        database::{get_embedded_db_connection, Database},
        identity::AuthId,
        ledger::{InMemLedger, SubmissionStage},
        prov::{
            to_json_ld::ToJson, ActivityId, AgentId, ChronicleIri, ChronicleTransactionId,
            EntityId, ProvModel,
        },
        signing::DirectoryStoredKeys,
    };
    use std::collections::HashMap;
    use tempfile::TempDir;
    use uuid::Uuid;

    use super::{CliModel, SubCommand};
    use crate::codegen::ChronicleDomainDef;

    struct TestDispatch {
        api: ApiDispatch,
        _db: Database, // share lifetime
    }

    impl TestDispatch {
        pub async fn dispatch(
            &mut self,
            command: ApiCommand,
            identity: AuthId,
        ) -> Result<Option<(Box<ProvModel>, ChronicleTransactionId)>, ApiError> {
            // We can sort of get final on chain state here by using a map of subject to model
            if let ApiResponse::Submission { .. } = self.api.dispatch(command, identity).await? {
                loop {
                    let submission = self.api.notify_commit.subscribe().recv().await.unwrap();

                    if let SubmissionStage::Committed(Ok(commit)) = submission {
                        break Ok(Some((commit.delta, commit.tx_id)));
                    }
                    if let SubmissionStage::Committed(Err((_, contradiction))) = submission {
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

    async fn test_api() -> TestDispatch {
        chronicle_telemetry::telemetry(None, chronicle_telemetry::ConsoleLogging::Pretty);

        let secretpath = TempDir::new().unwrap().into_path();

        let keystore_path = secretpath.clone();
        let keystore = DirectoryStoredKeys::new(keystore_path).unwrap();
        keystore.generate_chronicle().unwrap();

        let mut ledger = InMemLedger::new();
        let reader = ledger.reader();

        let (database, pool) = get_embedded_db_connection().await.unwrap();
        let dispatch = Api::new(
            pool,
            ledger,
            reader,
            &secretpath,
            SameUuid,
            HashMap::default(),
        )
        .await
        .unwrap();

        TestDispatch {
            api: dispatch,
            _db: database, // share the lifetime
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
    async fn agent_register_key() {
        let mut api = test_api().await;
        let id = ChronicleIri::from(common::prov::AgentId::from_external_id("test_agent"));
        let command_line =
            format!(r#"chronicle test-agent-agent register-key --namespace testns {id} -g "#);
        let cmd = get_api_cmd(&command_line);
        let delta = api
            .dispatch(cmd, AuthId::chronicle())
            .await
            .unwrap()
            .unwrap();
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
