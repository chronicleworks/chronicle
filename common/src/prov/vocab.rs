use iref::IriBuf;
use percent_encoding::{percent_encode, NON_ALPHANUMERIC};
use uuid::Uuid;

use super::{ActivityId, AgentId, Name, NamePart, Role};

#[derive(IriEnum, Clone, Copy, PartialEq, Eq, Hash)]
#[iri_prefix("chronicleop" = "http://blockchaintp.com/chronicleoperations/ns#")]
pub enum ChronicleOperations {
    #[iri("chronicleop:CreateNamespace")]
    CreateNamespace,
    #[iri("chronicleop:namespaceName")]
    NamespaceName,
    #[iri("chronicleop:namespaceUuid")]
    NamespaceUuid,
    #[iri("chronicleop:AgentExists")]
    AgentExists,
    #[iri("chronicleop:agentName")]
    AgentName,
    #[iri("chronicleop:agentUuid")]
    AgentUuid,
    #[iri("chronicleop:AgentActsOnBehalfOf")]
    AgentActsOnBehalfOf,
    #[iri("chronicleop:delegateId")]
    DelegateId,
    #[iri("chronicleop:responsibleId")]
    ResponsibleId,
    #[iri("chronicleop:RegisterKey")]
    RegisterKey,
    #[iri("chronicleop:publicKey")]
    PublicKey,
    #[iri("chronicleop:ActivityExists")]
    ActivityExists,
    #[iri("chronicleop:activityName")]
    ActivityName,
    #[iri("chronicleop:StartActivity")]
    StartActivity,
    #[iri("chronicleop:startActivityTime")]
    StartActivityTime,
    #[iri("chronicleop:endactivity")]
    EndActivity,
    #[iri("chronicleop:endActivityTime")]
    EndActivityTime,
    #[iri("chronicleop:WasAssociatedWith")]
    WasAssociatedWith,
    #[iri("chronicleop:ActivityUses")]
    ActivityUses,
    #[iri("chronicleop:entityName")]
    EntityName,
    #[iri("chronicleop:signature")]
    Signature,
    #[iri("chronicleop:identity")]
    Identity,
    #[iri("chronicleop:signatureTime")]
    SignatureTime,
    #[iri("chronicleop:locator")]
    Locator,
    #[iri("chronicleop:role")]
    Role,
    #[iri("chronicleop:EntityExists")]
    EntityExists,
    #[iri("chronicleop:WasGeneratedBy")]
    WasGeneratedBy,
    #[iri("chronicleop:EntityDerive")]
    EntityDerive,
    #[iri("chronicleop:derivationType")]
    DerivationType,
    #[iri("chronicleop:EntityHasEvidence")]
    EntityHasEvidence,
    #[iri("chronicleop:usedEntityName")]
    UsedEntityName,
    #[iri("chronicleop:SetAttributes")]
    SetAttributes,
    #[iri("chronicleop:attributes")]
    Attributes,
    #[iri("chronicleop:attribute")]
    Attribute,
    #[iri("chronicleop:domaintypeId")]
    DomaintypeId,
    #[iri("chronicleop:WasInformedBy")]
    WasInformedBy,
    #[iri("chronicleop:informedActivityName")]
    InformedActivityName,
    #[iri("chronicleop:informingActivityName")]
    InformingActivityName,
}

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
    #[iri("prov:qualifiedAssociation")]
    QualifiedAssociation,
    #[iri("prov:Association")]
    Association,
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
    #[iri("prov:qualifiedDelegation")]
    QualifiedDelegation,
    #[iri("prov:Delegation")]
    Delegation,
    #[iri("prov:agent")]
    Responsible,
    #[iri("prov:hadRole")]
    HadRole,
    #[iri("prov:hadActivity")]
    HadActivity,
    #[iri("prov:wasInformedBy")]
    WasInformedBy,
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
    #[iri("chronicle:Evidence")]
    Evidence,
    #[iri("chronicle:hasEvidence")]
    HasEvidence,
    #[iri("chronicle:hadEvidence")]
    HadEvidence,
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

/// Operations to format specific Iri kinds, using percentage encoding to ensure they are infallible
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
            "{}evidence:{}:{}",
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

    pub fn association(agent: &AgentId, activity: &ActivityId, role: &Option<Role>) -> IriBuf {
        IriBuf::new(&format!(
            "{}association:{}:{}:role={}",
            Self::PREFIX,
            Self::encode(agent.name_part().as_str()),
            Self::encode(activity.name_part().as_ref()),
            Self::encode(
                &role
                    .as_ref()
                    .map(|x| x.to_string())
                    .unwrap_or_else(|| "".to_owned())
            ),
        ))
        .unwrap()
    }

    pub fn delegation(
        delegate: &AgentId,
        responsible: &AgentId,
        activity: &Option<ActivityId>,
        role: &Option<Role>,
    ) -> IriBuf {
        IriBuf::new(&format!(
            "{}delegation:{}:{}:role={}:activity={}",
            Self::PREFIX,
            Self::encode(delegate.name_part().as_str()),
            Self::encode(responsible.name_part().as_str()),
            Self::encode(role.as_ref().map(|x| x.as_str()).unwrap_or("")),
            Self::encode(
                activity
                    .as_ref()
                    .map(|x| x.name_part().as_str())
                    .unwrap_or("")
            ),
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
