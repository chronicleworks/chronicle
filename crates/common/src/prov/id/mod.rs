#[cfg(feature = "graphql-bindings")]
mod graphlql_scalars;

use core::str::FromStr;

#[cfg(feature = "graphql-bindings")]
use async_graphql::OneofObject;

use iri_string::types::{IriString, UriString};
use tracing::trace;

#[cfg(feature = "diesel-bindings")]
mod diesel_bindings;

#[cfg(not(feature = "std"))]
use parity_scale_codec::{alloc::string::String, alloc::vec::Vec};

#[cfg(not(feature = "std"))]
use scale_info::{
    prelude::borrow::ToOwned, prelude::string::ToString, prelude::*,
};

#[cfg(feature = "diesel-bindings")]
use diesel::{AsExpression, FromSqlRow};
use serde::Serialize;
use uuid::Uuid;

use super::vocab::Chronicle;
#[cfg(feature = "std")]
use thiserror::Error;
#[cfg(not(feature = "std"))]
use thiserror_no_std::Error;

#[derive(Debug, Error)]
pub enum ParseIriError {
    #[error("Not an IRI")]
    NotAnIri(String),
    #[error("Unparsable Chronicle IRI")]
    UnparsableIri(String),
    #[error("Unparsable UUID")]
    UnparsableUuid(uuid::Error),
    #[error("Unexpected IRI type")]
    NotAChronicleUri(String),
    #[error("Expected {component}")]
    MissingComponent { component: String },
}

// Percent decoded, and has the correct authority

#[derive(Debug)]
pub struct ProbableChronicleCURIE(Vec<String>);

impl ProbableChronicleCURIE {
    fn from_string(str: String) -> Result<Self, ParseIriError> {
        let uri = iri_string::types::UriString::try_from(str)
            .map_err(|e| ParseIriError::NotAnIri(e.into_source()))?;

        Self::from_uri(uri)
    }

    // Take long or short form uris and return a short form iri
    fn from_uri(uri: UriString) -> Result<Self, ParseIriError> {
        let mut uri = uri;

        if uri.as_str().starts_with(Chronicle::LONG_PREFIX) {
            uri = UriString::from_str(
                &uri.as_str()
                    .replace(Chronicle::LONG_PREFIX, &(Chronicle::PREFIX.to_owned() + ":")),
            )
                .unwrap()
        }

        for prefix in Chronicle::LEGACY_PREFIXES {
            if uri.as_str().starts_with(prefix) {
                uri = UriString::from_str(
                    &uri.as_str().replace(prefix, &(Chronicle::PREFIX.to_owned() + ":")),
                )
                    .unwrap()
            }
        }

        let iri: IriString = uri.into();

        if iri.scheme_str() != Chronicle::PREFIX {
            return Err(ParseIriError::NotAChronicleUri(iri.to_string()));
        }

        Ok(Self(
            iri.path_str()
                .split(':')
                .map(|x| percent_encoding::percent_decode_str(x).decode_utf8_lossy().to_string())
                .collect::<Vec<_>>(),
        ))
    }

    fn path_components(&self) -> impl Iterator<Item=&'_ str> {
        self.0.iter().map(|x| x.as_ref())
    }
}

