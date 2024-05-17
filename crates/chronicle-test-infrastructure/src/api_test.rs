use api::commands::{
    ActivityCommand, AgentCommand, ApiCommand, EntityCommand, ImportCommand, NamespaceCommand,
};
use chrono::{TimeZone, Utc};
use common::{
    attributes::{Attribute, Attributes},
    identity::AuthId,
    prov::{
        json_ld::ToJson,
        operations::{ChronicleOperation, DerivationType},
        ActivityId, AgentId, DomaintypeId, EntityId, NamespaceId,
    },
};
use uuid::Uuid;

use crate::substitutes::test_api;

// Creates a mock file containing JSON-LD of the ChronicleOperations
// that would be created by the given command, although not in any particular order.
fn test_create_agent_operations_import() -> assert_fs::NamedTempFile {
    let file = assert_fs::NamedTempFile::new("import.json").unwrap();
    assert_fs::prelude::FileWriteStr::write_str(
        &file,
        r#"
        [
            {
                "@id": "_:n1",
                "@type": [
                "http://chronicle.works/chronicleoperations/ns#SetAttributes"
                ],
                "http://chronicle.works/chronicleoperations/ns#agentName": [
                {
                    "@value": "testagent"
                }
                ],
                "http://chronicle.works/chronicleoperations/ns#attributes": [
                {
                    "@type": "@json",
                    "@value": {}
                }
                ],
                "http://chronicle.works/chronicleoperations/ns#domaintypeId": [
                {
                    "@value": "type"
                }
                ],
                "http://chronicle.works/chronicleoperations/ns#namespaceName": [
                {
                    "@value": "testns"
                }
                ],
                "http://chronicle.works/chronicleoperations/ns#namespaceUuid": [
                {
                    "@value": "6803790d-5891-4dfa-b773-41827d2c630b"
                }
                ]
            },
            {
                "@id": "_:n1",
                "@type": [
                "http://chronicle.works/chronicleoperations/ns#CreateNamespace"
                ],
                "http://chronicle.works/chronicleoperations/ns#namespaceName": [
                {
                    "@value": "testns"
                }
                ],
                "http://chronicle.works/chronicleoperations/ns#namespaceUuid": [
                {
                    "@value": "6803790d-5891-4dfa-b773-41827d2c630b"
                }
                ]
            },
            {
                "@id": "_:n1",
                "@type": [
                "http://chronicle.works/chronicleoperations/ns#AgentExists"
                ],
                "http://chronicle.works/chronicleoperations/ns#agentName": [
                {
                    "@value": "testagent"
                }
                ],
                "http://chronicle.works/chronicleoperations/ns#namespaceName": [
                {
                    "@value": "testns"
                }
                ],
                "http://chronicle.works/chronicleoperations/ns#namespaceUuid": [
                {
                    "@value": "6803790d-5891-4dfa-b773-41827d2c630b"
                }
                ]
            }
        ]
         "#,
    )
        .unwrap();
    file
}

