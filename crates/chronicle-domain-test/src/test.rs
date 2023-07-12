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
    use async_sawtooth_sdk::prost::Message;
    use chronicle::{
        api::{
            chronicle_graphql::{OpaCheck, Store, Subscription},
            inmem::EmbeddedChronicleTp,
            Api, UuidGen,
        },
        async_graphql::{Request, Response, Schema},
        chrono::{DateTime, NaiveDate, Utc},
        common::{
            database::TemporaryDatabase,
            identity::AuthId,
            k256::sha2::{Digest, Sha256},
            opa::{CliPolicyLoader, ExecutorContext},
            signing::DirectoryStoredKeys,
        },
        serde_json, tokio,
        uuid::Uuid,
    };
    use core::future::Future;
    use opa_tp_protocol::state::{policy_address, policy_meta_address, PolicyMeta};
    use std::{collections::HashMap, time::Duration};
    use tempfile::TempDir;

    #[derive(Debug, Clone)]
    struct SameUuid;

    impl UuidGen for SameUuid {
        fn uuid() -> Uuid {
            Uuid::parse_str("5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea").unwrap()
        }
    }

    async fn test_schema<'a>() -> (Schema<Query, Mutation, Subscription>, TemporaryDatabase<'a>) {
        let loader = CliPolicyLoader::from_embedded_policy(
            "allow_transactions",
            "allow_transactions.allowed_users",
        )
        .unwrap();
        let opa_executor = ExecutorContext::from_loader(&loader).unwrap();

        test_schema_with_opa(opa_executor).await
    }

    async fn test_schema_blocked_api<'a>(
    ) -> (Schema<Query, Mutation, Subscription>, TemporaryDatabase<'a>) {
        let loader = CliPolicyLoader::from_embedded_policy(
            "allow_transactions",
            "allow_transactions.deny_all",
        )
        .unwrap();
        let opa_executor = ExecutorContext::from_loader(&loader).unwrap();

        test_schema_with_opa(opa_executor).await
    }

    async fn test_schema_with_opa<'a>(
        opa_executor: ExecutorContext,
    ) -> (Schema<Query, Mutation, Subscription>, TemporaryDatabase<'a>) {
        chronicle_telemetry::telemetry(None, chronicle_telemetry::ConsoleLogging::Pretty);

        let secretpath = TempDir::new().unwrap().into_path();

        let keystore_path = secretpath.clone();
        let keystore = DirectoryStoredKeys::new(keystore_path).unwrap();
        keystore.generate_chronicle().unwrap();

        let buf = async_sawtooth_sdk::messages::Setting {
            entries: vec![async_sawtooth_sdk::messages::setting::Entry {
                key: "chronicle.opa.policy_name".to_string(),
                value: "allow_transactions".to_string(),
            }],
        }
        .encode_to_vec();

        let setting_id = (
            chronicle_protocol::settings::sawtooth_settings_address("chronicle.opa.policy_name"),
            buf,
        );
        let buf = async_sawtooth_sdk::messages::Setting {
            entries: vec![async_sawtooth_sdk::messages::setting::Entry {
                key: "chronicle.opa.entrypoint".to_string(),
                value: "allow_transactions.allowed_users".to_string(),
            }],
        }
        .encode_to_vec();

        let setting_entrypoint = (
            chronicle_protocol::settings::sawtooth_settings_address("chronicle.opa.entrypoint"),
            buf,
        );

        let d = env!("CARGO_MANIFEST_DIR").to_owned() + "/../../policies/bundle.tar.gz";
        let bin = std::fs::read(d).unwrap();

        let meta = PolicyMeta {
            id: "allow_transactions".to_string(),
            hash: hex::encode(Sha256::digest(&bin)),
            policy_address: policy_address("allow_transactions"),
        };

        let tp = EmbeddedChronicleTp::new_with_state(
            vec![
                setting_id,
                setting_entrypoint,
                (policy_address("allow_transactions"), bin),
                (
                    policy_meta_address("allow_transactions"),
                    serde_json::to_vec(&meta).unwrap(),
                ),
            ]
            .into_iter()
            .collect(),
        )
        .unwrap();

        let ledger = tp.ledger.clone();

        let database = TemporaryDatabase::default();
        let pool = database.connection_pool().unwrap();
        let liveness_check_interval = None;

        let dispatch = Api::new(
            pool.clone(),
            ledger,
            &secretpath,
            SameUuid,
            HashMap::default(),
            None,
            liveness_check_interval,
        )
        .await
        .unwrap();

        let schema = Schema::build(Query, Mutation, Subscription)
            .extension(OpaCheck { claim_parser: None })
            .data(Store::new(pool))
            .data(dispatch)
            .data(AuthId::chronicle())
            .data(opa_executor)
            .finish();

        (schema, database)
    }

    #[tokio::test]
    async fn accept_long_form_including_original_name_iris() {
        let (schema, _database) = test_schema().await;

        insta::assert_toml_snapshot!(schema
          .execute(Request::new(
              r#"
          mutation {
              actedOnBehalfOf(
                  responsible: { id: "http://btp.works/chronicle/ns#agent:testagent" },
                  delegate: { id: "http://blockchaintp.com/chronicle/ns#agent:testdelegate" },
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

        tokio::time::sleep(Duration::from_millis(1500)).await;

        insta::assert_json_snapshot!(schema
          .execute(Request::new(
              r#"
          query {
              agentById(id: { id: "chronicle:agent:testdelegate" }) {
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
            "id": "chronicle:agent:testdelegate",
            "externalId": "testdelegate",
            "actedOnBehalfOf": [
              {
                "agent": {
                  "id": "chronicle:agent:testagent"
                },
                "role": "MANUFACTURER"
              }
            ]
          }
        }
        "###);
    }

    #[tokio::test]
    async fn activity_timeline_no_duplicates() {
        let (schema, _database) = test_schema().await;

        insta::assert_json_snapshot!(schema
        .execute(Request::new(
            r#"
            mutation defineContractorAndManufactureAndAssociate {
              defineContractorAgent(
                externalId: "testagent"
                attributes: { locationAttribute: "location" }
              ) {
                context
              }
              defineItemManufacturedActivity(
                externalId: "testactivity"
                attributes: { batchIdAttribute: "batchid" }
              ) {
                context
              }
              instantActivity(id: {externalId: "testactivity"}) {
                context
              }
              wasAssociatedWith(
                responsible: { externalId: "testagent" }
                activity: { externalId: "testactivity" }
                role: MANUFACTURER
              ) {
                context
              }
            }
            "#,
          ))
          .await, @r###"
        {
          "data": {
            "defineContractorAgent": {
              "context": "chronicle:agent:testagent"
            },
            "defineItemManufacturedActivity": {
              "context": "chronicle:activity:testactivity"
            },
            "instantActivity": {
              "context": "chronicle:activity:testactivity"
            },
            "wasAssociatedWith": {
              "context": "chronicle:agent:testagent"
            }
          }
        }
        "###);

        tokio::time::sleep(Duration::from_millis(1500)).await;

        insta::assert_json_snapshot!(schema
          .execute(Request::new(
              r#"
              mutation d1 {
                defineItemEntity(externalId: "entity1", attributes: {partIdAttribute: "partattr"}) {
                  context
                }
              }
              "#,
            ))
            .await, @r###"
        {
          "data": {
            "defineItemEntity": {
              "context": "chronicle:entity:entity1"
            }
          }
        }
        "###);

        tokio::time::sleep(Duration::from_millis(1500)).await;

        insta::assert_json_snapshot!(schema
          .execute(Request::new(
              r#"
              mutation d2 {
                defineItemEntity(externalId: "entity2", attributes: {partIdAttribute: "partattr"}) {
                  context
                }
              }
              "#,
            ))
            .await, @r###"
        {
          "data": {
            "defineItemEntity": {
              "context": "chronicle:entity:entity2"
            }
          }
        }
        "###);

        tokio::time::sleep(Duration::from_millis(1500)).await;

        insta::assert_json_snapshot!(schema
          .execute(Request::new(
              r#"
              mutation d3 {
                defineItemEntity(externalId: "entity3", attributes: {partIdAttribute: "partattr"}) {
                  context
                }
              }
              "#,
            ))
            .await, @r###"
        {
          "data": {
            "defineItemEntity": {
              "context": "chronicle:entity:entity3"
            }
          }
        }
        "###);

        tokio::time::sleep(Duration::from_millis(1500)).await;

        insta::assert_json_snapshot!(schema
          .execute(Request::new(
              r#"
              mutation g1 {
                wasGeneratedBy(activity: {externalId: "testactivity"}, id: {externalId: "entity1"}) {
                  context
                }
              }
              "#,
            ))
            .await, @r###"
        {
          "data": {
            "wasGeneratedBy": {
              "context": "chronicle:entity:entity1"
            }
          }
        }
        "###);

        tokio::time::sleep(Duration::from_millis(1500)).await;

        insta::assert_json_snapshot!(schema
          .execute(Request::new(
              r#"
              mutation g2 {
                wasGeneratedBy(activity: {externalId: "testactivity"}, id: {externalId: "entity2"}) {
                  context
                }
              }
              "#,
            ))
            .await, @r###"
        {
          "data": {
            "wasGeneratedBy": {
              "context": "chronicle:entity:entity2"
            }
          }
        }
        "###);

        tokio::time::sleep(Duration::from_millis(1500)).await;

        insta::assert_json_snapshot!(schema
          .execute(Request::new(
              r#"
              mutation g3 {
                wasGeneratedBy(activity: {externalId: "testactivity"}, id: {externalId: "entity3"}) {
                  context
                }
              }
              "#,
            ))
            .await, @r###"
        {
          "data": {
            "wasGeneratedBy": {
              "context": "chronicle:entity:entity3"
            }
          }
        }
        "###);

        tokio::time::sleep(Duration::from_millis(1500)).await;

        insta::assert_json_snapshot!(schema
          .execute(Request::new(
              r#"
              query a {
                activityTimeline(
                  activityTypes: [ItemManufacturedActivity]
                  forEntity: []
                  forAgent: [{externalId: "testagent"}]
                )
                {
                  nodes {
                    __typename
                    ... on ItemManufacturedActivity {
                      id
                      generated {
                        __typename
                        ... on ItemEntity {
                          id
                          partIdAttribute

                        }
                      }
                      wasAssociatedWith {
                        responsible {
                          agent {
                            __typename
                            ... on ContractorAgent {
                              id
                              locationAttribute
                            }
                          }
                        }
                      }
                    }
                  }
                }
              }
              "#,
            ))
            .await, @r###"
        {
          "data": {
            "activityTimeline": {
              "nodes": [
                {
                  "__typename": "ItemManufacturedActivity",
                  "id": "chronicle:activity:testactivity",
                  "generated": [
                    {
                      "__typename": "ItemEntity",
                      "id": "chronicle:entity:entity1",
                      "partIdAttribute": "partattr"
                    },
                    {
                      "__typename": "ItemEntity",
                      "id": "chronicle:entity:entity2",
                      "partIdAttribute": "partattr"
                    },
                    {
                      "__typename": "ItemEntity",
                      "id": "chronicle:entity:entity3",
                      "partIdAttribute": "partattr"
                    }
                  ],
                  "wasAssociatedWith": [
                    {
                      "responsible": {
                        "agent": {
                          "__typename": "ContractorAgent",
                          "id": "chronicle:agent:testagent",
                          "locationAttribute": "location"
                        }
                      }
                    }
                  ]
                }
              ]
            }
          }
        }
        "###);
    }

    #[tokio::test]
    async fn api_calls_resulting_in_no_data_changes_return_null() {
        let (schema, _database) = test_schema().await;

        let from = DateTime::<Utc>::from_utc(
            NaiveDate::from_ymd_opt(1968, 9, 1)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap(),
            Utc,
        )
        .checked_add_signed(chronicle::chrono::Duration::days(1))
        .unwrap()
        .to_rfc3339();

        let id_one = chronicle::async_graphql::Name::new("1");
        let id_two = chronicle::async_graphql::Name::new("2");

        insta::assert_json_snapshot!(schema
          .execute(Request::new(
            &format!(
            r#"
          mutation {{
            defineContractorAgent(
              externalId: "{id_one}",
              attributes: {{ locationAttribute: "location" }}
            ) {{
              context
              txId
            }}
            defineNCBAgent(
              externalId: "{id_two}",
              attributes: {{ manifestAttribute: "manifest" }}
            ) {{
              context
              txId
            }}
            actedOnBehalfOf(
              delegate: {{ externalId: "{id_one}", }}
              responsible: {{ externalId: "{id_two}" }}
              role: UNSPECIFIED
            ) {{
              context
              txId
            }}
            defineItemEntity(
              externalId: "{id_one}",
              attributes: {{ partIdAttribute: "part"}}
            ) {{
              context
              txId
            }}
            defineCertificateEntity(
              externalId: "{id_two}",
              attributes: {{ certIdAttribute: "cert" }}
            ) {{
              context
              txId
            }}
            defineItemCertifiedActivity(
              externalId: "{id_one}"
              attributes: {{ certIdAttribute: "cert" }}
            ) {{
              context
              txId
            }}
            defineItemManufacturedActivity(
              externalId: "{id_two}"
              attributes: {{ batchIdAttribute: "batch" }}
            ) {{
              context
              txId
            }}
            instantActivity(
              id: {{ externalId: "{id_one}" }}
              time: "{from}"
            ) {{
              context
              txId
            }}
            startActivity(
              id: {{ externalId: "{id_two}" }}
              time: "{from}"
            ) {{
              context
              txId
            }}
            endActivity(
              id: {{ externalId: "{id_two}" }}
              time: "{from}"
            ) {{
              context
              txId
            }}
            wasAssociatedWith(
              responsible: {{ externalId: "{id_one}" }}
              activity: {{ externalId: "{id_one}" }}
              role: UNSPECIFIED
            ) {{
              context
              txId
            }}
            used(
              activity: {{ externalId: "{id_one}" }}
              id: {{ externalId: "{id_one}" }}
            ) {{
              context
              txId
            }}
            wasInformedBy(
              informingActivity: {{ externalId: "{id_one}" }}
              activity: {{ externalId: "{id_two}" }}
            ) {{
              context
              txId
            }}
            wasGeneratedBy(
              id: {{ externalId: "{id_one}" }}
              activity: {{ externalId: "{id_one}" }}
            ) {{
              context
              txId
            }}
          }}
            "#
                ),
              ))
                .await, {
                  ".**.txId" => insta::dynamic_redaction(|value, _path| {
                      // assert that the value looks like a txId, i.e. not null
                      assert_eq!(value
                        .as_str()
                        .unwrap()
                        .chars()
                        .count(),
                        128
                    );
                      "[txId]"
                  }),
              }, @r###"
        {
          "data": {
            "defineContractorAgent": {
              "context": "chronicle:agent:1",
              "txId": "[txId]"
            },
            "defineNCBAgent": {
              "context": "chronicle:agent:2",
              "txId": "[txId]"
            },
            "actedOnBehalfOf": {
              "context": "chronicle:agent:2",
              "txId": "[txId]"
            },
            "defineItemEntity": {
              "context": "chronicle:entity:1",
              "txId": "[txId]"
            },
            "defineCertificateEntity": {
              "context": "chronicle:entity:2",
              "txId": "[txId]"
            },
            "defineItemCertifiedActivity": {
              "context": "chronicle:activity:1",
              "txId": "[txId]"
            },
            "defineItemManufacturedActivity": {
              "context": "chronicle:activity:2",
              "txId": "[txId]"
            },
            "instantActivity": {
              "context": "chronicle:activity:1",
              "txId": "[txId]"
            },
            "startActivity": {
              "context": "chronicle:activity:2",
              "txId": "[txId]"
            },
            "endActivity": {
              "context": "chronicle:activity:2",
              "txId": "[txId]"
            },
            "wasAssociatedWith": {
              "context": "chronicle:agent:1",
              "txId": "[txId]"
            },
            "used": {
              "context": "chronicle:entity:1",
              "txId": "[txId]"
            },
            "wasInformedBy": {
              "context": "chronicle:activity:2",
              "txId": "[txId]"
            },
            "wasGeneratedBy": {
              "context": "chronicle:entity:1",
              "txId": "[txId]"
            }
          }
        }
        "###);

        insta::assert_json_snapshot!(schema
          .execute(Request::new(
            &format!(
              r#"
            mutation {{
              defineContractorAgent(
                externalId: "{id_one}",
                attributes: {{ locationAttribute: "location" }}
              ) {{
                context
                txId
              }}
              defineNCBAgent(
                externalId: "{id_two}",
                attributes: {{ manifestAttribute: "manifest" }}
              ) {{
                context
                txId
              }}
              actedOnBehalfOf(
                delegate: {{ externalId: "{id_one}", }}
                responsible: {{ externalId: "{id_two}" }}
                role: UNSPECIFIED
              ) {{
                context
                txId
              }}
              defineItemEntity(
                externalId: "{id_one}",
                attributes: {{ partIdAttribute: "part"}}
              ) {{
                context
                txId
              }}
              defineCertificateEntity(
                externalId: "{id_two}",
                attributes: {{ certIdAttribute: "cert" }}
              ) {{
                context
                txId
              }}
              defineItemCertifiedActivity(
                externalId: "{id_one}"
                attributes: {{ certIdAttribute: "cert" }}
              ) {{
                context
                txId
              }}
              defineItemManufacturedActivity(
                externalId: "{id_two}"
                attributes: {{ batchIdAttribute: "batch" }}
              ) {{
                context
                txId
              }}
              instantActivity(
                id: {{ externalId: "{id_one}" }}
                time: "{from}"
              ) {{
                context
                txId
              }}
              startActivity(
                id: {{ externalId: "{id_two}" }}
                time: "{from}"
              ) {{
                context
                txId
              }}
              endActivity(
                id: {{ externalId: "{id_two}" }}
                time: "{from}"
              ) {{
                context
                txId
              }}
              wasAssociatedWith(
                responsible: {{ externalId: "{id_one}" }}
                activity: {{ externalId: "{id_one}" }}
                role: UNSPECIFIED
              ) {{
                context
                txId
              }}
              used(
                activity: {{ externalId: "{id_one}" }}
                id: {{ externalId: "{id_one}" }}
              ) {{
                context
                txId
              }}
              wasInformedBy(
                informingActivity: {{ externalId: "{id_one}" }}
                activity: {{ externalId: "{id_two}" }}
              ) {{
                context
                txId
              }}
              wasGeneratedBy(
                id: {{ externalId: "{id_one}" }}
                activity: {{ externalId: "{id_one}" }}
              ) {{
                context
                txId
              }}
            }}
              "#
              )))
                .await,
              @r###"
        {
          "data": {
            "defineContractorAgent": {
              "context": "chronicle:agent:1",
              "txId": null
            },
            "defineNCBAgent": {
              "context": "chronicle:agent:2",
              "txId": null
            },
            "actedOnBehalfOf": {
              "context": "chronicle:agent:2",
              "txId": null
            },
            "defineItemEntity": {
              "context": "chronicle:entity:1",
              "txId": null
            },
            "defineCertificateEntity": {
              "context": "chronicle:entity:2",
              "txId": null
            },
            "defineItemCertifiedActivity": {
              "context": "chronicle:activity:1",
              "txId": null
            },
            "defineItemManufacturedActivity": {
              "context": "chronicle:activity:2",
              "txId": null
            },
            "instantActivity": {
              "context": "chronicle:activity:1",
              "txId": null
            },
            "startActivity": {
              "context": "chronicle:activity:2",
              "txId": null
            },
            "endActivity": {
              "context": "chronicle:activity:2",
              "txId": null
            },
            "wasAssociatedWith": {
              "context": "chronicle:agent:1",
              "txId": null
            },
            "used": {
              "context": "chronicle:entity:1",
              "txId": null
            },
            "wasInformedBy": {
              "context": "chronicle:activity:2",
              "txId": null
            },
            "wasGeneratedBy": {
              "context": "chronicle:entity:1",
              "txId": null
            }
          }
        }
        "###);

        insta::assert_json_snapshot!(schema
          .execute(Request::new(
            &format!(
              r#"
            mutation {{
              defineContractorAgent(
                externalId: "{id_one}",
                attributes: {{ locationAttribute: "location" }}
              ) {{
                context
                txId
              }}
              defineNCBAgent(
                externalId: "{id_two}",
                attributes: {{ manifestAttribute: "manifest" }}
              ) {{
                context
                txId
              }}
              actedOnBehalfOf(
                delegate: {{ externalId: "{id_one}", }}
                responsible: {{ externalId: "{id_two}" }}
                role: UNSPECIFIED
              ) {{
                context
                txId
              }}
              defineItemEntity(
                externalId: "{id_one}",
                attributes: {{ partIdAttribute: "part"}}
              ) {{
                context
                txId
              }}
              defineCertificateEntity(
                externalId: "{id_two}",
                attributes: {{ certIdAttribute: "cert" }}
              ) {{
                context
                txId
              }}
              defineItemCertifiedActivity(
                externalId: "{id_one}"
                attributes: {{ certIdAttribute: "cert" }}
              ) {{
                context
                txId
              }}
              defineItemManufacturedActivity(
                externalId: "{id_two}"
                attributes: {{ batchIdAttribute: "batch" }}
              ) {{
                context
                txId
              }}
              instantActivity(
                id: {{ externalId: "{id_one}" }}
                time: "{from}"
              ) {{
                context
                txId
              }}
              startActivity(
                id: {{ externalId: "{id_two}" }}
                time: "{from}"
              ) {{
                context
                txId
              }}
              endActivity(
                id: {{ externalId: "{id_two}" }}
                time: "{from}"
              ) {{
                context
                txId
              }}
              wasAssociatedWith(
                responsible: {{ externalId: "{id_one}" }}
                activity: {{ externalId: "{id_one}" }}
                role: UNSPECIFIED
              ) {{
                context
                txId
              }}
              used(
                activity: {{ externalId: "{id_one}" }}
                id: {{ externalId: "{id_one}" }}
              ) {{
                context
                txId
              }}
              wasInformedBy(
                informingActivity: {{ externalId: "{id_one}" }}
                activity: {{ externalId: "{id_two}" }}
              ) {{
                context
                txId
              }}
              wasGeneratedBy(
                id: {{ externalId: "{id_one}" }}
                activity: {{ externalId: "{id_one}" }}
              ) {{
                context
                txId
              }}
            }}
              "#
              )))
                .await,
              @r###"
        {
          "data": {
            "defineContractorAgent": {
              "context": "chronicle:agent:1",
              "txId": null
            },
            "defineNCBAgent": {
              "context": "chronicle:agent:2",
              "txId": null
            },
            "actedOnBehalfOf": {
              "context": "chronicle:agent:2",
              "txId": null
            },
            "defineItemEntity": {
              "context": "chronicle:entity:1",
              "txId": null
            },
            "defineCertificateEntity": {
              "context": "chronicle:entity:2",
              "txId": null
            },
            "defineItemCertifiedActivity": {
              "context": "chronicle:activity:1",
              "txId": null
            },
            "defineItemManufacturedActivity": {
              "context": "chronicle:activity:2",
              "txId": null
            },
            "instantActivity": {
              "context": "chronicle:activity:1",
              "txId": null
            },
            "startActivity": {
              "context": "chronicle:activity:2",
              "txId": null
            },
            "endActivity": {
              "context": "chronicle:activity:2",
              "txId": null
            },
            "wasAssociatedWith": {
              "context": "chronicle:agent:1",
              "txId": null
            },
            "used": {
              "context": "chronicle:entity:1",
              "txId": null
            },
            "wasInformedBy": {
              "context": "chronicle:activity:2",
              "txId": null
            },
            "wasGeneratedBy": {
              "context": "chronicle:entity:1",
              "txId": null
            }
          }
        }
        "###);
    }

    #[tokio::test]
    async fn one_of_id_or_external() {
        let (schema, _database) = test_schema().await;

        let external_id_input = chronicle::async_graphql::Name::new("withexternalid");

        insta::assert_json_snapshot!(schema
          .execute(Request::new(
            &format!(
            r#"
          mutation {{
            defineContractorAgent(
              externalId: "{external_id_input}",
              attributes: {{ locationAttribute: "location" }}
            ) {{
              context
            }}
            defineAgent(
              externalId: "{external_id_input}",
              attributes: {{ type: "attribute" }}
            ) {{
              context
            }}
            actedOnBehalfOf(
              delegate: {{ externalId: "{external_id_input}", }}
              responsible: {{ externalId: "{external_id_input}" }}
              role: UNSPECIFIED
            ) {{
              context
            }}
            defineItemEntity(
              externalId: "{external_id_input}",
              attributes: {{ partIdAttribute: "part"}}
            ) {{
              context
            }}
            defineCertificateEntity(
              externalId: "{external_id_input}",
              attributes: {{ certIdAttribute: "cert" }}
            ) {{
              context
            }}
            wasDerivedFrom(
              generatedEntity: {{ externalId: "{external_id_input}" }}
              usedEntity: {{ externalId: "{external_id_input}" }}
            ) {{
              context
            }}
            wasRevisionOf(
              generatedEntity: {{ externalId: "{external_id_input}" }}
              usedEntity: {{ externalId: "{external_id_input}" }}
            ) {{
              context
            }}
            hadPrimarySource(
              generatedEntity: {{ externalId: "{external_id_input}" }}
              usedEntity: {{ externalId: "{external_id_input}" }}
            ) {{
              context
            }}
            wasQuotedFrom(
              generatedEntity: {{ externalId: "{external_id_input}" }}
              usedEntity: {{ externalId: "{external_id_input}" }}
            ) {{
              context
            }}
            generateKey(
              id: {{ externalId: "{external_id_input}" }}
            ) {{
              context
            }}
            defineItemCertifiedActivity(
              externalId: "{external_id_input}"
              attributes: {{ certIdAttribute: "cert" }}
            ) {{
              context
            }}
            defineActivity(
              externalId: "{external_id_input}"
              attributes: {{ type: "attr" }}
            ) {{
              context
            }}
            instantActivity(
              id: {{ externalId: "{external_id_input}" }}
            ) {{
              context
            }}
            wasAssociatedWith(
              responsible: {{ externalId: "{external_id_input}" }}
              activity: {{ externalId: "{external_id_input}" }}
              role: UNSPECIFIED
            ) {{
              context
            }}
            used(
              activity: {{ externalId: "{external_id_input}" }}
              id: {{ externalId: "{external_id_input}" }}
            ) {{
              context
            }}
            wasInformedBy(
              informingActivity: {{ externalId: "anotherexternalid" }}
              activity: {{ externalId: "{external_id_input}" }}
            ) {{
              context
            }}
            wasGeneratedBy(
              id: {{ externalId: "{external_id_input}" }}
              activity: {{ externalId: "{external_id_input}" }}
            ) {{
              context
            }}
          }}
            "#
                ),
              ))
                .await, @r###"
        {
          "data": {
            "defineContractorAgent": {
              "context": "chronicle:agent:withexternalid"
            },
            "defineAgent": {
              "context": "chronicle:agent:withexternalid"
            },
            "actedOnBehalfOf": {
              "context": "chronicle:agent:withexternalid"
            },
            "defineItemEntity": {
              "context": "chronicle:entity:withexternalid"
            },
            "defineCertificateEntity": {
              "context": "chronicle:entity:withexternalid"
            },
            "wasDerivedFrom": {
              "context": "chronicle:entity:withexternalid"
            },
            "wasRevisionOf": {
              "context": "chronicle:entity:withexternalid"
            },
            "hadPrimarySource": {
              "context": "chronicle:entity:withexternalid"
            },
            "wasQuotedFrom": {
              "context": "chronicle:entity:withexternalid"
            },
            "generateKey": {
              "context": "chronicle:agent:withexternalid"
            },
            "defineItemCertifiedActivity": {
              "context": "chronicle:activity:withexternalid"
            },
            "defineActivity": {
              "context": "chronicle:activity:withexternalid"
            },
            "instantActivity": {
              "context": "chronicle:activity:withexternalid"
            },
            "wasAssociatedWith": {
              "context": "chronicle:agent:withexternalid"
            },
            "used": {
              "context": "chronicle:entity:withexternalid"
            },
            "wasInformedBy": {
              "context": "chronicle:activity:withexternalid"
            },
            "wasGeneratedBy": {
              "context": "chronicle:entity:withexternalid"
            }
          }
        }
        "###);

        tokio::time::sleep(Duration::from_millis(1500)).await;

        insta::assert_json_snapshot!(schema
          .execute(Request::new(
            &format!(
            r#"
          query multipleQueries {{
            activityById(id: {{externalId: "{external_id_input}" }}) {{
              ... on ProvActivity {{
                id
                externalId
              }}
            }}
            agentById(id: {{externalId: "{external_id_input}" }}) {{
              ... on ProvAgent {{
                id
                externalId
              }}
            }}
            entityById(id: {{externalId: "{external_id_input}" }}) {{
              ... on CertificateEntity {{
                id
                externalId
              }}
            }}
          }}
          "#
        ),
      ))
        .await, @r###"
        {
          "data": {
            "activityById": {
              "id": "chronicle:activity:withexternalid",
              "externalId": "withexternalid"
            },
            "agentById": {
              "id": "chronicle:agent:withexternalid",
              "externalId": "withexternalid"
            },
            "entityById": {
              "id": "chronicle:entity:withexternalid",
              "externalId": "withexternalid"
            }
          }
        }
        "###);
    }

    #[tokio::test]
    async fn generic_json_object_attribute() {
        let (schema, _database) = test_schema().await;

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

        tokio::time::sleep(Duration::from_millis(1500)).await;

        insta::assert_json_snapshot!(schema
          .execute(Request::new(
            r#"
            query agent {
              agentById(id: {id: "chronicle:agent:testagent2" }) {
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

    #[tokio::test]
    async fn agent_delegation() {
        let (schema, _database) = test_schema().await;

        insta::assert_toml_snapshot!(schema
          .execute(Request::new(
              r#"
          mutation {
              actedOnBehalfOf(
                  responsible: { id: "chronicle:agent:testagent" },
                  delegate: { id: "chronicle:agent:testdelegate" },
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

        tokio::time::sleep(Duration::from_millis(1500)).await;

        insta::assert_json_snapshot!(schema
          .execute(Request::new(
              r#"
          query {
              agentById(id: { id: "chronicle:agent:testdelegate" }) {
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
            "id": "chronicle:agent:testdelegate",
            "externalId": "testdelegate",
            "actedOnBehalfOf": [
              {
                "agent": {
                  "id": "chronicle:agent:testagent"
                },
                "role": "MANUFACTURER"
              }
            ]
          }
        }
        "###);
    }

    #[tokio::test]
    async fn agent_delegation_for_activity() {
        let (schema, _database) = test_schema().await;

        // create contractors

        insta::assert_toml_snapshot!(schema
          .execute(Request::new(
              r#"
          mutation {
              defineContractorAgent(
                  externalId: "huey"
                  attributes: { locationAttribute: "location" }
              ) {
                  context
              }
            }
              "#,
            ))
            .await, @r###"
            [data.defineContractorAgent]
            context = 'chronicle:agent:huey'
          "###);

        tokio::time::sleep(Duration::from_millis(1500)).await;

        insta::assert_toml_snapshot!(schema
            .execute(Request::new(
                r#"
              mutation {
                defineContractorAgent(
                    externalId: "dewey"
                    attributes: { locationAttribute: "location" }
                ) {
                    context
                }
              }
              "#,
              ))
              .await, @r###"
              [data.defineContractorAgent]
              context = 'chronicle:agent:dewey'
              "###);

        tokio::time::sleep(Duration::from_millis(1500)).await;

        insta::assert_toml_snapshot!(schema
              .execute(Request::new(
                  r#"
                mutation {
                  defineContractorAgent(
                      externalId: "louie"
                      attributes: { locationAttribute: "location" }
                  ) {
                      context
                  }
                }
                "#,
                ))
                .await, @r###"
                [data.defineContractorAgent]
                context = 'chronicle:agent:louie'
                  "###);

        tokio::time::sleep(Duration::from_millis(1500)).await;

        // create activities

        insta::assert_toml_snapshot!(schema
          .execute(Request::new(
              r#"
            mutation {
              defineItemManufacturedActivity(
                externalId: "manufacture",
                attributes: { batchIdAttribute: "something" }
              ) {
                  context
              }
            }
      "#,
          ))
          .await, @r###"
        [data.defineItemManufacturedActivity]
        context = 'chronicle:activity:manufacture'
        "###);

        tokio::time::sleep(Duration::from_millis(1500)).await;

        insta::assert_toml_snapshot!(schema
          .execute(Request::new(
              r#"
            mutation {
              defineItemCertifiedActivity(
                externalId: "certification",
                attributes: { certIdAttribute: "something" }
              ) {
                  context
              }
           }
      "#,
          ))
          .await, @r###"
        [data.defineItemCertifiedActivity]
        context = 'chronicle:activity:certification'
        "###);

        tokio::time::sleep(Duration::from_millis(1500)).await;

        // associate contractors with activities

        insta::assert_toml_snapshot!(schema
          .execute(Request::new(
              r#"
            mutation {
              wasAssociatedWith(
                activity: { externalId: "manufacture" }
                responsible: { externalId: "huey" }
                role: MANUFACTURER
              ) {
                  context
               }
            }
      "#,
          ))
          .await, @r###"
          [data.wasAssociatedWith]
          context = 'chronicle:agent:huey'
        "###);

        tokio::time::sleep(Duration::from_millis(1500)).await;

        insta::assert_toml_snapshot!(schema
          .execute(Request::new(
              r#"
            mutation {
              wasAssociatedWith(
                activity: { externalId: "certification" }
                responsible: { externalId: "dewey" }
                role: CERTIFIER
              ) {
                  context
              }
            }
      "#,
          ))
          .await, @r###"
          [data.wasAssociatedWith]
          context = 'chronicle:agent:dewey'
        "###);

        tokio::time::sleep(Duration::from_millis(1500)).await;

        insta::assert_toml_snapshot!(schema
          .execute(Request::new(
              r#"
            mutation {
              actedOnBehalfOf(
                  activity: { externalId: "manufacture" }
                  responsible: { externalId: "huey" }
                  delegate: { externalId: "louie" }
                  role: SUBMITTER
                ) {
                  context
              }
          }
      "#,
          ))
          .await, @r###"
        [data.actedOnBehalfOf]
        context = 'chronicle:agent:huey'
        "###);

        tokio::time::sleep(Duration::from_millis(1500)).await;

        // check responsible and delegate are correct for activities

        insta::assert_json_snapshot!(schema
          .execute(Request::new(
              r#"
          query {
            activityById(id: { externalId: "manufacture" }) {
              ... on ItemManufacturedActivity {
                wasAssociatedWith {
                  responsible {
                    agent {
                      ... on ContractorAgent {
                        externalId
                      }
                    }
                    role
                  }
                  delegate {
                    agent {
                      ... on ContractorAgent {
                        externalId
                      }
                    }
                    role
                  }
                }
              }
            }
          }
      "#,
          ))
          .await.data, @r###"
          {
            "activityById": {
              "wasAssociatedWith": [
                {
                  "responsible": {
                    "agent": {
                      "externalId": "huey"
                    },
                    "role": "MANUFACTURER"
                  },
                  "delegate": {
                    "agent": {
                      "externalId": "louie"
                    },
                    "role": "SUBMITTER"
                  }
                }
              ]
            }
          }
        "###);

        insta::assert_json_snapshot!(schema
          .execute(Request::new(
              r#"
          query {
            activityById(id: { externalId: "certification" }) {
              ... on ItemCertifiedActivity {
                wasAssociatedWith {
                  responsible {
                    agent {
                      ... on ContractorAgent {
                        externalId
                      }
                    }
                    role
                  }
                  delegate {
                    agent {
                      ... on ContractorAgent {
                        externalId
                      }
                    }
                    role
                  }
                }
              }
            }
          }
      "#,
          ))
          .await.data, @r###"
          {
            "activityById": {
              "wasAssociatedWith": [
                {
                  "responsible": {
                    "agent": {
                      "externalId": "dewey"
                    },
                    "role": "CERTIFIER"
                  },
                  "delegate": null
                }
              ]
            }
          }
        "###);

        // use the same delegated contractor for two different roles

        insta::assert_toml_snapshot!(schema
          .execute(Request::new(
              r#"
            mutation {
              wasAssociatedWith(
                activity: { externalId: "manufacture" }
                responsible: { externalId: "huey" }
                role: CODIFIER
              ) {
                  context
               }
            }
      "#,
          ))
          .await, @r###"
          [data.wasAssociatedWith]
          context = 'chronicle:agent:huey'
        "###);

        tokio::time::sleep(Duration::from_millis(1500)).await;

        // check that same delegate is returned twice over

        insta::assert_json_snapshot!(schema
          .execute(Request::new(
              r#"
          query {
            activityById(id: { externalId: "manufacture" }) {
              ... on ItemManufacturedActivity {
                wasAssociatedWith {
                  responsible {
                    agent {
                      ... on ContractorAgent {
                        externalId
                      }
                    }
                    role
                  }
                  delegate {
                    agent {
                      ... on ContractorAgent {
                        externalId
                      }
                    }
                    role
                  }
                }
              }
            }
          }
      "#,
          ))
          .await.data, @r###"
          {
            "activityById": {
              "wasAssociatedWith": [
                {
                  "responsible": {
                    "agent": {
                      "externalId": "huey"
                    },
                    "role": "CODIFIER"
                  },
                  "delegate": {
                    "agent": {
                      "externalId": "louie"
                    },
                    "role": "SUBMITTER"
                  }
                },
                {
                  "responsible": {
                    "agent": {
                      "externalId": "huey"
                    },
                    "role": "MANUFACTURER"
                  },
                  "delegate": {
                    "agent": {
                      "externalId": "louie"
                    },
                    "role": "SUBMITTER"
                  }
                }
              ]
            }
          }
        "###);
    }

    #[tokio::test]
    async fn untyped_derivation() {
        let (schema, _database) = test_schema().await;

        insta::assert_toml_snapshot!(schema
          .execute(Request::new(
              r#"
          mutation {
              wasDerivedFrom(generatedEntity: {id: "chronicle:entity:testentity1" },
                             usedEntity: {id: "chronicle:entity:testentity2" }) {
                  context
              }
          }
      "#,
          ))
          .await, @r###"
        [data.wasDerivedFrom]
        context = 'chronicle:entity:testentity1'
        "###);

        tokio::time::sleep(Duration::from_millis(1000)).await;

        insta::assert_toml_snapshot!(schema
          .execute(Request::new(
              r#"
          query {
              entityById(id: {id: "chronicle:entity:testentity1" }) {
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
          .await, @r###"
        [data.entityById]
        id = 'chronicle:entity:testentity1'
        externalId = 'testentity1'

        [[data.entityById.wasDerivedFrom]]
        id = 'chronicle:entity:testentity2'
        "###);
    }

    #[tokio::test]
    async fn primary_source() {
        let (schema, _database) = test_schema().await;

        insta::assert_toml_snapshot!(schema
          .execute(Request::new(
              r#"
          mutation {
              hadPrimarySource(generatedEntity: {id: "chronicle:entity:testentity1" },
                             usedEntity: {id: "chronicle:entity:testentity2" }) {
                  context
              }
          }
      "#,
          ))
          .await, @r###"
        [data.hadPrimarySource]
        context = 'chronicle:entity:testentity1'
        "###);

        tokio::time::sleep(Duration::from_millis(1000)).await;

        insta::assert_toml_snapshot!(schema
          .execute(Request::new(
              r#"
          query {
              entityById(id: {id: "chronicle:entity:testentity1" }) {

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
          .await, @r###"
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
        let (schema, _database) = test_schema().await;

        insta::assert_toml_snapshot!(schema
          .execute(Request::new(
              r#"
          mutation {
              wasRevisionOf(generatedEntity: {id: "chronicle:entity:testentity1" },
                          usedEntity: {id: "chronicle:entity:testentity2" }) {
                  context
              }
          }
      "#,
          ))
          .await, @r###"
        [data.wasRevisionOf]
        context = 'chronicle:entity:testentity1'
        "###);

        tokio::time::sleep(Duration::from_millis(1000)).await;

        insta::assert_toml_snapshot!(schema
          .execute(Request::new(
              r#"
          query {
              entityById(id: {id: "chronicle:entity:testentity1" }) {
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
          .await, @r###"
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
        let (schema, _database) = test_schema().await;

        insta::assert_toml_snapshot!(schema
          .execute(Request::new(
              r#"
          mutation {
              wasQuotedFrom(generatedEntity: {id: "chronicle:entity:testentity1" },
                          usedEntity: {id: "chronicle:entity:testentity2" }) {
                  context
              }
          }
      "#,
          ))
          .await, @r###"
        [data.wasQuotedFrom]
        context = 'chronicle:entity:testentity1'
        "###);

        tokio::time::sleep(Duration::from_millis(1000)).await;

        insta::assert_toml_snapshot!(schema
          .execute(Request::new(
              r#"
          query {
              entityById(id: {id: "chronicle:entity:testentity1" }) {
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
          .await, @r###"
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
        let (schema, _database) = test_schema().await;

        insta::assert_toml_snapshot!(schema
          .execute(Request::new(
              r#"
          mutation {
              defineAgent(externalId:"testentity1", attributes: { type: "type" }) {
                  context
              }
          }
      "#,
          ))
          .await, @r###"
        [data.defineAgent]
        context = 'chronicle:agent:testentity1'
        "###);
    }

    #[tokio::test]
    async fn agent_by_type() {
        let (schema, _database) = test_schema().await;

        insta::assert_toml_snapshot!(schema
          .execute(Request::new(
              r#"
          mutation {
              defineContractorAgent(externalId:"testagent1", attributes: { locationAttribute: "somewhere" }) {
                  context
              }
          }
      "#,
          ))
          .await, @r###"
        [data.defineContractorAgent]
        context = 'chronicle:agent:testagent1'
        "###);

        tokio::time::sleep(Duration::from_millis(1000)).await;

        insta::assert_toml_snapshot!(schema
          .execute(Request::new(
              r#"
          query {
              agentsByType(agentType: ContractorAgent) {
                nodes {
                  ...on ContractorAgent {
                    id
                  }
                }
              }
          }"#,
          ))
          .await, @r###"
        [[data.agentsByType.nodes]]
        id = 'chronicle:agent:testagent1'
        "###);
    }

    #[tokio::test]
    async fn activity_by_type() {
        let (schema, _database) = test_schema().await;

        insta::assert_toml_snapshot!(schema
          .execute(Request::new(
              r#"
          mutation {
              defineItemCertifiedActivity(externalId:"testactivity1", attributes: { certIdAttribute: "something" }) {
                  context
              }
          }
      "#,
          ))
          .await, @r###"
        [data.defineItemCertifiedActivity]
        context = 'chronicle:activity:testactivity1'
        "###);

        tokio::time::sleep(Duration::from_millis(1000)).await;

        insta::assert_toml_snapshot!(schema
          .execute(Request::new(
              r#"
          query {
              activitiesByType(activityType: ItemCertifiedActivity) {
                nodes {
                  ...on ItemCertifiedActivity {
                    id
                  }
                }
              }
          }"#,
          ))
          .await, @r###"
        [[data.activitiesByType.nodes]]
        id = 'chronicle:activity:testactivity1'
        "###);
    }

    #[tokio::test]
    async fn entity_by_type() {
        let (schema, _database) = test_schema().await;

        insta::assert_toml_snapshot!(schema
          .execute(Request::new(
              r#"
          mutation {
              defineCertificateEntity(externalId:"testentity1", attributes: { certIdAttribute: "something" }) {
                  context
              }
          }
      "#,
          ))
          .await, @r###"
        [data.defineCertificateEntity]
        context = 'chronicle:entity:testentity1'
        "###);

        tokio::time::sleep(Duration::from_millis(1000)).await;

        let agent = schema
            .execute(Request::new(
                r#"
            query {
                entitiesByType(entityType: CertificateEntity) {
                  nodes {
                    ...on CertificateEntity {
                      id
                    }
                  }
                }
            }"#,
            ))
            .await;

        insta::assert_toml_snapshot!(agent, @r###"
        [[data.entitiesByType.nodes]]
        id = 'chronicle:entity:testentity1'
        "###);
    }

    #[tokio::test]
    async fn was_informed_by() {
        let (schema, _database) = test_schema().await;

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
              wasInformedBy(activity: {id: "chronicle:activity:testactivityid1" },
              informingActivity: {id: "chronicle:activity:testactivityid2" },)
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

        tokio::time::sleep(Duration::from_millis(1000)).await;

        // query WasInformedBy relationship
        insta::assert_toml_snapshot!(schema
            .execute(Request::new(
                r#"
            query test {
                activityById(id: {id: "chronicle:activity:testactivityid1" }) {
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

        tokio::time::sleep(Duration::from_millis(1000)).await;

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

        tokio::time::sleep(Duration::from_millis(1000)).await;

        // establish another WasInformedBy relationship
        insta::assert_toml_snapshot!(schema
          .execute(Request::new(
              r#"
          mutation execagain {
              wasInformedBy(activity: {id: "chronicle:activity:testactivityid1" },
              informingActivity: {id: "chronicle:activity:testactivityid3" },)
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

        tokio::time::sleep(Duration::from_millis(1000)).await;

        // query WasInformedBy relationship
        insta::assert_toml_snapshot!(schema
            .execute(Request::new(
                r#"
                query test {
                  activityById(id: {id: "chronicle:activity:testactivityid1" }) {
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
    async fn was_attributed_to() {
        let (schema, _database) = test_schema().await;

        let test_activity = chronicle::async_graphql::Name::new("ItemCertified");

        let from = DateTime::<Utc>::from_utc(
            NaiveDate::from_ymd_opt(2023, 3, 20)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap(),
            Utc,
        )
        .checked_add_signed(chronicle::chrono::Duration::days(1))
        .unwrap()
        .to_rfc3339();

        let test_entity = chronicle::async_graphql::Name::new("Certificate");

        let test_agent = chronicle::async_graphql::Name::new("Certifier");

        let to = DateTime::<Utc>::from_utc(
            NaiveDate::from_ymd_opt(2023, 3, 21)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap(),
            Utc,
        )
        .checked_add_signed(chronicle::chrono::Duration::days(1))
        .unwrap()
        .to_rfc3339();

        // create an activity that used an entity and was associated with an agent
        insta::assert_json_snapshot!(schema
          .execute(Request::new(
            &format!(
            r#"
              mutation certifiedActivity {{
                defineContractorAgent(externalId: "{test_agent}", attributes: {{ locationAttribute: "SomeLocation"}}) {{
                  context
                }}
                defineItemCertifiedActivity(externalId: "{test_activity}", attributes: {{ certIdAttribute: "SomeCertId" }}) {{
                  context
                  }}
                defineCertificateEntity(externalId: "{test_entity}", attributes: {{ certIdAttribute: "SomeCertId" }}) {{
                  context
                }}
                startActivity(
                  id: {{ externalId: "{test_activity}" }}
                  time: "{from}"
                ) {{
                  context
                }}
                used(id: {{ id: "chronicle:entity:{test_entity}" }}, activity: {{ id: "chronicle:activity:{test_activity}" }}) {{
                  context
                }}
                wasAssociatedWith( role: CERTIFIER, responsible: {{ id: "chronicle:agent:{test_agent}" }}, activity: {{id: "chronicle:activity:{test_activity}" }}) {{
                  context
                }}
                endActivity(
                  id: {{ externalId: "{test_activity}" }}
                  time: "{to}"
                ) {{
                  context
                }}
              }}
          "#,
          )))
          .await, @r###"
        {
          "data": {
            "defineContractorAgent": {
              "context": "chronicle:agent:Certifier"
            },
            "defineItemCertifiedActivity": {
              "context": "chronicle:activity:ItemCertified"
            },
            "defineCertificateEntity": {
              "context": "chronicle:entity:Certificate"
            },
            "startActivity": {
              "context": "chronicle:activity:ItemCertified"
            },
            "used": {
              "context": "chronicle:entity:Certificate"
            },
            "wasAssociatedWith": {
              "context": "chronicle:agent:Certifier"
            },
            "endActivity": {
              "context": "chronicle:activity:ItemCertified"
            }
          }
        }
        "###);

        tokio::time::sleep(Duration::from_millis(1000)).await;

        // attribute the entity to the agent
        insta::assert_json_snapshot!(schema
              .execute(Request::new(
                &format!(
                r#"
                  mutation attribution {{
                    wasAttributedTo( role: CERTIFIER, responsible: {{ id: "chronicle:agent:{test_agent}" }}, entity: {{id: "chronicle:entity:{test_entity}" }}) {{
                      context
                    }}
                  }}
              "#,
              )))
              .await, @r###"
              {
                "data": {
                  "wasAttributedTo": {
                    "context": "chronicle:agent:Certifier"
                  }
                }
              }
              "###);

        tokio::time::sleep(Duration::from_millis(1000)).await;

        // query WasAttributedTo relationship
        insta::assert_toml_snapshot!(schema
                  .execute(Request::new(
                    &format!(
                r#"
                  query queryWasAttributedTo {{
                    entityById(id: {{ id: "chronicle:entity:{test_entity}" }}) {{
                      ... on CertificateEntity {{
                          id
                          externalId
                          wasAttributedTo {{
                            responsible {{
                              role
                              agent {{
                                ... on ContractorAgent {{
                                  externalId
                                  id
                                  locationAttribute
                                }}
                              }}
                            }}
                          }}
                      }}
                    }}
                  }}
              "#,
              )))
              .await, @r###"
        [data.entityById]
        id = 'chronicle:entity:Certificate'
        externalId = 'Certificate'

        [[data.entityById.wasAttributedTo]]
        [data.entityById.wasAttributedTo.responsible]
        role = 'CERTIFIER'

        [data.entityById.wasAttributedTo.responsible.agent]
        externalId = 'Certifier'
        id = 'chronicle:agent:Certifier'
        locationAttribute = 'SomeLocation'
        "###);

        tokio::time::sleep(Duration::from_millis(1000)).await;

        insta::assert_toml_snapshot!(schema
                  .execute(Request::new(
                r#"
                  query queryWasAttributedToAnotherWay {
                    entitiesByType(entityType: CertificateEntity) {
                      nodes {
                        ... on CertificateEntity {
                          id
                          externalId
                          wasAttributedTo {
                            responsible {
                              role
                              agent {
                                ... on ContractorAgent {
                                  externalId
                                  id
                                  locationAttribute
                                }
                              }
                            }
                          }
                        }
                      }
                    }
                  }
              "#,
                ))
                .await, @r###"
        [[data.entitiesByType.nodes]]
        id = 'chronicle:entity:Certificate'
        externalId = 'Certificate'

        [[data.entitiesByType.nodes.wasAttributedTo]]
        [data.entitiesByType.nodes.wasAttributedTo.responsible]
        role = 'CERTIFIER'

        [data.entitiesByType.nodes.wasAttributedTo.responsible.agent]
        externalId = 'Certifier'
        id = 'chronicle:agent:Certifier'
        locationAttribute = 'SomeLocation'
        "###);

        tokio::time::sleep(Duration::from_millis(1000)).await;

        insta::assert_toml_snapshot!(schema
                .execute(Request::new(
                  &format!(
                    r#"
                query queryAgentAttribution {{
                  agentById(id: {{externalId: "{test_agent}" }}) {{
                    ... on ContractorAgent {{
                      id
                      externalId
                      attribution {{
                        attributed {{
                          role
                          entity {{
                            ... on CertificateEntity {{
                              externalId
                              id
                              certIdAttribute
                            }}
                          }}
                        }}
                      }}
                    }}
                  }}
                }}
            "#,
                )))
                .await, @r###"
        [data.agentById]
        id = 'chronicle:agent:Certifier'
        externalId = 'Certifier'

        [[data.agentById.attribution]]
        [data.agentById.attribution.attributed]
        role = 'CERTIFIER'

        [data.agentById.attribution.attributed.entity]
        externalId = 'Certificate'
        id = 'chronicle:entity:Certificate'
        certIdAttribute = 'SomeCertId'
        "###);

        tokio::time::sleep(Duration::from_millis(1000)).await;

        insta::assert_toml_snapshot!(schema
                .execute(Request::new(
                    r#"
                query queryAgentAttributionAnotherWay {
                  agentsByType(agentType: ContractorAgent) {
                    nodes {
                      ... on ContractorAgent {
                        id
                        externalId
                        attribution {
                          attributed {
                            role
                            entity {
                              ... on CertificateEntity {
                                externalId
                                id
                                certIdAttribute
                              }
                            }
                          }
                        }
                      }
                    }
                  }
                }
            "#,
                ))
                .await, @r###"
        [[data.agentsByType.nodes]]
        id = 'chronicle:agent:Certifier'
        externalId = 'Certifier'

        [[data.agentsByType.nodes.attribution]]
        [data.agentsByType.nodes.attribution.attributed]
        role = 'CERTIFIER'

        [data.agentsByType.nodes.attribution.attributed.entity]
        externalId = 'Certificate'
        id = 'chronicle:entity:Certificate'
        certIdAttribute = 'SomeCertId'
        "###);

        tokio::time::sleep(Duration::from_millis(1000)).await;

        // create another agent and attribute the entity to that other agent as well
        insta::assert_json_snapshot!(schema
              .execute(Request::new(
                &format!(
                r#"
                  mutation anotherAgent {{
                    defineContractorAgent(externalId: "Certifier2", attributes: {{ locationAttribute: "AnotherLocation"}}) {{
                      context
                    }}
                    wasAttributedTo( role: UNSPECIFIED, responsible: {{ id: "chronicle:agent:Certifier2" }}, entity: {{id: "chronicle:entity:{test_entity}" }}) {{
                      context
                    }}
                  }}
              "#,
              )))
              .await, @r###"
        {
          "data": {
            "defineContractorAgent": {
              "context": "chronicle:agent:Certifier2"
            },
            "wasAttributedTo": {
              "context": "chronicle:agent:Certifier2"
            }
          }
        }
        "###);

        tokio::time::sleep(Duration::from_millis(1000)).await;

        // query WasAttributedTo relationship
        insta::assert_toml_snapshot!(schema
              .execute(Request::new(
                &format!(
                  r#"
                query queryWasAttributedToSecondAgent {{
                  entityById(id: {{id: "chronicle:entity:{test_entity}" }}) {{
                    ... on CertificateEntity {{
                      id
                      externalId
                      wasAttributedTo {{
                        responsible {{
                          role
                          agent {{
                            ... on ContractorAgent {{
                              externalId
                              id
                              locationAttribute
                            }}
                          }}
                        }}
                      }}
                    }}
                  }}
                }}
            "#,
              )))
              .await, @r###"
        [data.entityById]
        id = 'chronicle:entity:Certificate'
        externalId = 'Certificate'

        [[data.entityById.wasAttributedTo]]
        [data.entityById.wasAttributedTo.responsible]
        role = 'CERTIFIER'

        [data.entityById.wasAttributedTo.responsible.agent]
        externalId = 'Certifier'
        id = 'chronicle:agent:Certifier'
        locationAttribute = 'SomeLocation'

        [[data.entityById.wasAttributedTo]]
        [data.entityById.wasAttributedTo.responsible]
        role = 'UNSPECIFIED'

        [data.entityById.wasAttributedTo.responsible.agent]
        externalId = 'Certifier2'
        id = 'chronicle:agent:Certifier2'
        locationAttribute = 'AnotherLocation'
        "###);

        tokio::time::sleep(Duration::from_millis(1000)).await;

        insta::assert_toml_snapshot!(schema
            .execute(Request::new(
                r#"
            query queryWasAttributedToSecondAgentAnotherWay {
                entitiesByType(entityType: CertificateEntity) {
                  nodes {
                    ... on CertificateEntity {
                        id
                        externalId
                        wasAttributedTo {
                          responsible {
                            role
                            agent {
                              ... on ContractorAgent {
                                externalId
                                id
                                locationAttribute
                              }
                            }
                          }
                        }
                    }
                }
              }
            }
        "#,
            ))
            .await, @r###"
        [[data.entitiesByType.nodes]]
        id = 'chronicle:entity:Certificate'
        externalId = 'Certificate'

        [[data.entitiesByType.nodes.wasAttributedTo]]
        [data.entitiesByType.nodes.wasAttributedTo.responsible]
        role = 'CERTIFIER'

        [data.entitiesByType.nodes.wasAttributedTo.responsible.agent]
        externalId = 'Certifier'
        id = 'chronicle:agent:Certifier'
        locationAttribute = 'SomeLocation'

        [[data.entitiesByType.nodes.wasAttributedTo]]
        [data.entitiesByType.nodes.wasAttributedTo.responsible]
        role = 'UNSPECIFIED'

        [data.entitiesByType.nodes.wasAttributedTo.responsible.agent]
        externalId = 'Certifier2'
        id = 'chronicle:agent:Certifier2'
        locationAttribute = 'AnotherLocation'
        "###);

        tokio::time::sleep(Duration::from_millis(1000)).await;

        insta::assert_toml_snapshot!(schema
                .execute(Request::new(
                    r#"
                query querySecondAgentAttribution {
                  agentById(id: {externalId: "Certifier2" }) {
                    ... on ContractorAgent {
                      id
                      externalId
                      attribution {
                        attributed {
                          role
                          entity {
                            ... on CertificateEntity {
                              externalId
                              id
                              certIdAttribute
                            }
                          }
                        }
                      }
                    }
                  }
                }
            "#,
              ))
              .await, @r###"
        [data.agentById]
        id = 'chronicle:agent:Certifier2'
        externalId = 'Certifier2'

        [[data.agentById.attribution]]
        [data.agentById.attribution.attributed]
        role = 'UNSPECIFIED'

        [data.agentById.attribution.attributed.entity]
        externalId = 'Certificate'
        id = 'chronicle:entity:Certificate'
        certIdAttribute = 'SomeCertId'
        "###);

        tokio::time::sleep(Duration::from_millis(1000)).await;

        insta::assert_toml_snapshot!(schema
                .execute(Request::new(
                    r#"
                query querySecondAgentAttributionAnotherWay {
                  agentsByType(agentType: ContractorAgent) {
                    nodes {
                      ... on ContractorAgent {
                        id
                        externalId
                        attribution {
                          attributed {
                            role
                            entity {
                              ... on CertificateEntity {
                                  externalId
                                  id
                                  certIdAttribute
                              }
                            }
                          }
                        }
                      }
                    }
                  }
                }
            "#,
                ))
                .await, @r###"
        [[data.agentsByType.nodes]]
        id = 'chronicle:agent:Certifier'
        externalId = 'Certifier'

        [[data.agentsByType.nodes.attribution]]
        [data.agentsByType.nodes.attribution.attributed]
        role = 'CERTIFIER'

        [data.agentsByType.nodes.attribution.attributed.entity]
        externalId = 'Certificate'
        id = 'chronicle:entity:Certificate'
        certIdAttribute = 'SomeCertId'

        [[data.agentsByType.nodes]]
        id = 'chronicle:agent:Certifier2'
        externalId = 'Certifier2'

        [[data.agentsByType.nodes.attribution]]
        [data.agentsByType.nodes.attribution.attributed]
        role = 'UNSPECIFIED'

        [data.agentsByType.nodes.attribution.attributed.entity]
        externalId = 'Certificate'
        id = 'chronicle:entity:Certificate'
        certIdAttribute = 'SomeCertId'
        "###);
    }

    #[tokio::test]
    async fn generated() {
        let (schema, _database) = test_schema().await;

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
              wasGeneratedBy(activity: { id: "chronicle:activity:testactivity1" },
              id: { id: "chronicle:entity:testentity1" },)
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

        tokio::time::sleep(Duration::from_millis(1000)).await;

        // query Generated relationship
        insta::assert_toml_snapshot!(schema
            .execute(Request::new(
                r#"
            query test {
                activityById(id: {id: "chronicle:activity:testactivity1" }) {
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
                wasGeneratedBy(id: { id: "chronicle:entity:testitem" },
                activity: { id: "chronicle:activity:testactivityid1" },)
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

        tokio::time::sleep(Duration::from_millis(1000)).await;

        // query Generated relationship
        insta::assert_toml_snapshot!(schema
          .execute(Request::new(
              r#"
              query testagain {
                activityById(id: {id: "chronicle:activity:testactivity1" }) {
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
        let (schema, _database) = test_schema().await;

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

        tokio::time::sleep(Duration::from_millis(1000)).await;

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

        tokio::time::sleep(Duration::from_millis(1000)).await;

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

        tokio::time::sleep(Duration::from_millis(1000)).await;

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

        tokio::time::sleep(Duration::from_millis(1000)).await;

        let from = DateTime::<Utc>::from_utc(
            NaiveDate::from_ymd_opt(1968, 9, 1)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap(),
            Utc,
        );

        for i in 1..10 {
            let activity_name = if i % 2 == 0 {
                format!("testactivity{i}")
            } else {
                format!("anothertestactivity{i}")
            };

            if (i % 2) == 0 {
                let res = schema
                    .execute(Request::new(
                        &format!(
                            r#"
                    mutation {{
                      defineItemCertifiedActivity(externalId:"{activity_name}", attributes: {{ certIdAttribute: "testcertid" }}) {{
                            context
                        }}
                    }}
                "#
                        ),
                    ))
                    .await;

                assert_eq!(res.errors, vec![]);
            } else {
                let res = schema
                    .execute(Request::new(&format!(
                        r#"
                    mutation {{
                      defineItemCodifiedActivity(externalId:"{activity_name}") {{
                            context
                        }}
                    }}
                "#
                    )))
                    .await;

                assert_eq!(res.errors, vec![]);
            }

            tokio::time::sleep(Duration::from_millis(1000)).await;
            let res = schema
                .execute(Request::new(format!(
                    r#"
            mutation {{
                used(id: {{ id: "chronicle:entity:testentity1" }}, activity: {{id: "chronicle:activity:{activity_name}" }}) {{
                    context
                }}
            }}
        "#
                )))
                .await;
            assert_eq!(res.errors, vec![]);

            tokio::time::sleep(Duration::from_millis(1000)).await;

            let res = schema
                .execute(Request::new(format!(
                    r#"
                  mutation {{
                      startActivity( time: "{}", id: {{id: "chronicle:activity:{}" }}) {{
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
                endActivity( time: "{}", id: {{ id: "chronicle:activity:{}" }}) {{
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

            tokio::time::sleep(Duration::from_millis(1000)).await;

            let agent = if i % 2 == 0 {
                "testagent1"
            } else {
                "testagent2"
            };

            let res = schema
                .execute(Request::new(format!(
                    r#"
            mutation {{
                wasAssociatedWith( role: CERTIFIER, responsible: {{ id: "chronicle:agent:{agent}" }}, activity: {{id: "chronicle:activity:{activity_name}" }}) {{
                    context
                }}
            }}
        "#
                )))
                .await;

            tokio::time::sleep(Duration::from_millis(1000)).await;

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

        // As previous but omitting forEntity and forAgent
        insta::assert_json_snapshot!(schema
          .execute(Request::new(
              r#"
              query {
              activityTimeline(
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
                forAgent: [{id: "chronicle:agent:testagent2"}],
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

        // As previous but omitting forEntity and activityTypes
        insta::assert_json_snapshot!(schema
          .execute(Request::new(
              r#"
              query {
              activityTimeline(
                forAgent: [{id: "chronicle:agent:testagent2"}],
                order: NEWEST_FIRST,
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
        let (schema, _database) = test_schema().await;

        for i in 0..100 {
            let res = schema
                .execute(Request::new(format!(
                    r#"
            mutation {{
                defineContractorAgent(externalId:"testagent{i}", attributes: {{ locationAttribute: "testattribute" }}) {{
                    context
                }}
            }}
        "#
                )))
                .await;

            assert_eq!(res.errors, vec![]);
        }

        tokio::time::sleep(Duration::from_millis(1000)).await;

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

    #[tokio::test]
    async fn subscribe_commit_notification() {
        use chronicle::async_graphql::futures_util::StreamExt;

        let (schema, _database) = test_schema().await;

        let mut stream = schema.execute_stream(Request::new(
            r#"
          subscription {
            commitNotifications {
              stage
            }
          }
          "#
            .to_string(),
        ));

        insta::assert_json_snapshot!(schema
          .execute(Request::new(
              r#"
            mutation {
              defineContractorAgent(
                externalId: "testagent"
                attributes: { locationAttribute: "location" }
              ) {
                context
              }
            }
            "#,
            ))
            .await, @r#"
          {
            "data": {
              "defineContractorAgent": {
                "context": "chronicle:agent:testagent"
              }
            }
          }
          "#);

        let res = stream.next().await.unwrap();

        insta::assert_json_snapshot!(res, @r###"
        {
          "data": {
            "commitNotifications": {
              "stage": "COMMIT"
            }
          }
        }
        "###);
    }

    async fn subscription_response(
        schema: &Schema<Query, Mutation, Subscription>,
        subscription: &str,
        mutation: &str,
    ) -> Response {
        use futures::StreamExt;

        let mut stream = schema.execute_stream(Request::new(subscription));
        assert!(schema.execute(Request::new(mutation)).await.is_ok());
        stream.next().await.unwrap()
    }

    struct SchemaPair<'a> {
        schema_allow: Schema<Query, Mutation, Subscription>,
        schema_deny: Schema<Query, Mutation, Subscription>,
        _databases: (TemporaryDatabase<'a>, TemporaryDatabase<'a>),
    }

    impl<'a> SchemaPair<'a> {
        fn new(
            (schema_allow, database_allow): (
                Schema<Query, Mutation, Subscription>,
                TemporaryDatabase<'a>,
            ),
            (schema_deny, database_deny): (
                Schema<Query, Mutation, Subscription>,
                TemporaryDatabase<'a>,
            ),
        ) -> Self {
            Self {
                schema_allow,
                schema_deny,
                _databases: (database_allow, database_deny),
            }
        }

        async fn check_responses(
            res_allow: impl Future<Output = Response>,
            res_deny: impl Future<Output = Response>,
        ) {
            use chronicle::async_graphql::Value;

            let res_allow = res_allow.await;
            let res_deny = res_deny.await;

            assert_ne!(res_allow.data, Value::Null);
            assert!(res_allow.errors.is_empty());

            assert_eq!(res_deny.data, Value::Null);
            assert!(!res_deny.errors.is_empty());
        }

        async fn check_responses_qm(&self, query: &str) {
            Self::check_responses(
                self.schema_allow.execute(Request::new(query)),
                self.schema_deny.execute(Request::new(query)),
            )
            .await;
        }

        async fn check_responses_s(&self, subscription: &str, mutation: &str) {
            Self::check_responses(
                subscription_response(&self.schema_allow, subscription, mutation),
                subscription_response(&self.schema_deny, subscription, mutation),
            )
            .await;
        }
    }

    #[tokio::test]
    async fn query_api_secured() {
        let schemas = SchemaPair::new(test_schema().await, test_schema_blocked_api().await);

        schemas
            .check_responses_qm(
                r#"
              query {
                activityTimeline(
                  activityTypes: [],
                  forEntity: [],
                  forAgent: [],
                ) {
                  edges {
                    cursor
                  }
                }
              }"#,
            )
            .await;

        schemas
            .check_responses_qm(
                r#"
              query {
                agentsByType(
                  agentType: ContractorAgent
                ) {
                  nodes {
                    ...on ContractorAgent {
                      id
                    }
                  }
                }
              }"#,
            )
            .await;

        schemas
            .check_responses_qm(
                r#"
              query {
                activitiesByType(
                  activityType: ItemCertifiedActivity
                ) {
                  nodes {
                    ...on ItemCertifiedActivity {
                      id
                    }
                  }
                }
              }"#,
            )
            .await;

        schemas
            .check_responses_qm(
                r#"
              query {
                entitiesByType(
                  entityType: CertificateEntity
                ) {
                  nodes {
                    ...on CertificateEntity {
                      id
                    }
                  }
                }
              }"#,
            )
            .await;

        schemas
            .check_responses_qm(
                r#"
              query {
                agentById(id: { id: "chronicle:agent:testagent" }) {
                  ... on ProvAgent {
                    id
                  }
                }
              }"#,
            )
            .await;

        schemas
            .check_responses_qm(
                r#"
              query {
                activityById(id: { id: "chronicle:activity:testactivity" }) {
                  ... on ProvActivity {
                    id
                  }
                }
              }"#,
            )
            .await;

        schemas
            .check_responses_qm(
                r#"
              query {
                entityById(id: { id: "chronicle:entity:testentity" }) {
                  ... on ProvEntity {
                    id
                  }
                }
              }"#,
            )
            .await;
    }

    #[tokio::test]
    async fn subscribe_api_secured() {
        let loader = CliPolicyLoader::from_embedded_policy(
            "allow_transactions",
            "allow_transactions.allow_defines",
        )
        .unwrap();
        let opa_executor = ExecutorContext::from_loader(&loader).unwrap();
        let test_schema_allow_defines = test_schema_with_opa(opa_executor).await;
        let schemas = SchemaPair::new(test_schema().await, test_schema_allow_defines);

        schemas
            .check_responses_s(
                r#"
              subscription {
                commitNotifications {
                  stage
                }
              }"#,
                r#"
              mutation {
                defineContractorAgent(
                  externalId: "testagent"
                  attributes: { locationAttribute: "location" }
                ) {
                  context
                }
              }"#,
            )
            .await;
    }

    #[tokio::test]
    async fn mutation_api_secured() {
        let schemas = SchemaPair::new(test_schema().await, test_schema_blocked_api().await);

        schemas
            .check_responses_qm(
                r#"
              mutation {
                defineAgent(
                  externalId: "test agent",
                  attributes: {}
                ) { context }
              }"#,
            )
            .await;

        schemas
            .check_responses_qm(
                r#"
              mutation {
                defineActivity(
                  externalId: "test activity",
                  attributes: {}
                ) { context }
              }"#,
            )
            .await;

        schemas
            .check_responses_qm(
                r#"
              mutation {
                defineEntity(
                  externalId: "test entity",
                  attributes: {}
                ) { context }
              }"#,
            )
            .await;

        schemas
            .check_responses_qm(
                r#"
              mutation {
                actedOnBehalfOf(
                  responsible: { externalId: "test agent 1" },
                  delegate: { externalId: "test agent 2" },
                  role: MANUFACTURER
                ) { context }
              }"#,
            )
            .await;

        schemas
            .check_responses_qm(
                r#"
              mutation {
                wasDerivedFrom(
                  generatedEntity: { externalId: "test entity 1" },
                  usedEntity: { externalId: "test entity 2" }
                ) { context }
              }"#,
            )
            .await;

        schemas
            .check_responses_qm(
                r#"
              mutation {
                wasRevisionOf(
                  generatedEntity: { externalId: "test entity 1" },
                  usedEntity: { externalId: "test entity 2" }
                ) { context }
              }"#,
            )
            .await;

        schemas
            .check_responses_qm(
                r#"
              mutation {
                hadPrimarySource(
                  generatedEntity: { externalId: "test entity 1" },
                  usedEntity: { externalId: "test entity 2" }
                ) { context }
              }
              "#,
            )
            .await;

        schemas
            .check_responses_qm(
                r#"
              mutation {
                wasQuotedFrom(
                  generatedEntity: { externalId: "test entity 1" },
                  usedEntity: { externalId: "test entity 2" }
                ) { context }
              }"#,
            )
            .await;

        schemas
            .check_responses_qm(
                r#"
              mutation {
                generateKey(
                  id: { externalId: "test agent" }
                ) { context }
              }"#,
            )
            .await;

        schemas
            .check_responses_qm(
                r#"
              mutation {
                instantActivity(
                  id: { externalId: "test activity 1" }
                ) { context }
              }"#,
            )
            .await;

        schemas
            .check_responses_qm(
                r#"
              mutation {
                startActivity(
                  id: { externalId: "test activity 2" }
                ) { context }
              }"#,
            )
            .await;

        schemas
            .check_responses_qm(
                r#"
              mutation {
                endActivity(
                  id: { externalId: "test activity 2" }
                ) { context }
              }"#,
            )
            .await;

        schemas
            .check_responses_qm(
                r#"
              mutation {
                wasAssociatedWith(
                  responsible: { externalId: "test agent" },
                  activity: { externalId: "test activity" },
                  role: MANUFACTURER
                ) { context }
              }"#,
            )
            .await;

        schemas
            .check_responses_qm(
                r#"
              mutation {
                used(
                  activity: { externalId: "test activity" },
                  id: { externalId: "test entity" }
                ) { context }
              }"#,
            )
            .await;

        schemas
            .check_responses_qm(
                r#"
              mutation {
                wasInformedBy(
                  activity: { externalId: "test activity 1" },
                  informingActivity: { externalId: "test activity 2" }
                ) { context }
              }"#,
            )
            .await;

        schemas
            .check_responses_qm(
                r#"
              mutation {
                wasGeneratedBy(
                  activity: { externalId: "test activity" },
                  id: { externalId: "test entity" }
                ) { context }
              }"#,
            )
            .await;
    }

    #[tokio::test]
    async fn mutation_generated_api_secured() {
        let schemas = SchemaPair::new(test_schema().await, test_schema_blocked_api().await);

        schemas
            .check_responses_qm(
                r#"
              mutation {
                defineContractorAgent(
                  externalId: "test agent"
                  attributes: { locationAttribute: "location" }
                ) { context }
              }"#,
            )
            .await;

        schemas
            .check_responses_qm(
                r#"
              mutation {
                defineItemCertifiedActivity(
                  externalId: "test activity",
                  attributes: { certIdAttribute: "12345" }
                ) { context }
              }"#,
            )
            .await;

        schemas
            .check_responses_qm(
                r#"
              mutation {
                defineCertificateEntity(
                  externalId: "test entity",
                  attributes: { certIdAttribute: "12345" }
                ) { context }
              }"#,
            )
            .await;
    }
}
