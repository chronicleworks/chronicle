#[derive(IriEnum, Clone, Copy, PartialEq, Eq, Hash)]
#[iri_prefix("prov" = "http://www.w3.org/ns/prov#")]
pub enum Prov {
    #[iri("prov:Agent")]
    Agent,
    #[iri("prov:Entity")]
    Entity,
    #[iri("prov:Activity")]
    Activity,
}

#[derive(IriEnum, Clone, Copy, PartialEq, Eq, Hash)]
#[iri_prefix("chronicle" = "http://blockchaintp.com/chonicle/ns#")]
pub enum Chronicle {
    #[iri("chronicle:hasPublicKey")]
    HasPublicKey,
}
