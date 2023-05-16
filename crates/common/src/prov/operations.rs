use chrono::{DateTime, Utc};
use diesel::{
    backend::Backend,
    deserialize::FromSql,
    serialize::{Output, ToSql},
    sql_types::Integer,
    AsExpression, QueryId, SqlType,
};

use uuid::Uuid;

use crate::attributes::Attributes;

use super::{
    ActivityId, AgentId, AssociationId, AttributionId, DelegationId, EntityId, ExternalId,
    IdentityId, NamespaceId, Role,
};

#[derive(
    QueryId,
    SqlType,
    AsExpression,
    Debug,
    Copy,
    Clone,
    PartialEq,
    Ord,
    PartialOrd,
    Eq,
    Hash,
    Serialize,
    Deserialize,
)]
#[diesel(sql_type = Integer)]
#[repr(i32)]
pub enum DerivationType {
    None,
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
            DerivationType::None => (-1).to_sql(out),
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
    fn from_sql(bytes: <DB as Backend>::RawValue<'_>) -> diesel::deserialize::Result<Self> {
        match i32::from_sql(bytes)? {
            -1 => Ok(DerivationType::None),
            1 => Ok(DerivationType::Revision),
            2 => Ok(DerivationType::Quotation),
            3 => Ok(DerivationType::PrimarySource),
            _ => Err("Unrecognized enum variant".into()),
        }
    }
}

