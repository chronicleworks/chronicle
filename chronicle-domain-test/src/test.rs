use chronicle::{
    api::chronicle_graphql::ChronicleGraphQl, bootstrap, codegen::ChronicleDomainDef, tokio,
};
use main::{Mutation, Query};

#[allow(dead_code)]
mod main;

///Entry point here is jigged a little, as we want to run unit tests, see chronicle-untyped for the actual pattern
#[tokio::main]
pub async fn main() {
    let s = r#"
    name: "chronicle"
    attributes:
      NYU:
        type: "String"
      UCL:
        type: "Int"
      LSE:
        type: "Bool"
    agents:
      DOE:
        attributes:
          - NYU
          - UCL
          - LSE
    entities:
      NEH:
        attributes:
          - NYU
          - UCL
          - LSE
      NIH:
        attributes:
          - NYU
          - UCL
          - LSE
    activities:
      RND:
        attributes:
          - NYU
          - UCL
          - LSE
      RNR:
        attributes:
          - NYU
          - UCL
          - LSE
    roles:
        - VIP
        - SMH
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
    async fn agent_delegation() {
        let schema = test_schema().await;

        let created = schema
            .execute(Request::new(
                r#"
            mutation {
                actedOnBehalfOf(
                    responsible: "chronicle:agent:DOE",
                    delegate: "chronicle:agent:VIP",
                    role: VIP
                    ) {
                    context
                }
            }
        "#,
            ))
            .await;

        tokio::time::sleep(Duration::from_millis(1000)).await;

        insta::assert_toml_snapshot!(created, @r###"
        [data.actedOnBehalfOf]
        context = 'chronicle:agent:DOE'
        "###);

        tokio::time::sleep(Duration::from_millis(1000)).await;

        let derived = schema
            .execute(Request::new(
                r#"
            query {
                agentById(id: "chronicle:agent:DOE") {
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
            .await;
        insta::assert_json_snapshot!(derived.data, @r###"
        {
          "agentById": {
            "id": "chronicle:agent:DOE",
            "externalId": "DOE",
            "actedOnBehalfOf": [
              {
                "agent": {
                  "id": "chronicle:agent:VIP"
                },
                "role": "VIP"
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
                wasDerivedFrom(generatedEntity: "chronicle:entity:NEH",
                               usedEntity: "chronicle:entity:NIH") {
                    context
                }
            }
        "#,
            ))
            .await;

        insta::assert_toml_snapshot!(created, @r###"
        [data.wasDerivedFrom]
        context = 'chronicle:entity:NEH'
        "###);

        tokio::time::sleep(Duration::from_millis(100)).await;
        let derived = schema
            .execute(Request::new(
                r#"
            query {
                entityById(id: "chronicle:entity:NEH") {
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
        id = 'chronicle:entity:NEH'
        externalId = 'NEH'

        [[data.entityById.wasDerivedFrom]]
        id = 'chronicle:entity:NIH'
        "###);
    }

    #[tokio::test]
    async fn primary_source() {
        let schema = test_schema().await;

        let created = schema
            .execute(Request::new(
                r#"
            mutation {
                hadPrimarySource(generatedEntity: "chronicle:entity:NEH",
                               usedEntity: "chronicle:entity:NIH") {
                    context
                }
            }
        "#,
            ))
            .await;

        insta::assert_toml_snapshot!(created, @r###"
        [data.hadPrimarySource]
        context = 'chronicle:entity:NEH'
        "###);

        tokio::time::sleep(Duration::from_millis(100)).await;
        let derived = schema
            .execute(Request::new(
                r#"
            query {
                entityById(id: "chronicle:entity:NEH") {

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
        id = 'chronicle:entity:NEH'
        externalId = 'NEH'

        [[data.entityById.wasDerivedFrom]]
        id = 'chronicle:entity:NIH'

        [[data.entityById.hadPrimarySource]]
        id = 'chronicle:entity:NIH'
        "###);
    }

    #[tokio::test]
    async fn revision() {
        let schema = test_schema().await;

        let created = schema
            .execute(Request::new(
                r#"
            mutation {
                wasRevisionOf(generatedEntity: "chronicle:entity:NEH",
                            usedEntity: "chronicle:entity:NIH") {
                    context
                }
            }
        "#,
            ))
            .await;

        insta::assert_toml_snapshot!(created, @r###"
        [data.wasRevisionOf]
        context = 'chronicle:entity:NEH'
        "###);

        tokio::time::sleep(Duration::from_millis(100)).await;
        let derived = schema
            .execute(Request::new(
                r#"
            query {
                entityById(id: "chronicle:entity:NEH") {
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
        id = 'chronicle:entity:NEH'
        externalId = 'NEH'

        [[data.entityById.wasDerivedFrom]]
        id = 'chronicle:entity:NIH'

        [[data.entityById.wasRevisionOf]]
        id = 'chronicle:entity:NIH'
        "###);
    }

    #[tokio::test]
    async fn quotation() {
        let schema = test_schema().await;

        let created = schema
            .execute(Request::new(
                r#"
            mutation {
                wasQuotedFrom(generatedEntity: "chronicle:entity:NEH",
                            usedEntity: "chronicle:entity:NIH") {
                    context
                }
            }
        "#,
            ))
            .await;

        insta::assert_toml_snapshot!(created, @r###"
        [data.wasQuotedFrom]
        context = 'chronicle:entity:NEH'
        "###);

        tokio::time::sleep(Duration::from_millis(100)).await;
        let derived = schema
            .execute(Request::new(
                r#"
            query {
                entityById(id: "chronicle:entity:NEH") {
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
        id = 'chronicle:entity:NEH'
        externalId = 'NEH'

        [[data.entityById.wasDerivedFrom]]
        id = 'chronicle:entity:NIH'

        [[data.entityById.wasQuotedFrom]]
        id = 'chronicle:entity:NIH'
        "###);
    }

    #[tokio::test]
    async fn agent_can_be_created() {
        let schema = test_schema().await;

        let create = schema
            .execute(Request::new(
                r#"
            mutation {
                agent(externalId:"DOE", attributes: { type: "LSE" }) {
                    context
                }
            }
        "#,
            ))
            .await;

        insta::assert_toml_snapshot!(create, @r###"
        [data.agent]
        context = 'chronicle:agent:DOE'
        "###);
    }

    #[tokio::test]
    async fn was_informed_by() {
        let schema = test_schema().await;

        // create an activity
        let activity1 = schema
                    .execute(Request::new(
                        r#"
                    mutation one {
                      rnd(externalId:"urban development", attributes: { NYUattribute: "real estate", UCLattribute: 1, LSEattribute: false }) {
                            context
                        }
                    }
                "#
                        ),
                    )
                    .await;
        insta::assert_toml_snapshot!(activity1, @r###"
        [data.rnd]
        context = 'chronicle:activity:urban%20development'
        "###);
        tokio::time::sleep(Duration::from_millis(100)).await;

        // create another activity
        let activity2 = schema
                    .execute(Request::new(
                        r#"
                    mutation two {
                      rnr(externalId:"researching urban history", attributes: { NYUattribute: "string", UCLattribute: 1, LSEattribute: false }) {
                            context
                        }
                    }
                "#
                        ),
                    )
                    .await;
        insta::assert_toml_snapshot!(activity2, @r###"
        [data.rnr]
        context = 'chronicle:activity:researching%20urban%20history'
        "###);
        tokio::time::sleep(Duration::from_millis(100)).await;

        // establish WasInformedBy relationship
        let was_informed_by = schema
            .execute(Request::new(
                r#"
            mutation exec {
                wasInformedBy(activity: "chronicle:activity:urban%20development",
                informingActivity: "chronicle:activity:researching%20urban%20history",)
                {
                    context
                }
            }
        "#,
            ))
            .await;
        insta::assert_toml_snapshot!(was_informed_by, @r###"
        [data.wasInformedBy]
        context = 'chronicle:activity:urban%20development'
        "###);
        tokio::time::sleep(Duration::from_millis(100)).await;

        // query WasInformedBy relationship
        let response = schema
            .execute(Request::new(
                r#"
            query test {
                activityById(id: "chronicle:activity:urban%20development") {
                    ... on RND {
                        id
                        externalId
                        wasInformedBy {
                            ... on RNR {
                                id
                                externalId
                            }
                        }
                    }
                }
            }
        "#,
            ))
            .await;
        insta::assert_toml_snapshot!(response, @r###"
        [data.activityById]
        id = 'chronicle:activity:urban%20development'
        externalId = 'urban development'

        [[data.activityById.wasInformedBy]]
        id = 'chronicle:activity:researching%20urban%20history'
        externalId = 'researching urban history'
        "###);
        tokio::time::sleep(Duration::from_millis(100)).await;

        // create a third activity
        let activity3 = schema
                    .execute(Request::new(
                        r#"
                    mutation three {
                      rnr(externalId:"travel", attributes: { NYUattribute: "str", UCLattribute: 2, LSEattribute: true }) {
                            context
                        }
                    }
                "#
                        ),
                    )
                    .await;
        insta::assert_toml_snapshot!(activity3, @r###"
        [data.rnr]
        context = 'chronicle:activity:travel'
        "###);
        tokio::time::sleep(Duration::from_millis(100)).await;

        // establish another WasInformedBy relationship
        let was_informed_by2 = schema
            .execute(Request::new(
                r#"
            mutation execagain {
                wasInformedBy(activity: "chronicle:activity:urban%20development",
                informingActivity: "chronicle:activity:travel",)
                {
                    context
                }
            }
        "#,
            ))
            .await;
        insta::assert_toml_snapshot!(was_informed_by2, @r###"
        [data.wasInformedBy]
        context = 'chronicle:activity:urban%20development'
        "###);
        tokio::time::sleep(Duration::from_millis(100)).await;

        // query WasInformedBy relationship
        let response = schema
            .execute(Request::new(
                r#"
            query testagain {
                activityById(id: "chronicle:activity:urban%20development") {
                    ... on RND {
                        id
                        externalId
                        wasInformedBy {
                            ... on RNR {
                                id
                                externalId
                            }
                        }
                    }
                }
            }
        "#,
            ))
            .await;
        insta::assert_toml_snapshot!(response, @r###"
        [data.activityById]
        id = 'chronicle:activity:urban%20development'
        externalId = 'urban development'

        [[data.activityById.wasInformedBy]]
        id = 'chronicle:activity:researching%20urban%20history'
        externalId = 'researching urban history'

        [[data.activityById.wasInformedBy]]
        id = 'chronicle:activity:travel'
        externalId = 'travel'
        "###);
    }

    #[tokio::test]
    async fn generated() {
        let schema = test_schema().await;

        // create an entity
        let entity = schema
                    .execute(Request::new(
                        r#"
                    mutation entity {
                      neh(externalId:"endowment", attributes: { NYUattribute: "string", UCLattribute: 1, LSEattribute: false }) {
                            context
                        }
                    }
                "#
                        ),
                    )
                    .await;
        insta::assert_toml_snapshot!(entity, @r###"
        [data.neh]
        context = 'chronicle:entity:endowment'
        "###);
        tokio::time::sleep(Duration::from_millis(100)).await;

        // create an activity
        let activity = schema
                    .execute(Request::new(
                        r#"
                    mutation activity {
                      rnd(externalId:"researching congestion", attributes: { NYUattribute: "string", UCLattribute: 1, LSEattribute: false }) {
                            context
                        }
                    }
                "#
                        ),
                    )
                    .await;
        insta::assert_toml_snapshot!(activity, @r###"
        [data.rnd]
        context = 'chronicle:activity:researching%20congestion'
        "###);
        tokio::time::sleep(Duration::from_millis(100)).await;

        // establish Generated relationship
        let generated = schema
            .execute(Request::new(
                r#"
            mutation generated {
                wasGeneratedBy(activity: "chronicle:activity:damming",
                id: "chronicle:activity:tide",)
                {
                    context
                }
            }
        "#,
            ))
            .await;
        insta::assert_toml_snapshot!(generated, @r###"
        [data.wasGeneratedBy]
        context = 'chronicle:entity:tide'
        "###);
        tokio::time::sleep(Duration::from_millis(100)).await;

        // query Generated relationship
        let response = schema
            .execute(Request::new(
                r#"
            query test {
                activityById(id: "chronicle:activity:researching%20congestion") {
                    ... on RND {
                        id
                        externalId
                        generated {
                            ... on NEH {
                                id
                                externalId
                            }
                        }
                    }
                }
            }
        "#,
            ))
            .await;
        insta::assert_toml_snapshot!(response, @r###"
        [data.activityById]
        id = 'chronicle:activity:researching%20congestion'
        externalId = 'researching congestion'

        [[data.activityById.generated]]
        id = 'chronicle:entity:endowment'
        externalId = 'endowment'
        "###);
        tokio::time::sleep(Duration::from_millis(100)).await;

        // create another entity
        let entity2 = schema
                    .execute(Request::new(
                        r#"
                    mutation second {
                      theSea(externalId:"storm", attributes: { stringAttribute: "str", intAttribute: 2, boolAttribute: true }) {
                            context
                        }
                    }
                "#
                        ),
                    )
                    .await;
        insta::assert_toml_snapshot!(entity2, @r###"
        [data.theSea]
        context = 'chronicle:entity:storm'
        "###);
        tokio::time::sleep(Duration::from_millis(100)).await;

        // establish another Generated relationship
        let generated2 = schema
            .execute(Request::new(
                r#"
            mutation again {
                wasGeneratedBy(id: "chronicle:entity:storm",
                activity: "chronicle:activity:damming",)
                {
                    context
                }
            }
        "#,
            ))
            .await;
        insta::assert_toml_snapshot!(generated2, @r###"
        [data.wasGeneratedBy]
        context = 'chronicle:entity:storm'
        "###);
        tokio::time::sleep(Duration::from_millis(100)).await;

        // query Generated relationship
        let response = schema
            .execute(Request::new(
                r#"
            query testagain {
                activityById(id: "chronicle:entity:damming") {
                    ... on Gardening {
                        id
                        externalId
                        generated {
                            ... on TheSea {
                                id
                                externalId
                            }
                        }
                    }
                }
            }
        "#,
            ))
            .await;
        insta::assert_toml_snapshot!(response, @r###"
        [data.activityById]
        id = 'chronicle:activity:damming'
        externalId = 'damming'

        [[data.activityById.generated]]
        id = 'chronicle:entity:tide'
        externalId = 'tide'

        [[data.activityById.generated]]
        id = 'chronicle:entity:storm'
        externalId = 'storm'
        "###);
    }

    #[tokio::test]
    async fn query_activity_timeline() {
        let schema = test_schema().await;

        let res = schema
                .execute(Request::new(
                    r#"
            mutation {
                doe(externalId:"minister", attributes: { NYUattribute: "string", UCLattribute: 1, LSEattribute: false }) {
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
                doe(externalId:"inspector", attributes: { NYUattribute: "string", UCLattribute: 1, LSEattribute: false }) {
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
                neh(externalId:"fundraising", attributes: { NYUattribute: "string", UCLattribute: 1, LSEattribute: false }) {
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
                nih(externalId:"health", attributes: { NYUattribute: "string", UCLattribute: 1, LSEattribute: false }) {
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
                format!("rnd{}", i)
            } else {
                format!("rnr{}", i)
            };

            if (i % 2) == 0 {
                let res = schema
                    .execute(Request::new(
                        &format!(
                            r#"
                    mutation {{
                        rnd(externalId:"{}", attributes: {{ NYUattribute: "string", UCLattribute: 1, LSEattribute: false }}) {{
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
                        rnr(externalId:"{}", attributes: {{ NYUattribute: "string", UCLattribute: 1, LSEattribute: false }}) {{
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
                used(id: "chronicle:entity:fundraising", activity: "chronicle:activity:{}") {{
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
                      startActivity( time: "{}", id: "http://chronicle:activity:{}") {{
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
                endActivity( time: "{}", id: "http://chronicle:activity:{}") {{
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

            let agent = if i % 2 == 0 { "minister" } else { "inspector" };

            let res = schema
                .execute(Request::new(format!(
                    r#"
            mutation {{
                wasAssociatedWith( role: VIP, responsible: "chronicle:agent:{}", activity: "http://chronicle:activity:{}") {{
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
                            ... on RND {
                                id
                                externalId
                                NYUattribute
                                UCLattribute
                                LSEattribute
                                started
                                ended
                                wasAssociatedWith {
                                        responsible {
                                            agent {
                                                ... on DOE {
                                                    id
                                                    externalId
                                                }
                                            }
                                            role
                                        }
                                }
                                used {
                                    ... on NEH {
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
                    "__typename": "RNR"
                  },
                  "cursor": "0"
                },
                {
                  "node": {
                    "__typename": "RND",
                    "id": "chronicle:activity:rnd2",
                    "externalId": "rnd2",
                    "NYUattribute": "string",
                    "UCLattribute": 1,
                    "LSEattribute": null,
                    "started": "1968-09-03T00:00:00+00:00",
                    "ended": "1968-09-03T00:00:00+00:00",
                    "wasAssociatedWith": [
                      {
                        "responsible": {
                          "agent": {
                            "id": "chronicle:agent:minister",
                            "externalId": "minister"
                          },
                          "role": "VIP"
                        }
                      }
                    ],
                    "used": [
                      {
                        "id": "chronicle:entity:fundraising",
                        "externalId": "fundraising"
                      }
                    ]
                  },
                  "cursor": "1"
                },
                {
                  "node": {
                    "__typename": "RNR"
                  },
                  "cursor": "2"
                },
                {
                  "node": {
                    "__typename": "RND",
                    "id": "chronicle:activity:rnd4",
                    "externalId": "rnd4",
                    "NYUattribute": "string",
                    "UCLattribute": 1,
                    "LSEattribute": null,
                    "started": "1968-09-05T00:00:00+00:00",
                    "ended": "1968-09-05T00:00:00+00:00",
                    "wasAssociatedWith": [
                      {
                        "responsible": {
                          "agent": {
                            "id": "chronicle:agent:minister",
                            "externalId": "minister"
                          },
                          "role": "VIP"
                        }
                      }
                    ],
                    "used": [
                      {
                        "id": "chronicle:entity:fundraising",
                        "externalId": "fundraising"
                      }
                    ]
                  },
                  "cursor": "3"
                },
                {
                  "node": {
                    "__typename": "RNR"
                  },
                  "cursor": "4"
                },
                {
                  "node": {
                    "__typename": "RND",
                    "id": "chronicle:activity:rnd6",
                    "externalId": "rnd6",
                    "NYUattribute": "string",
                    "UCLattribute": 1,
                    "LSEattribute": null,
                    "started": "1968-09-07T00:00:00+00:00",
                    "ended": "1968-09-07T00:00:00+00:00",
                    "wasAssociatedWith": [
                      {
                        "responsible": {
                          "agent": {
                            "id": "chronicle:agent:minister",
                            "externalId": "minister"
                          },
                          "role": "VIP"
                        }
                      }
                    ],
                    "used": [
                      {
                        "id": "chronicle:entity:fundraising",
                        "externalId": "fundraising"
                      }
                    ]
                  },
                  "cursor": "5"
                },
                {
                  "node": {
                    "__typename": "RNR"
                  },
                  "cursor": "6"
                },
                {
                  "node": {
                    "__typename": "RND",
                    "id": "chronicle:activity:rnd8",
                    "externalId": "rnd8",
                    "NYUattribute": "string",
                    "UCLattribute": 1,
                    "LSEattribute": null,
                    "started": "1968-09-09T00:00:00+00:00",
                    "ended": "1968-09-09T00:00:00+00:00",
                    "wasAssociatedWith": [
                      {
                        "responsible": {
                          "agent": {
                            "id": "chronicle:agent:minister",
                            "externalId": "minister"
                          },
                          "role": "VIP"
                        }
                      }
                    ],
                    "used": [
                      {
                        "id": "chronicle:entity:fundraising",
                        "externalId": "fundraising"
                      }
                    ]
                  },
                  "cursor": "7"
                },
                {
                  "node": {
                    "__typename": "RNR"
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
                            ... on RND {
                                id
                                externalId
                                NYUattribute
                                UCLattribute
                                LSEattribute
                                started
                                ended
                                wasAssociatedWith {
                                        responsible {
                                            agent {
                                                ... on DOE {
                                                    id
                                                    externalId
                                                }
                                            }
                                            role
                                        }
                                }
                                used {
                                    ... on NEH {
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
                    "__typename": "RNR"
                  },
                  "cursor": "0"
                },
                {
                  "node": {
                    "__typename": "RND",
                    "id": "chronicle:activity:rnd8",
                    "externalId": "rnd8",
                    "NYUattribute": "string",
                    "UCLattribute": 1,
                    "LSEattribute": null,
                    "started": "1968-09-09T00:00:00+00:00",
                    "ended": "1968-09-09T00:00:00+00:00",
                    "wasAssociatedWith": [
                      {
                        "responsible": {
                          "agent": {
                            "id": "chronicle:agent:minister",
                            "externalId": "minister"
                          },
                          "role": "VIP"
                        }
                      }
                    ],
                    "used": [
                      {
                        "id": "chronicle:entity:fundraising",
                        "externalId": "fundraising"
                      }
                    ]
                  },
                  "cursor": "1"
                },
                {
                  "node": {
                    "__typename": "RNR"
                  },
                  "cursor": "2"
                },
                {
                  "node": {
                    "__typename": "RND",
                    "id": "chronicle:activity:rnd6",
                    "externalId": "rnd6",
                    "NYUattribute": "string",
                    "UCLattribute": 1,
                    "LSEattribute": null,
                    "started": "1968-09-07T00:00:00+00:00",
                    "ended": "1968-09-07T00:00:00+00:00",
                    "wasAssociatedWith": [
                      {
                        "responsible": {
                          "agent": {
                            "id": "chronicle:agent:minister",
                            "externalId": "minister"
                          },
                          "role": "VIP"
                        }
                      }
                    ],
                    "used": [
                      {
                        "id": "chronicle:entity:fundraising",
                        "externalId": "fundraising"
                      }
                    ]
                  },
                  "cursor": "3"
                },
                {
                  "node": {
                    "__typename": "RNR"
                  },
                  "cursor": "4"
                },
                {
                  "node": {
                    "__typename": "RND",
                    "id": "chronicle:activity:rnd4",
                    "externalId": "rnd4",
                    "NYUattribute": "string",
                    "UCLattribute": 1,
                    "LSEattribute": null,
                    "started": "1968-09-05T00:00:00+00:00",
                    "ended": "1968-09-05T00:00:00+00:00",
                    "wasAssociatedWith": [
                      {
                        "responsible": {
                          "agent": {
                            "id": "chronicle:agent:minister",
                            "externalId": "minister"
                          },
                          "role": "VIP"
                        }
                      }
                    ],
                    "used": [
                      {
                        "id": "chronicle:entity:fundraising",
                        "externalId": "fundraising"
                      }
                    ]
                  },
                  "cursor": "5"
                },
                {
                  "node": {
                    "__typename": "RNR"
                  },
                  "cursor": "6"
                },
                {
                  "node": {
                    "__typename": "RND",
                    "id": "chronicle:activity:rnd2",
                    "externalId": "rnd2",
                    "NYUattribute": "string",
                    "UCLattribute": 1,
                    "LSEattribute": null,
                    "started": "1968-09-03T00:00:00+00:00",
                    "ended": "1968-09-03T00:00:00+00:00",
                    "wasAssociatedWith": [
                      {
                        "responsible": {
                          "agent": {
                            "id": "chronicle:agent:minister",
                            "externalId": "minister"
                          },
                          "role": "VIP"
                        }
                      }
                    ],
                    "used": [
                      {
                        "id": "chronicle:entity:fundraising",
                        "externalId": "fundraising"
                      }
                    ]
                  },
                  "cursor": "7"
                },
                {
                  "node": {
                    "__typename": "RNR"
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
                  activityTypes: [RND, RNR],
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
                "endCursor": "8"
              },
              "edges": [
                {
                  "node": {
                    "__typename": "RNR"
                  },
                  "cursor": "0"
                },
                {
                  "node": {
                    "__typename": "RND"
                  },
                  "cursor": "1"
                },
                {
                  "node": {
                    "__typename": "RNR"
                  },
                  "cursor": "2"
                },
                {
                  "node": {
                    "__typename": "RND"
                  },
                  "cursor": "3"
                },
                {
                  "node": {
                    "__typename": "RNR"
                  },
                  "cursor": "4"
                },
                {
                  "node": {
                    "__typename": "RND"
                  },
                  "cursor": "5"
                },
                {
                  "node": {
                    "__typename": "RNR"
                  },
                  "cursor": "6"
                },
                {
                  "node": {
                    "__typename": "RND"
                  },
                  "cursor": "7"
                },
                {
                  "node": {
                    "__typename": "RNR"
                  },
                  "cursor": "8"
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
                  forAgent: ["chronicle:agent:inspector"],
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
                    "__typename": "RNR"
                  },
                  "cursor": "0"
                },
                {
                  "node": {
                    "__typename": "RNR"
                  },
                  "cursor": "1"
                },
                {
                  "node": {
                    "__typename": "RNR"
                  },
                  "cursor": "2"
                },
                {
                  "node": {
                    "__typename": "RNR"
                  },
                  "cursor": "3"
                },
                {
                  "node": {
                    "__typename": "RNR"
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
                doe(externalId:"minister{}", attributes: {{ NYUattribute: "String", UCLattribute: 1, LSEattribute: false }}) {{
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
                agentsByType(agentType: DOE) {
                    pageInfo {
                        hasPreviousPage
                        hasNextPage
                        startCursor
                        endCursor
                    }
                    edges {
                        node {
                            __typename
                            ... on DOE {
                                id
                                externalId
                                NYUattribute
                                UCLattribute
                                LSEattribute
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
                    "__typename": "DOE",
                    "id": "chronicle:agent:minister0",
                    "externalId": "minister0",
                    "NYUattribute": "String",
                    "UCLattribute": 1,
                    "LSEattribute": false
                  },
                  "cursor": "0"
                },
                {
                  "node": {
                    "__typename": "DOE",
                    "id": "chronicle:agent:minister1",
                    "externalId": "minister1",
                    "NYUattribute": "String",
                    "UCLattribute": 1,
                    "LSEattribute": false
                  },
                  "cursor": "1"
                },
                {
                  "node": {
                    "__typename": "DOE",
                    "id": "chronicle:agent:minister10",
                    "externalId": "minister10",
                    "NYUattribute": "String",
                    "UCLattribute": 1,
                    "LSEattribute": false
                  },
                  "cursor": "2"
                },
                {
                  "node": {
                    "__typename": "DOE",
                    "id": "chronicle:agent:minister11",
                    "externalId": "minister11",
                    "NYUattribute": "String",
                    "UCLattribute": 1,
                    "LSEattribute": false
                  },
                  "cursor": "3"
                },
                {
                  "node": {
                    "__typename": "DOE",
                    "id": "chronicle:agent:minister12",
                    "externalId": "minister12",
                    "NYUattribute": "String",
                    "UCLattribute": 1,
                    "LSEattribute": false
                  },
                  "cursor": "4"
                },
                {
                  "node": {
                    "__typename": "DOE",
                    "id": "chronicle:agent:minister13",
                    "externalId": "minister13",
                    "NYUattribute": "String",
                    "UCLattribute": 1,
                    "LSEattribute": false
                  },
                  "cursor": "5"
                },
                {
                  "node": {
                    "__typename": "DOE",
                    "id": "chronicle:agent:minister14",
                    "externalId": "minister14",
                    "NYUattribute": "String",
                    "UCLattribute": 1,
                    "LSEattribute": false
                  },
                  "cursor": "6"
                },
                {
                  "node": {
                    "__typename": "DOE",
                    "id": "chronicle:agent:minister15",
                    "externalId": "minister15",
                    "NYUattribute": "String",
                    "UCLattribute": 1,
                    "LSEattribute": false
                  },
                  "cursor": "7"
                },
                {
                  "node": {
                    "__typename": "DOE",
                    "id": "chronicle:agent:minister16",
                    "externalId": "minister16",
                    "NYUattribute": "String",
                    "UCLattribute": 1,
                    "LSEattribute": false
                  },
                  "cursor": "8"
                },
                {
                  "node": {
                    "__typename": "DOE",
                    "id": "chronicle:agent:minister17",
                    "externalId": "minister17",
                    "NYUattribute": "String",
                    "UCLattribute": 1,
                    "LSEattribute": false
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
                agentsByType(agentType: DOE, first: 20, after: "3") {
                    pageInfo {
                        hasPreviousPage
                        hasNextPage
                        startCursor
                        endCursor
                    }
                    edges {
                        node {
                            __typename
                            ... on DOE {
                                id
                                externalId
                                NYUattribute
                                UCLattribute
                                LSEattribute
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
                    "__typename": "DOE",
                    "id": "chronicle:agent:minister12",
                    "externalId": "minister12",
                    "NYUattribute": "String",
                    "UCLattribute": 1,
                    "LSEattribute": false
                  },
                  "cursor": "0"
                },
                {
                  "node": {
                    "__typename": "DOE",
                    "id": "chronicle:agent:minister13",
                    "externalId": "minister13",
                    "NYUattribute": "String",
                    "UCLattribute": 1,
                    "LSEattribute": false
                  },
                  "cursor": "1"
                },
                {
                  "node": {
                    "__typename": "DOE",
                    "id": "chronicle:agent:minister14",
                    "externalId": "minister14",
                    "NYUattribute": "String",
                    "UCLattribute": 1,
                    "LSEattribute": false
                  },
                  "cursor": "2"
                },
                {
                  "node": {
                    "__typename": "DOE",
                    "id": "chronicle:agent:minister15",
                    "externalId": "minister15",
                    "NYUattribute": "String",
                    "UCLattribute": 1,
                    "LSEattribute": false
                  },
                  "cursor": "3"
                },
                {
                  "node": {
                    "__typename": "DOE",
                    "id": "chronicle:agent:minister16",
                    "externalId": "minister16",
                    "NYUattribute": "String",
                    "UCLattribute": 1,
                    "LSEattribute": false
                  },
                  "cursor": "4"
                },
                {
                  "node": {
                    "__typename": "DOE",
                    "id": "chronicle:agent:minister17",
                    "externalId": "minister17",
                    "NYUattribute": "String",
                    "UCLattribute": 1,
                    "LSEattribute": false
                  },
                  "cursor": "5"
                },
                {
                  "node": {
                    "__typename": "DOE",
                    "id": "chronicle:agent:minister18",
                    "externalId": "minister18",
                    "NYUattribute": "String",
                    "UCLattribute": 1,
                    "LSEattribute": false
                  },
                  "cursor": "6"
                },
                {
                  "node": {
                    "__typename": "DOE",
                    "id": "chronicle:agent:minister19",
                    "externalId": "minister19",
                    "NYUattribute": "String",
                    "UCLattribute": 1,
                    "LSEattribute": false
                  },
                  "cursor": "7"
                },
                {
                  "node": {
                    "__typename": "DOE",
                    "id": "chronicle:agent:minister2",
                    "externalId": "minister2",
                    "NYUattribute": "String",
                    "UCLattribute": 1,
                    "LSEattribute": false
                  },
                  "cursor": "8"
                },
                {
                  "node": {
                    "__typename": "DOE",
                    "id": "chronicle:agent:minister20",
                    "externalId": "minister20",
                    "NYUattribute": "String",
                    "UCLattribute": 1,
                    "LSEattribute": false
                  },
                  "cursor": "9"
                },
                {
                  "node": {
                    "__typename": "DOE",
                    "id": "chronicle:agent:minister21",
                    "externalId": "minister21",
                    "NYUattribute": "String",
                    "UCLattribute": 1,
                    "LSEattribute": false
                  },
                  "cursor": "10"
                },
                {
                  "node": {
                    "__typename": "DOE",
                    "id": "chronicle:agent:minister22",
                    "externalId": "minister22",
                    "NYUattribute": "String",
                    "UCLattribute": 1,
                    "LSEattribute": false
                  },
                  "cursor": "11"
                },
                {
                  "node": {
                    "__typename": "DOE",
                    "id": "chronicle:agent:minister23",
                    "externalId": "minister23",
                    "NYUattribute": "String",
                    "UCLattribute": 1,
                    "LSEattribute": false
                  },
                  "cursor": "12"
                },
                {
                  "node": {
                    "__typename": "DOE",
                    "id": "chronicle:agent:minister24",
                    "externalId": "minister24",
                    "NYUattribute": "String",
                    "UCLattribute": 1,
                    "LSEattribute": false
                  },
                  "cursor": "13"
                },
                {
                  "node": {
                    "__typename": "DOE",
                    "id": "chronicle:agent:minister25",
                    "externalId": "minister25",
                    "NYUattribute": "String",
                    "UCLattribute": 1,
                    "LSEattribute": false
                  },
                  "cursor": "14"
                },
                {
                  "node": {
                    "__typename": "DOE",
                    "id": "chronicle:agent:minister26",
                    "externalId": "minister26",
                    "NYUattribute": "String",
                    "UCLattribute": 1,
                    "LSEattribute": false
                  },
                  "cursor": "15"
                },
                {
                  "node": {
                    "__typename": "DOE",
                    "id": "chronicle:agent:minister27",
                    "externalId": "minister27",
                    "NYUattribute": "String",
                    "UCLattribute": 1,
                    "LSEattribute": false
                  },
                  "cursor": "16"
                },
                {
                  "node": {
                    "__typename": "DOE",
                    "id": "chronicle:agent:minister28",
                    "externalId": "minister28",
                    "NYUattribute": "String",
                    "UCLattribute": 1,
                    "LSEattribute": false
                  },
                  "cursor": "17"
                },
                {
                  "node": {
                    "__typename": "DOE",
                    "id": "chronicle:agent:minister29",
                    "externalId": "minister29",
                    "NYUattribute": "String",
                    "UCLattribute": 1,
                    "LSEattribute": false
                  },
                  "cursor": "18"
                },
                {
                  "node": {
                    "__typename": "DOE",
                    "id": "chronicle:agent:minister3",
                    "externalId": "minister3",
                    "NYUattribute": "String",
                    "UCLattribute": 1,
                    "LSEattribute": false
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
                agentsByType(agentType: DOE, first: 20, after: "90") {
                    pageInfo {
                        hasPreviousPage
                        hasNextPage
                        startCursor
                        endCursor
                    }
                    edges {
                        node {
                            __typename
                            ... on DOE {
                                id
                                externalId
                                NYUattribute
                                UCLattribute
                                LSEattribute
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
                    "__typename": "DOE",
                    "id": "chronicle:agent:minister91",
                    "externalId": "minister91",
                    "NYUattribute": "String",
                    "UCLattribute": 1,
                    "LSEattribute": false
                  },
                  "cursor": "0"
                },
                {
                  "node": {
                    "__typename": "DOE",
                    "id": "chronicle:agent:minister92",
                    "externalId": "minister92",
                    "NYUattribute": "String",
                    "UCLattribute": 1,
                    "LSEattribute": false
                  },
                  "cursor": "1"
                },
                {
                  "node": {
                    "__typename": "DOE",
                    "id": "chronicle:agent:minister93",
                    "externalId": "minister93",
                    "NYUattribute": "String",
                    "UCLattribute": 1,
                    "LSEattribute": false
                  },
                  "cursor": "2"
                },
                {
                  "node": {
                    "__typename": "DOE",
                    "id": "chronicle:agent:minister94",
                    "externalId": "minister94",
                    "NYUattribute": "String",
                    "UCLattribute": 1,
                    "LSEattribute": false
                  },
                  "cursor": "3"
                },
                {
                  "node": {
                    "__typename": "DOE",
                    "id": "chronicle:agent:minister95",
                    "externalId": "minister95",
                    "NYUattribute": "String",
                    "UCLattribute": 1,
                    "LSEattribute": false
                  },
                  "cursor": "4"
                },
                {
                  "node": {
                    "__typename": "DOE",
                    "id": "chronicle:agent:minister96",
                    "externalId": "minister96",
                    "NYUattribute": "String",
                    "UCLattribute": 1,
                    "LSEattribute": false
                  },
                  "cursor": "5"
                },
                {
                  "node": {
                    "__typename": "DOE",
                    "id": "chronicle:agent:minister97",
                    "externalId": "minister97",
                    "NYUattribute": "String",
                    "UCLattribute": 1,
                    "LSEattribute": false
                  },
                  "cursor": "6"
                },
                {
                  "node": {
                    "__typename": "DOE",
                    "id": "chronicle:agent:minister98",
                    "externalId": "minister98",
                    "NYUattribute": "String",
                    "UCLattribute": 1,
                    "LSEattribute": false
                  },
                  "cursor": "7"
                },
                {
                  "node": {
                    "__typename": "DOE",
                    "id": "chronicle:agent:minister99",
                    "externalId": "minister99",
                    "NYUattribute": "String",
                    "UCLattribute": 1,
                    "LSEattribute": false
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
