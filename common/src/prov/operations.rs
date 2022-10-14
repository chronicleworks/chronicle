use chrono::{DateTime, Utc};
use diesel::{
    backend::Backend,
    deserialize::FromSql,
    serialize::{Output, ToSql},
    sql_types::Integer,
    QueryId, SqlType,
};

use uuid::Uuid;

use crate::attributes::Attributes;

use super::{
    ActivityId, AgentId, AssociationId, DelegationId, EntityId, ExternalId, IdentityId,
    NamespaceId, Role,
};

#[derive(QueryId, SqlType, Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

#[derive(Serialize, Deserialize, Eq, PartialEq, Debug, Clone)]
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

#[derive(Serialize, Deserialize, Eq, PartialEq, Debug, Clone)]
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

#[derive(Serialize, Deserialize, Eq, PartialEq, Debug, Clone)]
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

#[derive(Serialize, Deserialize, Eq, PartialEq, Debug, Clone)]
pub struct RegisterKey {
    pub namespace: NamespaceId,
    pub id: AgentId,
    pub publickey: String,
}

#[derive(Serialize, Deserialize, Eq, PartialEq, Debug, Clone)]
pub struct ActivityExists {
    pub namespace: NamespaceId,
    pub external_id: ExternalId,
}

#[derive(Serialize, Deserialize, Eq, PartialEq, Debug, Clone)]
pub struct StartActivity {
    pub namespace: NamespaceId,
    pub id: ActivityId,
    pub time: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Eq, PartialEq, Debug, Clone)]
pub struct EndActivity {
    pub namespace: NamespaceId,
    pub id: ActivityId,
    pub time: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Eq, PartialEq, Debug, Clone)]
pub struct ActivityUses {
    pub namespace: NamespaceId,
    pub id: EntityId,
    pub activity: ActivityId,
}

#[derive(Serialize, Deserialize, Eq, PartialEq, Debug, Clone)]
pub struct EntityExists {
    pub namespace: NamespaceId,
    pub external_id: ExternalId,
}

#[derive(Serialize, Deserialize, Eq, PartialEq, Debug, Clone)]
pub struct WasGeneratedBy {
    pub namespace: NamespaceId,
    pub id: EntityId,
    pub activity: ActivityId,
}

#[derive(Serialize, Deserialize, Eq, PartialEq, Debug, Clone)]
pub struct EntityDerive {
    pub namespace: NamespaceId,
    pub id: EntityId,
    pub used_id: EntityId,
    pub activity_id: Option<ActivityId>,
    pub typ: Option<DerivationType>,
}

#[derive(Serialize, Deserialize, Eq, PartialEq, Debug, Clone)]
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

#[derive(Serialize, Deserialize, Eq, PartialEq, Debug, Clone)]
pub struct EntityHasEvidence {
    pub namespace: NamespaceId,
    pub id: EntityId,
    pub agent: AgentId,
    pub identityid: Option<IdentityId>,
    pub signature: Option<String>,
    pub locator: Option<String>,
    pub signature_time: Option<DateTime<Utc>>,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct WasInformedBy {
    pub namespace: NamespaceId,
    pub activity: ActivityId,
    pub informing_activity: ActivityId,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct Generated {
    pub namespace: NamespaceId,
    pub id: ActivityId,
    pub entity: EntityId,
}

#[derive(Serialize, Deserialize, Eq, PartialEq, Debug, Clone)]
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

#[derive(Serialize, Deserialize, Eq, PartialEq, Debug, Clone)]
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
    WasInformedBy(WasInformedBy),
    Generated(Generated),
}
