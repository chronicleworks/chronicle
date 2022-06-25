use iref::IriBuf;
use percent_encoding::{percent_encode, NON_ALPHANUMERIC};
use uuid::Uuid;

use super::Name;

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
    #[iri("prov:wasDerivedFrom")]
    WasDerivedFrom,
    #[iri("prov:hadPrimarySource")]
    HadPrimarySource,
    #[iri("prov:wasQuotedFrom")]
    WasQuotedFrom,
    #[iri("prov:wasRevisionOf")]
    WasRevisionOf,
    #[iri("prov:actedOnBehalfOf")]
    ActedOnBehalfOf,
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
    #[iri("chronicle:value")]
    Value,
}

/// Operarations to format specific Iri kinds, using percentage encoding to ensure they are infallible
impl Chronicle {
    pub const PREFIX: &'static str = "http://blockchaintp.com/chronicle/ns#";

    fn encode(s: &str) -> String {
        percent_encode(s.as_bytes(), NON_ALPHANUMERIC).to_string()
    }

    pub fn namespace(name: &Name, id: &Uuid) -> IriBuf {
        IriBuf::new(&format!(
            "{}ns:{}:{}",
            Self::PREFIX,
            Self::encode(name.as_str()),
            id
        ))
        .unwrap()
    }

    pub fn agent(name: &Name) -> IriBuf {
        IriBuf::new(&format!(
            "{}agent:{}",
            Self::PREFIX,
            Self::encode(name.as_str())
        ))
        .unwrap()
    }

    pub fn activity(name: &Name) -> IriBuf {
        IriBuf::new(&format!(
            "{}activity:{}",
            Self::PREFIX,
            Self::encode(name.as_str())
        ))
        .unwrap()
    }

    pub fn entity(name: &Name) -> IriBuf {
        IriBuf::new(&format!(
            "{}entity:{}",
            Self::PREFIX,
            Self::encode(name.as_str())
        ))
        .unwrap()
    }

    pub fn domaintype(name: &Name) -> IriBuf {
        IriBuf::new(&format!(
            "{}domaintype:{}",
            Self::PREFIX,
            Self::encode(name.as_str())
        ))
        .unwrap()
    }

    pub fn attachment(entity_name: &Name, signature: impl AsRef<str>) -> IriBuf {
        IriBuf::new(&format!(
            "{}attachment:{}:{}",
            Self::PREFIX,
            Self::encode(entity_name.as_str()),
            Self::encode(signature.as_ref())
        ))
        .unwrap()
    }

    pub fn identity(agent_name: &Name, public_key: impl AsRef<str>) -> IriBuf {
        IriBuf::new(&format!(
            "{}identity:{}:{}",
            Self::PREFIX,
            Self::encode(agent_name.as_str()),
            Self::encode(public_key.as_ref())
        ))
        .unwrap()
    }
}

/// As these operations are meant to be infallible, prop test them to ensure
#[cfg(test)]
#[allow(clippy::useless_conversion)]
mod test {
    use crate::prov::{ActivityId, AgentId, EntityId, Name, NamespaceId};

    use super::Chronicle;
    use iref::IriBuf;
    use proptest::prelude::*;
    use uuid::Uuid;

    proptest! {
    #![proptest_config(ProptestConfig {
            max_shrink_iters: std::u32::MAX, verbose: 0, .. ProptestConfig::default()
    })]
        #[test]
        fn namespace(name in ".*") {
            NamespaceId::try_from(
                IriBuf::from(Chronicle::namespace(&Name::from(name), &Uuid::new_v4())).as_iri()
            ).unwrap();
        }

        #[test]
        fn agent(name in ".*") {
            AgentId::try_from(
                IriBuf::from(Chronicle::agent(&Name::from(name))).as_iri()
            ).unwrap();
        }

        #[test]
        fn entity(name in ".*") {
            EntityId::try_from(
             IriBuf::from(Chronicle::entity(&Name::from(name))).as_iri()
            ).unwrap();
        }

        #[test]
        fn activity(name in ".*") {
            ActivityId::try_from(
                IriBuf::from(Chronicle::activity(&Name::from(name))).as_iri()
            ).unwrap();
        }
    }
}
