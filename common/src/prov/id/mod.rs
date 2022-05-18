mod graphlql_scalars;
pub use graphlql_scalars::*;

use std::fmt::Display;

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
    UnparsableIri {iri: IriRefBuf} = "Unparseable IRI",
    UnparsableUuid {source: uuid::Error } = "Unparseable UUID",
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
pub struct Name(String);

impl Display for Name {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<DB> ToSql<Text, DB> for Name
where
    DB: Backend,
    String: ToSql<Text, DB>,
{
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, DB>) -> diesel::serialize::Result {
        self.0.to_sql(out)
    }
}

impl<DB> FromSql<Text, DB> for Name
where
    DB: Backend,
    String: FromSql<Text, DB>,
{
    fn from_sql(bytes: diesel::backend::RawValue<'_, DB>) -> diesel::deserialize::Result<Self> {
        Ok(Self(String::from_sql(bytes)?))
    }
}

impl<T> From<T> for Name
where
    T: AsRef<str>,
{
    fn from(s: T) -> Self {
        Name(s.as_ref().to_owned())
    }
}

impl Name {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for &Name {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

pub trait NamePart<'a> {
    fn name_part(&'a self) -> &'a Name;
}

pub trait UuidPart<'a> {
    fn uuid_part(&'a self) -> &'a Uuid;
}

pub trait SignaturePart<'a> {
    fn signature_part(&'a self) -> &'a str;
}

pub trait PublicKeyPart<'a> {
    fn public_key_part(&'a self) -> &'a str;
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone)]
pub enum ChronicleIri {
    Attachment(AttachmentId),
    Identity(IdentityId),
    Namespace(NamespaceId),
    Domaintype(DomaintypeId),
    Entity(EntityId),
    Agent(AgentId),
    Activity(ActivityId),
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
        }
    }
}

impl From<AttachmentId> for ChronicleIri {
    fn from(val: AttachmentId) -> Self {
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

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone)]
pub struct AttachmentId {
    name: Name,
    signature: String,
}

impl Display for AttachmentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(Into::<IriRefBuf>::into(self).as_str())
    }
}

impl From<&AttachmentId> for IriRefBuf {
    fn from(val: &AttachmentId) -> Self {
        Chronicle::attachment(val.name_part(), &val.signature).into()
    }
}

impl AttachmentId {
    pub fn from_name(name: impl AsRef<str>, signature: impl AsRef<str>) -> Self {
        Self {
            name: name.as_ref().into(),
            signature: signature.as_ref().to_string(),
        }
    }
}

impl<'a> NamePart<'a> for AttachmentId {
    fn name_part(&'a self) -> &'a Name {
        &self.name
    }
}

impl<'a> SignaturePart<'a> for AttachmentId {
    fn signature_part(&'a self) -> &'a str {
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

impl<'a> TryFrom<Iri<'a>> for AttachmentId {
    type Error = ParseIriError;

    fn try_from(value: Iri) -> Result<Self, Self::Error> {
        match fragment_components(value).as_slice() {
            [_, name, signature] => Ok(Self {
                name: Name::from(name),
                signature: signature.to_string(),
            }),
            _ => Err(ParseIriError::UnparsableIri { iri: value.into() }),
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone)]
pub struct IdentityId {
    name: Name,
    public_key: String,
}

impl Display for IdentityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(Into::<IriRefBuf>::into(self).as_str())
    }
}

impl IdentityId {
    pub fn from_name(name: impl AsRef<str>, public_key: impl AsRef<str>) -> Self {
        Self {
            name: name.as_ref().into(),
            public_key: public_key.as_ref().to_string(),
        }
    }
}

impl<'a> NamePart<'a> for IdentityId {
    fn name_part(&'a self) -> &'a Name {
        &self.name
    }
}

impl<'a> PublicKeyPart<'a> for IdentityId {
    fn public_key_part(&'a self) -> &'a str {
        &self.public_key
    }
}

impl<'a> TryFrom<Iri<'a>> for IdentityId {
    type Error = ParseIriError;

    fn try_from(value: Iri) -> Result<Self, Self::Error> {
        match fragment_components(value).as_slice() {
            [_, name, public_key] => Ok(Self {
                name: Name::from(name.as_str()),
                public_key: public_key.to_string(),
            }),

            _ => Err(ParseIriError::UnparsableIri { iri: value.into() }),
        }
    }
}

impl From<&IdentityId> for IriRefBuf {
    fn from(val: &IdentityId) -> Self {
        Chronicle::identity(val.name_part(), &val.public_key).into()
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone)]
pub struct DomaintypeId(Name);

impl Display for DomaintypeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(Into::<IriRefBuf>::into(self).as_str())
    }
}

impl<'a> NamePart<'a> for DomaintypeId {
    fn name_part(&'a self) -> &'a Name {
        &self.0
    }
}

impl DomaintypeId {
    pub fn from_name(name: impl AsRef<str>) -> Self {
        Self(name.as_ref().into())
    }
}

impl<'a> TryFrom<Iri<'a>> for DomaintypeId {
    type Error = ParseIriError;

    fn try_from(value: Iri) -> Result<Self, Self::Error> {
        match fragment_components(value).as_slice() {
            [_, name] => Ok(Self(Name::from(name.as_str()))),

            _ => Err(ParseIriError::UnparsableIri { iri: value.into() }),
        }
    }
}

impl From<&DomaintypeId> for IriRefBuf {
    fn from(val: &DomaintypeId) -> Self {
        Chronicle::domaintype(&val.0).into()
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone)]
pub struct NamespaceId {
    name: Name,
    uuid: Uuid,
}

impl Display for NamespaceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(Into::<IriRefBuf>::into(self).as_str())
    }
}

impl NamespaceId {
    pub fn from_name(name: impl AsRef<str>, uuid: Uuid) -> Self {
        Self {
            name: name.as_ref().into(),
            uuid,
        }
    }
}

impl<'a> NamePart<'a> for NamespaceId {
    fn name_part(&'a self) -> &'a Name {
        &self.name
    }
}

impl<'a> UuidPart<'a> for NamespaceId {
    fn uuid_part(&'a self) -> &'a Uuid {
        &self.uuid
    }
}

impl<'a> TryFrom<Iri<'a>> for NamespaceId {
    type Error = ParseIriError;