impl core::fmt::Display for ProbableChronicleCURIE {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0.join(":"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(
    feature = "parity-encoding",
    derive(
        scale_info::TypeInfo,
        parity_scale_codec::Encode,
        parity_scale_codec::Decode,
        scale_encode::EncodeAsType
    )
)]
#[cfg_attr(feature = "diesel-bindings", derive(AsExpression, FromSqlRow))]
#[cfg_attr(feature = "diesel-bindings", diesel(sql_type = diesel::sql_types::Text))]
pub struct Role(pub String);

impl core::fmt::Display for Role {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<T> From<T> for Role
    where
        T: AsRef<str>,
{
    fn from(s: T) -> Self {
        Role(s.as_ref().to_owned())
    }
}

impl Role {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for &Role {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(
    feature = "parity-encoding",
    derive(
        scale_info::TypeInfo,
        parity_scale_codec::Encode,
        parity_scale_codec::Decode,
        scale_encode::EncodeAsType
    )
)]
#[cfg_attr(feature = "diesel-bindings", derive(AsExpression, FromSqlRow))]
#[cfg_attr(feature = "diesel-bindings", diesel(sql_type = diesel::sql_types::Text))]
pub struct ExternalId(String);

impl core::fmt::Display for ExternalId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(feature = "graphql-bindings")]
async_graphql::scalar!(ExternalId);

impl<T> From<T> for ExternalId
    where
        T: AsRef<str>,
{
    fn from(s: T) -> Self {
        ExternalId(s.as_ref().to_owned())
    }
}

impl ExternalId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for &ExternalId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

pub trait ExternalIdPart {
    fn external_id_part(&self) -> &ExternalId;
}

pub trait UuidPart {
    fn uuid_part(&self) -> Uuid;
}

/// Transform a chronicle IRI into its long-form representation
pub trait FromCompact {
    fn de_compact(&self) -> String;
}

impl<T: core::fmt::Display> FromCompact for T {
    fn de_compact(&self) -> String {
        let replace = Chronicle::PREFIX.to_string() + ":";
        self.to_string().replace(&replace, Chronicle::LONG_PREFIX)
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone, Ord, PartialOrd)]
#[cfg_attr(
    feature = "parity-encoding",
    derive(
        scale_info::TypeInfo,
        parity_scale_codec::Encode,
        parity_scale_codec::Decode,
        scale_encode::EncodeAsType
    )
)]
pub enum ChronicleIri {
    Namespace(NamespaceId),
    Domaintype(DomaintypeId),
    Entity(EntityId),
    Agent(AgentId),
    Activity(ActivityId),
    Association(AssociationId),
    Attribution(AttributionId),
    Delegation(DelegationId),
}

#[cfg(feature = "parity-encoding")]
impl parity_scale_codec::MaxEncodedLen for ChronicleIri {
    fn max_encoded_len() -> usize {
        2048usize
    }
}

impl core::fmt::Display for ChronicleIri {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ChronicleIri::Namespace(id) => write!(f, "{id}"),
            ChronicleIri::Domaintype(id) => write!(f, "{id}"),
            ChronicleIri::Entity(id) => write!(f, "{id}"),
            ChronicleIri::Agent(id) => write!(f, "{id}"),
            ChronicleIri::Activity(id) => write!(f, "{id}"),
            ChronicleIri::Association(id) => write!(f, "{id}"),
            ChronicleIri::Attribution(id) => write!(f, "{id}"),
            ChronicleIri::Delegation(id) => write!(f, "{id}"),
        }
    }
}

impl From<NamespaceId> for ChronicleIri {
    fn from(val: NamespaceId) -> Self {
        ChronicleIri::Namespace(val)
    }
}

impl From<DomaintypeId> for ChronicleIri {
    fn from(val: DomaintypeId) -> Self {
        ChronicleIri::Domaintype(val)
    }
}

impl From<EntityId> for ChronicleIri {
    fn from(val: EntityId) -> Self {
        ChronicleIri::Entity(val)
    }
}

impl From<AgentId> for ChronicleIri {
    fn from(val: AgentId) -> Self {
        ChronicleIri::Agent(val)
    }
}

impl From<ActivityId> for ChronicleIri {
    fn from(val: ActivityId) -> Self {
        ChronicleIri::Activity(val)
    }
}

impl From<AssociationId> for ChronicleIri {
    fn from(val: AssociationId) -> Self {
        ChronicleIri::Association(val)
    }
}

impl From<AttributionId> for ChronicleIri {
    fn from(val: AttributionId) -> Self {
        ChronicleIri::Attribution(val)
    }
}

impl From<DelegationId> for ChronicleIri {
    fn from(val: DelegationId) -> Self {
        ChronicleIri::Delegation(val)
    }
}

impl core::str::FromStr for ChronicleIri {
    type Err = ParseIriError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        trace!(parsing_iri = %s);
        //Compacted form, expand

        let iri = ProbableChronicleCURIE::from_string(s.to_owned())?;

