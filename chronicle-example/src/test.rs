use chronicle::codegen::ChronicleDomainDef;
use chronicle::tokio;
use chronicle::{api::chronicle_graphql::ChronicleGraphQl, bootstrap};
use main::{Mutation, Query};

#[allow(dead_code)]
mod main;

///Entry point here is jigged a little, as we want to run unit tests, see chronicle-untyped for the actual pattern
#[tokio::main]
pub async fn main() {
    let s = r#"
name: "chronicle"
attributes:
  Title:
    type: "String"
  Location:
    type: "String"
  PurchaseValue:
    type: "String"
  PurchaseValueCurrency:
    type: "String"
  Description:
    type: "String"
  Name:
    type: "String"
agents:
  Member:
    attributes:
      - Name
  Artist:
    attributes:
      - Name
entities:
  Artwork:
    attributes:
      - Title
  ArtworkDetails:
    attributes:
      - Title
      - Description
activities:
  Exhibited:
    attributes:
      - Location
  Created:
    attributes:
      - Title
  Sold:
    attributes:
      - PurchaseValue
      - PurchaseValueCurrency
  Transferred:
    attributes: []
roles:
  - Buyer
  - Seller
  - Broker
  - Editor
  - Creator
"#
    .to_string();

    let model = ChronicleDomainDef::from_input_string(&s).unwrap();

    bootstrap(model, ChronicleGraphQl::new(Query, Mutation)).await
}

#[cfg(test)]
mod test {
    use super::{Mutation, Query};
    use chronicle::api::chronicle_graphql::{Store, Subscription};
    use chronicle::api::{Api, ConnectionOptions, UuidGen};
    use chronicle::async_graphql::{Request, Schema};
    use chronicle::chrono::{DateTime, NaiveDate, Utc};
    use chronicle::common::ledger::InMemLedger;
    use chronicle::tokio;
    use chronicle::uuid::Uuid;
    use diesel::r2d2::Pool;
    use diesel::{r2d2::ConnectionManager, SqliteConnection};
    use std::time::Duration;
    use tempfile::TempDir;

    #[derive(Debug, Clone)]
    struct SameUuid;

    impl UuidGen for SameUuid {
        fn uuid() -> Uuid {
            Uuid::parse_str("5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea").unwrap()
        }
    }

