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
                    "@type": iref::Iri::from(super::vocab::ChronicleOperations::ActivityUses).as_str(),
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
            ChronicleOperation::GenerateEntity(GenerateEntity {
                namespace,
                id,
                activity,
            }) => {
                let mut o = json::object! {
                    "@id": "_:n1",
                    "@type": iref::Iri::from(super::vocab::ChronicleOperations::GenerateEntity).as_str(),
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
            ChronicleOperation::EntityAttach(EntityAttach {
                namespace,
                identityid: _,
                id,
                locator: _,
                agent,
                signature: _,
                signature_time: _,
            }) => {
                let mut o = json::object! {
                    "@id": "_:n1",
                    "@type": iref::Iri::from(super::vocab::ChronicleOperations::EntityAttach).as_str(),
                };

                let (key, value) = namespacename(namespace);
                o.insert(&key, value).ok();

                let (key, value) = namespaceuuid(namespace);
                o.insert(&key, value).ok();

                let (key, value) = entity_name(id.name_part());
                o.insert(&key, value).ok();

                let (key, value) = agentname(agent.name_part());
                o.insert(&key, value).ok();

                o
            }
            ChronicleOperation::EntityDerive(EntityDerive {
                namespace,
                id,
                used_id,
                activity_id,
                typ,
            }) => {
                let mut o = json::object! {
                    "@id": "_:n1",
                    "@type": iref::Iri::from(super::vocab::ChronicleOperations::EntityDerive).as_str(),
                };

                let (key, value) = namespacename(namespace);
                o.insert(&key, value).ok();

                let (key, value) = namespaceuuid(namespace);
                o.insert(&key, value).ok();

                let (key, value) = entity_name(id.name_part());
                o.insert(&key, value).ok();

                let (key, value) = entity_name(used_id.name_part());
                o.insert(&key, value).ok();

                if let Some(activity) = activity_id {
                    let (key, value) = activityname(activity.name_part());
                    o.insert(&key, value).ok();
                }

                if let Some(typ) = typ {
                    let typ = match typ {
                        DerivationType::Revision => "Revision",
                        DerivationType::Quotation => "Quotation",
                        DerivationType::PrimarySource => "PrimarySource",
                    };

                    let key = iref::Iri::from(super::vocab::ChronicleOperations::DerivationType)
                        .to_string();
                    let mut value: Vec<JsonValue> = Vec::new();
                    let object = json::object! {
                        "@value": typ,
                    };
                    value.push(object);
                    o.insert(&key, value).ok();
                }

                o
            }
            ChronicleOperation::SetAttributes(SetAttributes::Entity {
                namespace,
                id,
                attributes,
            }) => {
                let mut o = json::object! {
                    "@id": "_:n1",
                    "@type": iref::Iri::from(super::vocab::ChronicleOperations::SetAttributes).as_str(),
                };

                let (key, value) = namespacename(namespace);
                o.insert(&key, value).ok();

                let (key, value) = namespaceuuid(namespace);
                o.insert(&key, value).ok();

                let (key, value) = entity_name(id.name_part());
                o.insert(&key, value).ok();

                let mut attributes_object = json::object! {
                    "@id": id.name_part().to_string(),
                    "@type": iref::Iri::from(super::vocab::ChronicleOperations::Attributes).as_str(),
                };

                if let Some(domaintypeid) = &attributes.typ {
                    let key = iref::Iri::from(super::vocab::ChronicleOperations::DomaintypeId)
                        .to_string();
                    let mut value: Vec<JsonValue> = Vec::new();
                    let object = json::object! {
                        "@value": domaintypeid.name_part().to_string(),
                    };
                    value.push(object);
                    attributes_object.insert(&key, value).ok();
                }

                let key =
                    iref::Iri::from(super::vocab::ChronicleOperations::Attributes).to_string();
                let value: Vec<JsonValue> = vec![attributes_object];

                // let mut v = Vec::new();
                // for (key, value) in attributes.attributes {
                //     v.push(json::object! { })
                // }

                // let object = json::object! {
                //     "@value": domaintypeid.name_part().to_string(),
                // };

                o.insert(&key, value).ok();
                // attributes are JSON objects in structure
                o
            }
            ChronicleOperation::SetAttributes(SetAttributes::Activity {
                namespace,
                id,
                attributes,
            }) => {
                let mut o = json::object! {
                    "@id": "_:n1",
                    "@type": iref::Iri::from(super::vocab::ChronicleOperations::SetAttributes).as_str(),
                };

                let (key, value) = namespacename(namespace);
                o.insert(&key, value).ok();

                let (key, value) = namespaceuuid(namespace);
                o.insert(&key, value).ok();

                let (key, value) = activityname(id.name_part());
                o.insert(&key, value).ok();

                let mut attributes_object = json::object! {
                    "@id": id.name_part().to_string(),
                    "@type": iref::Iri::from(super::vocab::ChronicleOperations::Attributes).as_str(),
                };

                if let Some(domaintypeid) = &attributes.typ {
                    let key = iref::Iri::from(super::vocab::ChronicleOperations::DomaintypeId)
                        .to_string();
                    let mut value: Vec<JsonValue> = Vec::new();
                    let object = json::object! {
                        "@value": domaintypeid.name_part().to_string(),
                    };
                    value.push(object);
                    attributes_object.insert(&key, value).ok();
                }

                let key =
                    iref::Iri::from(super::vocab::ChronicleOperations::Attributes).to_string();
                let value: Vec<JsonValue> = vec![attributes_object];

                // let mut v = Vec::new();
                // for (key, value) in attributes.attributes {
                //     v.push(json::object! { })
                // }

                // let object = json::object! {
                //     "@value": domaintypeid.name_part().to_string(),
                // };

                o.insert(&key, value).ok();
                // attributes are JSON objects in structure
                o
            }
            ChronicleOperation::SetAttributes(SetAttributes::Agent {
                namespace,
                id,
                attributes,
            }) => {
                let mut o = json::object! {
                    "@id": "_:n1",
                    "@type": iref::Iri::from(super::vocab::ChronicleOperations::SetAttributes).as_str(),
                };

                let (key, value) = namespacename(namespace);
                o.insert(&key, value).ok();

                let (key, value) = namespaceuuid(namespace);
                o.insert(&key, value).ok();

                let (key, value) = agentname(id.name_part());
                o.insert(&key, value).ok();

                let mut attributes_object = json::object! {
                    "@id": id.name_part().to_string(),
                    "@type": iref::Iri::from(super::vocab::ChronicleOperations::Attributes).as_str(),
                };

                if let Some(domaintypeid) = &attributes.typ {
                    let key = iref::Iri::from(super::vocab::ChronicleOperations::DomaintypeId)
                        .to_string();
                    let mut value: Vec<JsonValue> = Vec::new();
                    let object = json::object! {
                        "@value": domaintypeid.name_part().to_string(),
                    };
                    value.push(object);
                    attributes_object.insert(&key, value).ok();
                }

                let key =
                    iref::Iri::from(super::vocab::ChronicleOperations::Attributes).to_string();
                let value: Vec<JsonValue> = vec![attributes_object];

                // let mut v = Vec::new();
                // for (key, value) in attributes.attributes {
                //     v.push(json::object! { })
                // }

                // let object = json::object! {
                //     "@value": domaintypeid.name_part().to_string(),
                // };

                o.insert(&key, value).ok();
                // attributes are JSON objects in structure
                o
            }
        };
        operation.push(o);
        super::ExpandedJson(operation.into())
    }
}

