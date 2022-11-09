use chronicle::{
    api::chronicle_graphql::ChronicleGraphQl, bootstrap, codegen::ChronicleDomainDef, tokio,
};
use generated::{Mutation, Query};

#[allow(dead_code)]
mod generated;

///Entry point here is jigged a little, as we want to run unit tests, see chronicle-untyped for the actual pattern
#[tokio::main]
pub async fn main() {
    let s = r#"
    name: "airworthiness"
    attributes:
      CertId:
        type: "String"
      BatchId:
        type: "String"
      PartId:
        type: "String"
      Location:
        type: "String"
      Manifest:
        type: JSON
    agents:
      Contractor:
        attributes:
          - Location
      NCB:
        attributes:
          - Manifest
    entities:
      Certificate:
        attributes:
          - CertId
      Item:
        attributes:
          - PartId
      NCBRecord:
        attributes: []
    activities:
      ItemCertified:
        attributes:
          - CertId
      ItemCodified:
        attributes: []
      ItemManufactured:
        attributes:
          - BatchId
    roles:
      - CERTIFIER
      - CODIFIER
      - MANUFACTURER
      - SUBMITTER
     "#
    .to_string();

    let model = ChronicleDomainDef::from_input_string(&s).unwrap();

    bootstrap(model, ChronicleGraphQl::new(Query, Mutation)).await
}

#[cfg(test)]
mod test {
    use super::{Mutation, Query};
    use chronicle::{
        api::{
            chronicle_graphql::{Store, Subscription},
            Api, ConnectionOptions, UuidGen,
        },
        async_graphql::{Request, Schema},
        chrono::{DateTime, NaiveDate, Utc},
        common::ledger::InMemLedger,
        tokio,
        uuid::Uuid,
    };
    use diesel::{
        r2d2::{ConnectionManager, Pool},
        SqliteConnection,
    };
    use std::{collections::HashMap, time::Duration};
    use tempfile::TempDir;

    #[derive(Debug, Clone)]
    struct SameUuid;

    impl UuidGen for SameUuid {
        fn uuid() -> Uuid {
            Uuid::parse_str("5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea").unwrap()
        }
    }

    async fn test_schema() -> Schema<Query, Mutation, Subscription> {
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
            HashMap::default(),
        )
        .await
        .unwrap();