        //TODO: this just needs to extract the first path component
        match iri.path_components().collect::<Vec<_>>().as_slice() {
            ["agent", ..] => Ok(AgentId::try_from(iri)?.into()),
            ["ns", ..] => Ok(NamespaceId::try_from(iri)?.into()),
            ["activity", ..] => Ok(ActivityId::try_from(iri)?.into()),
            ["entity", ..] => Ok(EntityId::try_from(iri)?.into()),
            ["domaintype", ..] => Ok(DomaintypeId::try_from(iri)?.into()),
            ["association", ..] => Ok(AssociationId::try_from(iri)?.into()),
            ["attribution", ..] => Ok(AttributionId::try_from(iri)?.into()),
            ["delegation", ..] => Ok(DelegationId::try_from(iri)?.into()),
            _ => Err(ParseIriError::UnparsableIri(s.to_string())),
        }
    }
}

impl ChronicleIri {
    // Coerce this to a `NamespaceId`, if possible
    pub fn namespace(self) -> Result<NamespaceId, ParseIriError> {
        match self {
            ChronicleIri::Namespace(ns) => Ok(ns),
            _ => Err(ParseIriError::NotAChronicleUri(self.to_string())),
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct ChronicleJSON(pub serde_json::Value);

fn optional_component(external_id: &str, component: &str) -> Result<Option<String>, ParseIriError> {
    let kv = format!("{external_id}=");
    if !component.starts_with(&*kv) {
        return Err(ParseIriError::MissingComponent { component: external_id.to_string() });
    }

    match component.replace(&*kv, "") {
        s if s.is_empty() => Ok(None),
        s => Ok(Some(s)),
    }
}

// A composite identifier of agent, activity and role
#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone, Ord, PartialOrd)]
#[cfg_attr(
    feature = "parity-encoding",
    derive(
        scale_info::TypeInfo,
        parity_scale_codec::Encode,
        parity_scale_codec::Decode,
        scale_encode::EncodeAsType
    )
)]
pub struct DelegationId {
    delegate: ExternalId,
    responsible: ExternalId,
    activity: Option<ExternalId>,
    role: Option<Role>,
}

impl core::fmt::Display for DelegationId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(Into::<UriString>::into(self).as_str())
    }
}

impl DelegationId {
    pub fn from_component_ids(
        delegate: &AgentId,
        responsible: &AgentId,
        activity: Option<&ActivityId>,
        role: Option<impl AsRef<str>>,
    ) -> Self {
        Self {
            delegate: delegate.external_id_part().clone(),
            responsible: responsible.external_id_part().clone(),
            activity: activity.map(|x| ExternalIdPart::external_id_part(x).to_owned()),
            role: role.map(|x| Role::from(x.as_ref())),
        }
    }

    pub fn delegate(&self) -> AgentId {
        AgentId::from_external_id(&self.delegate)
    }

    pub fn responsible(&self) -> AgentId {
        AgentId::from_external_id(&self.responsible)
    }

    pub fn activity(&self) -> Option<ActivityId> {
        self.activity.as_ref().map(ActivityId::from_external_id)
    }

    pub fn role(&self) -> &Option<Role> {
        &self.role
    }
}

impl TryFrom<String> for DelegationId {
    type Error = ParseIriError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        ProbableChronicleCURIE::from_string(value)?.try_into()
    }
}

impl TryFrom<UriString> for DelegationId {
    type Error = ParseIriError;

    fn try_from(value: UriString) -> Result<Self, Self::Error> {
        ProbableChronicleCURIE::from_uri(value)?.try_into()
    }
}

impl TryFrom<ProbableChronicleCURIE> for DelegationId {
    type Error = ParseIriError;

    fn try_from(iri: ProbableChronicleCURIE) -> Result<Self, Self::Error> {
        match iri.path_components().collect::<Vec<_>>().as_slice() {
            [_, delegate, responsible, role, activity] => Ok(Self {
                delegate: ExternalId::from(delegate),
                responsible: ExternalId::from(responsible),
                role: optional_component("role", role)?.map(Role::from),
                activity: optional_component("activity", activity)?.map(ExternalId::from),
            }),

            _ => Err(ParseIriError::UnparsableIri(iri.to_string())),
        }
    }
}