#[cfg(test)]
mod test {
    use crate::prov::{ActivityId, AgentId, DomaintypeId, EntityId, IdentityId, NamespaceId};

    use super::{ChronicleOperation, DerivationType};

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
                "@value": "d1874d17-e4f2-442d-ac8b-936346da0022"
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
                "@value": "testns"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceUuid": [
              {
                "@value": "6206e90b-3245-49d0-a122-f4d0fdd660b6"
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
                "@value": "test_activity"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#AgentName": [
              {
                "@value": "test_agent"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#DelegateId": [
              {
                "@value": "test_delegate"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceName": [
              {
                "@value": "testns"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceUuid": [
              {
                "@value": "5f461b9e-308c-42da-aa47-0a1948e9ea99"
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
                "@value": "test_agent"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceName": [
              {
                "@value": "testns"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceUuid": [
              {
                "@value": "ac514336-f093-41a9-8bb3-c226184b92f2"
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
                "@value": "test_activity"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceName": [
              {
                "@value": "testns"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceUuid": [
              {
                "@value": "77ffb610-e7cf-4ede-a4d9-6a6feb392dc6"
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
                "@value": "test_activity"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#AgentName": [
              {
                "@value": "test_agent"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceName": [
              {
                "@value": "testns"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceUuid": [
              {
                "@value": "047c27d5-8c8f-4e33-a173-dde5daa0a8e4"
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
                "@value": "test_activity"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#AgentName": [
              {
                "@value": "test_agent"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceName": [
              {
                "@value": "testns"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceUuid": [
              {
                "@value": "f87efa5d-14c5-4316-b365-8041013ea1c2"
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
            "@type": "http://blockchaintp.com/chronicleoperations/ns#ActivityUses",
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
                "@value": "797f7912-8f8c-48ef-b874-71058238a0e8"
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
                "@value": "6fdb91be-d1d5-4853-9f4e-236491f43aed"
              }
            ]
          }
        ]
        "###);
    }

    #[tokio::test]
    async fn test_generate_entity() {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid::Uuid::new_v4());
        let id = EntityId::from_name("test_entity");
        let activity = ActivityId::from_name("test_activity");
        let op: ChronicleOperation =
            super::ChronicleOperation::GenerateEntity(crate::prov::operations::GenerateEntity {
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
            "@type": "http://blockchaintp.com/chronicleoperations/ns#GenerateEntity",
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
                "@value": "a1a4d2e9-9c92-4c8e-95b6-60a761f5a830"
              }
            ]
          }
        ]
        "###);
    }

    #[tokio::test]
    async fn test_entity_attach() {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid::Uuid::new_v4());
        let id = EntityId::from_name("test_entity");
        let agent = crate::prov::AgentId::from_name("test_agent");
        let public_key =
            "02197db854d8c6a488d4a0ef3ef1fcb0c06d66478fae9e87a237172cf6f6f7de23".to_string();
        let time = chrono::DateTime::<chrono::Utc>::from_utc(
            chrono::NaiveDateTime::from_timestamp(61, 0),
            chrono::Utc,
        );
        let op: ChronicleOperation =
            super::ChronicleOperation::EntityAttach(crate::prov::operations::EntityAttach {
                namespace,
                identityid: IdentityId::from_name("name", public_key),
                id,
                locator: Some(String::from("nothing")),
                agent,
                signature: String::from("string"),
                signature_time: time,
            });

        let x = op.to_json();
        let x: serde_json::Value = serde_json::from_str(&x.0.to_string()).unwrap();
        insta::assert_json_snapshot!(&x, @r###"
        [
          {
            "@id": "_:n1",
            "@type": "http://blockchaintp.com/chronicleoperations/ns#EntityAttach",
            "http://blockchaintp.com/chronicleoperations/ns#AgentName": [
              {
                "@value": "test_agent"
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
                "@value": "30ddb264-b38f-4bd2-bbdb-924af861771a"
              }
            ]
          }
        ]
        "###);
    }

    #[tokio::test]
    async fn test_entity_derive() {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid::Uuid::new_v4());
        let id = EntityId::from_name("test_entity");
        let used_id = EntityId::from_name("test_used_entity");
        let activity_id = Some(ActivityId::from_name("test_activity"));
        let typ = Some(DerivationType::Revision);
        let op: ChronicleOperation =
            super::ChronicleOperation::EntityDerive(crate::prov::operations::EntityDerive {
                namespace,
                id,
                used_id,
                activity_id,
                typ,
            });

        let x = op.to_json();
        let x: serde_json::Value = serde_json::from_str(&x.0.to_string()).unwrap();
        insta::assert_json_snapshot!(&x, @r###"
        [
          {
            "@id": "_:n1",
            "@type": "http://blockchaintp.com/chronicleoperations/ns#EntityDerive",
            "http://blockchaintp.com/chronicleoperations/ns#ActivityName": [
              {
                "@value": "test_activity"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#DerivationType": [
              {
                "@value": "Revision"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#EntityName": [
              {
                "@value": "test_used_entity"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceName": [
              {
                "@value": "testns"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceUuid": [
              {
                "@value": "636790cc-8909-48b3-ac64-ab12bac5956f"
              }
            ]
          }
        ]
        "###);
    }

    #[tokio::test]
    async fn test_set_attributes_entity() {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid::Uuid::new_v4());
        let id = EntityId::from_name("test_entity");
        let domain = DomaintypeId::from_name("test_domain");
        let attributes = crate::attributes::Attributes {
            typ: Some(domain),
            attributes: std::collections::HashMap::new(),
        };
        let op: ChronicleOperation = super::ChronicleOperation::SetAttributes(
            crate::prov::operations::SetAttributes::Entity {
                namespace,
                id,
                attributes,
            },
        );
        let x = op.to_json();
        let x: serde_json::Value = serde_json::from_str(&x.0.to_string()).unwrap();
        insta::assert_json_snapshot!(&x, @r###"
        [
          {
            "@id": "_:n1",
            "@type": "http://blockchaintp.com/chronicleoperations/ns#SetAttributes",
            "http://blockchaintp.com/chronicleoperations/ns#Attributes": [
              {
                "@id": "test_entity",
                "@type": "http://blockchaintp.com/chronicleoperations/ns#Attributes",
                "http://blockchaintp.com/chronicleoperations/ns#DomaintypeId": [
                  {
                    "@value": "test_domain"
                  }
                ]
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
                "@value": "8d59fa94-ed1c-49dc-abf9-51da2053ee3d"
              }
            ]
          }
        ]
        "###);
    }
}
