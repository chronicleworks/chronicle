use iref::IriBuf;
use uuid::Uuid;

use super::{AgentId, EntityId};

#[derive(IriEnum, Clone, Copy, PartialEq, Eq, Hash)]
#[iri_prefix("prov" = "http://www.w3.org/ns/prov#")]
pub enum Prov {
    #[iri("prov:Agent")]
    Agent,
    #[iri("prov:Entity")]
    Entity,
    #[iri("prov:Activity")]
    Activity,
    #[iri("prov:wasAssociatedWith")]
    WasAssociatedWith,
    #[iri("prov:wasGeneratedBy")]
    WasGeneratedBy,
    #[iri("prov:used")]
    Used,
    #[iri("prov:wasAttributedTo")]
    WasAttributedTo,
    #[iri("prov:startedAtTime")]
    StartedAtTime,
    #[iri("prov:endedAtTime")]
    EndedAtTime,
}

#[derive(IriEnum, Clone, Copy, PartialEq, Eq, Hash)]
#[iri_prefix("rdfs" = "http://www.w3.org/2000/01/rdf-schema#")]
pub enum Rdfs {
    #[iri("rdfs:Label")]
    Label,
}

#[derive(IriEnum, Clone, Copy, PartialEq, Eq, Hash)]
#[iri_prefix("chronicle" = "http://blockchaintp.com/chronicle/ns#")]
pub enum Chronicle {
    #[iri("chronicle:Identity")]
    Identity,
    #[iri("chronicle:hasIdentity")]
    HasIdentity,
    #[iri("chronicle:hadIdentity")]
    HadIdentity,
    #[iri("chronicle:Namespace")]
    Namespace,
    #[iri("chronicle:hasNamespace")]
    HasNamespace,
    #[iri("chronicle:Attachment")]
    Attachment,
    #[iri("chronicle:hasAttachment")]
    HasAttachment,
    #[iri("chronicle:hadAttachment")]
    HadAttachment,
    #[iri("chronicle:publicKey")]
    PublicKey,
    #[iri("chronicle:entitySignature")]
    Signature,
    #[iri("chronicle:entityLocator")]
    Locator,
    #[iri("chronicle:signedAtTime")]
    SignedAtTime,
    #[iri("chronicle:signedBy")]
    SignedBy,
}

impl Chronicle {
    const PREFIX: &'static str = "http://blockchaintp.com/chronicle/ns#";

    pub fn namespace(name: &str, id: &Uuid) -> IriBuf {
        IriBuf::new(&format!("{}ns:{}:{}", Self::PREFIX, name, id)).unwrap()
    }

    pub fn agent(name: &str) -> IriBuf {
        IriBuf::new(&format!("{}agent:{}", Self::PREFIX, name)).unwrap()
    }

    pub fn activity(name: &str) -> IriBuf {
        IriBuf::new(&format!("{}activity:{}", Self::PREFIX, name)).unwrap()
    }

    pub fn entity(name: &str) -> IriBuf {
        IriBuf::new(&format!("{}entity:{}", Self::PREFIX, name)).unwrap()
    }

    pub fn domaintype(name: &str) -> IriBuf {
        IriBuf::new(&format!("{}domaintype:{}", Self::PREFIX, name)).unwrap()
    }

    pub fn attachment(entity: &EntityId, signature: &str) -> IriBuf {
        IriBuf::new(&format!(
            "{}attachment:{}:{}",
            Self::PREFIX,
            entity.decompose(),
            signature
        ))
        .unwrap()
    }

    pub fn identity(agent: &AgentId, public_key: &str) -> IriBuf {
        IriBuf::new(&format!(
            "{}identity:{}:{}",
            Self::PREFIX,
            agent.decompose(),
            public_key
        ))
        .unwrap()
    }
}
