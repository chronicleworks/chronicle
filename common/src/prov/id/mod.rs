mod graphlql_scalars;
pub use graphlql_scalars::*;
use tracing::trace;

use std::{fmt::Display, str::FromStr};

use diesel::{
    backend::Backend,
    deserialize::FromSql,
    serialize::{Output, ToSql},
    sql_types::Text,
    AsExpression, FromSqlRow,
};
use iref::{Iri, IriRefBuf};
use serde::Serialize;
use uuid::Uuid;

use super::vocab::Chronicle;

custom_error::custom_error! {pub ParseIriError
    NotAnIri {source: iref::Error } = "Invalid IRI",
    UnparsableIri {iri: IriRefBuf} = "Unparsable chronicle IRI",
    UnparsableUuid {source: uuid::Error } = "Unparsable UUID",
    IncorrectIriKind = "Unexpected Iri type",
    MissingComponent{component: String} = "Expected {component}",
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
    AsExpression,
    FromSqlRow,
)]
#[diesel(sql_type = diesel::sql_types::Text)]
pub struct Role(String);

impl Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<DB> ToSql<Text, DB> for Role
where
    DB: Backend,
    String: ToSql<Text, DB>,
{
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, DB>) -> diesel::serialize::Result {
        self.0.to_sql(out)
    }
}

impl<DB> FromSql<Text, DB> for Role
where
    DB: Backend,
    String: FromSql<Text, DB>,
{
    fn from_sql(bytes: diesel::backend::RawValue<'_, DB>) -> diesel::deserialize::Result<Self> {
        Ok(Self(String::from_sql(bytes)?))
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
    AsExpression,
    FromSqlRow,
)]
#[diesel(sql_type = diesel::sql_types::Text)]
pub struct ExternalId(String);

impl Display for ExternalId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<DB> ToSql<Text, DB> for ExternalId
where
    DB: Backend,
    String: ToSql<Text, DB>,
{
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, DB>) -> diesel::serialize::Result {
        self.0.to_sql(out)
    }
}

impl<DB> FromSql<Text, DB> for ExternalId
where
    DB: Backend,
    String: FromSql<Text, DB>,
{
    fn from_sql(bytes: diesel::backend::RawValue<'_, DB>) -> diesel::deserialize::Result<Self> {
        Ok(Self(String::from_sql(bytes)?))
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

pub trait SignaturePart {
    fn signature_part(&self) -> &str;
}

pub trait PublicKeyPart {
    fn public_key_part(&self) -> &str;
}

/// Transform a chronicle IRI into its compact representation
pub trait AsCompact {
    fn compact(&self) -> String;
}

pub trait TryFromCompact {
    fn de_compact(&self) -> Result<String, ParseIriError>;
}

impl<T: Display> AsCompact for T {
    fn compact(&self) -> String {
        self.to_string().replace(Chronicle::PREFIX, "chronicle:")
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone, Ord, PartialOrd)]
pub enum ChronicleIri {
    Attachment(EvidenceId),
    Identity(IdentityId),
    Namespace(NamespaceId),
    Domaintype(DomaintypeId),
    Entity(EntityId),
    Agent(AgentId),
    Activity(ActivityId),
    Association(AssociationId),
    Delegation(DelegationId),
}

impl Display for ChronicleIri {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChronicleIri::Attachment(id) => write!(f, "{}", id),
            ChronicleIri::Identity(id) => write!(f, "{}", id),
            ChronicleIri::Namespace(id) => write!(f, "{}", id),
            ChronicleIri::Domaintype(id) => write!(f, "{}", id),
            ChronicleIri::Entity(id) => write!(f, "{}", id),
            ChronicleIri::Agent(id) => write!(f, "{}", id),
            ChronicleIri::Activity(id) => write!(f, "{}", id),
            ChronicleIri::Association(id) => write!(f, "{}", id),
            ChronicleIri::Delegation(id) => write!(f, "{}", id),
        }
    }
}

impl From<EvidenceId> for ChronicleIri {
    fn from(val: EvidenceId) -> Self {
        ChronicleIri::Attachment(val)
    }
}