    async fn test_schema() -> Schema<Query, Mutation, Subscription> {
        telemetry::telemetry(None, true);

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
    async fn agent_delegation() {
        let schema = test_schema().await;

        let created = schema
            .execute(Request::new(
                r#"
            mutation {
                actedOnBehalfOf(
                    responsible: "http://blockchaintp.com/chronicle/ns#agent:responsible",
                    delegate: "http://blockchaintp.com/chronicle/ns#agent:delegate",
                    role: DELEGATE
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
                    ... on ProvAgent {
                        id
                        name
                        actedOnBehalfOf {
                            agent {
                                ... on ProvAgent {
                                    id
                                }
                            }
                            role
                        }
                    }
                }
            }
        "#,
            ))
            .await;
        insta::assert_json_snapshot!(derived.data);
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
                    ... on ProvEntity {
                        id
                        name
                        wasDerivedFrom {
                            ... on ProvEntity {
                                id
                            }
                        }
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

                    ... on ProvEntity {
                        id
                        name
                        wasDerivedFrom {
                            ... on ProvEntity {
                                id
                            }
                        }
                        hadPrimarySource{
                            ... on ProvEntity {
                                id
                            }
                        }
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
                    ... on ProvEntity {
                        id
                        name
                        wasDerivedFrom {
                            ... on ProvEntity {
                                id
                            }
                        }
                        wasRevisionOf {
                            ... on ProvEntity {
                                id
                            }
                        }
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
                    ... on ProvEntity {
                        id
                        name
                        wasDerivedFrom {
                            ... on ProvEntity {
                                id
                            }
                        }
                        wasQuotedFrom {
                            ... on ProvEntity {
                                id
                            }
                        }
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
    async fn query_activity_timeline() {
        let schema = test_schema().await;

        let res = schema
                .execute(Request::new(
                    r#"
            mutation {
                friend(name:"ringo", attributes: { stringAttribute: "string", intAttribute: 1, boolAttribute: false }) {
                    context
                }
            }
        "#,
                ))
                .await;

        assert_eq!(res.errors, vec![]);

        tokio::time::sleep(Duration::from_millis(100)).await;

        let res = schema
                .execute(Request::new(
                    r#"
            mutation {
                theSea(name:"coral", attributes: { stringAttribute: "string", intAttribute: 1, boolAttribute: false }) {
                    context
                }
            }
        "#,
                ))
                .await;

        assert_eq!(res.errors, vec![]);

        tokio::time::sleep(Duration::from_millis(100)).await;

        let from = DateTime::<Utc>::from_utc(NaiveDate::from_ymd(1968, 9, 1).and_hms(0, 0, 0), Utc);

        for i in 1..10 {
            let res = schema
                .execute(Request::new(format!(
                    r#"
            mutation {{
                gardening(name:"gardening{}", attributes: {{ stringAttribute: "String", intAttribute: 1, boolAttribute: false }}) {{
                    context
                }}
            }}
        "#,
                    i
                )))
                .await;
            assert_eq!(res.errors, vec![]);

            tokio::time::sleep(Duration::from_millis(100)).await;
            let res = schema
                .execute(Request::new(format!(
                    r#"
            mutation {{
                used(id: "http://blockchaintp.com/chronicle/ns#entity:coral", activity: "http://blockchaintp.com/chronicle/ns#activity:gardening{}") {{
                    context
                }}
            }}
        "#,
                    i
                )))
                .await;
            assert_eq!(res.errors, vec![]);

            tokio::time::sleep(Duration::from_millis(100)).await;

            let res = schema
                .execute(Request::new(format!(
                    r#"
            mutation {{
                endActivity( time: "{}", id: "http://http://blockchaintp.com/chronicle/ns#activity:gardening{}") {{
                    context
                }}
            }}
        "#,
                   from.checked_add_signed(chronicle::chrono::Duration::days(i)).unwrap().to_rfc3339() ,i
                )))
                .await;

            assert_eq!(res.errors, vec![]);

            tokio::time::sleep(Duration::from_millis(100)).await;

            let res = schema
                .execute(Request::new(format!(
                    r#"
            mutation {{
                wasAssociatedWith( role: RESPONSIBLE, responsible: "http://blockchaintp.com/chronicle/ns#agent:ringo", activity: "http://http://blockchaintp.com/chronicle/ns#activity:gardening{}") {{
                    context
                }}
            }}
        "#, i
                )))
                .await;

            tokio::time::sleep(Duration::from_millis(100)).await;

            assert_eq!(res.errors, vec![]);
        }

        tokio::time::sleep(Duration::from_millis(100)).await;

        let default_cursor = schema
            .execute(Request::new(
                r#"
                query {
                activityTimeline(forEntity: ["http://blockchaintp.com/chronicle/ns#entity:coral"],
                                from: "1968-01-01T00:00:00Z",
                                to: "2030-01-01T00:00:00Z",
                                activityTypes: [GARDENING],
                                ) {
                    pageInfo {
                        hasPreviousPage
                        hasNextPage
                        startCursor
                        endCursor
                    }
                    edges {
                        node {
                            __typename
                            ... on Gardening {
                                id
                                name
                                stringAttribute
                                intAttribute
                                boolAttribute
                                wasAssociatedWith {
                                        responsible {
                                            agent {
                                                ... on Friend {
                                                    id
                                                    name
                                                }
                                            }
                                            role
                                        }
                                }
                                used {
                                    ... on TheSea {
                                        id
                                        name
                                    }
                                }
                            }
                       }
                        cursor
                    }
                }
                }
        "#,
            ))
            .await;

        insta::assert_json_snapshot!(default_cursor);
    }

    #[tokio::test]
    async fn query_agents_by_cursor() {
        let schema = test_schema().await;

        for i in 0..100 {
            let res = schema
                .execute(Request::new(format!(
                    r#"
            mutation {{
                friend(name:"bobross{}", attributes: {{ stringAttribute: "String", intAttribute: 1, boolAttribute: false }}) {{
                    context
                }}
            }}
        "#,
                    i
                )))
                .await;

            assert_eq!(res.errors, vec![]);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;

        let default_cursor = schema
            .execute(Request::new(
                r#"
                query {
                agentsByType(agentType: FRIEND) {
                    pageInfo {
                        hasPreviousPage
                        hasNextPage
                        startCursor
                        endCursor
                    }
                    edges {
                        node {
                            __typename
                            ... on Friend {
                                id
                                name
                                stringAttribute
                                intAttribute
                                boolAttribute
                            }
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
                agentsByType(agentType: FRIEND, first: 20, after: "3") {
                    pageInfo {
                        hasPreviousPage
                        hasNextPage
                        startCursor
                        endCursor
                    }
                    edges {
                        node {
                            __typename
                            ... on Friend {
                                id
                                name
                                stringAttribute
                                intAttribute
                                boolAttribute
                            }
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
                agentsByType(agentType: FRIEND, first: 20, after: "90") {
                    pageInfo {
                        hasPreviousPage
                        hasNextPage
                        startCursor
                        endCursor
                    }
                    edges {
                        node {
                            __typename
                            ... on Friend {
                                id
                                name
                                stringAttribute
                                intAttribute
                                boolAttribute
                            }
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
