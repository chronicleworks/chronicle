//! The default graphql api - only abstract resources
//! We delegate to the underlying concrete, attributeless graphql objects in the api crate,
//! wrapping those in our types here.
#![cfg_attr(feature = "strict", deny(warnings))]
use api::{
    self,
    chronicle_graphql::{self, ChronicleGraphQl, Evidence, Identity, Namespace, Submission},
};
use async_graphql::{
    connection::{Connection, EmptyFields},
    *,
};
use bootstrap::*;
use chrono::{DateTime, Utc};
use common::prov::{vocab::Prov, ActivityId, AgentId, DomaintypeId, EntityId};
use iref::Iri;

#[derive(Default, InputObject)]
pub struct Attributes {
    #[graphql(name = "type")]
    pub typ: Option<String>,
}

impl From<Attributes> for common::attributes::Attributes {
    fn from(attributes: Attributes) -> Self {
        common::attributes::Attributes {
            typ: attributes.typ.map(|typ| DomaintypeId::from_name(&typ)),
            ..Default::default()
        }
    }
}

pub struct Activity(chronicle_graphql::Activity);

#[Object]
impl Activity {
    async fn id(&self) -> ActivityId {
        ActivityId::from_name(&*self.0.name)
    }

    async fn namespace<'a>(&self, ctx: &Context<'a>) -> async_graphql::Result<Namespace> {
        chronicle_graphql::activity::namespace(self.0.namespace_id, ctx).await
    }

    async fn name(&self) -> &str {
        &self.0.name
    }

    async fn started(&self) -> Option<DateTime<Utc>> {
        self.0.started.map(|x| DateTime::from_utc(x, Utc))
    }

    async fn ended(&self) -> Option<DateTime<Utc>> {
        self.0.ended.map(|x| DateTime::from_utc(x, Utc))
    }

    #[graphql(name = "type")]
    async fn typ(&self) -> String {
        if let Some(ref typ) = self.0.domaintype {
            typ.to_string()
        } else {
            Iri::from(Prov::Activity).to_string()
        }
    }

    async fn was_associated_with<'a>(
        &self,
        ctx: &Context<'a>,
    ) -> async_graphql::Result<Vec<Agent>> {
        Ok(
            chronicle_graphql::activity::was_associated_with(self.0.id, ctx)
                .await?
                .into_iter()
                .map(Agent)
                .collect(),
        )
    }

    async fn used<'a>(&self, ctx: &Context<'a>) -> async_graphql::Result<Vec<Entity>> {
        Ok(chronicle_graphql::activity::used(self.0.id, ctx)
            .await?
            .into_iter()
            .map(Entity)
            .collect())
    }
}
pub struct Entity(chronicle_graphql::Entity);

#[Object]
impl Entity {
    async fn id(&self) -> EntityId {
        EntityId::from_name(&*self.0.name)
    }

    async fn namespace<'a>(&self, ctx: &Context<'a>) -> async_graphql::Result<Namespace> {
        chronicle_graphql::entity::namespace(self.0.namespace_id, ctx).await
    }

    async fn name(&self) -> &str {
        &self.0.name
    }

    #[graphql(name = "type")]
    async fn typ(&self) -> String {
        if let Some(ref typ) = self.0.domaintype {
            typ.to_string()
        } else {
            Iri::from(Prov::Agent).to_string()
        }
    }

    async fn evidence<'a>(&self, ctx: &Context<'a>) -> async_graphql::Result<Option<Evidence>> {
        chronicle_graphql::entity::evidence(self.0.attachment_id, ctx).await
    }

    async fn was_generated_by<'a>(
        &self,
        ctx: &Context<'a>,
    ) -> async_graphql::Result<Vec<Activity>> {
        Ok(chronicle_graphql::entity::was_generated_by(self.0.id, ctx)
            .await?
            .into_iter()
            .map(Activity)
            .collect())
    }

    async fn was_derived_from<'a>(&self, ctx: &Context<'a>) -> async_graphql::Result<Vec<Entity>> {
        Ok(chronicle_graphql::entity::was_derived_from(self.0.id, ctx)
            .await?
            .into_iter()
            .map(Entity)
            .collect())
    }

    async fn had_primary_source<'a>(
        &self,
        ctx: &Context<'a>,
    ) -> async_graphql::Result<Vec<Entity>> {
        Ok(
            chronicle_graphql::entity::had_primary_source(self.0.id, ctx)
                .await?
                .into_iter()
                .map(Entity)
                .collect(),
        )
    }

    async fn was_revision_of<'a>(&self, ctx: &Context<'a>) -> async_graphql::Result<Vec<Entity>> {
        Ok(chronicle_graphql::entity::was_revision_of(self.0.id, ctx)
            .await?
            .into_iter()
            .map(Entity)
            .collect())
    }

    async fn was_quoted_from<'a>(&self, ctx: &Context<'a>) -> async_graphql::Result<Vec<Entity>> {
        Ok(chronicle_graphql::entity::was_quoted_from(self.0.id, ctx)
            .await?
            .into_iter()
            .map(Entity)
            .collect())
    }
}