impl From<IdentityId> for ChronicleIri {
    fn from(val: IdentityId) -> Self {
        ChronicleIri::Identity(val)
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

impl From<DelegationId> for ChronicleIri {
    fn from(val: DelegationId) -> Self {
        ChronicleIri::Delegation(val)
    }
}

impl FromStr for ChronicleIri {
    type Err = ParseIriError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        trace!(parsing_iri = %s);
        //Compacted form, expand
        let iri = {
            if s.starts_with("chronicle:") {
                s.replace("chronicle:", Chronicle::PREFIX)
            } else {
                s.to_owned()
            }
        };

        let iri = IriRefBuf::from_str(&iri)?;

        match fragment_components(iri.as_iri()?)
            .iter()
            .map(|x| x.as_str())
            .collect::<Vec<_>>()
            .as_slice()
        {
            ["agent", ..] => Ok(AgentId::try_from(iri.as_iri()?)?.into()),
            ["ns", ..] => Ok(NamespaceId::try_from(iri.as_iri()?)?.into()),
            ["activity", ..] => Ok(ActivityId::try_from(iri.as_iri()?)?.into()),
            ["entity", ..] => Ok(EntityId::try_from(iri.as_iri()?)?.into()),
            ["domaintype", ..] => Ok(DomaintypeId::try_from(iri.as_iri()?)?.into()),
            ["evidence", ..] => Ok(EvidenceId::try_from(iri.as_iri()?)?.into()),
            ["identity", ..] => Ok(IdentityId::try_from(iri.as_iri()?)?.into()),
            ["association", ..] => Ok(AssociationId::try_from(iri.as_iri()?)?.into()),
            ["delegation", ..] => Ok(DelegationId::try_from(iri.as_iri()?)?.into()),
            _ => Err(ParseIriError::UnparsableIri { iri }),
        }
    }
}

impl ChronicleIri {
    // Coerce this to a `NamespaceId`, if possible
    pub fn namespace(self) -> Result<NamespaceId, ParseIriError> {
        match self {
            ChronicleIri::Namespace(ns) => Ok(ns),
            _ => Err(ParseIriError::IncorrectIriKind),
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone, Ord, PartialOrd)]
pub struct EvidenceId {
    external_id: ExternalId,
    signature: String,
}

impl Display for EvidenceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(Into::<IriRefBuf>::into(self).as_str())
    }
}

impl From<&EvidenceId> for IriRefBuf {
    fn from(val: &EvidenceId) -> Self {
        Chronicle::attachment(val.external_id_part(), &val.signature).into()
    }
}

impl EvidenceId {
    pub fn from_external_id(external_id: impl AsRef<str>, signature: impl AsRef<str>) -> Self {
        Self {
            external_id: external_id.as_ref().into(),
            signature: signature.as_ref().to_string(),
        }
    }
}

impl ExternalIdPart for EvidenceId {
    fn external_id_part(&self) -> &ExternalId {
        &self.external_id
    }
}

impl SignaturePart for EvidenceId {
    fn signature_part(&self) -> &str {
        &self.signature
    }
}

fn fragment_components(iri: Iri) -> Vec<String> {
    match iri.fragment() {
        Some(fragment) => fragment
            .as_str()
            .split(':')
            .map(|s| {
                percent_encoding::percent_decode_str(s)
                    .decode_utf8_lossy()
                    .to_string()
            })
            .collect(),
        None => vec![],
    }
}

fn optional_component(external_id: &str, component: &str) -> Result<Option<String>, ParseIriError> {
    let kv = format!("{}=", external_id);
    if !component.starts_with(&*kv) {
        return Err(ParseIriError::MissingComponent {
            component: external_id.to_string(),
        });
    }

    match component.replace(&*kv, "") {
        s if s.is_empty() => Ok(None),
        s => Ok(Some(s)),
    }
}

impl<'a> TryFrom<Iri<'a>> for EvidenceId {
    type Error = ParseIriError;

    fn try_from(value: Iri) -> Result<Self, Self::Error> {
        match fragment_components(value).as_slice() {
            [_, external_id, signature] => Ok(Self {
                external_id: ExternalId::from(external_id),
                signature: signature.to_string(),
            }),
            _ => Err(ParseIriError::UnparsableIri { iri: value.into() }),
        }
    }
}

