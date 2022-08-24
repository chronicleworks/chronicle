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
      String:
        type: "String"
      Int:
        type: "Int"
      Bool:
        type: "Bool"
    agents:
      friend:
        attributes:
          - String
          - Int
          - Bool
    entities:
      octopi:
        attributes:
          - String
          - Int
          - Bool
      the sea:
        attributes:
          - String
          - Int
          - Bool
    activities:
      gardening:
        attributes:
          - String
          - Int
          - Bool
      swim about:
        attributes:
          - String
          - Int
          - Bool
    roles:
        - delegate
        - responsible
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

        insta::assert_toml_snapshot!(created, @r###"
        [data.actedOnBehalfOf]
        context = 'http://blockchaintp.com/chronicle/ns#agent:responsible'
        "###);

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
        insta::assert_json_snapshot!(derived.data, @r###"
        {
          "agentById": {
            "id": "http://blockchaintp.com/chronicle/ns#agent:responsible",
            "name": "responsible",
            "actedOnBehalfOf": [
              {
                "agent": {
                  "id": "http://blockchaintp.com/chronicle/ns#agent:delegate"
                },
                "role": "DELEGATE"
              }
            ]
          }
        }
        "###);
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

        insta::assert_toml_snapshot!(created, @r###"
        [data.wasDerivedFrom]
        context = 'http://blockchaintp.com/chronicle/ns#entity:generated'
        "###);

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
        insta::assert_toml_snapshot!(derived, @r###"
        [data.entityById]
        id = 'http://blockchaintp.com/chronicle/ns#entity:generated'
        name = 'generated'

        [[data.entityById.wasDerivedFrom]]
        id = 'http://blockchaintp.com/chronicle/ns#entity:used'
        "###);
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

        insta::assert_toml_snapshot!(created, @r###"
        [data.hadPrimarySource]
        context = 'http://blockchaintp.com/chronicle/ns#entity:generated'
        "###);

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
        insta::assert_toml_snapshot!(derived, @r###"
        [data.entityById]
        id = 'http://blockchaintp.com/chronicle/ns#entity:generated'
        name = 'generated'

        [[data.entityById.wasDerivedFrom]]
        id = 'http://blockchaintp.com/chronicle/ns#entity:used'

        [[data.entityById.hadPrimarySource]]
        id = 'http://blockchaintp.com/chronicle/ns#entity:used'
        "###);
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

        insta::assert_toml_snapshot!(created, @r###"
        [data.wasRevisionOf]
        context = 'http://blockchaintp.com/chronicle/ns#entity:generated'
        "###);

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
        insta::assert_toml_snapshot!(derived, @r###"
        [data.entityById]
        id = 'http://blockchaintp.com/chronicle/ns#entity:generated'
        name = 'generated'

        [[data.entityById.wasDerivedFrom]]
        id = 'http://blockchaintp.com/chronicle/ns#entity:used'

        [[data.entityById.wasRevisionOf]]
        id = 'http://blockchaintp.com/chronicle/ns#entity:used'
        "###);
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

        insta::assert_toml_snapshot!(created, @r###"
        [data.wasQuotedFrom]
        context = 'http://blockchaintp.com/chronicle/ns#entity:generated'
        "###);

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
        insta::assert_toml_snapshot!(derived, @r###"
        [data.entityById]
        id = 'http://blockchaintp.com/chronicle/ns#entity:generated'
        name = 'generated'

        [[data.entityById.wasDerivedFrom]]
        id = 'http://blockchaintp.com/chronicle/ns#entity:used'

        [[data.entityById.wasQuotedFrom]]
        id = 'http://blockchaintp.com/chronicle/ns#entity:used'
        "###);
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

        insta::assert_toml_snapshot!(create, @r###"
        [data.agent]
        context = 'http://blockchaintp.com/chronicle/ns#agent:bobross'
        "###);
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
                friend(name:"john", attributes: { stringAttribute: "string", intAttribute: 1, boolAttribute: false }) {
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

        let res = schema
                .execute(Request::new(
                    r#"
            mutation {
                theSea(name:"fish", attributes: { stringAttribute: "string", intAttribute: 1, boolAttribute: false }) {
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
            let activity_name = if i % 2 == 0 {
                format!("gardening{}", i)
            } else {
                format!("swimming{}", i)
            };

            if (i % 2) == 0 {
                let res = schema
                    .execute(Request::new(
                        &format!(
                            r#"
                    mutation {{
                        gardening(name:"{}", attributes: {{ stringAttribute: "string", intAttribute: 1, boolAttribute: false }}) {{
                            context
                        }}
                    }}
                "#,
                            activity_name
                        ),
                    ))
                    .await;

                assert_eq!(res.errors, vec![]);
            } else {
                let res = schema
                    .execute(Request::new(
                        &format!(
                            r#"
                    mutation {{
                        swimAbout(name:"{}", attributes: {{ stringAttribute: "string", intAttribute: 1, boolAttribute: false }}) {{
                            context
                        }}
                    }}
                "#,
                            activity_name
                        ),
                    ))
                    .await;

                assert_eq!(res.errors, vec![]);
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
            let res = schema
                .execute(Request::new(format!(
                    r#"
            mutation {{
                used(id: "http://blockchaintp.com/chronicle/ns#entity:coral", activity: "http://blockchaintp.com/chronicle/ns#activity:{}") {{
                    context
                }}
            }}
        "#,
                    activity_name
                )))
                .await;
            assert_eq!(res.errors, vec![]);

            tokio::time::sleep(Duration::from_millis(100)).await;

            let res = schema
                .execute(Request::new(format!(
                    r#"
            mutation {{
                endActivity( time: "{}", id: "http://http://blockchaintp.com/chronicle/ns#activity:{}") {{
                    context
                }}
            }}
        "#,
                   from.checked_add_signed(chronicle::chrono::Duration::days(i)).unwrap().to_rfc3339() , activity_name
                )))
                .await;

            assert_eq!(res.errors, vec![]);

            tokio::time::sleep(Duration::from_millis(100)).await;

            let agent = if i % 2 == 0 { "ringo" } else { "john" };

            let res = schema
                .execute(Request::new(format!(
                    r#"
            mutation {{
                wasAssociatedWith( role: RESPONSIBLE, responsible: "http://blockchaintp.com/chronicle/ns#agent:{}", activity: "http://http://blockchaintp.com/chronicle/ns#activity:{}") {{
                    context
                }}
            }}
        "#, agent, activity_name
                )))
                .await;

            tokio::time::sleep(Duration::from_millis(100)).await;

            assert_eq!(res.errors, vec![]);
        }

        tokio::time::sleep(Duration::from_millis(100)).await;

        let entire_timeline_in_order = schema
            .execute(Request::new(
                r#"
                query {
                activityTimeline(
                  forEntity: [],
                  forAgent: [],
                  order: OLDEST_FIRST,
                  activityTypes: [],
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
                                started
                                ended
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

        insta::assert_json_snapshot!(entire_timeline_in_order, @r###"
        {
          "data": {
            "activityTimeline": {
              "pageInfo": {
                "hasPreviousPage": false,
                "hasNextPage": false,
                "startCursor": "0",
                "endCursor": "8"
              },
              "edges": [
                {
                  "node": {
                    "__typename": "SwimAbout"
                  },
                  "cursor": "0"
                },
                {
                  "node": {
                    "__typename": "Gardening",
                    "id": "http://blockchaintp.com/chronicle/ns#entity:gardening2",
                    "name": "gardening2",
                    "stringAttribute": "string",
                    "intAttribute": 1,
                    "boolAttribute": false,
                    "started": "1968-09-03T00:00:00+00:00",
                    "ended": "1968-09-03T00:00:00+00:00",
                    "wasAssociatedWith": [
                      {
                        "responsible": {
                          "agent": {
                            "id": "http://blockchaintp.com/chronicle/ns#agent:ringo",
                            "name": "ringo"
                          },
                          "role": "RESPONSIBLE"
                        }
                      }
                    ],
                    "used": [
                      {
                        "id": "http://blockchaintp.com/chronicle/ns#entity:coral",
                        "name": "coral"
                      }
                    ]
                  },
                  "cursor": "1"
                },
                {
                  "node": {
                    "__typename": "SwimAbout"
                  },
                  "cursor": "2"
                },
                {
                  "node": {
                    "__typename": "Gardening",
                    "id": "http://blockchaintp.com/chronicle/ns#entity:gardening4",
                    "name": "gardening4",
                    "stringAttribute": "string",
                    "intAttribute": 1,
                    "boolAttribute": false,
                    "started": "1968-09-05T00:00:00+00:00",
                    "ended": "1968-09-05T00:00:00+00:00",
                    "wasAssociatedWith": [
                      {
                        "responsible": {
                          "agent": {
                            "id": "http://blockchaintp.com/chronicle/ns#agent:ringo",
                            "name": "ringo"
                          },
                          "role": "RESPONSIBLE"
                        }
                      }
                    ],
                    "used": [
                      {
                        "id": "http://blockchaintp.com/chronicle/ns#entity:coral",
                        "name": "coral"
                      }
                    ]
                  },
                  "cursor": "3"
                },
                {
                  "node": {
                    "__typename": "SwimAbout"
                  },
                  "cursor": "4"
                },
                {
                  "node": {
                    "__typename": "Gardening",
                    "id": "http://blockchaintp.com/chronicle/ns#entity:gardening6",
                    "name": "gardening6",
                    "stringAttribute": "string",
                    "intAttribute": 1,
                    "boolAttribute": false,
                    "started": "1968-09-07T00:00:00+00:00",
                    "ended": "1968-09-07T00:00:00+00:00",
                    "wasAssociatedWith": [
                      {
                        "responsible": {
                          "agent": {
                            "id": "http://blockchaintp.com/chronicle/ns#agent:ringo",
                            "name": "ringo"
                          },
                          "role": "RESPONSIBLE"
                        }
                      }
                    ],
                    "used": [
                      {
                        "id": "http://blockchaintp.com/chronicle/ns#entity:coral",
                        "name": "coral"
                      }
                    ]
                  },
                  "cursor": "5"
                },
                {
                  "node": {
                    "__typename": "SwimAbout"
                  },
                  "cursor": "6"
                },
                {
                  "node": {
                    "__typename": "Gardening",
                    "id": "http://blockchaintp.com/chronicle/ns#entity:gardening8",
                    "name": "gardening8",
                    "stringAttribute": "string",
                    "intAttribute": 1,
                    "boolAttribute": false,
                    "started": "1968-09-09T00:00:00+00:00",
                    "ended": "1968-09-09T00:00:00+00:00",
                    "wasAssociatedWith": [
                      {
                        "responsible": {
                          "agent": {
                            "id": "http://blockchaintp.com/chronicle/ns#agent:ringo",
                            "name": "ringo"
                          },
                          "role": "RESPONSIBLE"
                        }
                      }
                    ],
                    "used": [
                      {
                        "id": "http://blockchaintp.com/chronicle/ns#entity:coral",
                        "name": "coral"
                      }
                    ]
                  },
                  "cursor": "7"
                },
                {
                  "node": {
                    "__typename": "SwimAbout"
                  },
                  "cursor": "8"
                }
              ]
            }
          }
        }
        "###);

        let entire_timeline_reverse_order = schema
            .execute(Request::new(
                r#"
                query {
                activityTimeline(
                  forEntity: [],
                  forAgent: [],
                  order: NEWEST_FIRST,
                  activityTypes: [],
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
                                started
                                ended
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

        insta::assert_json_snapshot!(entire_timeline_reverse_order, @r###"
        {
          "data": {
            "activityTimeline": {
              "pageInfo": {
                "hasPreviousPage": false,
                "hasNextPage": false,
                "startCursor": "0",
                "endCursor": "8"
              },
              "edges": [
                {
                  "node": {
                    "__typename": "SwimAbout"
                  },
                  "cursor": "0"
                },
                {
                  "node": {
                    "__typename": "Gardening",
                    "id": "http://blockchaintp.com/chronicle/ns#entity:gardening8",
                    "name": "gardening8",
                    "stringAttribute": "string",
                    "intAttribute": 1,
                    "boolAttribute": false,
                    "started": "1968-09-09T00:00:00+00:00",
                    "ended": "1968-09-09T00:00:00+00:00",
                    "wasAssociatedWith": [
                      {
                        "responsible": {
                          "agent": {
                            "id": "http://blockchaintp.com/chronicle/ns#agent:ringo",
                            "name": "ringo"
                          },
                          "role": "RESPONSIBLE"
                        }
                      }
                    ],
                    "used": [
                      {
                        "id": "http://blockchaintp.com/chronicle/ns#entity:coral",
                        "name": "coral"
                      }
                    ]
                  },
                  "cursor": "1"
                },
                {
                  "node": {
                    "__typename": "SwimAbout"
                  },
                  "cursor": "2"
                },
                {
                  "node": {
                    "__typename": "Gardening",
                    "id": "http://blockchaintp.com/chronicle/ns#entity:gardening6",
                    "name": "gardening6",
                    "stringAttribute": "string",
                    "intAttribute": 1,
                    "boolAttribute": false,
                    "started": "1968-09-07T00:00:00+00:00",
                    "ended": "1968-09-07T00:00:00+00:00",
                    "wasAssociatedWith": [
                      {
                        "responsible": {
                          "agent": {
                            "id": "http://blockchaintp.com/chronicle/ns#agent:ringo",
                            "name": "ringo"
                          },
                          "role": "RESPONSIBLE"
                        }
                      }
                    ],
                    "used": [
                      {
                        "id": "http://blockchaintp.com/chronicle/ns#entity:coral",
                        "name": "coral"
                      }
                    ]
                  },
                  "cursor": "3"
                },
                {
                  "node": {
                    "__typename": "SwimAbout"
                  },
                  "cursor": "4"
                },
                {
                  "node": {
                    "__typename": "Gardening",
                    "id": "http://blockchaintp.com/chronicle/ns#entity:gardening4",
                    "name": "gardening4",
                    "stringAttribute": "string",
                    "intAttribute": 1,
                    "boolAttribute": false,
                    "started": "1968-09-05T00:00:00+00:00",
                    "ended": "1968-09-05T00:00:00+00:00",
                    "wasAssociatedWith": [
                      {
                        "responsible": {
                          "agent": {
                            "id": "http://blockchaintp.com/chronicle/ns#agent:ringo",
                            "name": "ringo"
                          },
                          "role": "RESPONSIBLE"
                        }
                      }
                    ],
                    "used": [
                      {
                        "id": "http://blockchaintp.com/chronicle/ns#entity:coral",
                        "name": "coral"
                      }
                    ]
                  },
                  "cursor": "5"
                },
                {
                  "node": {
                    "__typename": "SwimAbout"
                  },
                  "cursor": "6"
                },
                {
                  "node": {
                    "__typename": "Gardening",
                    "id": "http://blockchaintp.com/chronicle/ns#entity:gardening2",
                    "name": "gardening2",
                    "stringAttribute": "string",
                    "intAttribute": 1,
                    "boolAttribute": false,
                    "started": "1968-09-03T00:00:00+00:00",
                    "ended": "1968-09-03T00:00:00+00:00",
                    "wasAssociatedWith": [
                      {
                        "responsible": {
                          "agent": {
                            "id": "http://blockchaintp.com/chronicle/ns#agent:ringo",
                            "name": "ringo"
                          },
                          "role": "RESPONSIBLE"
                        }
                      }
                    ],
                    "used": [
                      {
                        "id": "http://blockchaintp.com/chronicle/ns#entity:coral",
                        "name": "coral"
                      }
                    ]
                  },
                  "cursor": "7"
                },
                {
                  "node": {
                    "__typename": "SwimAbout"
                  },
                  "cursor": "8"
                }
              ]
            }
          }
        }
        "###);

        let by_activity_type = schema
            .execute(Request::new(
                r#"
                query {
                activityTimeline(
                  forEntity: [],
                  forAgent: [],
                  order: NEWEST_FIRST,
                  activityTypes: [SWIM_ABOUT],
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
                        }
                        cursor
                    }
                }
                }
        "#,
            ))
            .await;

        insta::assert_json_snapshot!(by_activity_type, @r###"
        {
          "data": {
            "activityTimeline": {
              "pageInfo": {
                "hasPreviousPage": false,
                "hasNextPage": false,
                "startCursor": "0",
                "endCursor": "4"
              },
              "edges": [
                {
                  "node": {
                    "__typename": "SwimAbout"
                  },
                  "cursor": "0"
                },
                {
                  "node": {
                    "__typename": "SwimAbout"
                  },
                  "cursor": "1"
                },
                {
                  "node": {
                    "__typename": "SwimAbout"
                  },
                  "cursor": "2"
                },
                {
                  "node": {
                    "__typename": "SwimAbout"
                  },
                  "cursor": "3"
                },
                {
                  "node": {
                    "__typename": "SwimAbout"
                  },
                  "cursor": "4"
                }
              ]
            }
          }
        }
        "###);

        let by_agent = schema
            .execute(Request::new(
                r#"
                query {
                activityTimeline(
                  forEntity: [],
                  forAgent: ["http://blockchaintp.com/chronicle/ns#agent:john"],
                  order: NEWEST_FIRST,
                  activityTypes: [],
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
                        }
                        cursor
                    }
                }
                }
        "#,
            ))
            .await;

        insta::assert_json_snapshot!(by_agent, @r###"
        {
          "data": {
            "activityTimeline": {
              "pageInfo": {
                "hasPreviousPage": false,
                "hasNextPage": false,
                "startCursor": "0",
                "endCursor": "4"
              },
              "edges": [
                {
                  "node": {
                    "__typename": "SwimAbout"
                  },
                  "cursor": "0"
                },
                {
                  "node": {
                    "__typename": "SwimAbout"
                  },
                  "cursor": "1"
                },
                {
                  "node": {
                    "__typename": "SwimAbout"
                  },
                  "cursor": "2"
                },
                {
                  "node": {
                    "__typename": "SwimAbout"
                  },
                  "cursor": "3"
                },
                {
                  "node": {
                    "__typename": "SwimAbout"
                  },
                  "cursor": "4"
                }
              ]
            }
          }
        }
        "###);
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

        insta::assert_json_snapshot!(default_cursor, @r###"
        {
          "data": {
            "agentsByType": {
              "pageInfo": {
                "hasPreviousPage": false,
                "hasNextPage": true,
                "startCursor": "0",
                "endCursor": "9"
              },
              "edges": [
                {
                  "node": {
                    "__typename": "Friend",
                    "id": "http://blockchaintp.com/chronicle/ns#agent:bobross0",
                    "name": "bobross0",
                    "stringAttribute": "String",
                    "intAttribute": 1,
                    "boolAttribute": false
                  },
                  "cursor": "0"
                },
                {
                  "node": {
                    "__typename": "Friend",
                    "id": "http://blockchaintp.com/chronicle/ns#agent:bobross1",
                    "name": "bobross1",
                    "stringAttribute": "String",
                    "intAttribute": 1,
                    "boolAttribute": false
                  },
                  "cursor": "1"
                },
                {
                  "node": {
                    "__typename": "Friend",
                    "id": "http://blockchaintp.com/chronicle/ns#agent:bobross10",
                    "name": "bobross10",
                    "stringAttribute": "String",
                    "intAttribute": 1,
                    "boolAttribute": false
                  },
                  "cursor": "2"
                },
                {
                  "node": {
                    "__typename": "Friend",
                    "id": "http://blockchaintp.com/chronicle/ns#agent:bobross11",
                    "name": "bobross11",
                    "stringAttribute": "String",
                    "intAttribute": 1,
                    "boolAttribute": false
                  },
                  "cursor": "3"
                },
                {
                  "node": {
                    "__typename": "Friend",
                    "id": "http://blockchaintp.com/chronicle/ns#agent:bobross12",
                    "name": "bobross12",
                    "stringAttribute": "String",
                    "intAttribute": 1,
                    "boolAttribute": false
                  },
                  "cursor": "4"
                },
                {
                  "node": {
                    "__typename": "Friend",
                    "id": "http://blockchaintp.com/chronicle/ns#agent:bobross13",
                    "name": "bobross13",
                    "stringAttribute": "String",
                    "intAttribute": 1,
                    "boolAttribute": false
                  },
                  "cursor": "5"
                },
                {
                  "node": {
                    "__typename": "Friend",
                    "id": "http://blockchaintp.com/chronicle/ns#agent:bobross14",
                    "name": "bobross14",
                    "stringAttribute": "String",
                    "intAttribute": 1,
                    "boolAttribute": false
                  },
                  "cursor": "6"
                },
                {
                  "node": {
                    "__typename": "Friend",
                    "id": "http://blockchaintp.com/chronicle/ns#agent:bobross15",
                    "name": "bobross15",
                    "stringAttribute": "String",
                    "intAttribute": 1,
                    "boolAttribute": false
                  },
                  "cursor": "7"
                },
                {
                  "node": {
                    "__typename": "Friend",
                    "id": "http://blockchaintp.com/chronicle/ns#agent:bobross16",
                    "name": "bobross16",
                    "stringAttribute": "String",
                    "intAttribute": 1,
                    "boolAttribute": false
                  },
                  "cursor": "8"
                },
                {
                  "node": {
                    "__typename": "Friend",
                    "id": "http://blockchaintp.com/chronicle/ns#agent:bobross17",
                    "name": "bobross17",
                    "stringAttribute": "String",
                    "intAttribute": 1,
                    "boolAttribute": false
                  },
                  "cursor": "9"
                }
              ]
            }
          }
        }
        "###);

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

        insta::assert_json_snapshot!(middle_cursor, @r###"
        {
          "data": {
            "agentsByType": {
              "pageInfo": {
                "hasPreviousPage": true,
                "hasNextPage": true,
                "startCursor": "0",
                "endCursor": "19"
              },
              "edges": [
                {
                  "node": {
                    "__typename": "Friend",
                    "id": "http://blockchaintp.com/chronicle/ns#agent:bobross12",
                    "name": "bobross12",
                    "stringAttribute": "String",
                    "intAttribute": 1,
                    "boolAttribute": false
                  },
                  "cursor": "0"
                },
                {
                  "node": {
                    "__typename": "Friend",
                    "id": "http://blockchaintp.com/chronicle/ns#agent:bobross13",
                    "name": "bobross13",
                    "stringAttribute": "String",
                    "intAttribute": 1,
                    "boolAttribute": false
                  },
                  "cursor": "1"
                },
                {
                  "node": {
                    "__typename": "Friend",
                    "id": "http://blockchaintp.com/chronicle/ns#agent:bobross14",
                    "name": "bobross14",
                    "stringAttribute": "String",
                    "intAttribute": 1,
                    "boolAttribute": false
                  },
                  "cursor": "2"
                },
                {
                  "node": {
                    "__typename": "Friend",
                    "id": "http://blockchaintp.com/chronicle/ns#agent:bobross15",
                    "name": "bobross15",
                    "stringAttribute": "String",
                    "intAttribute": 1,
                    "boolAttribute": false
                  },
                  "cursor": "3"
                },
                {
                  "node": {
                    "__typename": "Friend",
                    "id": "http://blockchaintp.com/chronicle/ns#agent:bobross16",
                    "name": "bobross16",
                    "stringAttribute": "String",
                    "intAttribute": 1,
                    "boolAttribute": false
                  },
                  "cursor": "4"
                },
                {
                  "node": {
                    "__typename": "Friend",
                    "id": "http://blockchaintp.com/chronicle/ns#agent:bobross17",
                    "name": "bobross17",
                    "stringAttribute": "String",
                    "intAttribute": 1,
                    "boolAttribute": false
                  },
                  "cursor": "5"
                },
                {
                  "node": {
                    "__typename": "Friend",
                    "id": "http://blockchaintp.com/chronicle/ns#agent:bobross18",
                    "name": "bobross18",
                    "stringAttribute": "String",
                    "intAttribute": 1,
                    "boolAttribute": false
                  },
                  "cursor": "6"
                },
                {
                  "node": {
                    "__typename": "Friend",
                    "id": "http://blockchaintp.com/chronicle/ns#agent:bobross19",
                    "name": "bobross19",
                    "stringAttribute": "String",
                    "intAttribute": 1,
                    "boolAttribute": false
                  },
                  "cursor": "7"
                },
                {
                  "node": {
                    "__typename": "Friend",
                    "id": "http://blockchaintp.com/chronicle/ns#agent:bobross2",
                    "name": "bobross2",
                    "stringAttribute": "String",
                    "intAttribute": 1,
                    "boolAttribute": false
                  },
                  "cursor": "8"
                },
                {
                  "node": {
                    "__typename": "Friend",
                    "id": "http://blockchaintp.com/chronicle/ns#agent:bobross20",
                    "name": "bobross20",
                    "stringAttribute": "String",
                    "intAttribute": 1,
                    "boolAttribute": false
                  },
                  "cursor": "9"
                },
                {
                  "node": {
                    "__typename": "Friend",
                    "id": "http://blockchaintp.com/chronicle/ns#agent:bobross21",
                    "name": "bobross21",
                    "stringAttribute": "String",
                    "intAttribute": 1,
                    "boolAttribute": false
                  },
                  "cursor": "10"
                },
                {
                  "node": {
                    "__typename": "Friend",
                    "id": "http://blockchaintp.com/chronicle/ns#agent:bobross22",
                    "name": "bobross22",
                    "stringAttribute": "String",
                    "intAttribute": 1,
                    "boolAttribute": false
                  },
                  "cursor": "11"
                },
                {
                  "node": {
                    "__typename": "Friend",
                    "id": "http://blockchaintp.com/chronicle/ns#agent:bobross23",
                    "name": "bobross23",
                    "stringAttribute": "String",
                    "intAttribute": 1,
                    "boolAttribute": false
                  },
                  "cursor": "12"
                },
                {
                  "node": {
                    "__typename": "Friend",
                    "id": "http://blockchaintp.com/chronicle/ns#agent:bobross24",
                    "name": "bobross24",
                    "stringAttribute": "String",
                    "intAttribute": 1,
                    "boolAttribute": false
                  },
                  "cursor": "13"
                },
                {
                  "node": {
                    "__typename": "Friend",
                    "id": "http://blockchaintp.com/chronicle/ns#agent:bobross25",
                    "name": "bobross25",
                    "stringAttribute": "String",
                    "intAttribute": 1,
                    "boolAttribute": false
                  },
                  "cursor": "14"
                },
                {
                  "node": {
                    "__typename": "Friend",
                    "id": "http://blockchaintp.com/chronicle/ns#agent:bobross26",
                    "name": "bobross26",
                    "stringAttribute": "String",
                    "intAttribute": 1,
                    "boolAttribute": false
                  },
                  "cursor": "15"
                },
                {
                  "node": {
                    "__typename": "Friend",
                    "id": "http://blockchaintp.com/chronicle/ns#agent:bobross27",
                    "name": "bobross27",
                    "stringAttribute": "String",
                    "intAttribute": 1,
                    "boolAttribute": false
                  },
                  "cursor": "16"
                },
                {
                  "node": {
                    "__typename": "Friend",
                    "id": "http://blockchaintp.com/chronicle/ns#agent:bobross28",
                    "name": "bobross28",
                    "stringAttribute": "String",
                    "intAttribute": 1,
                    "boolAttribute": false
                  },
                  "cursor": "17"
                },
                {
                  "node": {
                    "__typename": "Friend",
                    "id": "http://blockchaintp.com/chronicle/ns#agent:bobross29",
                    "name": "bobross29",
                    "stringAttribute": "String",
                    "intAttribute": 1,
                    "boolAttribute": false
                  },
                  "cursor": "18"
                },
                {
                  "node": {
                    "__typename": "Friend",
                    "id": "http://blockchaintp.com/chronicle/ns#agent:bobross3",
                    "name": "bobross3",
                    "stringAttribute": "String",
                    "intAttribute": 1,
                    "boolAttribute": false
                  },
                  "cursor": "19"
                }
              ]
            }
          }
        }
        "###);

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

        insta::assert_json_snapshot!(out_of_bound_cursor , @r###"
        {
          "data": {
            "agentsByType": {
              "pageInfo": {
                "hasPreviousPage": true,
                "hasNextPage": false,
                "startCursor": "0",
                "endCursor": "8"
              },
              "edges": [
                {
                  "node": {
                    "__typename": "Friend",
                    "id": "http://blockchaintp.com/chronicle/ns#agent:bobross91",
                    "name": "bobross91",
                    "stringAttribute": "String",
                    "intAttribute": 1,
                    "boolAttribute": false
                  },
                  "cursor": "0"
                },
                {
                  "node": {
                    "__typename": "Friend",
                    "id": "http://blockchaintp.com/chronicle/ns#agent:bobross92",
                    "name": "bobross92",
                    "stringAttribute": "String",
                    "intAttribute": 1,
                    "boolAttribute": false
                  },
                  "cursor": "1"
                },
                {
                  "node": {
                    "__typename": "Friend",
                    "id": "http://blockchaintp.com/chronicle/ns#agent:bobross93",
                    "name": "bobross93",
                    "stringAttribute": "String",
                    "intAttribute": 1,
                    "boolAttribute": false
                  },
                  "cursor": "2"
                },
                {
                  "node": {
                    "__typename": "Friend",
                    "id": "http://blockchaintp.com/chronicle/ns#agent:bobross94",
                    "name": "bobross94",
                    "stringAttribute": "String",
                    "intAttribute": 1,
                    "boolAttribute": false
                  },
                  "cursor": "3"
                },
                {
                  "node": {
                    "__typename": "Friend",
                    "id": "http://blockchaintp.com/chronicle/ns#agent:bobross95",
                    "name": "bobross95",
                    "stringAttribute": "String",
                    "intAttribute": 1,
                    "boolAttribute": false
                  },
                  "cursor": "4"
                },
                {
                  "node": {
                    "__typename": "Friend",
                    "id": "http://blockchaintp.com/chronicle/ns#agent:bobross96",
                    "name": "bobross96",
                    "stringAttribute": "String",
                    "intAttribute": 1,
                    "boolAttribute": false
                  },
                  "cursor": "5"
                },
                {
                  "node": {
                    "__typename": "Friend",
                    "id": "http://blockchaintp.com/chronicle/ns#agent:bobross97",
                    "name": "bobross97",
                    "stringAttribute": "String",
                    "intAttribute": 1,
                    "boolAttribute": false
                  },
                  "cursor": "6"
                },
                {
                  "node": {
                    "__typename": "Friend",
                    "id": "http://blockchaintp.com/chronicle/ns#agent:bobross98",
                    "name": "bobross98",
                    "stringAttribute": "String",
                    "intAttribute": 1,
                    "boolAttribute": false
                  },
                  "cursor": "7"
                },
                {
                  "node": {
                    "__typename": "Friend",
                    "id": "http://blockchaintp.com/chronicle/ns#agent:bobross99",
                    "name": "bobross99",
                    "stringAttribute": "String",
                    "intAttribute": 1,
                    "boolAttribute": false
                  },
                  "cursor": "8"
                }
              ]
            }
          }
        }
        "###);
    }
}
