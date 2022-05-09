//! The default graphql api - only abstract resources
//! We delegate to the underlying concrete, attributeless graphql objects in the api crate,
//! wrapping those in our types here.
#![cfg_attr(feature = "strict", deny(warnings))]
use api::graphql::{self, Namespace, Submission};
use api::{self, graphql::Identity};
use async_graphql::*;
use bootstrap::*;
use chrono::{DateTime, Utc};
use clap_complete::Shell;
use common::attributes::Attributes;
use common::prov::vocab::{Chronicle, Prov};
use iref::Iri;
use tracing::error;
use url::Url;
use user_error::UFE;

pub struct DelegatedActivity(graphql::Activity);

#[Object]
impl DelegatedActivity {
    async fn id(&self) -> ID {
        ID::from(Chronicle::activity(&*self.0.name).to_string())
    }
}

pub struct DelegatedEntity(graphql::Entity);

#[Object]
impl DelegatedEntity {
    async fn id(&self) -> ID {
        ID::from(Chronicle::entity(&*self.0.name).to_string())
    }
}

pub struct DelegatedAgent(graphql::Agent);

#[Object]
impl DelegatedAgent {
    async fn id(&self) -> ID {
        ID::from(Chronicle::agent(&*self.0.name).to_string())
    }

    async fn namespace<'a>(&self, ctx: &Context<'a>) -> async_graphql::Result<Namespace> {
        graphql::agent::namespace(self.0.namespace_id, ctx).await
    }

    async fn name(&self) -> &str {
        &self.0.name
    }

    async fn identity<'a>(&self, ctx: &Context<'a>) -> async_graphql::Result<Option<Identity>> {
        graphql::agent::identity(self.0.identity_id, ctx).await
    }

    async fn acted_on_behalf_of<'a>(
        &self,
        ctx: &Context<'a>,
    ) -> async_graphql::Result<Vec<DelegatedAgent>> {
        Ok(graphql::agent::acted_on_behalf_of(self.0.id, ctx)
            .await?
            .into_iter()
            .map(Self)
            .collect())
    }

    #[graphql(name = "type")]
    async fn typ(&self) -> String {
        Iri::from(Prov::Agent).to_string()
    }
}

pub struct Mutation;

#[Object]
impl Mutation {
    pub async fn agent<'a>(
        &self,
        ctx: &Context<'a>,
        name: String,
        namespace: Option<String>,
        attributes: Attributes,
    ) -> async_graphql::Result<Submission> {
        graphql::mutation::agent(ctx, name, namespace, Attributes::default()).await
    }

    pub async fn activity<'a>(
        &self,
        ctx: &Context<'a>,
        name: String,
        namespace: Option<String>,
        attributes: Attributes,
    ) -> async_graphql::Result<Submission> {
        graphql::mutation::activity(ctx, name, namespace, Attributes::default()).await
    }

    pub async fn entity<'a>(
        &self,
        ctx: &Context<'a>,
        name: String,
        namespace: Option<String>,
        attributes: Attributes,
    ) -> async_graphql::Result<Submission> {
        graphql::mutation::entity(ctx, name, namespace, Attributes::default()).await
    }
    pub async fn acted_on_behalf_of<'a>(
        &self,
        ctx: &Context<'a>,
        namespace: Option<String>,
        responsible: ID,
        delegate: ID,
    ) -> async_graphql::Result<Submission> {
        graphql::mutation::acted_on_behalf_of(ctx, namespace, responsible, delegate).await
    }

    pub async fn was_derived_from<'a>(
        &self,
        ctx: &Context<'a>,
        namespace: Option<String>,
        generated_entity: ID,
        used_entity: ID,
    ) -> async_graphql::Result<Submission> {
        graphql::mutation::was_derived_from(ctx, namespace, generated_entity, used_entity).await
    }

    pub async fn was_revision_of<'a>(
        &self,
        ctx: &Context<'a>,
        namespace: Option<String>,
        generated_entity: ID,
        used_entity: ID,
    ) -> async_graphql::Result<Submission> {
        graphql::mutation::was_revision_of(ctx, namespace, generated_entity, used_entity).await
    }
    pub async fn had_primary_source<'a>(
        &self,
        ctx: &Context<'a>,
        namespace: Option<String>,
        generated_entity: ID,
        used_entity: ID,
    ) -> async_graphql::Result<Submission> {
        graphql::mutation::had_primary_source(ctx, namespace, generated_entity, used_entity).await
    }
    pub async fn was_quoted_from<'a>(
        &self,
        ctx: &Context<'a>,
        namespace: Option<String>,
        generated_entity: ID,
        used_entity: ID,
    ) -> async_graphql::Result<Submission> {
        graphql::mutation::was_quoted_from(ctx, namespace, generated_entity, used_entity).await
    }

    pub async fn generate_key<'a>(
        &self,
        ctx: &Context<'a>,
        name: String,
        namespace: Option<String>,
    ) -> async_graphql::Result<Submission> {
        graphql::mutation::generate_key(ctx, name, namespace).await
    }

    pub async fn start_activity<'a>(
        &self,
        ctx: &Context<'a>,
        name: String,
        namespace: Option<String>,
        agent: String,
        time: Option<DateTime<Utc>>,
    ) -> async_graphql::Result<Submission> {
        graphql::mutation::start_activity(ctx, name, namespace, agent, time).await
    }

    pub async fn end_activity<'a>(
        &self,
        ctx: &Context<'a>,
        name: String,
        namespace: Option<String>,
        agent: String,
        time: Option<DateTime<Utc>>,
    ) -> async_graphql::Result<Submission> {
        graphql::mutation::end_activity(ctx, name, namespace, agent, time).await
    }

    pub async fn used<'a>(
        &self,
        ctx: &Context<'a>,
        activity: String,
        name: String,
        namespace: Option<String>,
        _typ: Option<String>,
    ) -> async_graphql::Result<Submission> {
        graphql::mutation::used(ctx, activity, name, namespace).await
    }

    pub async fn was_generated_by<'a>(
        &self,
        ctx: &Context<'a>,
        activity: String,
        name: String,
        namespace: Option<String>,
    ) -> async_graphql::Result<Submission> {
        graphql::mutation::was_generated_by(ctx, activity, name, namespace).await
    }

    pub async fn has_attachment<'a>(
        &self,
        ctx: &Context<'a>,
        name: String,
        namespace: Option<String>,
        attachment: Upload,
        on_behalf_of_agent: String,
        locator: String,
    ) -> async_graphql::Result<Submission> {
        graphql::mutation::has_attachment(
            ctx,
            name,
            namespace,
            attachment,
            on_behalf_of_agent,
            locator,
        )
        .await
    }
}