impl TryFrom<i32> for DerivationType {
    type Error = &'static str;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            -1 => Ok(DerivationType::None),
            1 => Ok(DerivationType::Revision),
            2 => Ok(DerivationType::Quotation),
            3 => Ok(DerivationType::PrimarySource),
            _ => Err("Unrecognized enum variant when converting from 'i32'"),
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

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct CreateNamespace {
    pub id: NamespaceId,
    pub external_id: ExternalId,
    pub uuid: Uuid,
}

impl CreateNamespace {
    pub fn new(id: NamespaceId, external_id: impl AsRef<str>, uuid: Uuid) -> Self {
        Self {
            id,
            external_id: external_id.as_ref().into(),
            uuid,
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct AgentExists {
    pub namespace: NamespaceId,
    pub external_id: ExternalId,
}

impl AgentExists {
    pub fn new(namespace: NamespaceId, external_id: impl AsRef<str>) -> Self {
        Self {
            namespace,
            external_id: external_id.as_ref().into(),
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct ActsOnBehalfOf {
    pub id: DelegationId,
    pub role: Option<Role>,
    pub activity_id: Option<ActivityId>,
    pub responsible_id: AgentId,
    pub delegate_id: AgentId,
    pub namespace: NamespaceId,
}

impl ActsOnBehalfOf {
    pub fn new(
        namespace: &NamespaceId,
        responsible_id: &AgentId,
        delegate_id: &AgentId,
        activity_id: Option<&ActivityId>,
        role: Option<Role>,
    ) -> Self {
        Self {
            namespace: namespace.clone(),
            id: DelegationId::from_component_ids(
                delegate_id,
                responsible_id,
                activity_id,
                role.as_ref(),
            ),
            role,
            activity_id: activity_id.cloned(),
            responsible_id: responsible_id.clone(),
            delegate_id: delegate_id.clone(),
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct RegisterKey {
    pub namespace: NamespaceId,
    pub id: AgentId,
    pub publickey: String,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct ActivityExists {
    pub namespace: NamespaceId,
    pub external_id: ExternalId,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct StartActivity {
    pub namespace: NamespaceId,
    pub id: ActivityId,
    pub time: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct EndActivity {
    pub namespace: NamespaceId,
    pub id: ActivityId,
    pub time: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct ActivityUses {
    pub namespace: NamespaceId,
    pub id: EntityId,
    pub activity: ActivityId,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct EntityExists {
    pub namespace: NamespaceId,
    pub external_id: ExternalId,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct WasGeneratedBy {
    pub namespace: NamespaceId,
    pub id: EntityId,
    pub activity: ActivityId,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct EntityDerive {
    pub namespace: NamespaceId,
    pub id: EntityId,
    pub used_id: EntityId,
    pub activity_id: Option<ActivityId>,
    pub typ: DerivationType,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct WasAssociatedWith {
    pub id: AssociationId,
    pub role: Option<Role>,
    pub namespace: NamespaceId,
    pub activity_id: ActivityId,
    pub agent_id: AgentId,
}

impl WasAssociatedWith {
    pub fn new(
        namespace: &NamespaceId,
        activity_id: &ActivityId,
        agent_id: &AgentId,
        role: Option<Role>,
    ) -> Self {
        Self {
            id: AssociationId::from_component_ids(agent_id, activity_id, role.as_ref()),
            role,
            namespace: namespace.clone(),
            activity_id: activity_id.clone(),
            agent_id: agent_id.clone(),
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct WasAttributedTo {
    pub id: AttributionId,
    pub role: Option<Role>,
    pub namespace: NamespaceId,
    pub entity_id: EntityId,
    pub agent_id: AgentId,
}

impl WasAttributedTo {
    pub fn new(
        namespace: &NamespaceId,
        entity_id: &EntityId,
        agent_id: &AgentId,
        role: Option<Role>,
    ) -> Self {
        Self {
            id: AttributionId::from_component_ids(agent_id, entity_id, role.as_ref()),
            role,
            namespace: namespace.clone(),
            entity_id: entity_id.clone(),
            agent_id: agent_id.clone(),
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct EntityHasEvidence {
    pub namespace: NamespaceId,
    pub id: EntityId,
    pub agent: AgentId,
    pub identityid: Option<IdentityId>,
    pub signature: Option<String>,
    pub locator: Option<String>,
    pub signature_time: Option<DateTime<Utc>>,
}

impl std::fmt::Debug for EntityHasEvidence {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        fmt.debug_struct("EntityHasEvidence")
            .field("namespace", &self.namespace)
            .field("id", &self.id)
            .field("agent", &self.agent)
            .field("identityid", &self.identityid)
            .field("signature", &"***SECRET***")
            .field("locator", &self.locator)
            .field("signature_time", &self.signature_time)
            .finish()
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct WasInformedBy {
    pub namespace: NamespaceId,
    pub activity: ActivityId,
    pub informing_activity: ActivityId,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
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

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub enum ChronicleOperation {
    CreateNamespace(CreateNamespace),
    AgentExists(AgentExists),
    AgentActsOnBehalfOf(ActsOnBehalfOf),
    RegisterKey(RegisterKey),
    ActivityExists(ActivityExists),
    StartActivity(StartActivity),
    EndActivity(EndActivity),
    ActivityUses(ActivityUses),
    EntityExists(EntityExists),
    WasGeneratedBy(WasGeneratedBy),
    EntityDerive(EntityDerive),
    EntityHasEvidence(EntityHasEvidence),
    SetAttributes(SetAttributes),
    WasAssociatedWith(WasAssociatedWith),
    WasAttributedTo(WasAttributedTo),
    WasInformedBy(WasInformedBy),
}

impl ChronicleOperation {
    /// Returns a reference to the `NamespaceId` of the `ChronicleOperation`
    pub fn namespace(&self) -> &NamespaceId {
        match self {
            ChronicleOperation::ActivityExists(o) => &o.namespace,
            ChronicleOperation::AgentExists(o) => &o.namespace,
            ChronicleOperation::AgentActsOnBehalfOf(o) => &o.namespace,
            ChronicleOperation::CreateNamespace(o) => &o.id,
            ChronicleOperation::RegisterKey(o) => &o.namespace,
            ChronicleOperation::StartActivity(o) => &o.namespace,
            ChronicleOperation::EndActivity(o) => &o.namespace,
            ChronicleOperation::ActivityUses(o) => &o.namespace,
            ChronicleOperation::EntityExists(o) => &o.namespace,
            ChronicleOperation::WasGeneratedBy(o) => &o.namespace,
            ChronicleOperation::EntityDerive(o) => &o.namespace,
            ChronicleOperation::EntityHasEvidence(o) => &o.namespace,
            ChronicleOperation::SetAttributes(o) => match o {
                SetAttributes::Activity { namespace, .. } => namespace,
                SetAttributes::Agent { namespace, .. } => namespace,
                SetAttributes::Entity { namespace, .. } => namespace,
            },
            ChronicleOperation::WasAssociatedWith(o) => &o.namespace,
            ChronicleOperation::WasAttributedTo(o) => &o.namespace,
            ChronicleOperation::WasInformedBy(o) => &o.namespace,
        }
    }
}

#[cfg(test)]
mod test {
    use chrono::{TimeZone, Utc};
    use uuid::Uuid;

    use api::UuidGen;

    use crate::prov::{operations::EntityHasEvidence, AgentId, EntityId, IdentityId, NamespaceId};

    #[derive(Debug, Clone)]
    struct SameUuid;

    impl UuidGen for SameUuid {
        fn uuid() -> Uuid {
            Uuid::parse_str("5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea").unwrap()
        }
    }

    #[test]
    fn entity_has_evidence_custom_debug() {
        let namespace = NamespaceId::from_external_id("test_namespace", SameUuid::uuid());
        let id = EntityId::from_external_id("test_external_id");
        let locator = Some("test_locator".to_string());
        let agent = AgentId::from_external_id("test_agent");
        let pk = "some_public_key".to_string();
        let signature = "test_signature".to_string();
        let identity_id = IdentityId::from_external_id("test_agent", &pk);
        let signature_time = Utc.with_ymd_and_hms(2014, 7, 8, 9, 10, 11).unwrap();

        let entity_has_evidence = EntityHasEvidence {
            namespace,
            id,
            locator,
            agent,
            signature: Some(signature),
            identityid: Some(identity_id),
            signature_time: Some(signature_time),
        };

        insta::assert_debug_snapshot!(entity_has_evidence, @r###"
        EntityHasEvidence {
            namespace: NamespaceId {
                external_id: ExternalId(
                    "test_namespace",
                ),
                uuid: 5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea,
            },
            id: EntityId(
                ExternalId(
                    "test_external_id",
                ),
            ),
            agent: AgentId(
                ExternalId(
                    "test_agent",
                ),
            ),
            identityid: Some(
                IdentityId {
                    external_id: ExternalId(
                        "test_agent",
                    ),
                    public_key: "some_public_key",
                },
            ),
            signature: "***SECRET***",
            locator: Some(
                "test_locator",
            ),
            signature_time: Some(
                2014-07-08T09:10:11Z,
            ),
        }
        "###);
    }
}