        Schema::build(Query, Mutation, Subscription)
            .data(Store::new(pool))
            .data(dispatch)
            .finish()
    }

    #[tokio::test]
    async fn generic_json_object_attribute() {
        let schema = test_schema().await;

        // We doctor the test JSON input data as done in the `async_graphql` library JSON tests
        // https://docs.rs/async-graphql/latest/src/async_graphql/types/json.rs.html#20.
        // In fact, if you make the JSON input more complex, nesting data further, etc, it will cause
        // "expected Name" errors. However, complex JSON inputs have been tested with success in the GraphQL
        // Playground, as documented here: https://blockchaintp.atlassian.net/l/cp/0aocArV4
        let res = schema
            .execute(Request::new(
                r#"
            mutation {
              defineNCBAgent(
                  externalId: "testagent2"
                  attributes: { manifestAttribute: {
                    username: "test",
                    email: "test@test.cz",
                    phone: "479332973",
                    firstName: "David",
                    lastName: "Test",
                  } }
                ) {
                  context
                }
              }
        "#,
            ))
            .await;

        assert_eq!(res.errors, vec![]);

        tokio::time::sleep(Duration::from_millis(1000)).await;

        insta::assert_json_snapshot!(schema
          .execute(Request::new(
            r#"
            query agent {
              agentById(id: "chronicle:agent:testagent2") {
                __typename
                ... on NCBAgent {
                  id
                  manifestAttribute
                }
              }
            }
            "#,
          ))
          .await, @r###"
        {
          "data": {
            "agentById": {
              "__typename": "NCBAgent",
              "id": "chronicle:agent:testagent2",
              "manifestAttribute": {
                "email": "test@test.cz",
                "firstName": "David",
                "lastName": "Test",
                "phone": "479332973",
                "username": "test"
              }
            }
          }
        }
        "###);
    }

    // Note that this test demonstrates Chronicle accepting
    // long-form and short-form iris as input
    #[tokio::test]
    async fn agent_delegation() {
        let schema = test_schema().await;

        insta::assert_toml_snapshot!(schema
          .execute(Request::new(
              r#"
          mutation {
              actedOnBehalfOf(
                  responsible: "chronicle:agent:testagent",
                  delegate: "http://blockchaintp.com/chronicle/ns#agent:testdelegate",
                  role: MANUFACTURER
                  ) {
                  context
              }
          }
      "#,
          ))
          .await, @r###"
        [data.actedOnBehalfOf]
        context = 'chronicle:agent:testagent'
        "###);

        tokio::time::sleep(Duration::from_millis(1000)).await;

        insta::assert_json_snapshot!(schema
          .execute(Request::new(
              r#"
          query {
              agentById(id: "chronicle:agent:testagent") {
                  ... on ProvAgent {
                      id
                      externalId
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
          .await.data, @r###"
        {
          "agentById": {
            "id": "chronicle:agent:testagent",
            "externalId": "testagent",
            "actedOnBehalfOf": [
              {
                "agent": {
                  "id": "chronicle:agent:testdelegate"
                },
                "role": "MANUFACTURER"
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
                wasDerivedFrom(generatedEntity: "chronicle:entity:testentity1",
                               usedEntity: "chronicle:entity:testentity2") {
                    context
                }
            }
        "#,
            ))
            .await;

        insta::assert_toml_snapshot!(created, @r###"
        [data.wasDerivedFrom]
        context = 'chronicle:entity:testentity1'
        "###);

        tokio::time::sleep(Duration::from_millis(100)).await;
        let derived = schema
            .execute(Request::new(
                r#"
            query {
                entityById(id: "chronicle:entity:testentity1") {
                    ... on ProvEntity {
                        id
                        externalId
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
        id = 'chronicle:entity:testentity1'
        externalId = 'testentity1'

        [[data.entityById.wasDerivedFrom]]
        id = 'chronicle:entity:testentity2'
        "###);
    }

    #[tokio::test]
    async fn primary_source() {
        let schema = test_schema().await;

        let created = schema
            .execute(Request::new(
                r#"
            mutation {
                hadPrimarySource(generatedEntity: "chronicle:entity:testentity1",
                               usedEntity: "chronicle:entity:testentity2") {
                    context
                }
            }
        "#,
            ))
            .await;

        insta::assert_toml_snapshot!(created, @r###"
        [data.hadPrimarySource]
        context = 'chronicle:entity:testentity1'
        "###);

        tokio::time::sleep(Duration::from_millis(100)).await;
        let derived = schema
            .execute(Request::new(
                r#"
            query {
                entityById(id: "chronicle:entity:testentity1") {

                    ... on ProvEntity {
                        id
                        externalId
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
        id = 'chronicle:entity:testentity1'
        externalId = 'testentity1'

        [[data.entityById.wasDerivedFrom]]
        id = 'chronicle:entity:testentity2'

        [[data.entityById.hadPrimarySource]]
        id = 'chronicle:entity:testentity2'
        "###);
    }

    #[tokio::test]
    async fn revision() {
        let schema = test_schema().await;

        let created = schema
            .execute(Request::new(
                r#"
            mutation {
                wasRevisionOf(generatedEntity: "chronicle:entity:testentity1",
                            usedEntity: "chronicle:entity:testentity2") {
                    context
                }
            }
        "#,
            ))
            .await;

        insta::assert_toml_snapshot!(created, @r###"
        [data.wasRevisionOf]
        context = 'chronicle:entity:testentity1'
        "###);

        tokio::time::sleep(Duration::from_millis(100)).await;
        let derived = schema
            .execute(Request::new(
                r#"
            query {
                entityById(id: "chronicle:entity:testentity1") {
                    ... on ProvEntity {
                        id
                        externalId
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
        id = 'chronicle:entity:testentity1'
        externalId = 'testentity1'

        [[data.entityById.wasDerivedFrom]]
        id = 'chronicle:entity:testentity2'

        [[data.entityById.wasRevisionOf]]
        id = 'chronicle:entity:testentity2'
        "###);
    }

    #[tokio::test]
    async fn quotation() {
        let schema = test_schema().await;

        let created = schema
            .execute(Request::new(
                r#"
            mutation {
                wasQuotedFrom(generatedEntity: "chronicle:entity:testentity1",
                            usedEntity: "chronicle:entity:testentity2") {
                    context
                }
            }
        "#,
            ))
            .await;

        insta::assert_toml_snapshot!(created, @r###"
        [data.wasQuotedFrom]
        context = 'chronicle:entity:testentity1'
        "###);

        tokio::time::sleep(Duration::from_millis(100)).await;
        let derived = schema
            .execute(Request::new(
                r#"
            query {
                entityById(id: "chronicle:entity:testentity1") {
                    ... on ProvEntity {
                        id
                        externalId
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
        id = 'chronicle:entity:testentity1'
        externalId = 'testentity1'

        [[data.entityById.wasDerivedFrom]]
        id = 'chronicle:entity:testentity2'

        [[data.entityById.wasQuotedFrom]]
        id = 'chronicle:entity:testentity2'
        "###);
    }

    #[tokio::test]
    async fn agent_can_be_created() {
        let schema = test_schema().await;

        let create = schema
            .execute(Request::new(
                r#"
            mutation {
                defineAgent(externalId:"testentity1", attributes: { type: "type" }) {
                    context
                }
            }
        "#,
            ))
            .await;

        insta::assert_toml_snapshot!(create, @r###"
        [data.defineAgent]
        context = 'chronicle:agent:testentity1'
        "###);
    }

    #[tokio::test]
    async fn was_informed_by() {
        let schema = test_schema().await;

        // create an activity
        insta::assert_toml_snapshot!(schema
          .execute(Request::new(
              r#"
                  mutation one {
                    defineItemCertifiedActivity(externalId:"testactivityid1", attributes: { certIdAttribute: "testcertid" }) {
                          context
                      }
                  }
              "#,
          ))
          .await, @r###"
        [data.defineItemCertifiedActivity]
        context = 'chronicle:activity:testactivityid1'
        "###);

        // create another activity
        insta::assert_toml_snapshot!(schema
          .execute(Request::new(
              r#"
          mutation two {
            defineItemManufacturedActivity(externalId:"testactivityid2", attributes: { batchIdAttribute: "testbatchid" }) {
                  context
              }
          }
      "#
              ),
          )
          .await, @r###"
        [data.defineItemManufacturedActivity]
        context = 'chronicle:activity:testactivityid2'
        "###);

        // establish WasInformedBy relationship
        insta::assert_toml_snapshot!(schema
          .execute(Request::new(
              r#"
          mutation exec {
              wasInformedBy(activity: "chronicle:activity:testactivityid1",
              informingActivity: "chronicle:activity:testactivityid2",)
              {
                  context
              }
          }
      "#,
          ))
          .await, @r###"
        [data.wasInformedBy]
        context = 'chronicle:activity:testactivityid1'
        "###);

        tokio::time::sleep(Duration::from_millis(100)).await;

        // query WasInformedBy relationship
        insta::assert_toml_snapshot!(schema
            .execute(Request::new(
                r#"
            query test {
                activityById(id: "chronicle:activity:testactivityid1") {
                    ... on ItemCertifiedActivity {
                        id
                        externalId
                        wasInformedBy {
                            ... on ItemManufacturedActivity {
                                batchIdAttribute
                                id
                                externalId
                            }
                        }
                    }
                }
            }
        "#,
            ))
            .await, @r###"
        [data.activityById]
        id = 'chronicle:activity:testactivityid1'
        externalId = 'testactivityid1'

        [[data.activityById.wasInformedBy]]
        batchIdAttribute = 'testbatchid'
        id = 'chronicle:activity:testactivityid2'
        externalId = 'testactivityid2'
        "###);

        tokio::time::sleep(Duration::from_millis(100)).await;

        // create a third activity
        insta::assert_toml_snapshot!(schema
          .execute(Request::new(
              r#"
                    mutation three {
                      defineItemCodifiedActivity(externalId:"testactivityid3") {
                            context
                        }
                    }
                "#,
          ))
          .await, @r###"
        [data.defineItemCodifiedActivity]
        context = 'chronicle:activity:testactivityid3'
        "###);

        tokio::time::sleep(Duration::from_millis(100)).await;

        // establish another WasInformedBy relationship
        insta::assert_toml_snapshot!(schema
          .execute(Request::new(
              r#"
          mutation execagain {
              wasInformedBy(activity: "chronicle:activity:testactivityid1",
              informingActivity: "chronicle:activity:testactivityid3",)
              {
                  context
              }
          }
      "#,
          ))
          .await, @r###"
        [data.wasInformedBy]
        context = 'chronicle:activity:testactivityid1'
        "###);

        tokio::time::sleep(Duration::from_millis(100)).await;

        // query WasInformedBy relationship
        insta::assert_toml_snapshot!(schema
            .execute(Request::new(
                r#"
                query test {
                  activityById(id: "chronicle:activity:testactivityid1") {
                      ... on ItemCertifiedActivity {
                          id
                          externalId
                          wasInformedBy {
                              ... on ItemManufacturedActivity {
                                  id
                                  externalId
                              }
                              ... on ItemCodifiedActivity {
                                  id
                                  externalId
                              }
                          }
                      }
                  }
              }
        "#,
            ))
            .await, @r###"
        [data.activityById]
        id = 'chronicle:activity:testactivityid1'
        externalId = 'testactivityid1'

        [[data.activityById.wasInformedBy]]
        id = 'chronicle:activity:testactivityid2'
        externalId = 'testactivityid2'

        [[data.activityById.wasInformedBy]]
        id = 'chronicle:activity:testactivityid3'
        externalId = 'testactivityid3'
        "###);
    }

    #[tokio::test]
    async fn generated() {
        let schema = test_schema().await;

        // create an activity
        insta::assert_toml_snapshot!(schema
          .execute(Request::new(
              r#"
                  mutation activity {
                    defineItemCertifiedActivity(externalId:"testactivity1", attributes: { certIdAttribute: "testcertid" }) {
                          context
                      }
                  }
              "#,
          ))
          .await, @r###"
        [data.defineItemCertifiedActivity]
        context = 'chronicle:activity:testactivity1'
        "###);

        // create an entity
        insta::assert_toml_snapshot!(schema
          .execute(Request::new(
              r#"
                  mutation entity {
                    defineNCBRecordEntity(externalId:"testentity1") {
                          context
                      }
                  }
              "#,
          ))
          .await, @r###"
        [data.defineNCBRecordEntity]
        context = 'chronicle:entity:testentity1'
        "###);

        // establish Generated relationship
        insta::assert_toml_snapshot!(schema
          .execute(Request::new(
              r#"
          mutation generated {
              wasGeneratedBy(activity: "chronicle:activity:testactivity1",
              id: "chronicle:entity:testentity1",)
              {
                  context
              }
          }
      "#,
          ))
          .await, @r###"
        [data.wasGeneratedBy]
        context = 'chronicle:entity:testentity1'
        "###);

        tokio::time::sleep(Duration::from_millis(100)).await;

        // query Generated relationship
        insta::assert_toml_snapshot!(schema
            .execute(Request::new(
                r#"
            query test {
                activityById(id: "chronicle:activity:testactivity1") {
                  ... on ItemCertifiedActivity {
                        id
                        externalId
                        certIdAttribute
                        generated {
                            ... on NCBRecordEntity {
                              id
                              externalId
                            }
                        }
                    }
                }
            }
        "#,
            ))
            .await, @r###"
        [data.activityById]
        id = 'chronicle:activity:testactivity1'
        externalId = 'testactivity1'
        certIdAttribute = 'testcertid'

        [[data.activityById.generated]]
        id = 'chronicle:entity:testentity1'
        externalId = 'testentity1'
        "###);

        // The following demonstrates that a second wasGeneratedBy
        // relationship cannot be made once the first has been established.

        // create another entity
        insta::assert_toml_snapshot!(schema
            .execute(Request::new(
                r#"
            mutation second {
              defineItemEntity(externalId:"testitem", attributes: { partIdAttribute: "testpartid" }) {
                    context
                }
            }
        "#
                ),
            )
            .await, @r###"
        [data.defineItemEntity]
        context = 'chronicle:entity:testitem'
        "###);

        // establish another Generated relationship
        insta::assert_toml_snapshot!(schema
            .execute(Request::new(
                r#"
            mutation again {
                wasGeneratedBy(id: "chronicle:entity:testitem",
                activity: "chronicle:activity:testactivityid1",)
                {
                    context
                }
            }
        "#,
            ))
            .await, @r###"
        [data.wasGeneratedBy]
        context = 'chronicle:entity:testitem'
        "###);

        // query Generated relationship
        insta::assert_toml_snapshot!(schema
          .execute(Request::new(
              r#"
              query testagain {
                activityById(id: "chronicle:activity:testactivity1") {
                  ... on ItemCertifiedActivity {
                        id
                        externalId
                        certIdAttribute
                        generated {
                            ... on ItemEntity {
                                id
                                externalId
                            }
                            ... on NCBRecordEntity {
                              id
                              externalId
                            }
                        }
                    }
                }
            }
    "#,
          ))
          .await, @r###"
        [data.activityById]
        id = 'chronicle:activity:testactivity1'
        externalId = 'testactivity1'
        certIdAttribute = 'testcertid'

        [[data.activityById.generated]]
        id = 'chronicle:entity:testentity1'
        externalId = 'testentity1'
        "###);
    }

    #[tokio::test]
    async fn query_activity_timeline() {
        let schema = test_schema().await;

        let res = schema
                .execute(Request::new(
                    r#"
            mutation {
                defineContractorAgent(externalId:"testagent1", attributes: { locationAttribute: "testlocation" }) {
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
              defineNCBAgent(
                  externalId: "testagent2"
                  attributes: { manifestAttribute: {
                    username: "test",
                    email: "test@test.cz",
                  } }
                ) {
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
              defineCertificateEntity(externalId:"testentity1", attributes: { certIdAttribute: "testcertid" }) {
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
              defineNCBRecordEntity(externalId:"testentity2") {
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
                format!("testactivity{}", i)
            } else {
                format!("anothertestactivity{}", i)
            };

            if (i % 2) == 0 {
                let res = schema
                    .execute(Request::new(
                        &format!(
                            r#"
                    mutation {{
                      defineItemCertifiedActivity(externalId:"{}", attributes: {{ certIdAttribute: "testcertid" }}) {{
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
                    .execute(Request::new(&format!(
                        r#"
                    mutation {{
                      defineItemCodifiedActivity(externalId:"{}") {{
                            context
                        }}
                    }}
                "#,
                        activity_name
                    )))
                    .await;

                assert_eq!(res.errors, vec![]);
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
            let res = schema
                .execute(Request::new(format!(
                    r#"
            mutation {{
                used(id: "chronicle:entity:testentity1", activity: "chronicle:activity:{}") {{
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
                      startActivity( time: "{}", id: "chronicle:activity:{}") {{
                          context
                      }}
                  }}
                "#,
                    from.checked_add_signed(chronicle::chrono::Duration::days(i))
                        .unwrap()
                        .to_rfc3339(),
                    activity_name
                )))
                .await;

            assert_eq!(res.errors, vec![]);

            let res = schema
                .execute(Request::new(format!(
                    r#"
            mutation {{
                endActivity( time: "{}", id: "chronicle:activity:{}") {{
                    context
                }}
            }}
        "#,
                    from.checked_add_signed(chronicle::chrono::Duration::days(i))
                        .unwrap()
                        .to_rfc3339(),
                    activity_name
                )))
                .await;

            assert_eq!(res.errors, vec![]);

            tokio::time::sleep(Duration::from_millis(100)).await;

            let agent = if i % 2 == 0 {
                "testagent1"
            } else {
                "testagent2"
            };

            let res = schema
                .execute(Request::new(format!(
                    r#"
            mutation {{
                wasAssociatedWith( role: CERTIFIER, responsible: "chronicle:agent:{}", activity: "chronicle:activity:{}") {{
                    context
                }}
            }}
        "#, agent, activity_name
                )))
                .await;

            tokio::time::sleep(Duration::from_millis(100)).await;

            assert_eq!(res.errors, vec![]);
        }

        tokio::time::sleep(Duration::from_millis(3000)).await;

        // Entire timeline in order
        insta::assert_json_snapshot!(schema
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
                          ... on ItemCertifiedActivity {
                            id
                            externalId
                            started
                            ended
                            wasAssociatedWith {
                                    responsible {
                                      agent {
                                        ... on ContractorAgent {
                                            id
                                            externalId
                                        }
                                        ... on NCBAgent {
                                          id
                                          externalId
                                          manifestAttribute
                                        }
                                    }
                                        role
                                    }
                            }
                            used {
                                ... on CertificateEntity {
                                  id
                                  externalId
                                }
                                ... on NCBRecordEntity {
                                  id
                                  externalId
                                }
                            }
                        }
                          ... on ItemCodifiedActivity {
                            id
                            externalId
                            started
                            ended
                            wasAssociatedWith {
                                    responsible {
                                        agent {
                                            ... on ContractorAgent {
                                                id
                                                externalId
                                            }
                                            ... on NCBAgent {
                                              id
                                              externalId
                                              manifestAttribute
                                            }
                                        }
                                        role
                                    }
                            }
                            used {
                                ... on CertificateEntity {
                                  id
                                  externalId
                                }
                                ... on NCBRecordEntity {
                                  id
                                  externalId
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
          .await, @r###"
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
                    "__typename": "ItemCodifiedActivity",
                    "id": "chronicle:activity:anothertestactivity1",
                    "externalId": "anothertestactivity1",
                    "started": "1968-09-02T00:00:00+00:00",
                    "ended": "1968-09-02T00:00:00+00:00",
                    "wasAssociatedWith": [
                      {
                        "responsible": {
                          "agent": {
                            "id": "chronicle:agent:testagent2",
                            "externalId": "testagent2",
                            "manifestAttribute": {
                              "email": "test@test.cz",
                              "username": "test"
                            }
                          },
                          "role": "CERTIFIER"
                        }
                      }
                    ],
                    "used": [
                      {
                        "id": "chronicle:entity:testentity1",
                        "externalId": "testentity1"
                      }
                    ]
                  },
                  "cursor": "0"
                },
                {
                  "node": {
                    "__typename": "ItemCertifiedActivity",
                    "id": "chronicle:activity:testactivity2",
                    "externalId": "testactivity2",
                    "started": "1968-09-03T00:00:00+00:00",
                    "ended": "1968-09-03T00:00:00+00:00",
                    "wasAssociatedWith": [
                      {
                        "responsible": {
                          "agent": {
                            "id": "chronicle:agent:testagent1",
                            "externalId": "testagent1"
                          },
                          "role": "CERTIFIER"
                        }
                      }
                    ],
                    "used": [
                      {
                        "id": "chronicle:entity:testentity1",
                        "externalId": "testentity1"
                      }
                    ]
                  },
                  "cursor": "1"
                },
                {
                  "node": {
                    "__typename": "ItemCodifiedActivity",
                    "id": "chronicle:activity:anothertestactivity3",
                    "externalId": "anothertestactivity3",
                    "started": "1968-09-04T00:00:00+00:00",
                    "ended": "1968-09-04T00:00:00+00:00",
                    "wasAssociatedWith": [
                      {
                        "responsible": {
                          "agent": {
                            "id": "chronicle:agent:testagent2",
                            "externalId": "testagent2",
                            "manifestAttribute": {
                              "email": "test@test.cz",
                              "username": "test"
                            }
                          },
                          "role": "CERTIFIER"
                        }
                      }
                    ],
                    "used": [
                      {
                        "id": "chronicle:entity:testentity1",
                        "externalId": "testentity1"
                      }
                    ]
                  },
                  "cursor": "2"
                },
                {
                  "node": {
                    "__typename": "ItemCertifiedActivity",
                    "id": "chronicle:activity:testactivity4",
                    "externalId": "testactivity4",
                    "started": "1968-09-05T00:00:00+00:00",
                    "ended": "1968-09-05T00:00:00+00:00",
                    "wasAssociatedWith": [
                      {
                        "responsible": {
                          "agent": {
                            "id": "chronicle:agent:testagent1",
                            "externalId": "testagent1"
                          },
                          "role": "CERTIFIER"
                        }
                      }
                    ],
                    "used": [
                      {
                        "id": "chronicle:entity:testentity1",
                        "externalId": "testentity1"
                      }
                    ]
                  },
                  "cursor": "3"
                },
                {
                  "node": {
                    "__typename": "ItemCodifiedActivity",
                    "id": "chronicle:activity:anothertestactivity5",
                    "externalId": "anothertestactivity5",
                    "started": "1968-09-06T00:00:00+00:00",
                    "ended": "1968-09-06T00:00:00+00:00",
                    "wasAssociatedWith": [
                      {
                        "responsible": {
                          "agent": {
                            "id": "chronicle:agent:testagent2",
                            "externalId": "testagent2",
                            "manifestAttribute": {
                              "email": "test@test.cz",
                              "username": "test"
                            }
                          },
                          "role": "CERTIFIER"
                        }
                      }
                    ],
                    "used": [
                      {
                        "id": "chronicle:entity:testentity1",
                        "externalId": "testentity1"
                      }
                    ]
                  },
                  "cursor": "4"
                },
                {
                  "node": {
                    "__typename": "ItemCertifiedActivity",
                    "id": "chronicle:activity:testactivity6",
                    "externalId": "testactivity6",
                    "started": "1968-09-07T00:00:00+00:00",
                    "ended": "1968-09-07T00:00:00+00:00",
                    "wasAssociatedWith": [
                      {
                        "responsible": {
                          "agent": {
                            "id": "chronicle:agent:testagent1",
                            "externalId": "testagent1"
                          },
                          "role": "CERTIFIER"
                        }
                      }
                    ],
                    "used": [
                      {
                        "id": "chronicle:entity:testentity1",
                        "externalId": "testentity1"
                      }
                    ]
                  },
                  "cursor": "5"
                },
                {
                  "node": {
                    "__typename": "ItemCodifiedActivity",
                    "id": "chronicle:activity:anothertestactivity7",
                    "externalId": "anothertestactivity7",
                    "started": "1968-09-08T00:00:00+00:00",
                    "ended": "1968-09-08T00:00:00+00:00",
                    "wasAssociatedWith": [
                      {
                        "responsible": {
                          "agent": {
                            "id": "chronicle:agent:testagent2",
                            "externalId": "testagent2",
                            "manifestAttribute": {
                              "email": "test@test.cz",
                              "username": "test"
                            }
                          },
                          "role": "CERTIFIER"
                        }
                      }
                    ],
                    "used": [
                      {
                        "id": "chronicle:entity:testentity1",
                        "externalId": "testentity1"
                      }
                    ]
                  },
                  "cursor": "6"
                },
                {
                  "node": {
                    "__typename": "ItemCertifiedActivity",
                    "id": "chronicle:activity:testactivity8",
                    "externalId": "testactivity8",
                    "started": "1968-09-09T00:00:00+00:00",
                    "ended": "1968-09-09T00:00:00+00:00",
                    "wasAssociatedWith": [
                      {
                        "responsible": {
                          "agent": {
                            "id": "chronicle:agent:testagent1",
                            "externalId": "testagent1"
                          },
                          "role": "CERTIFIER"
                        }
                      }
                    ],
                    "used": [
                      {
                        "id": "chronicle:entity:testentity1",
                        "externalId": "testentity1"
                      }
                    ]
                  },
                  "cursor": "7"
                },
                {
                  "node": {
                    "__typename": "ItemCodifiedActivity",
                    "id": "chronicle:activity:anothertestactivity9",
                    "externalId": "anothertestactivity9",
                    "started": "1968-09-10T00:00:00+00:00",
                    "ended": "1968-09-10T00:00:00+00:00",
                    "wasAssociatedWith": [
                      {
                        "responsible": {
                          "agent": {
                            "id": "chronicle:agent:testagent2",
                            "externalId": "testagent2",
                            "manifestAttribute": {
                              "email": "test@test.cz",
                              "username": "test"
                            }
                          },
                          "role": "CERTIFIER"
                        }
                      }
                    ],
                    "used": [
                      {
                        "id": "chronicle:entity:testentity1",
                        "externalId": "testentity1"
                      }
                    ]
                  },
                  "cursor": "8"
                }
              ]
            }
          }
        }
        "###);

        // Entire timeline reverse order
        insta::assert_json_snapshot!(schema
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
                          ... on ItemCertifiedActivity {
                            id
                            externalId
                            started
                            ended
                            wasAssociatedWith {
                                    responsible {
                                        agent {
                                            ... on ContractorAgent {
                                                id
                                                externalId
                                            }
                                            ... on NCBAgent {
                                              id
                                              externalId
                                              manifestAttribute
                                            }
                                        }
                                        role
                                    }
                            }
                            used {
                                ... on CertificateEntity {
                                  id
                                  externalId
                                }
                                ... on NCBRecordEntity {
                                  id
                                  externalId
                                }
                            }
                        }
                        ... on ItemCodifiedActivity {
                          id
                          externalId
                          started
                          ended
                          wasAssociatedWith {
                                  responsible {
                                      agent {
                                          ... on ContractorAgent {
                                                id
                                                externalId
                                            }
                                          ... on NCBAgent {
                                              id
                                              externalId
                                              manifestAttribute
                                            }
                                      }
                                      role
                                  }
                          }
                          used {
                          ... on CertificateEntity {
                            id
                            externalId
                          }
                          ... on NCBRecordEntity {
                            id
                            externalId
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
          .await, @r###"
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
                    "__typename": "ItemCodifiedActivity",
                    "id": "chronicle:activity:anothertestactivity9",
                    "externalId": "anothertestactivity9",
                    "started": "1968-09-10T00:00:00+00:00",
                    "ended": "1968-09-10T00:00:00+00:00",
                    "wasAssociatedWith": [
                      {
                        "responsible": {
                          "agent": {
                            "id": "chronicle:agent:testagent2",
                            "externalId": "testagent2",
                            "manifestAttribute": {
                              "email": "test@test.cz",
                              "username": "test"
                            }
                          },
                          "role": "CERTIFIER"
                        }
                      }
                    ],
                    "used": [
                      {
                        "id": "chronicle:entity:testentity1",
                        "externalId": "testentity1"
                      }
                    ]
                  },
                  "cursor": "0"
                },
                {
                  "node": {
                    "__typename": "ItemCertifiedActivity",
                    "id": "chronicle:activity:testactivity8",
                    "externalId": "testactivity8",
                    "started": "1968-09-09T00:00:00+00:00",
                    "ended": "1968-09-09T00:00:00+00:00",
                    "wasAssociatedWith": [
                      {
                        "responsible": {
                          "agent": {
                            "id": "chronicle:agent:testagent1",
                            "externalId": "testagent1"
                          },
                          "role": "CERTIFIER"
                        }
                      }
                    ],
                    "used": [
                      {
                        "id": "chronicle:entity:testentity1",
                        "externalId": "testentity1"
                      }
                    ]
                  },
                  "cursor": "1"
                },
                {
                  "node": {
                    "__typename": "ItemCodifiedActivity",
                    "id": "chronicle:activity:anothertestactivity7",
                    "externalId": "anothertestactivity7",
                    "started": "1968-09-08T00:00:00+00:00",
                    "ended": "1968-09-08T00:00:00+00:00",
                    "wasAssociatedWith": [
                      {
                        "responsible": {
                          "agent": {
                            "id": "chronicle:agent:testagent2",
                            "externalId": "testagent2",
                            "manifestAttribute": {
                              "email": "test@test.cz",
                              "username": "test"
                            }
                          },
                          "role": "CERTIFIER"
                        }
                      }
                    ],
                    "used": [
                      {
                        "id": "chronicle:entity:testentity1",
                        "externalId": "testentity1"
                      }
                    ]
                  },
                  "cursor": "2"
                },
                {
                  "node": {
                    "__typename": "ItemCertifiedActivity",
                    "id": "chronicle:activity:testactivity6",
                    "externalId": "testactivity6",
                    "started": "1968-09-07T00:00:00+00:00",
                    "ended": "1968-09-07T00:00:00+00:00",
                    "wasAssociatedWith": [
                      {
                        "responsible": {
                          "agent": {
                            "id": "chronicle:agent:testagent1",
                            "externalId": "testagent1"
                          },
                          "role": "CERTIFIER"
                        }
                      }
                    ],
                    "used": [
                      {
                        "id": "chronicle:entity:testentity1",
                        "externalId": "testentity1"
                      }
                    ]
                  },
                  "cursor": "3"
                },
                {
                  "node": {
                    "__typename": "ItemCodifiedActivity",
                    "id": "chronicle:activity:anothertestactivity5",
                    "externalId": "anothertestactivity5",
                    "started": "1968-09-06T00:00:00+00:00",
                    "ended": "1968-09-06T00:00:00+00:00",
                    "wasAssociatedWith": [
                      {
                        "responsible": {
                          "agent": {
                            "id": "chronicle:agent:testagent2",
                            "externalId": "testagent2",
                            "manifestAttribute": {
                              "email": "test@test.cz",
                              "username": "test"
                            }
                          },
                          "role": "CERTIFIER"
                        }
                      }
                    ],
                    "used": [
                      {
                        "id": "chronicle:entity:testentity1",
                        "externalId": "testentity1"
                      }
                    ]
                  },
                  "cursor": "4"
                },
                {
                  "node": {
                    "__typename": "ItemCertifiedActivity",
                    "id": "chronicle:activity:testactivity4",
                    "externalId": "testactivity4",
                    "started": "1968-09-05T00:00:00+00:00",
                    "ended": "1968-09-05T00:00:00+00:00",
                    "wasAssociatedWith": [
                      {
                        "responsible": {
                          "agent": {
                            "id": "chronicle:agent:testagent1",
                            "externalId": "testagent1"
                          },
                          "role": "CERTIFIER"
                        }
                      }
                    ],
                    "used": [
                      {
                        "id": "chronicle:entity:testentity1",
                        "externalId": "testentity1"
                      }
                    ]
                  },
                  "cursor": "5"
                },
                {
                  "node": {
                    "__typename": "ItemCodifiedActivity",
                    "id": "chronicle:activity:anothertestactivity3",
                    "externalId": "anothertestactivity3",
                    "started": "1968-09-04T00:00:00+00:00",
                    "ended": "1968-09-04T00:00:00+00:00",
                    "wasAssociatedWith": [
                      {
                        "responsible": {
                          "agent": {
                            "id": "chronicle:agent:testagent2",
                            "externalId": "testagent2",
                            "manifestAttribute": {
                              "email": "test@test.cz",
                              "username": "test"
                            }
                          },
                          "role": "CERTIFIER"
                        }
                      }
                    ],
                    "used": [
                      {
                        "id": "chronicle:entity:testentity1",
                        "externalId": "testentity1"
                      }
                    ]
                  },
                  "cursor": "6"
                },
                {
                  "node": {
                    "__typename": "ItemCertifiedActivity",
                    "id": "chronicle:activity:testactivity2",
                    "externalId": "testactivity2",
                    "started": "1968-09-03T00:00:00+00:00",
                    "ended": "1968-09-03T00:00:00+00:00",
                    "wasAssociatedWith": [
                      {
                        "responsible": {
                          "agent": {
                            "id": "chronicle:agent:testagent1",
                            "externalId": "testagent1"
                          },
                          "role": "CERTIFIER"
                        }
                      }
                    ],
                    "used": [
                      {
                        "id": "chronicle:entity:testentity1",
                        "externalId": "testentity1"
                      }
                    ]
                  },
                  "cursor": "7"
                },
                {
                  "node": {
                    "__typename": "ItemCodifiedActivity",
                    "id": "chronicle:activity:anothertestactivity1",
                    "externalId": "anothertestactivity1",
                    "started": "1968-09-02T00:00:00+00:00",
                    "ended": "1968-09-02T00:00:00+00:00",
                    "wasAssociatedWith": [
                      {
                        "responsible": {
                          "agent": {
                            "id": "chronicle:agent:testagent2",
                            "externalId": "testagent2",
                            "manifestAttribute": {
                              "email": "test@test.cz",
                              "username": "test"
                            }
                          },
                          "role": "CERTIFIER"
                        }
                      }
                    ],
                    "used": [
                      {
                        "id": "chronicle:entity:testentity1",
                        "externalId": "testentity1"
                      }
                    ]
                  },
                  "cursor": "8"
                }
              ]
            }
          }
        }
        "###);

        // By activity type

        // Note the case of `ItemCertified` and `ItemCodified` in the `activityTypes`
        // field of the query here, as it is not standard GraphQL but is tailored to
        // meet client requirements of preserving domain case inflections.
        insta::assert_json_snapshot!(schema
          .execute(Request::new(
              r#"
              query {
              activityTimeline(
                forEntity: [],
                forAgent: [],
                order: NEWEST_FIRST,
                activityTypes: [ItemCertifiedActivity, ItemCodifiedActivity],
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
          .await, @r###"
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
                    "__typename": "ItemCodifiedActivity"
                  },
                  "cursor": "0"
                },
                {
                  "node": {
                    "__typename": "ItemCertifiedActivity"
                  },
                  "cursor": "1"
                },
                {
                  "node": {
                    "__typename": "ItemCodifiedActivity"
                  },
                  "cursor": "2"
                },
                {
                  "node": {
                    "__typename": "ItemCertifiedActivity"
                  },
                  "cursor": "3"
                },
                {
                  "node": {
                    "__typename": "ItemCodifiedActivity"
                  },
                  "cursor": "4"
                },
                {
                  "node": {
                    "__typename": "ItemCertifiedActivity"
                  },
                  "cursor": "5"
                },
                {
                  "node": {
                    "__typename": "ItemCodifiedActivity"
                  },
                  "cursor": "6"
                },
                {
                  "node": {
                    "__typename": "ItemCertifiedActivity"
                  },
                  "cursor": "7"
                },
                {
                  "node": {
                    "__typename": "ItemCodifiedActivity"
                  },
                  "cursor": "8"
                }
              ]
            }
          }
        }
        "###);

        // By agent
        insta::assert_json_snapshot!(schema
          .execute(Request::new(
              r#"
              query {
              activityTimeline(
                forEntity: [],
                forAgent: ["chronicle:agent:testagent2"],
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
          .await, @r###"
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
                    "__typename": "ItemCodifiedActivity"
                  },
                  "cursor": "0"
                },
                {
                  "node": {
                    "__typename": "ItemCodifiedActivity"
                  },
                  "cursor": "1"
                },
                {
                  "node": {
                    "__typename": "ItemCodifiedActivity"
                  },
                  "cursor": "2"
                },
                {
                  "node": {
                    "__typename": "ItemCodifiedActivity"
                  },
                  "cursor": "3"
                },
                {
                  "node": {
                    "__typename": "ItemCodifiedActivity"
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
                defineContractorAgent(externalId:"testagent{}", attributes: {{ locationAttribute: "testattribute" }}) {{
                    context
                }}
            }}
        "#,
                    i
                )))
                .await;

            assert_eq!(res.errors, vec![]);
        }

        // Default cursor

        // Note the case of `Contractor` in the `agentsByType(agentType:` field of
        // the query here, as it is not standard GraphQL but is tailored to meet
        // client requirements of preserving domain case inflections.
        insta::assert_json_snapshot!(schema
          .execute(Request::new(
              r#"
              query {
              agentsByType(agentType: ContractorAgent) {
                  pageInfo {
                      hasPreviousPage
                      hasNextPage
                      startCursor
                      endCursor
                  }
                  edges {
                      node {
                          __typename
                          ... on ContractorAgent {
                              id
                              externalId
                              locationAttribute
                          }
                     }
                      cursor
                  }
              }
              }
      "#,
          ))
          .await, @r###"
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
                    "__typename": "ContractorAgent",
                    "id": "chronicle:agent:testagent0",
                    "externalId": "testagent0",
                    "locationAttribute": "testattribute"
                  },
                  "cursor": "0"
                },
                {
                  "node": {
                    "__typename": "ContractorAgent",
                    "id": "chronicle:agent:testagent1",
                    "externalId": "testagent1",
                    "locationAttribute": "testattribute"
                  },
                  "cursor": "1"
                },
                {
                  "node": {
                    "__typename": "ContractorAgent",
                    "id": "chronicle:agent:testagent10",
                    "externalId": "testagent10",
                    "locationAttribute": "testattribute"
                  },
                  "cursor": "2"
                },
                {
                  "node": {
                    "__typename": "ContractorAgent",
                    "id": "chronicle:agent:testagent11",
                    "externalId": "testagent11",
                    "locationAttribute": "testattribute"
                  },
                  "cursor": "3"
                },
                {
                  "node": {
                    "__typename": "ContractorAgent",
                    "id": "chronicle:agent:testagent12",
                    "externalId": "testagent12",
                    "locationAttribute": "testattribute"
                  },
                  "cursor": "4"
                },
                {
                  "node": {
                    "__typename": "ContractorAgent",
                    "id": "chronicle:agent:testagent13",
                    "externalId": "testagent13",
                    "locationAttribute": "testattribute"
                  },
                  "cursor": "5"
                },
                {
                  "node": {
                    "__typename": "ContractorAgent",
                    "id": "chronicle:agent:testagent14",
                    "externalId": "testagent14",
                    "locationAttribute": "testattribute"
                  },
                  "cursor": "6"
                },
                {
                  "node": {
                    "__typename": "ContractorAgent",
                    "id": "chronicle:agent:testagent15",
                    "externalId": "testagent15",
                    "locationAttribute": "testattribute"
                  },
                  "cursor": "7"
                },
                {
                  "node": {
                    "__typename": "ContractorAgent",
                    "id": "chronicle:agent:testagent16",
                    "externalId": "testagent16",
                    "locationAttribute": "testattribute"
                  },
                  "cursor": "8"
                },
                {
                  "node": {
                    "__typename": "ContractorAgent",
                    "id": "chronicle:agent:testagent17",
                    "externalId": "testagent17",
                    "locationAttribute": "testattribute"
                  },
                  "cursor": "9"
                }
              ]
            }
          }
        }
        "###);

        // Middle cursor

        // Note the case of `Contractor` in the `agentsByType(agentType:` field of
        // the query here, as it is not standard GraphQL but is tailored to meet
        // client requirements of preserving domain case inflections.
        insta::assert_json_snapshot!(schema
          .execute(Request::new(
              r#"
              query {
              agentsByType(agentType: ContractorAgent, first: 20, after: "3") {
                  pageInfo {
                      hasPreviousPage
                      hasNextPage
                      startCursor
                      endCursor
                  }
                  edges {
                      node {
                          __typename
                          ... on ContractorAgent {
                              id
                              externalId
                              locationAttribute
                          }
                      }
                      cursor
                  }
              }
              }
      "#,
          ))
          .await, @r###"
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
                    "__typename": "ContractorAgent",
                    "id": "chronicle:agent:testagent12",
                    "externalId": "testagent12",
                    "locationAttribute": "testattribute"
                  },
                  "cursor": "0"
                },
                {
                  "node": {
                    "__typename": "ContractorAgent",
                    "id": "chronicle:agent:testagent13",
                    "externalId": "testagent13",
                    "locationAttribute": "testattribute"
                  },
                  "cursor": "1"
                },
                {
                  "node": {
                    "__typename": "ContractorAgent",
                    "id": "chronicle:agent:testagent14",
                    "externalId": "testagent14",
                    "locationAttribute": "testattribute"
                  },
                  "cursor": "2"
                },
                {
                  "node": {
                    "__typename": "ContractorAgent",
                    "id": "chronicle:agent:testagent15",
                    "externalId": "testagent15",
                    "locationAttribute": "testattribute"
                  },
                  "cursor": "3"
                },
                {
                  "node": {
                    "__typename": "ContractorAgent",
                    "id": "chronicle:agent:testagent16",
                    "externalId": "testagent16",
                    "locationAttribute": "testattribute"
                  },
                  "cursor": "4"
                },
                {
                  "node": {
                    "__typename": "ContractorAgent",
                    "id": "chronicle:agent:testagent17",
                    "externalId": "testagent17",
                    "locationAttribute": "testattribute"
                  },
                  "cursor": "5"
                },
                {
                  "node": {
                    "__typename": "ContractorAgent",
                    "id": "chronicle:agent:testagent18",
                    "externalId": "testagent18",
                    "locationAttribute": "testattribute"
                  },
                  "cursor": "6"
                },
                {
                  "node": {
                    "__typename": "ContractorAgent",
                    "id": "chronicle:agent:testagent19",
                    "externalId": "testagent19",
                    "locationAttribute": "testattribute"
                  },
                  "cursor": "7"
                },
                {
                  "node": {
                    "__typename": "ContractorAgent",
                    "id": "chronicle:agent:testagent2",
                    "externalId": "testagent2",
                    "locationAttribute": "testattribute"
                  },
                  "cursor": "8"
                },
                {
                  "node": {
                    "__typename": "ContractorAgent",
                    "id": "chronicle:agent:testagent20",
                    "externalId": "testagent20",
                    "locationAttribute": "testattribute"
                  },
                  "cursor": "9"
                },
                {
                  "node": {
                    "__typename": "ContractorAgent",
                    "id": "chronicle:agent:testagent21",
                    "externalId": "testagent21",
                    "locationAttribute": "testattribute"
                  },
                  "cursor": "10"
                },
                {
                  "node": {
                    "__typename": "ContractorAgent",
                    "id": "chronicle:agent:testagent22",
                    "externalId": "testagent22",
                    "locationAttribute": "testattribute"
                  },
                  "cursor": "11"
                },
                {
                  "node": {
                    "__typename": "ContractorAgent",
                    "id": "chronicle:agent:testagent23",
                    "externalId": "testagent23",
                    "locationAttribute": "testattribute"
                  },
                  "cursor": "12"
                },
                {
                  "node": {
                    "__typename": "ContractorAgent",
                    "id": "chronicle:agent:testagent24",
                    "externalId": "testagent24",
                    "locationAttribute": "testattribute"
                  },
                  "cursor": "13"
                },
                {
                  "node": {
                    "__typename": "ContractorAgent",
                    "id": "chronicle:agent:testagent25",
                    "externalId": "testagent25",
                    "locationAttribute": "testattribute"
                  },
                  "cursor": "14"
                },
                {
                  "node": {
                    "__typename": "ContractorAgent",
                    "id": "chronicle:agent:testagent26",
                    "externalId": "testagent26",
                    "locationAttribute": "testattribute"
                  },
                  "cursor": "15"
                },
                {
                  "node": {
                    "__typename": "ContractorAgent",
                    "id": "chronicle:agent:testagent27",
                    "externalId": "testagent27",
                    "locationAttribute": "testattribute"
                  },
                  "cursor": "16"
                },
                {
                  "node": {
                    "__typename": "ContractorAgent",
                    "id": "chronicle:agent:testagent28",
                    "externalId": "testagent28",
                    "locationAttribute": "testattribute"
                  },
                  "cursor": "17"
                },
                {
                  "node": {
                    "__typename": "ContractorAgent",
                    "id": "chronicle:agent:testagent29",
                    "externalId": "testagent29",
                    "locationAttribute": "testattribute"
                  },
                  "cursor": "18"
                },
                {
                  "node": {
                    "__typename": "ContractorAgent",
                    "id": "chronicle:agent:testagent3",
                    "externalId": "testagent3",
                    "locationAttribute": "testattribute"
                  },
                  "cursor": "19"
                }
              ]
            }
          }
        }
        "###);

        // Out of bound cursor

        // Note the case of `Contractor` in the `agentsByType(agentType:` field of
        // the query here, as it is not standard GraphQL but is tailored to meet
        // client requirements of preserving domain case inflections.
        insta::assert_json_snapshot!(schema
          .execute(Request::new(
              r#"
              query {
              agentsByType(agentType: ContractorAgent, first: 20, after: "90") {
                  pageInfo {
                      hasPreviousPage
                      hasNextPage
                      startCursor
                      endCursor
                  }
                  edges {
                      node {
                          __typename
                          ... on ContractorAgent {
                              id
                              externalId
                              locationAttribute
                          }
                      }
                      cursor
                  }
              }
              }
      "#,
          ))
          .await, @r###"
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
                    "__typename": "ContractorAgent",
                    "id": "chronicle:agent:testagent91",
                    "externalId": "testagent91",
                    "locationAttribute": "testattribute"
                  },
                  "cursor": "0"
                },
                {
                  "node": {
                    "__typename": "ContractorAgent",
                    "id": "chronicle:agent:testagent92",
                    "externalId": "testagent92",
                    "locationAttribute": "testattribute"
                  },
                  "cursor": "1"
                },
                {
                  "node": {
                    "__typename": "ContractorAgent",
                    "id": "chronicle:agent:testagent93",
                    "externalId": "testagent93",
                    "locationAttribute": "testattribute"
                  },
                  "cursor": "2"
                },
                {
                  "node": {
                    "__typename": "ContractorAgent",
                    "id": "chronicle:agent:testagent94",
                    "externalId": "testagent94",
                    "locationAttribute": "testattribute"
                  },
                  "cursor": "3"
                },
                {
                  "node": {
                    "__typename": "ContractorAgent",
                    "id": "chronicle:agent:testagent95",
                    "externalId": "testagent95",
                    "locationAttribute": "testattribute"
                  },
                  "cursor": "4"
                },
                {
                  "node": {
                    "__typename": "ContractorAgent",
                    "id": "chronicle:agent:testagent96",
                    "externalId": "testagent96",
                    "locationAttribute": "testattribute"
                  },
                  "cursor": "5"
                },
                {
                  "node": {
                    "__typename": "ContractorAgent",
                    "id": "chronicle:agent:testagent97",
                    "externalId": "testagent97",
                    "locationAttribute": "testattribute"
                  },
                  "cursor": "6"
                },
                {
                  "node": {
                    "__typename": "ContractorAgent",
                    "id": "chronicle:agent:testagent98",
                    "externalId": "testagent98",
                    "locationAttribute": "testattribute"
                  },
                  "cursor": "7"
                },
                {
                  "node": {
                    "__typename": "ContractorAgent",
                    "id": "chronicle:agent:testagent99",
                    "externalId": "testagent99",
                    "locationAttribute": "testattribute"
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
