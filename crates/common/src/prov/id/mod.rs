#[cfg(feature = "graphql-bindings")]
mod graphlql_scalars;
#[cfg(feature = "graphql-bindings")]
use async_graphql::OneofObject;
#[cfg(feature = "graphql-bindings")]
pub use graphlql_scalars::*;

use iri_string::types::{IriAbsoluteString, UriAbsoluteString, UriRelativeStr};
use parity_scale_codec::{Decode, Encode};
use scale_info::{build::Fields, Path, Type, TypeInfo};
use tracing::trace;

#[cfg(feature = "diesel-bindings")]
mod diesel_bindings;

#[cfg(not(feature = "std"))]
use parity_scale_codec::{alloc::string::String, alloc::vec::Vec};

#[cfg(not(feature = "std"))]
use scale_info::{
	prelude::borrow::ToOwned, prelude::string::ToString, prelude::sync::Arc, prelude::*,
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
	IncorrectIriKind(String),
	#[error("Expected {component}")]
	MissingComponent { component: String },
}

// Percent decoded, and has the correct authority
pub struct ProbableChronicleIri(iri_string::types::IriAbsoluteString);

impl ProbableChronicleIri {
	fn from_string(str: String) -> Result<Self, ParseIriError> {
		let uri = iri_string::types::UriAbsoluteString::try_from(str)
			.map_err(|e| ParseIriError::NotAnIri(e.into_source()))?;

		Self::from_uri(uri)
	}

	fn from_uri(uri: UriAbsoluteString) -> Result<Self, ParseIriError> {
		let iri: IriAbsoluteString = uri.into();
		if iri.authority_str() != Some(Chronicle::PREFIX)
			&& iri.authority_str() != Some(Chronicle::LONG_PREFIX)
		{
			return Err(ParseIriError::IncorrectIriKind(iri.to_string()));
		}

		Ok(Self(iri))
	}

	fn path_components<'a>(&'a self) -> impl Iterator<Item = &'a str> {
		self.0.path_str().split(':')
	}
}

impl core::fmt::Display for ProbableChronicleIri {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		write!(f, "{}", self.0)
	}
}

#[derive(
	Debug,
	Clone,
	PartialEq,
	Eq,
	PartialOrd,
	Ord,
	Hash,
	Serialize,
	Deserialize,
	Encode,
	Decode,
	TypeInfo,
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

#[derive(
	Debug,
	Clone,
	PartialEq,
	Eq,
	PartialOrd,
	Ord,
	Hash,
	Serialize,
	Deserialize,
	Encode,
	Decode,
	TypeInfo,
)]
#[cfg_attr(feature = "diesel-bindings", derive(AsExpression, FromSqlRow))]
#[cfg_attr(feature = "diesel-bindings", diesel(sql_type = diesel::sql_types::Text))]
pub struct ExternalId(String);

impl core::fmt::Display for ExternalId {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		write!(f, "{}", self.0)
	}
}

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
	fn uuid_part(&self) -> &Uuid;
}

/// Transform a chronicle IRI into its compact representation
pub trait AsCompact {
	fn compact(&self) -> String;
}

impl<T: core::fmt::Display> AsCompact for T {
	fn compact(&self) -> String {
		self.to_string().replace(Chronicle::LONG_PREFIX, Chronicle::PREFIX)
	}
}

/// Transform a chronicle IRI into its long-form representation
pub trait FromCompact {
	fn de_compact(&self) -> String;
}

impl<T: core::fmt::Display> FromCompact for T {
	fn de_compact(&self) -> String {
		self.to_string().replace(Chronicle::PREFIX, Chronicle::LONG_PREFIX)
	}
}

#[derive(
	Serialize,
	Deserialize,
	Encode,
	Decode,
	TypeInfo,
	PartialEq,
	Eq,
	Hash,
	Debug,
	Clone,
	Ord,
	PartialOrd,
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

		let iri = ProbableChronicleIri::from_string(s.to_owned())?;

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
			_ => Err(ParseIriError::IncorrectIriKind(self.to_string())),
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
#[derive(
	Serialize,
	Deserialize,
	Encode,
	Decode,
	TypeInfo,
	PartialEq,
	Eq,
	Hash,
	Debug,
	Clone,
	Ord,
	PartialOrd,
)]
pub struct DelegationId {
	delegate: ExternalId,
	responsible: ExternalId,
	activity: Option<ExternalId>,
	role: Option<Role>,
}

