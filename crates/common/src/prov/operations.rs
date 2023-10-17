use chrono::{DateTime, Utc, NaiveDateTime};
use diesel::{
    backend::Backend,
    deserialize::FromSql,
    serialize::{Output, ToSql},
    sql_types::Integer,
    AsExpression, QueryId, SqlType,
};

use parity_scale_codec::{Encode, Decode, Input, Error};
use scale_info::{TypeInfo, Type, Path, build::Fields};
use uuid::Uuid;

use crate::attributes::Attributes;

use super::{
    ActivityId, AgentId, AssociationId, AttributionId, DelegationId, EntityId, ExternalId,
    NamespaceId, Role,
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

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct CreateNamespace {
    pub id: NamespaceId,
    pub external_id: ExternalId,
    pub uuid: Uuid,
}

impl Encode for CreateNamespace {
    fn encode_to<T: ?Sized + parity_scale_codec::Output>(&self, dest: &mut T) {
        self.id.encode_to(dest);
        self.external_id.encode_to(dest);
        self.uuid.as_bytes().encode_to(dest);
    }
}

impl Decode for CreateNamespace {
    fn decode<I: Input>(input: &mut I) -> Result<Self, parity_scale_codec::Error> {
        let id = NamespaceId::decode(input)?;
        let external_id = ExternalId::decode(input)?;
        let uuid_bytes = Vec::<u8>::decode(input)?;
        let uuid = Uuid::from_slice(&uuid_bytes).map_err(|_| "Error decoding UUID")?;

        Ok(Self {
            id,
            external_id,
            uuid,
        })
    }
}

impl TypeInfo for CreateNamespace {
    type Identity = Self;

    fn type_info() -> Type {
        Type::builder()
            .path(Path::new("CreateNamespace", "common"))
            .composite(
                Fields::named()
                    .field(|f| f.ty::<NamespaceId>().name("Id").type_name("NamespaceId"))
                    .field(|f| f.ty::<ExternalId>().name("ExternalId").type_name("ExternalId"))
                    .field(|f| f.ty::<Vec<u8>>().name("Uuid").type_name("Uuid"))
            )
    }
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

#[derive(Serialize, Deserialize,  Encode, Decode, TypeInfo, PartialEq, Eq, Debug, Clone)]
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

#[derive(Serialize, Deserialize, Encode, Decode, TypeInfo,  PartialEq, Eq, Debug, Clone)]
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

#[derive(Serialize, Deserialize,PartialEq, Eq, Debug, Clone)]
pub struct StartActivity {
    pub namespace: NamespaceId,
    pub id: ActivityId,
    pub time: DateTime<Utc>,
}

impl Encode for StartActivity {
    fn encode_to<T: ?Sized + parity_scale_codec::Output>(&self, dest: &mut T) {
        self.namespace.encode_to(dest);
        self.id.encode_to(dest);
        self.time.timestamp().encode_to(dest);
    }
}

impl Decode for StartActivity {
    fn decode<I: Input>(input: &mut I) -> Result<Self, Error> {
        let namespace = NamespaceId::decode(input)?;
        let id = ActivityId::decode(input)?;
        let time = DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(i64::decode(input)?, 0), Utc);
        Ok(Self { namespace, id, time })
    }
}

impl TypeInfo for StartActivity {
    type Identity = Self;

    fn type_info() -> Type {
        Type::builder()
            .path(Path::new("StartActivity", module_path!()))
            .composite(
                Fields::named()
                    .field(|f| f.ty::<NamespaceId>().name("Namespace").type_name("NamespaceId"))
                    .field(|f| f.ty::<ActivityId>().name("Id").type_name("ActivityId"))
                    .field(|f| f.ty::<Option<i64>>().name("Time"))
            )
    }
}


#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct EndActivity {
    pub namespace: NamespaceId,
    pub id: ActivityId,
    pub time: DateTime<Utc>,
}

impl Encode for EndActivity {
    fn encode_to<T: ?Sized + parity_scale_codec::Output>(&self, dest: &mut T) {
        self.namespace.encode_to(dest);
        self.id.encode_to(dest);
        self.time.timestamp().encode_to(dest);
    }
}

impl Decode for EndActivity {
    fn decode<I: Input>(input: &mut I) -> Result<Self, Error> {
        let namespace = NamespaceId::decode(input)?;
        let id = ActivityId::decode(input)?;
        let time = DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(i64::decode(input)?, 0), Utc);
        Ok(Self { namespace, id, time })
    }
}

impl TypeInfo for EndActivity {
    type Identity = Self;

    fn type_info() -> Type {
        Type::builder()
            .path(Path::new("EndActivity", module_path!()))
            .composite(
                Fields::named()
                    .field(|f| f.ty::<NamespaceId>().name("Namespace").type_name("NamespaceId"))
                    .field(|f| f.ty::<ActivityId>().name("Id").type_name("ActivityId"))
                    .field(|f| f.ty::<Option<i64>>().name("Time"))
            )
    }
}

#[derive(Serialize, Deserialize,Encode, Decode, TypeInfo,  PartialEq, Eq, Debug, Clone)]
pub struct ActivityUses {
    pub namespace: NamespaceId,
    pub id: EntityId,
    pub activity: ActivityId,
}

#[derive(Serialize, Deserialize,Encode, Decode, TypeInfo,  PartialEq, Eq, Debug, Clone)]
pub struct EntityExists {
    pub namespace: NamespaceId,
    pub external_id: ExternalId,
}

#[derive(Serialize, Deserialize,Encode, Decode, TypeInfo,  PartialEq, Eq, Debug, Clone)]
pub struct WasGeneratedBy {
    pub namespace: NamespaceId,
    pub id: EntityId,
    pub activity: ActivityId,
}

#[derive(Serialize, Deserialize,Encode, Decode, TypeInfo,  PartialEq, Eq, Debug, Clone)]
pub struct EntityDerive {
    pub namespace: NamespaceId,
    pub id: EntityId,
    pub used_id: EntityId,
    pub activity_id: Option<ActivityId>,
    pub typ: DerivationType,
}

#[derive(Serialize, Deserialize,Encode, Decode, TypeInfo,  PartialEq, Eq, Debug, Clone)]
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

#[derive(Serialize, Deserialize,Encode, Decode, TypeInfo,  PartialEq, Eq, Debug, Clone)]
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

#[derive(Serialize, Deserialize,Encode, Decode, TypeInfo,  PartialEq, Eq, Debug, Clone)]
pub struct WasInformedBy {
    pub namespace: NamespaceId,
    pub activity: ActivityId,
    pub informing_activity: ActivityId,
}

#[derive(Serialize, Deserialize,Encode,Decode,TypeInfo, PartialEq, Eq, Debug, Clone)]
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
