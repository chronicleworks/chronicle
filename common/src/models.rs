use iref::Iri;


#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct NamespaceId(String);

impl From<Iri<'_>> for NamespaceId {
    fn from(iri: Iri) -> Self {
        Self(iri.to_string())
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct EntityId(String);

impl From<Iri<'_>> for EntityId {
    fn from(iri: Iri) -> Self {
        Self(iri.to_string())
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct AgentId(String);

impl From<Iri<'_>> for AgentId {
    fn from(iri: Iri) -> Self {
        Self(iri.to_string())
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct ActivityId(String);

impl From<Iri<'_>> for ActivityId {
    fn from(iri: Iri) -> Self {
        Self(iri.to_string())
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct CreateNamespace {
    pub id: NamespaceId,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct CreateAgent {
    pub namespace: NamespaceId,
    pub id: AgentId,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct RegisterKey {
    pub namespace: NamespaceId,
    pub id: AgentId,
    pub publickey: String,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct CreateActivity {
    pub namespace: NamespaceId,
    pub id: ActivityId,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct StartActivity {
    pub namespace: NamespaceId,
    pub id: ActivityId,
    pub agent: AgentId,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct ActivityUses {
    pub namespace: NamespaceId,
    pub id: ActivityId,
    pub entity: EntityId,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct GenerateEntity {
    pub namespace: NamespaceId,
    pub id: ActivityId,
    pub entity: EntityId,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub enum ChronicleTransaction {
    CreateNamespace(CreateNamespace),
    CreateAgent(CreateAgent),
    RegisterKey(RegisterKey),
    CreateActivity(CreateActivity),
    StartActivity(StartActivity),
    ActivityUses(ActivityUses),
    GenerateEntity(GenerateEntity),
}