impl core::fmt::Display for DelegationId {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.write_str(Into::<UriAbsoluteString>::into(self).as_str())
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
		ProbableChronicleIri::from_string(value)?.try_into()
	}
}

impl TryFrom<UriAbsoluteString> for DelegationId {
	type Error = ParseIriError;

	fn try_from(value: UriAbsoluteString) -> Result<Self, Self::Error> {
		ProbableChronicleIri::from_uri(value)?.try_into()
	}
}

impl TryFrom<ProbableChronicleIri> for DelegationId {
	type Error = ParseIriError;

	fn try_from(iri: ProbableChronicleIri) -> Result<Self, Self::Error> {
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

impl From<&DelegationId> for UriAbsoluteString {
	fn from(val: &DelegationId) -> Self {
		Chronicle::delegation(
			&AgentId::from_external_id(&val.delegate),
			&AgentId::from_external_id(&val.responsible),
			&val.activity().map(|n| ActivityId::from_external_id(n.external_id_part())),
			&val.role,
		)
		.into()
	}
}

// A composite identifier of agent, activity and role
#[derive(
	Serialize,
	Deserialize,
	Encode,
	Decode,
	TypeInfo,
	PartialEq,
	Eq,
	Hash,
	Debug,
	Clone,
	Ord,
	PartialOrd,
)]
pub struct AssociationId {
	agent: ExternalId,
	activity: ExternalId,
	role: Option<Role>,
}

impl core::fmt::Display for AssociationId {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.write_str(Into::<UriAbsoluteString>::into(self).as_str())
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
		ProbableChronicleIri::from_string(value)?.try_into()
	}
}

impl TryFrom<UriAbsoluteString> for AssociationId {
	type Error = ParseIriError;

	fn try_from(value: UriAbsoluteString) -> Result<Self, Self::Error> {
		ProbableChronicleIri::from_uri(value)?.try_into()
	}
}

impl TryFrom<ProbableChronicleIri> for AssociationId {
	type Error = ParseIriError;

	fn try_from(iri: ProbableChronicleIri) -> Result<Self, Self::Error> {
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

impl From<&AssociationId> for UriAbsoluteString {
	fn from(val: &AssociationId) -> Self {
		Chronicle::association(
			&AgentId::from_external_id(&val.agent),
			&ActivityId::from_external_id(&val.activity),
			&val.role,
		)
		.into()
	}
}

// A composite identifier of agent, entity, and role
#[derive(
	Serialize,
	Deserialize,
	Encode,
	Decode,
	TypeInfo,
	PartialEq,
	Eq,
	Hash,
	Debug,
	Clone,
	Ord,
	PartialOrd,
)]
pub struct AttributionId {
	agent: ExternalId,
	entity: ExternalId,
	role: Option<Role>,
}

impl core::fmt::Display for AttributionId {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.write_str(Into::<UriAbsoluteString>::into(self).as_str())
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
		ProbableChronicleIri::from_string(value)?.try_into()
	}
}

impl TryFrom<UriAbsoluteString> for AttributionId {
	type Error = ParseIriError;

	fn try_from(value: UriAbsoluteString) -> Result<Self, Self::Error> {
		ProbableChronicleIri::from_uri(value)?.try_into()
	}
}

impl TryFrom<ProbableChronicleIri> for AttributionId {
	type Error = ParseIriError;

	fn try_from(iri: ProbableChronicleIri) -> Result<Self, Self::Error> {
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

impl From<&AttributionId> for UriAbsoluteString {
	fn from(val: &AttributionId) -> Self {
		Chronicle::attribution(
			&AgentId::from_external_id(&val.agent),
			&EntityId::from_external_id(&val.entity),
			&val.role,
		)
		.into()
	}
}

#[derive(
	Serialize,
	Deserialize,
	Encode,
	Decode,
	TypeInfo,
	PartialEq,
	Eq,
	Hash,
	Debug,
	Clone,
	Ord,
	PartialOrd,
)]
pub struct DomaintypeId(ExternalId);