pub struct Agent(chronicle_graphql::Agent);

#[Object]
impl Agent {
    async fn id(&self) -> AgentId {
        AgentId::from_name(&*self.0.name)
    }

    async fn name(&self) -> &str {
        &self.0.name
    }

    async fn namespace<'a>(&self, ctx: &Context<'a>) -> async_graphql::Result<Namespace> {
        chronicle_graphql::agent::namespace(self.0.namespace_id, ctx).await
    }

    async fn identity<'a>(&self, ctx: &Context<'a>) -> async_graphql::Result<Option<Identity>> {
        chronicle_graphql::agent::identity(self.0.identity_id, ctx).await
    }

    async fn acted_on_behalf_of<'a>(&self, ctx: &Context<'a>) -> async_graphql::Result<Vec<Agent>> {
        Ok(chronicle_graphql::agent::acted_on_behalf_of(self.0.id, ctx)
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

#[derive(Copy, Clone)]
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
        chronicle_graphql::mutation::agent(ctx, name, namespace, attributes.into()).await
    }

    pub async fn activity<'a>(
        &self,
        ctx: &Context<'a>,
        name: String,
        namespace: Option<String>,
        attributes: Attributes,
    ) -> async_graphql::Result<Submission> {
        chronicle_graphql::mutation::activity(ctx, name, namespace, attributes.into()).await
    }

    pub async fn entity<'a>(
        &self,
        ctx: &Context<'a>,
        name: String,
        namespace: Option<String>,
        attributes: Attributes,
    ) -> async_graphql::Result<Submission> {
        chronicle_graphql::mutation::entity(ctx, name, namespace, attributes.into()).await
    }

    pub async fn acted_on_behalf_of<'a>(
        &self,
        ctx: &Context<'a>,
        namespace: Option<String>,
        responsible: AgentId,
        delegate: AgentId,
    ) -> async_graphql::Result<Submission> {
        chronicle_graphql::mutation::acted_on_behalf_of(ctx, namespace, responsible, delegate).await
    }

    pub async fn was_derived_from<'a>(
        &self,
        ctx: &Context<'a>,
        namespace: Option<String>,
        generated_entity: EntityId,
        used_entity: EntityId,
    ) -> async_graphql::Result<Submission> {
        chronicle_graphql::mutation::was_derived_from(ctx, namespace, generated_entity, used_entity)
            .await
    }

    pub async fn was_revision_of<'a>(
        &self,
        ctx: &Context<'a>,
        namespace: Option<String>,
        generated_entity: EntityId,
        used_entity: EntityId,
    ) -> async_graphql::Result<Submission> {
        chronicle_graphql::mutation::was_revision_of(ctx, namespace, generated_entity, used_entity)
            .await
    }

    pub async fn had_primary_source<'a>(
        &self,
        ctx: &Context<'a>,
        namespace: Option<String>,
        generated_entity: EntityId,
        used_entity: EntityId,
    ) -> async_graphql::Result<Submission> {
        chronicle_graphql::mutation::had_primary_source(
            ctx,
            namespace,
            generated_entity,
            used_entity,
        )
        .await
    }

    pub async fn was_quoted_from<'a>(
        &self,
        ctx: &Context<'a>,
        namespace: Option<String>,
        generated_entity: EntityId,
        used_entity: EntityId,
    ) -> async_graphql::Result<Submission> {
        chronicle_graphql::mutation::was_quoted_from(ctx, namespace, generated_entity, used_entity)
            .await
    }

    pub async fn generate_key<'a>(
        &self,
        ctx: &Context<'a>,
        id: AgentId,
        namespace: Option<String>,
    ) -> async_graphql::Result<Submission> {
        chronicle_graphql::mutation::generate_key(ctx, id, namespace).await
    }

    pub async fn start_activity<'a>(
        &self,
        ctx: &Context<'a>,
        id: ActivityId,
        namespace: Option<String>,
        agent: AgentId,
        time: Option<DateTime<Utc>>,
    ) -> async_graphql::Result<Submission> {
        chronicle_graphql::mutation::start_activity(ctx, id, namespace, agent, time).await
    }

    pub async fn end_activity<'a>(
        &self,
        ctx: &Context<'a>,
        id: ActivityId,
        namespace: Option<String>,
        agent: AgentId,
        time: Option<DateTime<Utc>>,
    ) -> async_graphql::Result<Submission> {
        chronicle_graphql::mutation::end_activity(ctx, id, namespace, agent, time).await
    }

    pub async fn used<'a>(
        &self,
        ctx: &Context<'a>,
        activity: ActivityId,
        id: EntityId,
        namespace: Option<String>,
        _typ: Option<String>,
    ) -> async_graphql::Result<Submission> {
        chronicle_graphql::mutation::used(ctx, activity, id, namespace).await
    }

    pub async fn was_generated_by<'a>(
        &self,
        ctx: &Context<'a>,
        activity: ActivityId,
        id: EntityId,
        namespace: Option<String>,
    ) -> async_graphql::Result<Submission> {
        chronicle_graphql::mutation::was_generated_by(ctx, activity, id, namespace).await
    }

    pub async fn has_attachment<'a>(
        &self,
        ctx: &Context<'a>,
        id: EntityId,
        namespace: Option<String>,
        attachment: Upload,
        agent: AgentId,
        locator: String,
    ) -> async_graphql::Result<Submission> {
        chronicle_graphql::mutation::has_attachment(ctx, id, namespace, attachment, agent, locator)
            .await
    }
}