    fn try_from(value: Iri) -> Result<Self, Self::Error> {
        match fragment_components(value).as_slice() {
            [_, name, uuid] => Ok(Self {
                name: Name::from(name.as_str()),
                uuid: Uuid::parse_str(uuid.as_str())?,
            }),

            _ => Err(ParseIriError::UnparsableIri { iri: value.into() }),
        }
    }
}

impl From<&NamespaceId> for IriRefBuf {
    fn from(val: &NamespaceId) -> Self {
        Chronicle::namespace(&val.name, &val.uuid).into()
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone)]
pub struct EntityId(Name);

impl Display for EntityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(Into::<IriRefBuf>::into(self).as_str())
    }
}

impl EntityId {
    pub fn from_name(name: impl AsRef<str>) -> Self {
        Self(name.as_ref().into())
    }
}

impl<'a> NamePart<'a> for EntityId {
    fn name_part(&'a self) -> &'a Name {
        &self.0
    }
}

impl<'a> TryFrom<Iri<'a>> for EntityId {
    type Error = ParseIriError;

    fn try_from(value: Iri) -> Result<Self, Self::Error> {
        match fragment_components(value).as_slice() {
            [_, name] => Ok(Self(Name::from(name.as_str()))),

            _ => Err(ParseIriError::UnparsableIri { iri: value.into() }),
        }
    }
}

impl From<&EntityId> for IriRefBuf {
    fn from(val: &EntityId) -> Self {
        Chronicle::entity(&val.0).into()
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone)]
pub struct AgentId(Name);

impl Display for AgentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(Into::<IriRefBuf>::into(self).as_str())
    }
}

impl AgentId {
    pub fn from_name(name: impl AsRef<str>) -> Self {
        Self(name.as_ref().into())
    }
}

impl<'a> NamePart<'a> for AgentId {
    fn name_part(&'a self) -> &'a Name {
        &self.0
    }
}

impl<'a> TryFrom<Iri<'a>> for AgentId {
    type Error = ParseIriError;

    fn try_from(value: Iri) -> Result<Self, Self::Error> {
        match fragment_components(value).as_slice() {
            [_, name] => Ok(Self(Name::from(name.as_str()))),

            _ => Err(ParseIriError::UnparsableIri { iri: value.into() }),
        }
    }
}

impl From<&AgentId> for IriRefBuf {
    fn from(val: &AgentId) -> Self {
        Chronicle::agent(&val.0).into()
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone)]
pub struct ActivityId(Name);

impl Display for ActivityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(Into::<IriRefBuf>::into(self).as_str())
    }
}

impl ActivityId {
    pub fn from_name(name: impl AsRef<str>) -> Self {
        Self(name.as_ref().into())
    }
}

impl<'a> NamePart<'a> for ActivityId {
    fn name_part(&'a self) -> &'a Name {
        &self.0
    }
}

impl<'a> TryFrom<Iri<'a>> for ActivityId {
    type Error = ParseIriError;

    fn try_from(value: Iri) -> Result<Self, Self::Error> {
        match fragment_components(value).as_slice() {
            [_, name] => Ok(Self(Name::from(name.as_str()))),

            _ => Err(ParseIriError::UnparsableIri { iri: value.into() }),
        }
    }
}

impl From<&ActivityId> for IriRefBuf {
    fn from(val: &ActivityId) -> Self {
        Chronicle::activity(&val.0).into()
    }
}