impl core::fmt::Display for DomaintypeId {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.write_str(Into::<UriAbsoluteString>::into(self).as_str())
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
		ProbableChronicleIri::from_string(value)?.try_into()
	}
}

impl TryFrom<UriAbsoluteString> for DomaintypeId {
	type Error = ParseIriError;

	fn try_from(value: UriAbsoluteString) -> Result<Self, Self::Error> {
		ProbableChronicleIri::from_uri(value)?.try_into()
	}
}

impl TryFrom<ProbableChronicleIri> for DomaintypeId {
	type Error = ParseIriError;

	fn try_from(iri: ProbableChronicleIri) -> Result<Self, Self::Error> {
		match iri.path_components().collect::<Vec<_>>().as_slice() {
			[_, external_id] => Ok(Self(ExternalId::from(external_id))),
			_ => Err(ParseIriError::UnparsableIri(iri.to_string())),
		}
	}
}

impl From<&DomaintypeId> for UriAbsoluteString {
	fn from(val: &DomaintypeId) -> Self {
		Chronicle::domaintype(&val.0).into()
	}
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub struct UuidWrapper(Uuid);

impl From<Uuid> for UuidWrapper {
	fn from(uuid: Uuid) -> Self {
		Self(uuid)
	}
}

impl Encode for UuidWrapper {
	fn encode_to<T: ?Sized + parity_scale_codec::Output>(&self, dest: &mut T) {
		self.0.as_bytes().encode_to(dest);
	}
}

impl Decode for UuidWrapper {
	fn decode<I: parity_scale_codec::Input>(
		input: &mut I,
	) -> Result<Self, parity_scale_codec::Error> {
		let uuid_bytes = <[u8; 16]>::decode(input)?;
		let uuid = Uuid::from_slice(&uuid_bytes).map_err(|_| "Error decoding UUID")?;
		Ok(Self(uuid))
	}
}

impl TypeInfo for UuidWrapper {
	type Identity = Self;
	fn type_info() -> Type {
		Type::builder()
			.path(Path::new("UuidWrapper", module_path!()))
			.composite(Fields::unnamed().field(|f| f.ty::<[u8; 16]>().type_name("Uuid")))
	}
}

#[derive(
	Serialize,
	Deserialize,
	Decode,
	Encode,
	TypeInfo,
	PartialEq,
	Eq,
	Hash,
	Debug,
	Clone,
	Ord,
	PartialOrd,
)]
pub struct NamespaceId {
	external_id: ExternalId,
	uuid: UuidWrapper,
}

impl core::fmt::Display for NamespaceId {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.write_str(Into::<UriAbsoluteString>::into(self).as_str())
	}
}

impl NamespaceId {
	pub fn from_external_id(external_id: impl AsRef<str>, uuid: Uuid) -> Self {
		Self { external_id: external_id.as_ref().into(), uuid: uuid.into() }
	}
}

impl ExternalIdPart for NamespaceId {
	fn external_id_part(&self) -> &ExternalId {
		&self.external_id
	}
}

impl UuidPart for NamespaceId {
	fn uuid_part(&self) -> &Uuid {
		&self.uuid.0
	}
}

impl TryFrom<String> for NamespaceId {
	type Error = ParseIriError;

	fn try_from(value: String) -> Result<Self, Self::Error> {
		ProbableChronicleIri::from_string(value)?.try_into()
	}
}

impl TryFrom<UriAbsoluteString> for NamespaceId {
	type Error = ParseIriError;

	fn try_from(value: UriAbsoluteString) -> Result<Self, Self::Error> {
		ProbableChronicleIri::from_uri(value)?.try_into()
	}
}

impl TryFrom<ProbableChronicleIri> for NamespaceId {
	type Error = ParseIriError;

	fn try_from(iri: ProbableChronicleIri) -> Result<Self, Self::Error> {
		match iri.path_components().collect::<Vec<_>>().as_slice() {
			[_, external_id, uuid] => Ok(Self {
				external_id: ExternalId::from(external_id),
				uuid: Uuid::parse_str(uuid).map_err(ParseIriError::UnparsableUuid)?.into(),
			}),

			_ => Err(ParseIriError::UnparsableIri(iri.to_string())),
		}
	}
}

impl From<&NamespaceId> for UriAbsoluteString {
	fn from(val: &NamespaceId) -> Self {
		Chronicle::namespace(&val.external_id, &val.uuid.0).into()
	}
}