impl From<&DelegationId> for UriString {
    fn from(val: &DelegationId) -> Self {
        Chronicle::delegation(
            &AgentId::from_external_id(&val.delegate),
            &AgentId::from_external_id(&val.responsible),
            &val.activity().map(|n| ActivityId::from_external_id(n.external_id_part())),
            &val.role,
        )
            .unwrap()
    }
}

// A composite identifier of agent, activity and role
#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone, Ord, PartialOrd)]
#[cfg_attr(
    feature = "parity-encoding",
    derive(
        scale_info::TypeInfo,
        parity_scale_codec::Encode,
        parity_scale_codec::Decode,
        scale_encode::EncodeAsType
    )
)]
pub struct AssociationId {
    agent: ExternalId,
    activity: ExternalId,
    role: Option<Role>,
}

impl core::fmt::Display for AssociationId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(Into::<UriString>::into(self).as_str())
    }
}

impl AssociationId {
    pub fn from_component_ids(
        agent: &AgentId,
        activity: &ActivityId,
        role: Option<impl AsRef<str>>,
    ) -> Self {
        Self {
            agent: agent.external_id_part().clone(),
            activity: activity.external_id_part().clone(),
            role: role.map(|x| Role::from(x.as_ref())),
        }
    }

    pub fn agent(&self) -> AgentId {
        AgentId::from_external_id(&self.agent)
    }

    pub fn activity(&self) -> ActivityId {
        ActivityId::from_external_id(&self.activity)
    }
}

impl TryFrom<String> for AssociationId {
    type Error = ParseIriError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        ProbableChronicleCURIE::from_string(value)?.try_into()
    }
}

impl TryFrom<UriString> for AssociationId {
    type Error = ParseIriError;

    fn try_from(value: UriString) -> Result<Self, Self::Error> {
        ProbableChronicleCURIE::from_uri(value)?.try_into()
    }
}

impl TryFrom<ProbableChronicleCURIE> for AssociationId {
    type Error = ParseIriError;

    fn try_from(iri: ProbableChronicleCURIE) -> Result<Self, Self::Error> {
        match iri.path_components().collect::<Vec<_>>().as_slice() {
            [_, agent, activity, role] => Ok(Self {
                agent: ExternalId::from(agent),
                activity: ExternalId::from(activity),
                role: optional_component("role", role)?.map(Role::from),
            }),

            _ => Err(ParseIriError::UnparsableIri(iri.to_string())),
        }
    }
}

impl From<&AssociationId> for UriString {
    fn from(val: &AssociationId) -> Self {
        Chronicle::association(
            &AgentId::from_external_id(&val.agent),
            &ActivityId::from_external_id(&val.activity),
            &val.role,
        )
            .unwrap()
    }
}

// A composite identifier of agent, entity, and role
#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone, Ord, PartialOrd)]
#[cfg_attr(
    feature = "parity-encoding",
    derive(
        scale_info::TypeInfo,
        parity_scale_codec::Encode,
        parity_scale_codec::Decode,
        scale_encode::EncodeAsType
    )
)]
pub struct AttributionId {
    agent: ExternalId,
    entity: ExternalId,
    role: Option<Role>,
}

impl core::fmt::Display for AttributionId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(Into::<UriString>::into(self).as_str())
    }
}

impl AttributionId {
    pub fn from_component_ids(
        agent: &AgentId,
        entity: &EntityId,
        role: Option<impl AsRef<str>>,
    ) -> Self {
        Self {
            agent: agent.external_id_part().clone(),
            entity: entity.external_id_part().clone(),
            role: role.map(|x| Role::from(x.as_ref())),
        }
    }

    pub fn agent(&self) -> AgentId {
        AgentId::from_external_id(&self.agent)
    }

    pub fn entity(&self) -> EntityId {
        EntityId::from_external_id(&self.entity)
    }
}

impl TryFrom<String> for AttributionId {
    type Error = ParseIriError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        ProbableChronicleCURIE::from_string(value)?.try_into()
    }
}

impl TryFrom<UriString> for AttributionId {
    type Error = ParseIriError;

    fn try_from(value: UriString) -> Result<Self, Self::Error> {
        ProbableChronicleCURIE::from_uri(value)?.try_into()
    }
}