#[tokio::main]
async fn main() {
    let matches = cli().get_matches();

    if let Ok(generator) = matches.value_of_t::<Shell>("completions") {
        let mut app = cli();
        eprintln!("Generating completion file for {}...", generator);
        print_completions(generator, &mut app);
        std::process::exit(0);
    }

    if matches.is_present("export-schema") {
        print!("{}", api::exportable_schema());
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

#[cfg(test)]
mod test {
    use async_graphql::{Request, Schema};
    use common::ledger::InMemLedger;
    use diesel::{r2d2::ConnectionManager, SqliteConnection};
    use r2d2::Pool;
    use std::time::Duration;
    use tempfile::TempDir;
    use tracing::Level;
    use uuid::Uuid;

    use crate::{persistence::ConnectionOptions, Api, UuidGen};

    use super::{Store, Subscription};

    #[derive(Debug, Clone)]
    struct SameUuid;

    impl UuidGen for SameUuid {
        fn uuid() -> Uuid {
            Uuid::parse_str("5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea").unwrap()
        }
    }

    async fn test_schema() -> Schema<Query, Mutation, Subscription> {
        tracing_log::LogTracer::init_with_filter(tracing::log::LevelFilter::Trace).ok();
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
                busy_timeout: Some(Duration::from_secs(2)),
            }))
            .build(ConnectionManager::<SqliteConnection>::new(&*format!(
                "./sqlite_test/db{}.sqlite",
                dbid
            )))
            .unwrap();

        let (dispatch, _ui) = Api::new(
            None,
            pool.clone(),
            ledger,
            reader,
            &secretpath.into_path(),
            SameUuid,
        )
        .await
        .unwrap();

        Schema::build(Query, Mutation, Subscription)
            .data(Store::new(pool))
            .data(dispatch)
            .finish()
    }

    #[tokio::test]
    async fn delegation() {
        let schema = test_schema().await;

        let created = schema
            .execute(Request::new(
                r#"
            mutation {
                actedOnBehalfOf(
                    responsible: "http://blockchaintp.com/chronicle/ns#agent:responsible",
                    delegate: "http://blockchaintp.com/chronicle/ns#agent:delegate",
                    ) {
                    context
                }
            }
        "#,
            ))
            .await;

        insta::assert_toml_snapshot!(created);

        tokio::time::sleep(Duration::from_millis(100)).await;

        let derived = schema
            .execute(Request::new(
                r#"
            query {
                agentById(id: "http://blockchaintp.com/chronicle/ns#agent:responsible") {
                    actedOnBehalfOf {
                        id
                    }
                }
            }
        "#,
            ))
            .await;
        insta::assert_toml_snapshot!(derived);
    }

    #[tokio::test]
    async fn untyped_derivation() {
        let schema = test_schema().await;

        let created = schema
            .execute(Request::new(
                r#"
            mutation {
                wasDerivedFrom(generatedEntity: "http://blockchaintp.com/chronicle/ns#entity:generated",
                               usedEntity: "http://blockchaintp.com/chronicle/ns#entity:used") {
                    context
                }
            }
        "#,
            ))
            .await;

        insta::assert_toml_snapshot!(created);

        tokio::time::sleep(Duration::from_millis(100)).await;
        let derived = schema
            .execute(Request::new(
                r#"
            query {
                entityById(id: "http://blockchaintp.com/chronicle/ns#entity:generated") {
                    wasDerivedFrom {
                        id
                    }
                }
            }
        "#,
            ))
            .await;
        insta::assert_toml_snapshot!(derived);
    }

    #[tokio::test]
    async fn primary_source() {
        let schema = test_schema().await;

        let created = schema
            .execute(Request::new(
                r#"
            mutation {
                hadPrimarySource(generatedEntity: "http://blockchaintp.com/chronicle/ns#entity:generated",
                               usedEntity: "http://blockchaintp.com/chronicle/ns#entity:used") {
                    context
                }
            }
        "#,
            ))
            .await;

        insta::assert_toml_snapshot!(created);

        tokio::time::sleep(Duration::from_millis(100)).await;
        let derived = schema
            .execute(Request::new(
                r#"
            query {
                entityById(id: "http://blockchaintp.com/chronicle/ns#entity:generated") {
                    wasDerivedFrom {
                        id
                    }
                    hadPrimarySource {
                        id
                    }
                }
            }
        "#,
            ))
            .await;
        insta::assert_toml_snapshot!(derived);
    }

    #[tokio::test]
    async fn revision() {
        let schema = test_schema().await;

        let created = schema
            .execute(Request::new(
                r#"
            mutation {
                wasRevisionOf(generatedEntity: "http://blockchaintp.com/chronicle/ns#entity:generated",
                            usedEntity: "http://blockchaintp.com/chronicle/ns#entity:used") {
                    context
                }
            }
        "#,
            ))
            .await;

        insta::assert_toml_snapshot!(created);

        tokio::time::sleep(Duration::from_millis(100)).await;
        let derived = schema
            .execute(Request::new(
                r#"
            query {
                entityById(id: "http://blockchaintp.com/chronicle/ns#entity:generated") {
                    wasDerivedFrom {
                        id
                    }
                    wasRevisionOf {
                        id
                    }
                }
            }
        "#,
            ))
            .await;
        insta::assert_toml_snapshot!(derived);
    }

    #[tokio::test]
    async fn quotation() {
        let schema = test_schema().await;

        let created = schema
            .execute(Request::new(
                r#"
            mutation {
                wasQuotedFrom(generatedEntity: "http://blockchaintp.com/chronicle/ns#entity:generated",
                            usedEntity: "http://blockchaintp.com/chronicle/ns#entity:used") {
                    context
                }
            }
        "#,
            ))
            .await;

        insta::assert_toml_snapshot!(created);

        tokio::time::sleep(Duration::from_millis(100)).await;
        let derived = schema
            .execute(Request::new(
                r#"
            query {
                entityById(id: "http://blockchaintp.com/chronicle/ns#entity:generated") {
                    wasDerivedFrom {
                        id
                    }
                    wasQuotedFrom {
                        id
                    }
                }
            }
        "#,
            ))
            .await;
        insta::assert_toml_snapshot!(derived);
    }

    #[tokio::test]
    async fn agent_can_be_created() {
        let schema = test_schema().await;

        let create = schema
            .execute(Request::new(
                r#"
            mutation {
                agent(name:"bobross", typ: "artist") {
                    context
                }
            }
        "#,
            ))
            .await;

        insta::assert_toml_snapshot!(create);
    }

    #[tokio::test]
    async fn query_agents_by_cursor() {
        let schema = test_schema().await;

        for i in 0..100 {
            schema
                .execute(Request::new(format!(
                    r#"
            mutation {{
                agent(name:"bobross{}", typ: "artist") {{
                    context
                }}
            }}
        "#,
                    i
                )))
                .await;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;

        let default_cursor = schema
            .execute(Request::new(
                r#"
                query {
                agentsByType(typ: "artist") {
                    pageInfo {
                        hasPreviousPage
                        hasNextPage
                        startCursor
                        endCursor
                    }
                    edges {
                        node {
                            id,
                            name
                        }
                        cursor
                    }
                }
                }
        "#,
            ))
            .await;

        insta::assert_json_snapshot!(default_cursor);

        let middle_cursor = schema
            .execute(Request::new(
                r#"
                query {
                agentsByType(typ: "artist", first: 20, after: "3") {
                    pageInfo {
                        hasPreviousPage
                        hasNextPage
                        startCursor
                        endCursor
                    }
                    edges {
                        node {
                            id,
                            name
                        }
                        cursor
                    }
                }
                }
        "#,
            ))
            .await;

        insta::assert_json_snapshot!(middle_cursor);

        let out_of_bound_cursor = schema
            .execute(Request::new(
                r#"
                query {
                agentsByType(typ: "artist", first: 20, after: "90") {
                    pageInfo {
                        hasPreviousPage
                        hasNextPage
                        startCursor
                        endCursor
                    }
                    edges {
                        node {
                            id,
                            name
                        }
                        cursor
                    }
                }
                }
        "#,
            ))
            .await;

        insta::assert_json_snapshot!(out_of_bound_cursor);
    }
}