#[derive(
	Serialize,
	Deserialize,
	Encode,
	Decode,
	TypeInfo,
	PartialEq,
	Eq,
	Hash,
	Debug,
	Clone,
	Ord,
	PartialOrd,
)]
pub struct EntityId(ExternalId);

impl core::fmt::Display for EntityId {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.write_str(Into::<UriAbsoluteString>::into(self).as_str())
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
		ProbableChronicleIri::from_string(value)?.try_into()
	}
}

impl TryFrom<UriAbsoluteString> for EntityId {
	type Error = ParseIriError;

	fn try_from(value: UriAbsoluteString) -> Result<Self, Self::Error> {
		ProbableChronicleIri::from_uri(value)?.try_into()
	}
}

impl TryFrom<ProbableChronicleIri> for EntityId {
	type Error = ParseIriError;

	fn try_from(value: ProbableChronicleIri) -> Result<Self, Self::Error> {
		match value.path_components().collect::<Vec<_>>().as_slice() {
			[_, external_id] => Ok(Self(ExternalId::from(external_id))),

			_ => Err(ParseIriError::UnparsableIri(value.to_string())),
		}
	}
}

impl From<&EntityId> for UriAbsoluteString {
	fn from(val: &EntityId) -> Self {
		Chronicle::entity(&val.0).into()
	}
}

/// Input either a short-form `externalId`, e.g. "agreement",
/// or long-form Chronicle `id`, e.g. "chronicle:entity:agreement"
#[cfg_attr(feature = "graphql-bindings", derive(OneofObject))]
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

#[derive(
	Serialize,
	Deserialize,
	Encode,
	Decode,
	TypeInfo,
	PartialEq,
	Eq,
	Hash,
	Debug,
	Clone,
	Ord,
	PartialOrd,
)]
pub struct AgentId(ExternalId);

impl core::fmt::Display for AgentId {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.write_str(Into::<UriAbsoluteString>::into(self).as_str())
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
		ProbableChronicleIri::from_string(value)?.try_into()
	}
}

impl TryFrom<UriAbsoluteString> for AgentId {
	type Error = ParseIriError;

	fn try_from(value: UriAbsoluteString) -> Result<Self, Self::Error> {
		ProbableChronicleIri::from_uri(value)?.try_into()
	}
}

impl TryFrom<ProbableChronicleIri> for AgentId {
	type Error = ParseIriError;
	fn try_from(value: ProbableChronicleIri) -> Result<Self, Self::Error> {
		match value.path_components().collect::<Vec<_>>().as_slice() {
			[_, external_id] => Ok(Self(ExternalId::from(external_id))),
			_ => Err(ParseIriError::UnparsableIri(value.to_string())),
		}
	}
}

impl From<&AgentId> for UriAbsoluteString {
	fn from(val: &AgentId) -> Self {
		UriAbsoluteString::from(Chronicle::agent(&val.0))
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

#[derive(
	Serialize,
	Deserialize,
	Encode,
	Decode,
	TypeInfo,
	PartialEq,
	Eq,
	Hash,
	Debug,
	Clone,
	Ord,
	PartialOrd,
)]
pub struct ActivityId(ExternalId);

impl core::fmt::Display for ActivityId {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.write_str(UriAbsoluteString::from(self).as_str())
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
		ProbableChronicleIri::from_string(value)?.try_into()
	}
}

impl TryFrom<UriAbsoluteString> for ActivityId {
	type Error = ParseIriError;

	fn try_from(value: UriAbsoluteString) -> Result<Self, Self::Error> {
		ProbableChronicleIri::from_uri(value)?.try_into()
	}
}

impl TryFrom<ProbableChronicleIri> for ActivityId {
	type Error = ParseIriError;

	fn try_from(iri: ProbableChronicleIri) -> Result<Self, Self::Error> {
		match iri.path_components().collect::<Vec<_>>().as_slice() {
			[_, external_id] => Ok(Self(ExternalId::from(external_id))),

			_ => Err(ParseIriError::UnparsableIri(iri.to_string())),
		}
	}
}

impl From<&ActivityId> for UriAbsoluteString {
	fn from(val: &ActivityId) -> Self {
		Chronicle::activity(&val.0).into()
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
