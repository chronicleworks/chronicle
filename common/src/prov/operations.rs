use chrono::{DateTime, Utc};
use diesel::{
    backend::Backend,
    deserialize::FromSql,
    serialize::{Output, ToSql},
    sql_types::Integer,
    QueryId, SqlType,
};
use json::JsonValue;
// use iref::Iri;
// use json::object;
use uuid::Uuid;

use crate::attributes::Attributes;

use super::{
    ActivityId,
    AgentId,
    EntityId,
    IdentityId,
    Name,
    NamePart,
    NamespaceId,
    UuidPart,
    // vocab::ChronicleOperations,
    // ExpandedJson
};

#[derive(QueryId, SqlType, Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[diesel(sql_type = Integer)]
#[repr(i32)]
pub enum DerivationType {
    Revision,
    Quotation,
    PrimarySource,
}

impl<DB> ToSql<Integer, DB> for DerivationType
where
    DB: Backend,
    i32: ToSql<Integer, DB>,
{
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, DB>) -> diesel::serialize::Result {
        match self {
            DerivationType::Revision => 1.to_sql(out),
            DerivationType::Quotation => 2.to_sql(out),
            DerivationType::PrimarySource => 3.to_sql(out),
        }
    }
}

impl<DB> FromSql<Integer, DB> for DerivationType
where
    DB: Backend,
    i32: FromSql<Integer, DB>,
{
    fn from_sql(bytes: diesel::backend::RawValue<'_, DB>) -> diesel::deserialize::Result<Self> {
        match i32::from_sql(bytes)? {
            1 => Ok(DerivationType::Revision),
            2 => Ok(DerivationType::Quotation),
            3 => Ok(DerivationType::PrimarySource),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}

impl DerivationType {
    pub fn revision() -> Self {
        Self::Revision
    }

    pub fn quotation() -> Self {
        Self::Quotation
    }

    pub fn primary_source() -> Self {
        Self::PrimarySource
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct CreateNamespace {
    pub id: NamespaceId,
    pub name: Name,
    pub uuid: Uuid,
}

impl CreateNamespace {
    pub fn new(id: NamespaceId, name: impl AsRef<str>, uuid: Uuid) -> Self {
        Self {
            id,
            name: name.as_ref().into(),
            uuid,
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct CreateAgent {
    pub namespace: NamespaceId,
    pub name: Name,
}

impl CreateAgent {
    pub fn new(namespace: NamespaceId, name: impl AsRef<str>) -> Self {
        Self {
            namespace,
            name: name.as_ref().into(),
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct ActsOnBehalfOf {
    pub namespace: NamespaceId,
    pub id: AgentId,
    pub delegate_id: AgentId,
    pub activity_id: Option<ActivityId>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct RegisterKey {
    pub namespace: NamespaceId,
    pub id: AgentId,
    pub publickey: String,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct CreateActivity {
    pub namespace: NamespaceId,
    pub name: Name,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct StartActivity {
    pub namespace: NamespaceId,
    pub id: ActivityId,
    pub agent: AgentId,
    pub time: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct EndActivity {
    pub namespace: NamespaceId,
    pub id: ActivityId,
    pub agent: AgentId,
    pub time: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct ActivityUses {
    pub namespace: NamespaceId,
    pub id: EntityId,
    pub activity: ActivityId,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct CreateEntity {
    pub namespace: NamespaceId,
    pub name: Name,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct GenerateEntity {
    pub namespace: NamespaceId,
    pub id: EntityId,
    pub activity: ActivityId,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct EntityDerive {
    pub namespace: NamespaceId,
    pub id: EntityId,
    pub used_id: EntityId,
    pub activity_id: Option<ActivityId>,
    pub typ: Option<DerivationType>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct EntityAttach {
    pub namespace: NamespaceId,
    pub id: EntityId,
    pub agent: AgentId,
    pub identityid: IdentityId,
    pub signature: String,
    pub locator: Option<String>,
    pub signature_time: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub enum SetAttributes {
    Entity {
        namespace: NamespaceId,
        id: EntityId,
        attributes: Attributes,
    },
    Agent {
        namespace: NamespaceId,
        id: AgentId,
        attributes: Attributes,
    },
    Activity {
        namespace: NamespaceId,
        id: ActivityId,
        attributes: Attributes,
    },
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub enum ChronicleOperation {
    CreateNamespace(CreateNamespace),
    CreateAgent(CreateAgent),
    AgentActsOnBehalfOf(ActsOnBehalfOf),
    RegisterKey(RegisterKey),
    CreateActivity(CreateActivity),
    StartActivity(StartActivity),
    EndActivity(EndActivity),
    ActivityUses(ActivityUses),
    CreateEntity(CreateEntity),
    GenerateEntity(GenerateEntity),
    EntityDerive(EntityDerive),
    EntityAttach(EntityAttach),
    SetAttributes(SetAttributes),
}

fn namespacename(id: &NamespaceId) -> (String, Vec<JsonValue>) {
    let key = iref::Iri::from(super::vocab::ChronicleOperations::NamespaceName).to_string();
    let mut value: Vec<JsonValue> = Vec::new();
    let object = json::object! {
        "@value": id.name_part().to_string()
    };
    value.push(object);
    (key, value)
}

fn namespaceuuid(id: &NamespaceId) -> (String, Vec<JsonValue>) {
    let key = iref::Iri::from(super::vocab::ChronicleOperations::NamespaceUuid).to_string();
    let mut value: Vec<JsonValue> = Vec::new();
    let object = json::object! {
        "@value": id.uuid_part().to_string()
    };
    value.push(object);
    (key, value)
}

fn agentname(name: &Name) -> (String, Vec<JsonValue>) {
    let key = iref::Iri::from(super::vocab::ChronicleOperations::AgentName).to_string();
    let mut value: Vec<JsonValue> = Vec::new();
    let object = json::object! {
        "@value": name.to_string()
    };
    value.push(object);
    (key, value)
}

fn activityname(name: &Name) -> (String, Vec<JsonValue>) {
    let key = iref::Iri::from(super::vocab::ChronicleOperations::ActivityName).to_string();
    let mut value: Vec<JsonValue> = Vec::new();
    let object = json::object! {
        "@value": name.to_string()
    };
    value.push(object);
    (key, value)
}

fn operation_time(time: &DateTime<Utc>) -> (String, Vec<JsonValue>) {
    let key = iref::Iri::from(super::vocab::ChronicleOperations::StartActivityTime).to_string();
    let mut value: Vec<JsonValue> = Vec::new();
    let object = json::object! {
        "@value": time.to_rfc3339()
    };
    value.push(object);
    (key, value)
}

fn entity_name(id: &Name) -> (String, Vec<JsonValue>) {
    let key = iref::Iri::from(super::vocab::ChronicleOperations::EntityName).to_string();
    let mut value: Vec<JsonValue> = Vec::new();
    let object = json::object! {
        "@value": id.to_string()
    };
    value.push(object);
    (key, value)
}

impl ChronicleOperation {
    pub fn to_json(&self) -> super::ExpandedJson {
        let mut operation: Vec<JsonValue> = Vec::new();

        let o = match self {
            ChronicleOperation::CreateNamespace(CreateNamespace { id, .. }) => {
                let mut o = json::object! {
                    "@id": "_:n1",
                    "@type": iref::Iri::from(super::vocab::ChronicleOperations::CreateNamespace).as_str(),
                };

                let (key, value) = namespacename(id);
                o.insert(&key, value).ok();

                let (key, value) = namespaceuuid(id);
                o.insert(&key, value).ok();

                o
            }
            ChronicleOperation::CreateAgent(CreateAgent { namespace, name }) => {
                let mut o = json::object! {
                    "@id": "_:n1",
                    "@type": iref::Iri::from(super::vocab::ChronicleOperations::CreateAgent).as_str(),
                };

                let (key, value) = namespacename(namespace);
                o.insert(&key, value).ok();

                let (key, value) = namespaceuuid(namespace);
                o.insert(&key, value).ok();

                let (key, value) = agentname(name);
                o.insert(&key, value).ok();

                o
            }
            ChronicleOperation::AgentActsOnBehalfOf(ActsOnBehalfOf {
                namespace,
                id,
                delegate_id,
                activity_id,
            }) => {
                let mut o = json::object! {
                    "@id": "_:n1",
                    "@type": iref::Iri::from(super::vocab::ChronicleOperations::AgentActsOnBehalfOf).as_str(),
                };

                let (key, value) = namespacename(namespace);
                o.insert(&key, value).ok();

                let (key, value) = namespaceuuid(namespace);
                o.insert(&key, value).ok();

                let (key, value) = agentname(id.name_part());
                o.insert(&key, value).ok();

                let key =
                    iref::Iri::from(super::vocab::ChronicleOperations::DelegateId).to_string();
                let mut value: Vec<JsonValue> = Vec::new();
                let object = json::object! {
                    "@value": delegate_id.name_part().to_string()
                };
                value.push(object);
                o.insert(&key, value).ok();

                if let Some(activity_id) = activity_id {
                    let (key, value) = activityname(activity_id.name_part());
                    o.insert(&key, value).ok();
                }

                o
            }
            ChronicleOperation::RegisterKey(RegisterKey {
                namespace,
                id,
                publickey,
            }) => {
                let mut o = json::object! {
                    "@id": "_:n1",
                    "@type": iref::Iri::from(super::vocab::ChronicleOperations::RegisterKey).as_str(),
                };

                let (key, value) = namespacename(namespace);
                o.insert(&key, value).ok();

                let (key, value) = namespaceuuid(namespace);
                o.insert(&key, value).ok();

                let (key, value) = agentname(id.name_part());
                o.insert(&key, value).ok();

                let key = iref::Iri::from(super::vocab::ChronicleOperations::PublicKey).to_string();
                let mut value: Vec<JsonValue> = Vec::new();
                let object = json::object! {
                    "@value": publickey.to_owned()
                };
                value.push(object);
                o.insert(&key, value).ok();

                o
            }
            ChronicleOperation::CreateActivity(CreateActivity { namespace, name }) => {
                let mut o = json::object! {
                    "@id": "_:n1",
                    "@type": iref::Iri::from(super::vocab::ChronicleOperations::CreateActivity).as_str(),
                };

                let (key, value) = namespacename(namespace);
                o.insert(&key, value).ok();

                let (key, value) = namespaceuuid(namespace);
                o.insert(&key, value).ok();

                let (key, value) = activityname(name);
                o.insert(&key, value).ok();

                o
            }
            ChronicleOperation::StartActivity(StartActivity {
                namespace,
                id,
                agent,
                time,
            }) => {
                let mut o = json::object! {
                    "@id": "_:n1",
                    "@type": iref::Iri::from(super::vocab::ChronicleOperations::StartActivity).as_str(),
                };

                let (key, value) = namespacename(namespace);
                o.insert(&key, value).ok();

                let (key, value) = namespaceuuid(namespace);
                o.insert(&key, value).ok();

                let (key, value) = agentname(agent.name_part());
                o.insert(&key, value).ok();

                let (key, value) = activityname(id.name_part());
                o.insert(&key, value).ok();

                let (key, value) = operation_time(time);
                o.insert(&key, value).ok();

                o
            }
            ChronicleOperation::EndActivity(EndActivity {
                namespace,
                id,
                agent,
                time,
            }) => {
                let mut o = json::object! {
                    "@id": "_:n1",
                    "@type": iref::Iri::from(super::vocab::ChronicleOperations::EndActivity).as_str(),
                };

                let (key, value) = namespacename(namespace);
                o.insert(&key, value).ok();

                let (key, value) = namespaceuuid(namespace);
                o.insert(&key, value).ok();

                let (key, value) = activityname(id.name_part());
                o.insert(&key, value).ok();

                let (key, value) = agentname(agent.name_part());
                o.insert(&key, value).ok();

                let (key, value) = operation_time(time);
                o.insert(&key, value).ok();

                o
            }
            ChronicleOperation::ActivityUses(ActivityUses {
                namespace,
                id,
                activity,
            }) => {
                let mut o = json::object! {
                    "@id": "_:n1",
                    "@type": iref::Iri::from(super::vocab::ChronicleOperations::EndActivity).as_str(),
                };

                let (key, value) = namespacename(namespace);
                o.insert(&key, value).ok();

                let (key, value) = namespaceuuid(namespace);
                o.insert(&key, value).ok();

                let (key, value) = entity_name(id.name_part());
                o.insert(&key, value).ok();

                let (key, value) = activityname(activity.name_part());
                o.insert(&key, value).ok();

                o
            }
            ChronicleOperation::CreateEntity(CreateEntity { namespace, name }) => {
                let mut o = json::object! {
                    "@id": "_:n1",
                    "@type": iref::Iri::from(super::vocab::ChronicleOperations::CreateEntity).as_str(),
                };

                let (key, value) = namespacename(namespace);
                o.insert(&key, value).ok();

                let (key, value) = namespaceuuid(namespace);
                o.insert(&key, value).ok();

                let (key, value) = entity_name(name);
                o.insert(&key, value).ok();

                o
            }
            // ChronicleOperation::GenerateEntity(GenerateEntity {
            //     namespace,
            //     id,
            //     activity,
            // }) => unimplemented!(),
            // ChronicleOperation::EntityAttach(EntityAttach {
            //     // from proptest.rs
            //     // EntityAttach {
            //     //     namespace,
            //     //     id,
            //     //     locator,
            //     //     agent,
            //     //     signature,
            //     //     identityid,
            //     //     signature_time
            //     // }
            //     namespace,
            //     identityid: _,
            //     id,
            //     locator: _,
            //     agent,
            //     signature: _,
            //     signature_time: _,
            // }) => unimplemented!(),
            // ChronicleOperation::EntityDerive(EntityDerive {
            //     namespace,
            //     id,
            //     used_id,
            //     activity_id,
            //     typ,
            // }) => unimplemented!(),
            // ChronicleOperation::SetAttributes(SetAttributes::Entity {
            //     namespace,
            //     id,
            //     attributes,
            // }) => unimplemented!(),
            // ChronicleOperation::SetAttributes(SetAttributes::Activity {
            //     namespace,
            //     id,
            //     attributes,
            // }) => unimplemented!(),
            // ChronicleOperation::SetAttributes(SetAttributes::Agent {
            //     namespace,
            //     id,
            //     attributes,
            // }) => unimplemented!(),
            _ => unreachable!(),
        };
        operation.push(o);
        super::ExpandedJson(operation.into())
    }
}

#[cfg(test)]
mod test {
    use crate::prov::{ActivityId, AgentId, EntityId, NamespaceId};

    use super::ChronicleOperation;

    #[tokio::test]
    async fn test_create_namespace() {
        let name = "testns";
        let uuid = uuid::Uuid::new_v4();
        let id = NamespaceId::from_name(name, uuid);

        let op =
            super::ChronicleOperation::CreateNamespace(super::CreateNamespace::new(id, name, uuid));
        let x = op.to_json();
        let x: serde_json::Value = serde_json::from_str(&x.0.to_string()).unwrap();
        insta::assert_json_snapshot!(&x, @r###"
        [
          {
            "@id": "_:n1",
            "@type": "http://blockchaintp.com/chronicleoperations/ns#CreateNamespace",
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceName": [
              {
                "@value": "testns"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceUuid": [
              {
                "@value": "acd922b4-22cb-43f5-bf3c-829e4ea383c1"
              }
            ]
          }
        ]
        "###);
    }

    #[tokio::test]
    async fn test_create_agent() {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid::Uuid::new_v4());
        let name: crate::prov::Name =
            crate::prov::NamePart::name_part(&crate::prov::AgentId::from_name("funnyagent"))
                .clone();
        let op: ChronicleOperation =
            super::ChronicleOperation::CreateAgent(crate::prov::operations::CreateAgent {
                namespace,
                name,
            });
        let x = op.to_json();
        let x: serde_json::Value = serde_json::from_str(&x.0.to_string()).unwrap();
        insta::assert_json_snapshot!(&x, @r###"
        [
          {
            "@id": "_:n1",
            "@type": "http://blockchaintp.com/chronicleoperations/ns#CreateAgent",
            "http://blockchaintp.com/chronicleoperations/ns#AgentName": [
              {
                "@value": "funnyagent"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceName": [
              {
                "@value": "hilariousnamespacename"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceUuid": [
              {
                "@value": "3dd79c6d-9be8-4184-85e4-86c6854bd389"
              }
            ]
          }
        ]
        "###);
    }

    #[tokio::test]
    async fn test_agent_acts_on_behalf_of() {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid::Uuid::new_v4());
        let id = crate::prov::AgentId::from_name("test_agent");
        let delegate_id = AgentId::from_name("test_delegate");
        let activity_id = Some(ActivityId::from_name("test_activity"));

        let op: ChronicleOperation = super::ChronicleOperation::AgentActsOnBehalfOf(
            crate::prov::operations::ActsOnBehalfOf {
                namespace,
                id,
                delegate_id,
                activity_id,
            },
        );
        let x = op.to_json();
        let x: serde_json::Value = serde_json::from_str(&x.0.to_string()).unwrap();
        insta::assert_json_snapshot!(&x, @r###"
        [
          {
            "@id": "_:n1",
            "@type": "http://blockchaintp.com/chronicleoperations/ns#AgentActsOnBehalfOf",
            "http://blockchaintp.com/chronicleoperations/ns#ActivityName": [
              {
                "@value": "activity"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#AgentName": [
              {
                "@value": "funnyagent"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#DelegateId": [
              {
                "@value": "delegatename"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceName": [
              {
                "@value": "hilariousnamespacename"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceUuid": [
              {
                "@value": "be5c1b8d-5375-446b-a94d-e04a3ab161ac"
              }
            ]
          }
        ]
        "###);
    }

    #[tokio::test]
    async fn test_register_key() {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid::Uuid::new_v4());
        let id = crate::prov::AgentId::from_name("test_agent");
        let publickey =
            "02197db854d8c6a488d4a0ef3ef1fcb0c06d66478fae9e87a237172cf6f6f7de23".to_string();

        let op: ChronicleOperation =
            super::ChronicleOperation::RegisterKey(crate::prov::operations::RegisterKey {
                namespace,
                id,
                publickey,
            });

        let x = op.to_json();
        let x: serde_json::Value = serde_json::from_str(&x.0.to_string()).unwrap();
        insta::assert_json_snapshot!(&x, @r###"
        [
          {
            "@id": "_:n1",
            "@type": "http://blockchaintp.com/chronicleoperations/ns#RegisterKey",
            "http://blockchaintp.com/chronicleoperations/ns#AgentName": [
              {
                "@value": "funnyagent"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceName": [
              {
                "@value": "hilariousnamespacename"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceUuid": [
              {
                "@value": "0fb4fa0c-75a7-4205-b9cb-458708d98d09"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#PublicKey": [
              {
                "@value": "02197db854d8c6a488d4a0ef3ef1fcb0c06d66478fae9e87a237172cf6f6f7de23"
              }
            ]
          }
        ]
        "###);
    }

    #[tokio::test]
    async fn test_create_activity() {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid::Uuid::new_v4());
        let name =
            crate::prov::NamePart::name_part(&ActivityId::from_name("test_activity")).to_owned();

        let op: ChronicleOperation =
            super::ChronicleOperation::CreateActivity(crate::prov::operations::CreateActivity {
                namespace,
                name,
            });

        let x = op.to_json();
        let x: serde_json::Value = serde_json::from_str(&x.0.to_string()).unwrap();
        insta::assert_json_snapshot!(&x, @r###"
        [
          {
            "@id": "_:n1",
            "@type": "http://blockchaintp.com/chronicleoperations/ns#CreateActivity",
            "http://blockchaintp.com/chronicleoperations/ns#ActivityName": [
              {
                "@value": "activity_name"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceName": [
              {
                "@value": "hilariousnamespacename"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceUuid": [
              {
                "@value": "1b91974a-1279-4225-9454-3cfafc7723b6"
              }
            ]
          }
        ]
        "###);
    }

    #[tokio::test]
    async fn start_activity() {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid::Uuid::new_v4());
        let id = ActivityId::from_name("test_activity");
        let agent = crate::prov::AgentId::from_name("test_agent");
        let time = chrono::DateTime::<chrono::Utc>::from_utc(
            chrono::NaiveDateTime::from_timestamp(61, 0),
            chrono::Utc,
        );
        let op: ChronicleOperation =
            super::ChronicleOperation::StartActivity(crate::prov::operations::StartActivity {
                namespace,
                id,
                agent,
                time,
            });

        let x = op.to_json();
        let x: serde_json::Value = serde_json::from_str(&x.0.to_string()).unwrap();
        insta::assert_json_snapshot!(&x, @r###"
        [
          {
            "@id": "_:n1",
            "@type": "http://blockchaintp.com/chronicleoperations/ns#StartActivity",
            "http://blockchaintp.com/chronicleoperations/ns#ActivityName": [
              {
                "@value": "activity"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#AgentName": [
              {
                "@value": "funnyagent"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceName": [
              {
                "@value": "hilariousnamespacename"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceUuid": [
              {
                "@value": "efa01588-b2cf-4355-aa8e-502dea7b53ab"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#StartActivityTime": [
              {
                "@value": "1970-01-01T00:01:01+00:00"
              }
            ]
          }
        ]
        "###);
    }

    #[tokio::test]
    async fn test_end_activity() {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid::Uuid::new_v4());
        let id = ActivityId::from_name("test_activity");
        let agent = crate::prov::AgentId::from_name("test_agent");
        let time = chrono::DateTime::<chrono::Utc>::from_utc(
            chrono::NaiveDateTime::from_timestamp(61, 0),
            chrono::Utc,
        );
        let op: ChronicleOperation =
            super::ChronicleOperation::EndActivity(crate::prov::operations::EndActivity {
                namespace,
                id,
                agent,
                time,
            });

        let x = op.to_json();
        let x: serde_json::Value = serde_json::from_str(&x.0.to_string()).unwrap();
        insta::assert_json_snapshot!(&x, @r###"
        [
          {
            "@id": "_:n1",
            "@type": "http://blockchaintp.com/chronicleoperations/ns#EndActivity",
            "http://blockchaintp.com/chronicleoperations/ns#ActivityName": [
              {
                "@value": "activity"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#AgentName": [
              {
                "@value": "funnyagent"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceName": [
              {
                "@value": "hilariousnamespacename"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceUuid": [
              {
                "@value": "a9b54e25-f435-4625-b711-3add1ca01cef"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#StartActivityTime": [
              {
                "@value": "1970-01-01T00:01:01+00:00"
              }
            ]
          }
        ]
        "###);
    }

    #[tokio::test]
    async fn test_activity_uses() {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid::Uuid::new_v4());
        let id = EntityId::from_name("test_entity");
        let activity = ActivityId::from_name("test_activity");
        let op: ChronicleOperation =
            super::ChronicleOperation::ActivityUses(crate::prov::operations::ActivityUses {
                namespace,
                id,
                activity,
            });

        let x = op.to_json();
        let x: serde_json::Value = serde_json::from_str(&x.0.to_string()).unwrap();
        insta::assert_json_snapshot!(&x, @r###"
        [
          {
            "@id": "_:n1",
            "@type": "http://blockchaintp.com/chronicleoperations/ns#EndActivity",
            "http://blockchaintp.com/chronicleoperations/ns#ActivityName": [
              {
                "@value": "test_activity"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#EntityName": [
              {
                "@value": "test_entity"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceName": [
              {
                "@value": "testns"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceUuid": [
              {
                "@value": "b0035217-f951-4a45-a387-1add36fa1c59"
              }
            ]
          }
        ]
        "###);
    }

    #[tokio::test]
    async fn test_create_entity() {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid::Uuid::new_v4());
        let id = crate::prov::NamePart::name_part(&EntityId::from_name("test_entity")).to_owned();
        let op: ChronicleOperation =
            super::ChronicleOperation::CreateEntity(crate::prov::operations::CreateEntity {
                namespace,
                name: id,
            });

        let x = op.to_json();
        let x: serde_json::Value = serde_json::from_str(&x.0.to_string()).unwrap();
        insta::assert_json_snapshot!(&x, @r###"
        [
          {
            "@id": "_:n1",
            "@type": "http://blockchaintp.com/chronicleoperations/ns#CreateEntity",
            "http://blockchaintp.com/chronicleoperations/ns#EntityName": [
              {
                "@value": "test_entity"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceName": [
              {
                "@value": "testns"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceUuid": [
              {
                "@value": "70a00108-20f2-450c-b327-00d040fc113f"
              }
            ]
          }
        ]
        "###);
    }
}