impl TryFrom<ProbableChronicleCURIE> for AttributionId {
    type Error = ParseIriError;

    fn try_from(iri: ProbableChronicleCURIE) -> Result<Self, Self::Error> {
        match iri.path_components().collect::<Vec<_>>().as_slice() {
            [_, agent, entity, role] => Ok(Self {
                agent: ExternalId::from(agent),
                entity: ExternalId::from(entity),
                role: optional_component("role", role)?.map(Role::from),
            }),

            _ => Err(ParseIriError::UnparsableIri(iri.to_string())),
        }
    }
}

impl From<&AttributionId> for UriString {
    fn from(val: &AttributionId) -> Self {
        Chronicle::attribution(
            &AgentId::from_external_id(&val.agent),
            &EntityId::from_external_id(&val.entity),
            &val.role,
        )
            .unwrap()
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone, Ord, PartialOrd)]
#[cfg_attr(
    feature = "parity-encoding",
    derive(
        scale_info::TypeInfo,
        parity_scale_codec::Encode,
        parity_scale_codec::Decode,
        scale_encode::EncodeAsType
    )
)]
pub struct DomaintypeId(ExternalId);

impl core::fmt::Display for DomaintypeId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(Into::<UriString>::into(self).as_str())
    }
}

impl ExternalIdPart for DomaintypeId {
    fn external_id_part(&self) -> &ExternalId {
        &self.0
    }
}

impl DomaintypeId {
    pub fn from_external_id(external_id: impl AsRef<str>) -> Self {
        Self(external_id.as_ref().into())
    }
}

impl TryFrom<String> for DomaintypeId {
    type Error = ParseIriError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        ProbableChronicleCURIE::from_string(value)?.try_into()
    }
}

impl TryFrom<UriString> for DomaintypeId {
    type Error = ParseIriError;

    fn try_from(value: UriString) -> Result<Self, Self::Error> {
        ProbableChronicleCURIE::from_uri(value)?.try_into()
    }
}

impl TryFrom<ProbableChronicleCURIE> for DomaintypeId {
    type Error = ParseIriError;

    fn try_from(iri: ProbableChronicleCURIE) -> Result<Self, Self::Error> {
        match iri.path_components().collect::<Vec<_>>().as_slice() {
            [_, external_id] => Ok(Self(ExternalId::from(external_id))),
            _ => Err(ParseIriError::UnparsableIri(iri.to_string())),
        }
    }
}

impl From<&DomaintypeId> for UriString {
    fn from(val: &DomaintypeId) -> Self {
        Chronicle::domaintype(&val.0).unwrap()
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Clone, Ord, PartialOrd)]
#[cfg_attr(
    feature = "parity-encoding",
    derive(
        scale_info::TypeInfo,
        parity_scale_codec::Encode,
        parity_scale_codec::Decode,
        scale_encode::EncodeAsType
    )
)]
pub struct NamespaceId {
    external_id: ExternalId,
    uuid: [u8; 16],
}

impl core::fmt::Display for NamespaceId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(Into::<UriString>::into(self).as_str())
    }
}

impl core::fmt::Debug for NamespaceId {
    fn fmt(
        &self,
        f: &mut scale_info::prelude::fmt::Formatter<'_>,
    ) -> scale_info::prelude::fmt::Result {
        f.debug_struct("NamespaceId")
            .field("external_id", &self.external_id)
            .field("uuid", &Uuid::from_bytes(self.uuid))
            .finish()
    }
}

impl NamespaceId {
    pub fn from_external_id(external_id: impl AsRef<str>, uuid: Uuid) -> Self {
        Self { external_id: external_id.as_ref().into(), uuid: uuid.into_bytes() }
    }
}

impl ExternalIdPart for NamespaceId {
    fn external_id_part(&self) -> &ExternalId {
        &self.external_id
    }
}

impl UuidPart for NamespaceId {
    fn uuid_part(&self) -> Uuid {
        Uuid::from_bytes(self.uuid)
    }
}

impl TryFrom<&'_ str> for NamespaceId {
    type Error = ParseIriError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        ProbableChronicleCURIE::from_string(value.to_owned())?.try_into()
    }
}