#[tokio::test]
async fn test_import_operations() {
    let mut api = test_api().await;

    let file = test_create_agent_operations_import();

    let contents = std::fs::read_to_string(file.path()).unwrap();

    let json_array = serde_json::from_str::<Vec<serde_json::Value>>(&contents).unwrap();

    let mut operations = Vec::with_capacity(json_array.len());
    for value in json_array.into_iter() {
        let op = ChronicleOperation::from_json(&value)
            .await
            .expect("Failed to parse imported JSON-LD to ChronicleOperation");
        operations.push(op);
    }

    let identity = AuthId::chronicle();

    insta::assert_json_snapshot!(api
            .dispatch(ApiCommand::Import(ImportCommand {  operations: operations.clone() } ), identity.clone())
            .await
            .unwrap()
            .unwrap()
            .0
            .to_json()
            .compact_stable_order()
            .await
            .unwrap(), @r###"
 {
   "@context": "http://chronicle.works/chr/1.0/c.jsonld",
   "@graph": [
     {
       "@id": "chronicle:agent:testagent",
       "@type": "prov:Agent",
       "externalId": "testagent",
       "namespace": "chronicle:ns:testns:6803790d-5891-4dfa-b773-41827d2c630b",
       "value": {}
     },
     {
       "@id": "chronicle:ns:testns:6803790d-5891-4dfa-b773-41827d2c630b",
       "@type": "chronicle:Namespace",
       "externalId": "testns"
     }
   ]
 }
 "###);

    // Check that the operations that do not result in data changes are not submitted
    insta::assert_json_snapshot!(api
            .dispatch(ApiCommand::Import(ImportCommand {  operations } ), identity)
            .await
            .unwrap()
            .unwrap()
            .0, @r###"
 {
   "namespaces": {},
   "agents": {},
   "acted_on_behalf_of": {},
   "delegation": {},
   "entities": {},
   "derivation": {},
   "generation": {},
   "attribution": {},
   "activities": {},
   "was_informed_by": {},
   "generated": {},
   "association": {},
   "usage": {}
 }
 "###);
}

#[tokio::test]
async fn create_namespace() {
    let mut api = test_api().await;

    let identity = AuthId::chronicle();

    insta::assert_json_snapshot!(api
            .dispatch(ApiCommand::NameSpace(NamespaceCommand::Create {
                id: "testns".into(),
            }), identity)
            .await
            .unwrap()
            .unwrap()
            .0
            .to_json()
            .compact_stable_order()
            .await
            .unwrap(), @r###"
 {
   "@context": "http://chronicle.works/chr/1.0/c.jsonld"
 }
 "###);
}

#[tokio::test]
async fn create_agent() {
    let mut api = test_api().await;

    let identity = AuthId::chronicle();

    insta::assert_json_snapshot!(api.dispatch(ApiCommand::Agent(AgentCommand::Create {
            id: "testagent".into(),
            namespace: "testns".into(),
            attributes: Attributes::new(
                Some(DomaintypeId::from_external_id("test")),
                [
                    Attribute {
                        typ: "test".to_owned(),
                        value: serde_json::Value::String("test".to_owned()).into(),
                    },
                ]
                .into_iter()
                .collect(),
              ),
            }), identity)
            .await
            .unwrap()
            .unwrap()
            .0
            .to_json()
            .compact_stable_order()
            .await
            .unwrap(), @r###"
 {
   "@context": "http://chronicle.works/chr/1.0/c.jsonld",
   "@id": "chronicle:agent:testagent",
   "@type": [
     "prov:Agent",
     "chronicle:domaintype:test"
   ],
   "externalId": "testagent",
   "namespace": "chronicle:ns:testns:11111111-1111-1111-1111-111111111111",
   "value": {
     "test": "test"
   }
 }
 "###);
}

#[tokio::test]
async fn create_system_activity() {
    let mut api = test_api().await;

    let identity = AuthId::chronicle();

    insta::assert_json_snapshot!(api.dispatch(ApiCommand::Activity(ActivityCommand::Create {
            id: "testactivity".into(),
            namespace: common::prov::SYSTEM_ID.into(),
            attributes: Attributes::new(
                Some(DomaintypeId::from_external_id("test")),
                [
                    Attribute {
                        typ: "test".to_owned(),
                        value: serde_json::Value::String("test".to_owned()).into(),
                    },
                ]
                .into_iter()
                .collect(),
              ),
        }), identity)
        .await
        .unwrap()
        .unwrap()
        .0
        .to_json()
        .compact_stable_order()
        .await
        .unwrap(), @r###"
 {
   "@context": "http://chronicle.works/chr/1.0/c.jsonld",
   "@id": "chronicle:activity:testactivity",
   "@type": [
     "prov:Activity",
     "chronicle:domaintype:test"
   ],
   "externalId": "testactivity",
   "namespace": "chronicle:ns:chronicle-system:00000000-0000-0000-0000-000000000001",
   "value": {
     "test": "test"
   }
 }
 "###);
}

#[tokio::test]
async fn create_activity() {
    let mut api = test_api().await;

    let identity = AuthId::chronicle();

    insta::assert_json_snapshot!(api.dispatch(ApiCommand::Activity(ActivityCommand::Create {
            id: "testactivity".into(),
            namespace: "testns".into(),
            attributes: Attributes::new(
                Some(DomaintypeId::from_external_id("test")),
                [
                    Attribute {
                        typ: "test".to_owned(),
                        value: serde_json::Value::String("test".to_owned()).into(),
                    },
                ]
                .into_iter()
                .collect(),
              ),
        }), identity)
        .await
        .unwrap()
        .unwrap()
        .0
        .to_json()
        .compact_stable_order()
        .await
        .unwrap(), @r###"
 {
   "@context": "http://chronicle.works/chr/1.0/c.jsonld",
   "@id": "chronicle:activity:testactivity",
   "@type": [
     "prov:Activity",
     "chronicle:domaintype:test"
   ],
   "externalId": "testactivity",
   "namespace": "chronicle:ns:testns:11111111-1111-1111-1111-111111111111",
   "value": {
     "test": "test"
   }
 }
 "###);
}

#[tokio::test]
async fn start_activity() {
    let mut api = test_api().await;

    let identity = AuthId::chronicle();

    insta::assert_json_snapshot!(
        api.dispatch(ApiCommand::Agent(AgentCommand::Create {
            id: "testagent".into(),
            namespace: "testns".into(),
            attributes: Attributes::new(
                Some(DomaintypeId::from_external_id("test")),
                [
                    Attribute {
                        typ: "test".to_owned(),
                        value: serde_json::Value::String("test".to_owned()).into(),
                    },
                ]
                .into_iter()
                .collect(),
            ),
        }), identity.clone())
        .await
        .unwrap()
        .unwrap()
        .0
        .to_json()
        .compact_stable_order()
        .await
        .unwrap(), @r###"
 {
   "@context": "http://chronicle.works/chr/1.0/c.jsonld",
   "@id": "chronicle:agent:testagent",
   "@type": [
     "prov:Agent",
     "chronicle:domaintype:test"
   ],
   "externalId": "testagent",
   "namespace": "chronicle:ns:testns:11111111-1111-1111-1111-111111111111",
   "value": {
     "test": "test"
   }
 }
 "###);

    api.dispatch(
        ApiCommand::Agent(AgentCommand::UseInContext {
            id: AgentId::from_external_id("testagent"),
            namespace: "testns".into(),
        }),
        identity.clone(),
    )
        .await
        .unwrap();

    insta::assert_json_snapshot!(
        api.dispatch(ApiCommand::Activity(ActivityCommand::Start {
            id: ActivityId::from_external_id("testactivity"),
            namespace: "testns".into(),
            time: Some(Utc.with_ymd_and_hms(2014, 7, 8, 9, 10, 11).unwrap()),
            agent: None,
        }), identity)
        .await
        .unwrap()
        .unwrap()
        .0
        .to_json()
        .compact_stable_order()
        .await
        .unwrap(), @r###"
 {
   "@context": "http://chronicle.works/chr/1.0/c.jsonld",
   "@graph": [
     {
       "@id": "chronicle:activity:testactivity",
       "@type": "prov:Activity",
       "externalId": "testactivity",
       "namespace": "chronicle:ns:testns:11111111-1111-1111-1111-111111111111",
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
       "namespace": "chronicle:ns:testns:11111111-1111-1111-1111-111111111111",
       "prov:hadActivity": {
         "@id": "chronicle:activity:testactivity"
       }
     }
   ]
 }
 "###);
}

#[tokio::test]
async fn contradict_attributes() {
    let mut api = test_api().await;

    let identity = AuthId::chronicle();

    insta::assert_json_snapshot!(
        api.dispatch(ApiCommand::Agent(AgentCommand::Create {
            id: "testagent".into(),
            namespace: "testns".into(),
            attributes: Attributes::new(
                Some(DomaintypeId::from_external_id("test")),
                [
                    Attribute {
                        typ: "test".to_owned(),
                        value: serde_json::Value::String("test".to_owned()).into(),
                    },
                ]
                .into_iter()
                .collect(),
              ),
        }), identity.clone())
        .await
        .unwrap()
        .unwrap()
        .0
        .to_json()
        .compact_stable_order()
        .await
        .unwrap(), @r###"
 {
   "@context": "http://chronicle.works/chr/1.0/c.jsonld",
   "@id": "chronicle:agent:testagent",
   "@type": [
     "prov:Agent",
     "chronicle:domaintype:test"
   ],
   "externalId": "testagent",
   "namespace": "chronicle:ns:testns:11111111-1111-1111-1111-111111111111",
   "value": {
     "test": "test"
   }
 }
 "###);

    let res = api
        .dispatch(
            ApiCommand::Agent(AgentCommand::Create {
                id: "testagent".into(),
                namespace: "testns".into(),
                attributes: Attributes::new(
                    Some(DomaintypeId::from_external_id("test")),
                    [Attribute {
                        typ: "test".to_owned(),
                        value: serde_json::Value::String("test2".to_owned()).into(),
                    }]
                        .into_iter()
                        .collect(),
                ),
            }),
            identity,
        )
        .await;

    insta::assert_snapshot!(res.err().unwrap().to_string(), @r###"Contradiction: Contradiction { attribute value change: test Attribute { typ: "test", value: SerdeWrapper(String("test2")) } Attribute { typ: "test", value: SerdeWrapper(String("test")) } }"###);
}

#[tokio::test]
async fn contradict_start_time() {
    let mut api = test_api().await;

    let identity = AuthId::chronicle();

    insta::assert_json_snapshot!(
        api.dispatch(ApiCommand::Agent(AgentCommand::Create {
            id: "testagent".into(),
            namespace: "testns".into(),
            attributes: Attributes::new(
                Some(DomaintypeId::from_external_id("test")),
                [
                    Attribute {
                        typ: "test".to_owned(),
                        value: serde_json::Value::String("test".to_owned()).into(),
                    },
                ]
                .into_iter()
                .collect(),
            ),
        }), identity.clone())
        .await
        .unwrap()
        .unwrap()
        .0
        .to_json()
        .compact_stable_order()
        .await
        .unwrap(), @r###"
 {
   "@context": "http://chronicle.works/chr/1.0/c.jsonld",
   "@id": "chronicle:agent:testagent",
   "@type": [
     "prov:Agent",
     "chronicle:domaintype:test"
   ],
   "externalId": "testagent",
   "namespace": "chronicle:ns:testns:11111111-1111-1111-1111-111111111111",
   "value": {
     "test": "test"
   }
 }
 "###);

    api.dispatch(
        ApiCommand::Agent(AgentCommand::UseInContext {
            id: AgentId::from_external_id("testagent"),
            namespace: "testns".into(),
        }),
        identity.clone(),
    )
        .await
        .unwrap();

    insta::assert_json_snapshot!(
        api.dispatch(ApiCommand::Activity(ActivityCommand::Start {
            id: ActivityId::from_external_id("testactivity"),
            namespace: "testns".into(),
            time: Some(Utc.with_ymd_and_hms(2014, 7, 8, 9, 10, 11).unwrap()),
            agent: None,
        }), identity.clone())
        .await
        .unwrap()
        .unwrap()
        .0
        .to_json()
        .compact_stable_order()
        .await
        .unwrap(), @r###"
 {
   "@context": "http://chronicle.works/chr/1.0/c.jsonld",
   "@graph": [
     {
       "@id": "chronicle:activity:testactivity",
       "@type": "prov:Activity",
       "externalId": "testactivity",
       "namespace": "chronicle:ns:testns:11111111-1111-1111-1111-111111111111",
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
       "namespace": "chronicle:ns:testns:11111111-1111-1111-1111-111111111111",
       "prov:hadActivity": {
         "@id": "chronicle:activity:testactivity"
       }
     }
   ]
 }
 "###);

    // Should contradict
    let res = api
        .dispatch(
            ApiCommand::Activity(ActivityCommand::Start {
                id: ActivityId::from_external_id("testactivity"),
                namespace: "testns".into(),
                time: Some(Utc.with_ymd_and_hms(2018, 7, 8, 9, 10, 11).unwrap()),
                agent: None,
            }),
            identity,
        )
        .await;

    insta::assert_snapshot!(res.err().unwrap().to_string(), @"Contradiction: Contradiction { start date alteration: 2014-07-08T09:10:11+00:00 2018-07-08T09:10:11+00:00 }");
}

#[tokio::test]
async fn contradict_end_time() {
    let mut api = test_api().await;

    let identity = AuthId::chronicle();

    insta::assert_json_snapshot!(
        api.dispatch(ApiCommand::Agent(AgentCommand::Create {
            id: "testagent".into(),
            namespace: "testns".into(),
            attributes: Attributes::new(
                Some(DomaintypeId::from_external_id("test")),
                [
                    Attribute {
                        typ: "test".to_owned(),
                        value: serde_json::Value::String("test".to_owned()).into(),
                    },
                ]
                .into_iter()
                .collect(),
            ),
        }), identity.clone())
        .await
        .unwrap()
        .unwrap()
        .0
        .to_json()
        .compact_stable_order()
        .await
        .unwrap(), @r###"
 {
   "@context": "http://chronicle.works/chr/1.0/c.jsonld",
   "@id": "chronicle:agent:testagent",
   "@type": [
     "prov:Agent",
     "chronicle:domaintype:test"
   ],
   "externalId": "testagent",
   "namespace": "chronicle:ns:testns:11111111-1111-1111-1111-111111111111",
   "value": {
     "test": "test"
   }
 }
 "###);

    api.dispatch(
        ApiCommand::Agent(AgentCommand::UseInContext {
            id: AgentId::from_external_id("testagent"),
            namespace: "testns".into(),
        }),
        identity.clone(),
    )
        .await
        .unwrap();

    insta::assert_json_snapshot!(
        api.dispatch(ApiCommand::Activity(ActivityCommand::End {
            id: ActivityId::from_external_id("testactivity"),
            namespace: "testns".into(),
            time: Some(Utc.with_ymd_and_hms(2018, 7, 8, 9, 10, 11).unwrap()),
            agent: None,
        }), identity.clone())
        .await
        .unwrap()
        .unwrap()
        .0
        .to_json()
        .compact_stable_order()
        .await
        .unwrap(), @r###"
 {
   "@context": "http://chronicle.works/chr/1.0/c.jsonld",
   "@graph": [
     {
       "@id": "chronicle:activity:testactivity",
       "@type": "prov:Activity",
       "endTime": "2018-07-08T09:10:11+00:00",
       "externalId": "testactivity",
       "namespace": "chronicle:ns:testns:11111111-1111-1111-1111-111111111111",
       "prov:qualifiedAssociation": {
         "@id": "chronicle:association:testagent:testactivity:role="
       },
       "value": {},
       "wasAssociatedWith": [
         "chronicle:agent:testagent"
       ]
     },
     {
       "@id": "chronicle:association:testagent:testactivity:role=",
       "@type": "prov:Association",
       "agent": "chronicle:agent:testagent",
       "namespace": "chronicle:ns:testns:11111111-1111-1111-1111-111111111111",
       "prov:hadActivity": {
         "@id": "chronicle:activity:testactivity"
       }
     }
   ]
 }
 "###);

    // Should contradict
    let res = api
        .dispatch(
            ApiCommand::Activity(ActivityCommand::End {
                id: ActivityId::from_external_id("testactivity"),
                namespace: "testns".into(),
                time: Some(Utc.with_ymd_and_hms(2022, 7, 8, 9, 10, 11).unwrap()),
                agent: None,
            }),
            identity,
        )
        .await;

    insta::assert_snapshot!(res.err().unwrap().to_string(), @"Contradiction: Contradiction { end date alteration: 2018-07-08T09:10:11+00:00 2022-07-08T09:10:11+00:00 }");
}

#[tokio::test]
async fn end_activity() {
    let mut api = test_api().await;

    let identity = AuthId::chronicle();

    insta::assert_json_snapshot!(
        api.dispatch(ApiCommand::Agent(AgentCommand::Create {
            id: "testagent".into(),
            namespace: "testns".into(),
            attributes: Attributes::new(
                Some(DomaintypeId::from_external_id("test")),
                [
                    Attribute {
                        typ: "test".to_owned(),
                        value: serde_json::Value::String("test".to_owned()).into(),
                    },
                ]
                .into_iter()
                .collect(),
            ),
        }), identity.clone())
        .await
        .unwrap()
        .unwrap()
        .0
        .to_json()
        .compact_stable_order()
        .await
        .unwrap(), @r###"
 {
   "@context": "http://chronicle.works/chr/1.0/c.jsonld",
   "@id": "chronicle:agent:testagent",
   "@type": [
     "prov:Agent",
     "chronicle:domaintype:test"
   ],
   "externalId": "testagent",
   "namespace": "chronicle:ns:testns:11111111-1111-1111-1111-111111111111",
   "value": {
     "test": "test"
   }
 }
 "###);

    api.dispatch(
        ApiCommand::Agent(AgentCommand::UseInContext {
            id: AgentId::from_external_id("testagent"),
            namespace: "testns".into(),
        }),
        identity.clone(),
    )
        .await
        .unwrap();

    insta::assert_json_snapshot!(
        api.dispatch(ApiCommand::Activity(ActivityCommand::Start {
            id: ActivityId::from_external_id("testactivity"),
            namespace: "testns".into(),
            time: Some(Utc.with_ymd_and_hms(2014, 7, 8, 9, 10, 11).unwrap()),
            agent: None,
        }), identity.clone())
        .await
        .unwrap()
        .unwrap()
        .0
        .to_json()
        .compact_stable_order()
        .await
        .unwrap(), @r###"
 {
   "@context": "http://chronicle.works/chr/1.0/c.jsonld",
   "@graph": [
     {
       "@id": "chronicle:activity:testactivity",
       "@type": "prov:Activity",
       "externalId": "testactivity",
       "namespace": "chronicle:ns:testns:11111111-1111-1111-1111-111111111111",
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
       "namespace": "chronicle:ns:testns:11111111-1111-1111-1111-111111111111",
       "prov:hadActivity": {
         "@id": "chronicle:activity:testactivity"
       }
     }
   ]
 }
 "###);

    insta::assert_json_snapshot!(
        api.dispatch(ApiCommand::Activity(ActivityCommand::End {

            id: ActivityId::from_external_id("testactivity"),
            namespace: "testns".into(),
            time: Some(Utc.with_ymd_and_hms(2014, 7, 8, 9, 10, 11).unwrap()),
            agent: None,
        }), identity.clone())
        .await
        .unwrap()
        .unwrap()
        .0
        .to_json()
        .compact_stable_order()
        .await
        .unwrap(), @r###"
 {
   "@context": "http://chronicle.works/chr/1.0/c.jsonld",
   "@graph": [
     {
       "@id": "chronicle:activity:testactivity",
       "@type": "prov:Activity",
       "endTime": "2014-07-08T09:10:11+00:00",
       "externalId": "testactivity",
       "namespace": "chronicle:ns:testns:11111111-1111-1111-1111-111111111111",
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
       "namespace": "chronicle:ns:testns:11111111-1111-1111-1111-111111111111",
       "prov:hadActivity": {
         "@id": "chronicle:activity:testactivity"
       }
     }
   ]
 }
 "###);
}

#[tokio::test]
async fn activity_use() {
    let mut api = test_api().await;

    let identity = AuthId::chronicle();

    insta::assert_json_snapshot!(
        api.dispatch(ApiCommand::Agent(AgentCommand::Create {
            id: "testagent".into(),
            namespace: "testns".into(),
            attributes: Attributes::new(
                Some(DomaintypeId::from_external_id("test")),
                [
                    Attribute {
                        typ: "test".to_owned(),
                        value: serde_json::Value::String("test".to_owned()).into(),
                    },
                ]
                .into_iter()
                .collect(),
           ),
        }), identity.clone())
        .await
        .unwrap()
        .unwrap()
        .0
        .to_json()
        .compact_stable_order()
        .await
        .unwrap(), @r###"
 {
   "@context": "http://chronicle.works/chr/1.0/c.jsonld",
   "@id": "chronicle:agent:testagent",
   "@type": [
     "prov:Agent",
     "chronicle:domaintype:test"
   ],
   "externalId": "testagent",
   "namespace": "chronicle:ns:testns:11111111-1111-1111-1111-111111111111",
   "value": {
     "test": "test"
   }
 }
 "###);

    api.dispatch(
        ApiCommand::Agent(AgentCommand::UseInContext {
            id: AgentId::from_external_id("testagent"),
            namespace: "testns".into(),
        }),
        identity.clone(),
    )
        .await
        .unwrap();

    insta::assert_json_snapshot!(
        api.dispatch(ApiCommand::Activity(ActivityCommand::Create {
            id: "testactivity".into(),
            namespace: "testns".into(),
            attributes: Attributes::new(
                Some(DomaintypeId::from_external_id("test")),
                [
                    Attribute {
                        typ: "test".to_owned(),
                        value: serde_json::Value::String("test".to_owned()).into(),
                    },
                ]
                .into_iter()
                .collect(),
            ),
        }), identity.clone())
        .await
        .unwrap()
        .unwrap()
        .0
        .to_json()
        .compact_stable_order()
        .await
        .unwrap(), @r###"
 {
   "@context": "http://chronicle.works/chr/1.0/c.jsonld",
   "@id": "chronicle:activity:testactivity",
   "@type": [
     "prov:Activity",
     "chronicle:domaintype:test"
   ],
   "externalId": "testactivity",
   "namespace": "chronicle:ns:testns:11111111-1111-1111-1111-111111111111",
   "value": {
     "test": "test"
   }
 }
 "###);

    insta::assert_json_snapshot!(
        api.dispatch(ApiCommand::Activity(ActivityCommand::Use {
            id: EntityId::from_external_id("testentity"),
            namespace: "testns".into(),
            activity: ActivityId::from_external_id("testactivity"),
        }), identity.clone())
        .await
        .unwrap()
        .unwrap()
        .0
        .to_json()
        .compact_stable_order()
        .await
        .unwrap(), @r###"
 {
   "@context": "http://chronicle.works/chr/1.0/c.jsonld",
   "@graph": [
     {
       "@id": "chronicle:activity:testactivity",
       "@type": [
         "prov:Activity",
         "chronicle:domaintype:test"
       ],
       "externalId": "testactivity",
       "namespace": "chronicle:ns:testns:11111111-1111-1111-1111-111111111111",
       "used": [
         "chronicle:entity:testentity"
       ],
       "value": {
         "test": "test"
       }
     },
     {
       "@id": "chronicle:entity:testentity",
       "@type": "prov:Entity",
       "externalId": "testentity",
       "namespace": "chronicle:ns:testns:11111111-1111-1111-1111-111111111111",
       "value": {}
     }
   ]
 }
 "###);

    insta::assert_json_snapshot!(
        api.dispatch(ApiCommand::Activity(ActivityCommand::End {
            id: ActivityId::from_external_id("testactivity"),
            namespace: "testns".into(),
            time: Some(Utc.with_ymd_and_hms(2014, 7, 8, 9, 10, 11).unwrap()),
            agent: Some(AgentId::from_external_id("testagent")),
        }), identity)
        .await
        .unwrap()
        .unwrap()
        .0
        .to_json()
        .compact_stable_order()
        .await
        .unwrap(), @r###"
 {
   "@context": "http://chronicle.works/chr/1.0/c.jsonld",
   "@graph": [
     {
       "@id": "chronicle:activity:testactivity",
       "@type": [
         "prov:Activity",
         "chronicle:domaintype:test"
       ],
       "endTime": "2014-07-08T09:10:11+00:00",
       "externalId": "testactivity",
       "namespace": "chronicle:ns:testns:11111111-1111-1111-1111-111111111111",
       "prov:qualifiedAssociation": {
         "@id": "chronicle:association:testagent:testactivity:role="
       },
       "used": [
         "chronicle:entity:testentity"
       ],
       "value": {
         "test": "test"
       },
       "wasAssociatedWith": [
         "chronicle:agent:testagent"
       ]
     },
     {
       "@id": "chronicle:association:testagent:testactivity:role=",
       "@type": "prov:Association",
       "agent": "chronicle:agent:testagent",
       "namespace": "chronicle:ns:testns:11111111-1111-1111-1111-111111111111",
       "prov:hadActivity": {
         "@id": "chronicle:activity:testactivity"
       }
     }
   ]
 }
 "###);
}

#[tokio::test]
async fn activity_generate() {
    let mut api = test_api().await;

    let identity = AuthId::chronicle();

    insta::assert_json_snapshot!(
        api.dispatch(ApiCommand::Activity(ActivityCommand::Create {
            id: "testactivity".into(),
            namespace: "testns".into(),
            attributes: Attributes::new(
                Some(DomaintypeId::from_external_id("test")),
                [
                    Attribute {
                        typ: "test".to_owned(),
                        value: serde_json::Value::String("test".to_owned()).into(),
                    },
                ]
                .into_iter()
                .collect(),
            ),
        }), identity.clone())
        .await
        .unwrap()
        .unwrap()
        .0
        .to_json()
        .compact_stable_order()
        .await
        .unwrap(), @r###"
 {
   "@context": "http://chronicle.works/chr/1.0/c.jsonld",
   "@id": "chronicle:activity:testactivity",
   "@type": [
     "prov:Activity",
     "chronicle:domaintype:test"
   ],
   "externalId": "testactivity",
   "namespace": "chronicle:ns:testns:11111111-1111-1111-1111-111111111111",
   "value": {
     "test": "test"
   }
 }
 "###);

    insta::assert_json_snapshot!(
        api.dispatch(ApiCommand::Activity(ActivityCommand::Generate {
            id: EntityId::from_external_id("testentity"),
            namespace: "testns".into(),
            activity: ActivityId::from_external_id("testactivity"),
        }), identity)
        .await
        .unwrap()
        .unwrap()
        .0
        .to_json()
        .compact_stable_order()
        .await
        .unwrap(), @r###"
 {
   "@context": "http://chronicle.works/chr/1.0/c.jsonld",
   "@id": "chronicle:entity:testentity",
   "@type": "prov:Entity",
   "externalId": "testentity",
   "namespace": "chronicle:ns:testns:11111111-1111-1111-1111-111111111111",
   "value": {},
   "wasGeneratedBy": [
     "chronicle:activity:testactivity"
   ]
 }
 "###);
}

#[tokio::test]
async fn derive_entity_abstract() {
    let mut api = test_api().await;

    let identity = AuthId::chronicle();

    insta::assert_json_snapshot!(
        api.dispatch(ApiCommand::Entity(EntityCommand::Derive {
            id: EntityId::from_external_id("testgeneratedentity"),
            namespace: "testns".into(),
            activity: None,
            used_entity: EntityId::from_external_id("testusedentity"),
            derivation: DerivationType::None,
        }), identity)
        .await
        .unwrap()
        .unwrap()
        .0
        .to_json()
        .compact_stable_order()
        .await
        .unwrap(), @r###"
 {
   "@context": "http://chronicle.works/chr/1.0/c.jsonld",
   "@graph": [
     {
       "@id": "chronicle:entity:testgeneratedentity",
       "@type": "prov:Entity",
       "externalId": "testgeneratedentity",
       "namespace": "chronicle:ns:testns:11111111-1111-1111-1111-111111111111",
       "value": {},
       "wasDerivedFrom": [
         "chronicle:entity:testusedentity"
       ]
     },
     {
       "@id": "chronicle:entity:testusedentity",
       "@type": "prov:Entity",
       "externalId": "testusedentity",
       "namespace": "chronicle:ns:testns:11111111-1111-1111-1111-111111111111",
       "value": {}
     }
   ]
 }
 "###);
}

#[tokio::test]
async fn derive_entity_primary_source() {
    let mut api = test_api().await;

    let identity = AuthId::chronicle();

    insta::assert_json_snapshot!(
        api.dispatch(ApiCommand::Entity(EntityCommand::Derive {
            id: EntityId::from_external_id("testgeneratedentity"),
            namespace: "testns".into(),
            activity: None,
            derivation: DerivationType::PrimarySource,
            used_entity: EntityId::from_external_id("testusedentity"),
        }), identity)
        .await
        .unwrap()
        .unwrap()
        .0
        .to_json()
        .compact_stable_order()
        .await
        .unwrap(), @r###"
 {
   "@context": "http://chronicle.works/chr/1.0/c.jsonld",
   "@graph": [
     {
       "@id": "chronicle:entity:testgeneratedentity",
       "@type": "prov:Entity",
       "externalId": "testgeneratedentity",
       "hadPrimarySource": [
         "chronicle:entity:testusedentity"
       ],
       "namespace": "chronicle:ns:testns:11111111-1111-1111-1111-111111111111",
       "value": {}
     },
     {
       "@id": "chronicle:entity:testusedentity",
       "@type": "prov:Entity",
       "externalId": "testusedentity",
       "namespace": "chronicle:ns:testns:11111111-1111-1111-1111-111111111111",
       "value": {}
     }
   ]
 }
 "###);
}

#[tokio::test]
async fn derive_entity_revision() {
    let mut api = test_api().await;

    let identity = AuthId::chronicle();

    insta::assert_json_snapshot!(
        api.dispatch(ApiCommand::Entity(EntityCommand::Derive {
            id: EntityId::from_external_id("testgeneratedentity"),
            namespace: "testns".into(),
            activity: None,
            used_entity: EntityId::from_external_id("testusedentity"),
            derivation: DerivationType::Revision,
        }), identity)
        .await
        .unwrap()
        .unwrap()
        .0
        .to_json()
        .compact_stable_order()
        .await
        .unwrap(), @r###"
 {
   "@context": "http://chronicle.works/chr/1.0/c.jsonld",
   "@graph": [
     {
       "@id": "chronicle:entity:testgeneratedentity",
       "@type": "prov:Entity",
       "externalId": "testgeneratedentity",
       "namespace": "chronicle:ns:testns:11111111-1111-1111-1111-111111111111",
       "value": {},
       "wasRevisionOf": [
         "chronicle:entity:testusedentity"
       ]
     },
     {
       "@id": "chronicle:entity:testusedentity",
       "@type": "prov:Entity",
       "externalId": "testusedentity",
       "namespace": "chronicle:ns:testns:11111111-1111-1111-1111-111111111111",
       "value": {}
     }
   ]
 }
 "###);
}

#[tokio::test]
async fn derive_entity_quotation() {
    let mut api = test_api().await;

    let identity = AuthId::chronicle();

    insta::assert_json_snapshot!(
        api.dispatch(ApiCommand::Entity(EntityCommand::Derive {
            id: EntityId::from_external_id("testgeneratedentity"),
            namespace: "testns".into(),
            activity: None,
            used_entity: EntityId::from_external_id("testusedentity"),
            derivation: DerivationType::Quotation,
        }), identity)
        .await
        .unwrap()
        .unwrap()
        .0
        .to_json()
        .compact_stable_order()
        .await
        .unwrap(), @r###"
 {
   "@context": "http://chronicle.works/chr/1.0/c.jsonld",
   "@graph": [
     {
       "@id": "chronicle:entity:testgeneratedentity",
       "@type": "prov:Entity",
       "externalId": "testgeneratedentity",
       "namespace": "chronicle:ns:testns:11111111-1111-1111-1111-111111111111",
       "value": {},
       "wasQuotedFrom": [
         "chronicle:entity:testusedentity"
       ]
     },
     {
       "@id": "chronicle:entity:testusedentity",
       "@type": "prov:Entity",
       "externalId": "testusedentity",
       "namespace": "chronicle:ns:testns:11111111-1111-1111-1111-111111111111",
       "value": {}
     }
   ]
 }
 "###);
}