#[derive(Copy, Clone)]
pub struct Query;

#[Object]
impl Query {
    #[allow(clippy::too_many_arguments)]
    pub async fn agents_by_type<'a>(
        &self,
        ctx: &Context<'a>,
        agent_type: ID,
        namespace: Option<ID>,
        after: Option<String>,
        before: Option<String>,
        first: Option<i32>,
        last: Option<i32>,
    ) -> async_graphql::Result<Connection<i32, Agent, EmptyFields, EmptyFields>> {
        Ok(chronicle_graphql::query::agents_by_type(
            ctx, agent_type, namespace, after, before, first, last,
        )
        .await?
        .map_node(Agent))
    }

    pub async fn agent_by_id<'a>(
        &self,
        ctx: &Context<'a>,
        id: AgentId,
        namespace: Option<String>,
    ) -> async_graphql::Result<Option<Agent>> {
        Ok(chronicle_graphql::query::agent_by_id(ctx, id, namespace)
            .await?
            .map(Agent))
    }

    pub async fn entity_by_id<'a>(
        &self,
        ctx: &Context<'a>,
        id: EntityId,
        namespace: Option<String>,
    ) -> async_graphql::Result<Option<Entity>> {
        Ok(chronicle_graphql::query::entity_by_id(ctx, id, namespace)
            .await?
            .map(Entity))
    }
}

#[tokio::main]
pub async fn main() {
    bootstrap(ChronicleGraphQl::new(Query, Mutation)).await
}

#[cfg(test)]
mod test {
    use super::{Mutation, Query};
    use crate::chronicle_graphql::{Store, Subscription};
    use api::{Api, ConnectionOptions, UuidGen};
    use async_graphql::{Request, Schema};
    use common::ledger::InMemLedger;
    use diesel::{
        r2d2::{ConnectionManager, Pool},
        SqliteConnection,
    };
    use std::time::Duration;
    use tempfile::TempDir;
    use tracing::Level;
    use uuid::Uuid;

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

        let dispatch = Api::new(
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
                    id
                    name
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
                    id
                    name
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
                    id
                    name
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
                    id
                    name
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
                    id
                    name
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
                agent(name:"bobross", attributes: { type: "artist" }) {
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
                agent(name:"bobross{}", attributes: {{ type: "artist"}}) {{
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
                agentsByType(agentType: "artist") {
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
                agentsByType(agentType: "artist", first: 20, after: "3") {
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
                agentsByType(agentType: "artist", first: 20, after: "90") {
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