impl TryFrom<String> for NamespaceId {
    type Error = ParseIriError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        ProbableChronicleCURIE::from_string(value)?.try_into()
    }
}

impl TryFrom<UriString> for NamespaceId {
    type Error = ParseIriError;

    fn try_from(value: UriString) -> Result<Self, Self::Error> {
        ProbableChronicleCURIE::from_uri(value)?.try_into()
    }
}

impl TryFrom<ProbableChronicleCURIE> for NamespaceId {
    type Error = ParseIriError;

    fn try_from(iri: ProbableChronicleCURIE) -> Result<Self, Self::Error> {
        match iri.path_components().collect::<Vec<_>>().as_slice() {
            [_, external_id, uuid] => Ok(Self {
                external_id: ExternalId::from(external_id),
                uuid: Uuid::parse_str(uuid).map_err(ParseIriError::UnparsableUuid)?.into_bytes(),
            }),

            _ => Err(ParseIriError::UnparsableIri(format!("{:?}", iri))),
        }
    }
}

impl From<&NamespaceId> for UriString {
    fn from(val: &NamespaceId) -> Self {
        Chronicle::namespace(&val.external_id, &val.uuid_part()).unwrap()
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone, Ord, PartialOrd)]
#[cfg_attr(
    feature = "parity-encoding",
    derive(
        scale_info::TypeInfo,
        parity_scale_codec::Encode,
        parity_scale_codec::Decode,
        scale_encode::EncodeAsType
    )
)]
pub struct EntityId(ExternalId);

impl core::fmt::Display for EntityId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(Into::<UriString>::into(self).as_str())
    }
}

impl EntityId {
    pub fn from_external_id(external_id: impl AsRef<str>) -> Self {
        Self(external_id.as_ref().into())
    }
}

impl ExternalIdPart for EntityId {
    fn external_id_part(&self) -> &ExternalId {
        &self.0
    }
}

impl TryFrom<String> for EntityId {
    type Error = ParseIriError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        ProbableChronicleCURIE::from_string(value)?.try_into()
    }
}

impl TryFrom<UriString> for EntityId {
    type Error = ParseIriError;

    fn try_from(value: UriString) -> Result<Self, Self::Error> {
        ProbableChronicleCURIE::from_uri(value)?.try_into()
    }
}

impl TryFrom<ProbableChronicleCURIE> for EntityId {
    type Error = ParseIriError;

    fn try_from(value: ProbableChronicleCURIE) -> Result<Self, Self::Error> {
        match value.path_components().collect::<Vec<_>>().as_slice() {
            [_, external_id] => Ok(Self(ExternalId::from(external_id))),

            _ => Err(ParseIriError::UnparsableIri(value.to_string())),
        }
    }
}

impl From<&EntityId> for UriString {
    fn from(val: &EntityId) -> Self {
        Chronicle::entity(&val.0).unwrap()
    }
}

/// Input either a short-form `externalId`, e.g. "agreement",
/// or long-form Chronicle `id`, e.g. "chronicle:entity:agreement"
#[cfg_attr(feature = "graphql-bindings", derive(async_graphql::OneofObject))]
pub enum EntityIdOrExternal {
    ExternalId(String),
    Id(EntityId),
}

impl From<EntityIdOrExternal> for EntityId {
    fn from(input: EntityIdOrExternal) -> Self {
        match input {
            EntityIdOrExternal::ExternalId(external_id) => Self::from_external_id(external_id),
            EntityIdOrExternal::Id(id) => id,
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone, Ord, PartialOrd)]
#[cfg_attr(
    feature = "parity-encoding",
    derive(
        scale_info::TypeInfo,
        parity_scale_codec::Encode,
        parity_scale_codec::Decode,
        scale_encode::EncodeAsType
    )
)]
pub struct AgentId(ExternalId);

impl core::fmt::Display for AgentId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(Into::<UriString>::into(self).as_str())
    }
}

impl AgentId {
    pub fn from_external_id(external_id: impl AsRef<str>) -> Self {
        Self(external_id.as_ref().into())
    }
}

impl ExternalIdPart for AgentId {
    fn external_id_part(&self) -> &ExternalId {
        &self.0
    }
}