// A composite identifier of agent, activity and role
#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone, Ord, PartialOrd)]
pub struct DelegationId {
    delegate: ExternalId,
    responsible: ExternalId,
    activity: Option<ExternalId>,
    role: Option<Role>,
}

impl Display for DelegationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(Into::<IriRefBuf>::into(self).as_str())
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

impl<'a> TryFrom<Iri<'a>> for DelegationId {
    type Error = ParseIriError;

    fn try_from(value: Iri) -> Result<Self, Self::Error> {
        match fragment_components(value).as_slice() {
            [_, delegate, responsible, role, activity] => Ok(Self {
                delegate: ExternalId::from(delegate),
                responsible: ExternalId::from(responsible),
                role: optional_component("role", role)?.map(Role::from),
                activity: optional_component("activity", activity)?.map(ExternalId::from),
            }),

            _ => Err(ParseIriError::UnparsableIri { iri: value.into() }),
        }
    }
}

impl From<&DelegationId> for IriRefBuf {
    fn from(val: &DelegationId) -> Self {
        Chronicle::delegation(
            &AgentId::from_external_id(&val.delegate),
            &AgentId::from_external_id(&val.responsible),
            &val.activity()
                .map(|n| ActivityId::from_external_id(n.external_id_part())),
            &val.role,
        )
        .into()
    }
}

// A composite identifier of agent, activity and role
#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone, Ord, PartialOrd)]
pub struct AssociationId {
    agent: ExternalId,
    activity: ExternalId,
    role: Option<Role>,
}

impl Display for AssociationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(Into::<IriRefBuf>::into(self).as_str())
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

impl<'a> TryFrom<Iri<'a>> for AssociationId {
    type Error = ParseIriError;

    fn try_from(value: Iri) -> Result<Self, Self::Error> {
        match fragment_components(value).as_slice() {
            [_, agent, activity, role] => Ok(Self {
                agent: ExternalId::from(agent),
                activity: ExternalId::from(activity),
                role: optional_component("role", role)?.map(Role::from),
            }),

            _ => Err(ParseIriError::UnparsableIri { iri: value.into() }),
        }
    }
}

impl From<&AssociationId> for IriRefBuf {
    fn from(val: &AssociationId) -> Self {
        Chronicle::association(
            &AgentId::from_external_id(&val.agent),
            &ActivityId::from_external_id(&val.activity),
            &val.role,
        )
        .into()
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone, Ord, PartialOrd)]
pub struct IdentityId {
    external_id: ExternalId,
    public_key: String,
}

impl Display for IdentityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(Into::<IriRefBuf>::into(self).as_str())
    }
}

impl IdentityId {
    pub fn from_external_id(external_id: impl AsRef<str>, public_key: impl AsRef<str>) -> Self {
        Self {
            external_id: external_id.as_ref().into(),
            public_key: public_key.as_ref().to_string(),
        }
    }
}

impl ExternalIdPart for IdentityId {
    fn external_id_part(&self) -> &ExternalId {
        &self.external_id
    }
}

impl PublicKeyPart for IdentityId {
    fn public_key_part(&self) -> &str {
        &self.public_key
    }
}

impl<'a> TryFrom<Iri<'a>> for IdentityId {
    type Error = ParseIriError;

    fn try_from(value: Iri) -> Result<Self, Self::Error> {
        match fragment_components(value).as_slice() {
            [_, external_id, public_key] => Ok(Self {
                external_id: ExternalId::from(external_id.as_str()),
                public_key: public_key.to_string(),
            }),

            _ => Err(ParseIriError::UnparsableIri { iri: value.into() }),
        }
    }
}

impl From<&IdentityId> for IriRefBuf {
    fn from(val: &IdentityId) -> Self {
        Chronicle::identity(val.external_id_part(), &val.public_key).into()
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone, Ord, PartialOrd)]
pub struct DomaintypeId(ExternalId);

impl Display for DomaintypeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(Into::<IriRefBuf>::into(self).as_str())
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

