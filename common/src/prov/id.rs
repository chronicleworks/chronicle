use iref::AsIri;
use uuid::Uuid;

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone)]
pub struct DomaintypeId(String);

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
    pub fn decompose(&self) -> (&str, Uuid) {
        if let &[_, _, name, uuid, ..] = &self.0.split(':').collect::<Vec<_>>()[..] {
            return (name, Uuid::parse_str(uuid).unwrap());
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

impl Into<String> for DomaintypeId {
    fn into(self) -> String {
        self.0
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone)]
pub struct NamespaceId(String);

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
}

impl<S> From<S> for NamespaceId
where
    S: AsIri,
{
    fn from(iri: S) -> Self {
        Self(iri.as_iri().to_string())
    }
}

impl Into<String> for NamespaceId {
    fn into(self) -> String {
        self.0
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone)]
pub struct EntityId(String);

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

impl Into<String> for EntityId {
    fn into(self) -> String {
        self.0
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

impl Into<String> for AgentId {
    fn into(self) -> String {
        self.0
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

impl Into<String> for ActivityId {
    fn into(self) -> String {
        self.0
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