impl TryFrom<String> for AgentId {
    type Error = ParseIriError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        ProbableChronicleCURIE::from_string(value)?.try_into()
    }
}

impl TryFrom<UriString> for AgentId {
    type Error = ParseIriError;

    fn try_from(value: UriString) -> Result<Self, Self::Error> {
        ProbableChronicleCURIE::from_uri(value)?.try_into()
    }
}

impl TryFrom<ProbableChronicleCURIE> for AgentId {
    type Error = ParseIriError;

    fn try_from(value: ProbableChronicleCURIE) -> Result<Self, Self::Error> {
        match value.path_components().collect::<Vec<_>>().as_slice() {
            [_, external_id] => Ok(Self(ExternalId::from(external_id))),
            _ => Err(ParseIriError::UnparsableIri(value.to_string())),
        }
    }
}

impl From<&AgentId> for UriString {
    fn from(val: &AgentId) -> Self {
        Chronicle::agent(&val.0).unwrap()
    }
}

/// Input either a short-form `externalId`, e.g. "bob",
/// or long-form Chronicle `id`, e.g. "chronicle:agent:bob"
#[cfg_attr(feature = "graphql-bindings", derive(OneofObject))]
pub enum AgentIdOrExternal {
    ExternalId(String),
    Id(AgentId),
}

impl From<AgentIdOrExternal> for AgentId {
    fn from(input: AgentIdOrExternal) -> Self {
        match input {
            AgentIdOrExternal::ExternalId(external_id) => Self::from_external_id(external_id),
            AgentIdOrExternal::Id(id) => id,
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone, Ord, PartialOrd)]
#[cfg_attr(
    feature = "parity-encoding",
    derive(
        scale_info::TypeInfo,
        parity_scale_codec::Encode,
        parity_scale_codec::Decode,
        scale_encode::EncodeAsType
    )
)]
pub struct ActivityId(ExternalId);

impl core::fmt::Display for ActivityId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(UriString::from(self).as_str())
    }
}

impl ActivityId {
    pub fn from_external_id(external_id: impl AsRef<str>) -> Self {
        Self(external_id.as_ref().into())
    }
}

impl ExternalIdPart for ActivityId {
    fn external_id_part(&self) -> &ExternalId {
        &self.0
    }
}

impl TryFrom<String> for ActivityId {
    type Error = ParseIriError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        ProbableChronicleCURIE::from_string(value)?.try_into()
    }
}

impl TryFrom<UriString> for ActivityId {
    type Error = ParseIriError;

    fn try_from(value: UriString) -> Result<Self, Self::Error> {
        ProbableChronicleCURIE::from_uri(value)?.try_into()
    }
}

impl TryFrom<ProbableChronicleCURIE> for ActivityId {
    type Error = ParseIriError;

    fn try_from(iri: ProbableChronicleCURIE) -> Result<Self, Self::Error> {
        match iri.path_components().collect::<Vec<_>>().as_slice() {
            [_, external_id] => Ok(Self(ExternalId::from(external_id))),

            _ => Err(ParseIriError::UnparsableIri(iri.to_string())),
        }
    }
}

impl From<&ActivityId> for UriString {
    fn from(val: &ActivityId) -> Self {
        Chronicle::activity(&val.0).unwrap()
    }
}

/// Input either a short-form `externalId`, e.g. "record",
/// or long-form Chronicle `id`, e.g. "chronicle:activity:record"
#[cfg_attr(feature = "graphql-bindings", derive(OneofObject))]
pub enum ActivityIdOrExternal {
    ExternalId(String),
    Id(ActivityId),
}

impl From<ActivityIdOrExternal> for ActivityId {
    fn from(input: ActivityIdOrExternal) -> Self {
        match input {
            ActivityIdOrExternal::ExternalId(external_id) => Self::from_external_id(external_id),
            ActivityIdOrExternal::Id(id) => id,
        }
    }
}

/// A `Namespace` ID reserved for Chronicle system use.
pub const SYSTEM_ID: &str = "chronicle-system";

/// A `Namespace` UUID reserved for Chronicle system use.
pub const SYSTEM_UUID: &str = "00000000-0000-0000-0000-000000000001";
