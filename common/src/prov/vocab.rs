use iref::IriBuf;
use uuid::Uuid;

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
    #[iri("chronicle:hasPublicKey")]
    HasPublicKey,
    #[iri("chronicle:Namespace")]
    NamespaceType,
    #[iri("chronicle:hasNamespace")]
    HasNamespace,
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
        IriBuf::new(&format!("{}domaintype:{}", Self::PREFIX, name))
            .unwrap_or_else(|_| panic!("{}domaintype:{}", Self::PREFIX, name))
    }
}
