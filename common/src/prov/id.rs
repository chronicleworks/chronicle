use iref::AsIri;
use uuid::Uuid;

pub trait ChronicleIri: std::ops::Deref<Target = String> {}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone)]
pub struct AttachmentId(String);

impl ChronicleIri for AttachmentId {}

impl std::ops::Deref for AttachmentId {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AttachmentId {
    pub fn new<S>(s: S) -> Self
    where
        S: AsRef<str>,
    {
        AttachmentId(s.as_ref().to_owned())
    }

    /// Decompose into entity name / signature
    pub fn decompose(&self) -> (&str, &str) {
        if let &[_, _, name, signature, ..] = &self.0.split(':').collect::<Vec<_>>()[..] {
            return (name, signature);
        }

        unreachable!();
    }
}

impl<S> From<S> for AttachmentId
where
    S: AsIri,
{
    fn from(iri: S) -> Self {
        Self(iri.as_iri().to_string())
    }
}

impl From<AttachmentId> for String {
    fn from(val: AttachmentId) -> Self {
        val.0
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone)]
pub struct IdentityId(String);

impl ChronicleIri for IdentityId {}

impl std::ops::Deref for IdentityId {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl IdentityId {
    pub fn new<S>(s: S) -> Self
    where
        S: AsRef<str>,
    {
        IdentityId(s.as_ref().to_owned())
    }

    /// Decompose into agent name / public key
    pub fn decompose(&self) -> (&str, &str) {
        if let &[_, _, name, public_key, ..] = &self.0.split(':').collect::<Vec<_>>()[..] {
            return (name, public_key);
        }

        unreachable!();
    }
}

impl<S> From<S> for IdentityId
where
    S: AsIri,
{
    fn from(iri: S) -> Self {
        Self(iri.as_iri().to_string())
    }
}

impl From<IdentityId> for String {
    fn from(val: IdentityId) -> Self {
        val.0
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone)]
pub struct DomaintypeId(String);

impl ChronicleIri for DomaintypeId {}

impl std::ops::Deref for DomaintypeId {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DomaintypeId {
    pub fn new<S>(s: S) -> Self
    where
        S: AsRef<str>,
    {
        DomaintypeId(s.as_ref().to_owned())
    }

    /// Decompose a domain type id into its constituent parts, we need to preserve the type better to justify this implementation
    pub fn decompose(&self) -> &str {
        if let &[_, _, name, ..] = &self.0.split(':').collect::<Vec<_>>()[..] {
            return name;
        }

        unreachable!();
    }
}

impl<S> From<S> for DomaintypeId
where
    S: AsIri,
{
    fn from(iri: S) -> Self {
        Self(iri.as_iri().to_string())
    }
}

impl From<DomaintypeId> for String {
    fn from(val: DomaintypeId) -> Self {
        val.0
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone)]
pub struct NamespaceId(String);

impl ChronicleIri for NamespaceId {}

impl std::ops::Deref for NamespaceId {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl NamespaceId {
    pub fn new<S>(s: S) -> Self
    where
        S: AsRef<str>,
    {
        NamespaceId(s.as_ref().to_owned())
    }

    /// Decompose a namespace id into its constituent parts, we need to preserve the type better to justify this implementation
    pub fn decompose(&self) -> (&str, Uuid) {
        if let &[_, _, name, uuid, ..] = &self.0.split(':').collect::<Vec<_>>()[..] {
            return (name, Uuid::parse_str(uuid).unwrap());
        }

        unreachable!();
    }

    pub fn name_part(&self) -> &str {
        self.decompose().0
    }

    pub fn uuid_part(&self) -> Uuid {
        self.decompose().1
    }
}

impl<S> From<S> for NamespaceId
where
    S: AsIri,
{
    fn from(iri: S) -> Self {
        Self(iri.as_iri().to_string())
    }
}

impl From<NamespaceId> for String {
    fn from(val: NamespaceId) -> Self {
        val.0
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone)]
pub struct EntityId(String);

impl ChronicleIri for EntityId {}

impl std::ops::Deref for EntityId {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<S> From<S> for EntityId
where
    S: AsIri,
{
    fn from(iri: S) -> Self {
        Self(iri.as_iri().to_string())
    }
}

impl From<EntityId> for String {
    fn from(val: EntityId) -> Self {
        val.0
    }
}

impl EntityId {
    pub fn new<S>(s: S) -> Self
    where
        S: AsRef<str>,
    {
        Self(s.as_ref().to_owned())
    }

    /// Extract the activity name from an id
    pub fn decompose(&self) -> &str {
        if let &[_, _, name, ..] = &self.0.split(':').collect::<Vec<_>>()[..] {
            return name;
        }

        unreachable!();
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone)]
pub struct AgentId(String);
impl ChronicleIri for AgentId {}

impl From<AgentId> for String {
    fn from(val: AgentId) -> Self {
        val.0
    }
}

impl std::ops::Deref for AgentId {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AgentId {
    pub fn new<S>(s: S) -> Self
    where
        S: AsRef<str>,
    {
        Self(s.as_ref().to_owned())
    }

    /// Extract the agent name from an id
    pub fn decompose(&self) -> &str {
        if let &[_, _, name, ..] = &self.0.split(':').collect::<Vec<_>>()[..] {
            return name;
        }

        unreachable!();
    }
}

impl<S> From<S> for AgentId
where
    S: AsIri,
{
    fn from(iri: S) -> Self {
        Self(iri.as_iri().to_string())
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone)]
pub struct ActivityId(String);

impl ChronicleIri for ActivityId {}

impl From<ActivityId> for String {
    fn from(val: ActivityId) -> Self {
        val.0
    }
}

impl std::ops::Deref for ActivityId {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ActivityId {
    pub fn new<S>(s: S) -> Self
    where
        S: AsRef<str>,
    {
        Self(s.as_ref().to_owned())
    }

    /// Extract the activity name from an id
    pub fn decompose(&self) -> &str {
        if let &[_, _, name, ..] = &self.0.split(':').collect::<Vec<_>>()[..] {
            return name;
        }

        unreachable!();
    }
}

impl<S> From<S> for ActivityId
where
    S: AsIri,
{
    fn from(iri: S) -> Self {
        Self(iri.as_iri().to_string())
    }
}
