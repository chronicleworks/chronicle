use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use diesel::{
    backend::Backend,
    deserialize::FromSql,
    serialize::{Output, ToSql},
    sql_types::Integer,
    AsExpression, QueryId, SqlType,
};

use parity_scale_codec::{Decode, Encode, Error, Input};
use scale_info::{build::Fields, Path, Type, TypeInfo};
use uuid::Uuid;

use crate::attributes::Attributes;

use super::{
    ActivityId, AgentId, AssociationId, AttributionId, DelegationId, EntityId, ExternalId,
    NamespaceId, Role, UuidWrapper,
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
    Encode,
    Decode,
    TypeInfo,
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

#[derive(Serialize, Deserialize, Encode, Decode, TypeInfo, PartialEq, Eq, Debug, Clone)]
pub struct CreateNamespace {
    pub id: NamespaceId,
    pub external_id: ExternalId,
    pub uuid: UuidWrapper,
}

impl CreateNamespace {
    pub fn new(id: NamespaceId, external_id: impl AsRef<str>, uuid: Uuid) -> Self {
        Self {
            id,
            external_id: external_id.as_ref().into(),
            uuid: uuid.into(),
        }
    }
}

#[derive(Serialize, Deserialize, Encode, Decode, TypeInfo, PartialEq, Eq, Debug, Clone)]
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

#[derive(Serialize, Deserialize, Encode, Decode, TypeInfo, PartialEq, Eq, Debug, Clone)]
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

#[derive(Serialize, Deserialize, Encode, Decode, TypeInfo, PartialEq, Eq, Debug, Clone)]
pub struct ActivityExists {
    pub namespace: NamespaceId,
    pub external_id: ExternalId,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub struct TimeWrapper(pub DateTime<Utc>);

impl TimeWrapper {
    pub fn to_rfc3339(&self) -> String {
        self.0.to_rfc3339()
    }
}

impl std::fmt::Display for TimeWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.to_rfc3339())
    }
}

impl From<DateTime<Utc>> for TimeWrapper {
    fn from(dt: DateTime<Utc>) -> Self {
        TimeWrapper(dt)
    }
}

impl Encode for TimeWrapper {
    fn encode_to<T: ?Sized + parity_scale_codec::Output>(&self, dest: &mut T) {
        let timestamp = self.0.timestamp();
        let subsec_nanos = self.0.timestamp_subsec_nanos();
        (timestamp, subsec_nanos).encode_to(dest);
    }
}

impl Decode for TimeWrapper {
    fn decode<I: Input>(input: &mut I) -> Result<Self, Error> {
        let (timestamp, subsec_nanos) = <(i64, u32)>::decode(input)?;

        let datetime = Utc.from_utc_datetime(
            &NaiveDateTime::from_timestamp_opt(timestamp, subsec_nanos)
                .ok_or("Invalid timestamp")?,
        );

        Ok(Self(datetime))
    }
}

impl TypeInfo for TimeWrapper {
    type Identity = Self;

    fn type_info() -> Type {
        Type::builder()
            .path(Path::new("TimeWrapper", module_path!()))
            .composite(
                Fields::unnamed()
                    .field(|f| f.ty::<i64>().type_name("Timestamp"))
                    .field(|f| f.ty::<u32>().type_name("SubsecNanos")),
            )
    }
}

#[derive(Serialize, Deserialize, Encode, Decode, TypeInfo, PartialEq, Eq, Debug, Clone)]
pub struct StartActivity {
    pub namespace: NamespaceId,
    pub id: ActivityId,
    pub time: TimeWrapper,
}

#[derive(Serialize, Deserialize, Encode, Decode, TypeInfo, PartialEq, Eq, Debug, Clone)]
pub struct EndActivity {
    pub namespace: NamespaceId,
    pub id: ActivityId,
    pub time: TimeWrapper,
}

#[derive(Serialize, Deserialize, Encode, Decode, TypeInfo, PartialEq, Eq, Debug, Clone)]
pub struct ActivityUses {
    pub namespace: NamespaceId,
    pub id: EntityId,
    pub activity: ActivityId,
}

#[derive(Serialize, Deserialize, Encode, Decode, TypeInfo, PartialEq, Eq, Debug, Clone)]
pub struct EntityExists {
    pub namespace: NamespaceId,
    pub external_id: ExternalId,
}

#[derive(Serialize, Deserialize, Encode, Decode, TypeInfo, PartialEq, Eq, Debug, Clone)]
pub struct WasGeneratedBy {
    pub namespace: NamespaceId,
    pub id: EntityId,
    pub activity: ActivityId,
}

#[derive(Serialize, Deserialize, Encode, Decode, TypeInfo, PartialEq, Eq, Debug, Clone)]
pub struct EntityDerive {
    pub namespace: NamespaceId,
    pub id: EntityId,
    pub used_id: EntityId,
    pub activity_id: Option<ActivityId>,
    pub typ: DerivationType,
}

#[derive(Serialize, Deserialize, Encode, Decode, TypeInfo, PartialEq, Eq, Debug, Clone)]
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

#[derive(Serialize, Deserialize, Encode, Decode, TypeInfo, PartialEq, Eq, Debug, Clone)]
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

#[derive(Serialize, Deserialize, Encode, Decode, TypeInfo, PartialEq, Eq, Debug, Clone)]
pub struct WasInformedBy {
    pub namespace: NamespaceId,
    pub activity: ActivityId,
    pub informing_activity: ActivityId,
}

#[derive(Serialize, Deserialize, Encode, Decode, TypeInfo, PartialEq, Eq, Debug, Clone)]
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

#[derive(Serialize, Deserialize, Encode, Decode, TypeInfo, PartialEq, Eq, Debug, Clone)]
pub enum ChronicleOperation {
    CreateNamespace(CreateNamespace),
    AgentExists(AgentExists),
    AgentActsOnBehalfOf(ActsOnBehalfOf),
    ActivityExists(ActivityExists),
    StartActivity(StartActivity),
    EndActivity(EndActivity),
    ActivityUses(ActivityUses),
    EntityExists(EntityExists),
    WasGeneratedBy(WasGeneratedBy),
    EntityDerive(EntityDerive),
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
            ChronicleOperation::StartActivity(o) => &o.namespace,
            ChronicleOperation::EndActivity(o) => &o.namespace,
            ChronicleOperation::ActivityUses(o) => &o.namespace,
            ChronicleOperation::EntityExists(o) => &o.namespace,
            ChronicleOperation::WasGeneratedBy(o) => &o.namespace,
            ChronicleOperation::EntityDerive(o) => &o.namespace,
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