impl<'a> TryFrom<Iri<'a>> for DomaintypeId {
    type Error = ParseIriError;

    fn try_from(value: Iri) -> Result<Self, Self::Error> {
        match fragment_components(value).as_slice() {
            [_, external_id] => Ok(Self(ExternalId::from(external_id.as_str()))),
            _ => Err(ParseIriError::UnparsableIri { iri: value.into() }),
        }
    }
}

impl From<&DomaintypeId> for IriRefBuf {
    fn from(val: &DomaintypeId) -> Self {
        Chronicle::domaintype(&val.0).into()
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone, Ord, PartialOrd)]
pub struct NamespaceId {
    external_id: ExternalId,
    uuid: Uuid,
}

impl Display for NamespaceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(Into::<IriRefBuf>::into(self).as_str())
    }
}

impl NamespaceId {
    pub fn from_external_id(external_id: impl AsRef<str>, uuid: Uuid) -> Self {
        Self {
            external_id: external_id.as_ref().into(),
            uuid,
        }
    }
}

impl ExternalIdPart for NamespaceId {
    fn external_id_part(&self) -> &ExternalId {
        &self.external_id
    }
}

impl UuidPart for NamespaceId {
    fn uuid_part(&self) -> &Uuid {
        &self.uuid
    }
}

impl<'a> TryFrom<Iri<'a>> for NamespaceId {
    type Error = ParseIriError;

    fn try_from(value: Iri) -> Result<Self, Self::Error> {
        match fragment_components(value).as_slice() {
            [_, external_id, uuid] => Ok(Self {
                external_id: ExternalId::from(external_id.as_str()),
                uuid: Uuid::parse_str(uuid.as_str())?,
            }),

            _ => Err(ParseIriError::UnparsableIri { iri: value.into() }),
        }
    }
}

impl From<&NamespaceId> for IriRefBuf {
    fn from(val: &NamespaceId) -> Self {
        Chronicle::namespace(&val.external_id, &val.uuid).into()
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone, Ord, PartialOrd)]
pub struct EntityId(ExternalId);

impl Display for EntityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(Into::<IriRefBuf>::into(self).as_str())
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

impl<'a> TryFrom<Iri<'a>> for EntityId {
    type Error = ParseIriError;

    fn try_from(value: Iri) -> Result<Self, Self::Error> {
        match fragment_components(value).as_slice() {
            [_, external_id] => Ok(Self(ExternalId::from(external_id.as_str()))),

            _ => Err(ParseIriError::UnparsableIri { iri: value.into() }),
        }
    }
}

impl From<&EntityId> for IriRefBuf {
    fn from(val: &EntityId) -> Self {
        Chronicle::entity(&val.0).into()
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone, Ord, PartialOrd)]
pub struct AgentId(ExternalId);

impl Display for AgentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(Into::<IriRefBuf>::into(self).as_str())
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

impl<'a> TryFrom<Iri<'a>> for AgentId {
    type Error = ParseIriError;

    fn try_from(value: Iri) -> Result<Self, Self::Error> {
        match fragment_components(value).as_slice() {
            [_, external_id] => Ok(Self(ExternalId::from(external_id.as_str()))),

            _ => Err(ParseIriError::UnparsableIri { iri: value.into() }),
        }
    }
}

impl From<&AgentId> for IriRefBuf {
    fn from(val: &AgentId) -> Self {
        Chronicle::agent(&val.0).into()
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone, Ord, PartialOrd)]
pub struct ActivityId(ExternalId);

impl Display for ActivityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(Into::<IriRefBuf>::into(self).as_str())
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

impl<'a> TryFrom<Iri<'a>> for ActivityId {
    type Error = ParseIriError;

    fn try_from(value: Iri) -> Result<Self, Self::Error> {
        match fragment_components(value).as_slice() {
            [_, external_id] => Ok(Self(ExternalId::from(external_id.as_str()))),

            _ => Err(ParseIriError::UnparsableIri { iri: value.into() }),
        }
    }
}

impl From<&ActivityId> for IriRefBuf {
    fn from(val: &ActivityId) -> Self {
        Chronicle::activity(&val.0).into()
    }
}
