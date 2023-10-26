mod chronicle_operations {
	use iri_string::types::{IriAbsoluteString, UriAbsoluteString};

	#[derive(Clone, Copy, PartialEq, Eq, Hash)]
	pub enum ChronicleOperations {
		CreateNamespace,

		NamespaceName,
		NamespaceUuid,
		AgentExists,
		AgentName,
		AgentUuid,
		AgentActsOnBehalfOf,
		DelegateId,
		ResponsibleId,
		ActivityExists,
		ActivityName,
		StartActivity,
		StartActivityTime,
		EndActivity,
		EndActivityTime,
		WasAssociatedWith,
		WasAttributedTo,
		ActivityUses,
		EntityName,
		Locator,
		Role,
		EntityExists,
		WasGeneratedBy,
		EntityDerive,
		DerivationType,
		UsedEntityName,
		SetAttributes,
		Attributes,
		Attribute,
		DomaintypeId,
		WasInformedBy,
		InformingActivityName,
		Generated,
	}

	const ACTIVITY_EXISTS: &str = "http://btp.works/chronicleoperation/ns#ActivityExists";
	const ACTIVITY_NAME: &str = "http://btp.works/chronicleoperation/ns#ActivityName";
	const START_ACTIVITY: &str = "http://btp.works/chronicleoperation/ns#StartActivity";
	const START_ACTIVITY_TIME: &str = "http://btp.works/chronicleoperation/ns#StartActivityTime";
	const END_ACTIVITY: &str = "http://btp.works/chronicleoperation/ns#EndActivity";
	const END_ACTIVITY_TIME: &str = "http://btp.works/chronicleoperation/ns#EndActivityTime";
	const WAS_ASSOCIATED_WITH: &str = "http://btp.works/chronicleoperation/ns#WasAssociatedWith";
	const WAS_ATTRIBUTED_TO: &str = "http://btp.works/chronicleoperation/ns#WasAttributedTo";
	const ACTIVITY_USES: &str = "http://btp.works/chronicleoperation/ns#ActivityUses";
	const ENTITY_NAME: &str = "http://btp.works/chronicleoperation/ns#EntityName";
	const LOCATOR: &str = "http://btp.works/chronicleoperation/ns#Locator";
	const ROLE: &str = "http://btp.works/chronicleoperation/ns#Role";
	const ENTITY_EXISTS: &str = "http://btp.works/chronicleoperation/ns#EntityExists";
	const WAS_GENERATED_BY: &str = "http://btp.works/chronicleoperation/ns#WasGeneratedBy";
	const ENTITY_DERIVE: &str = "http://btp.works/chronicleoperation/ns#EntityDerive";
	const DERIVATION_TYPE: &str = "http://btp.works/chronicleoperation/ns#DerivationType";
	const USED_ENTITY_NAME: &str = "http://btp.works/chronicleoperation/ns#UsedEntityName";
	const SET_ATTRIBUTES: &str = "http://btp.works/chronicleoperation/ns#SetAttributes";
	const ATTRIBUTES: &str = "http://btp.works/chronicleoperation/ns#Attributes";
	const ATTRIBUTE: &str = "http://btp.works/chronicleoperation/ns#Attribute";
	const DOMAINTYPE_ID: &str = "http://btp.works/chronicleoperation/ns#DomaintypeId";
	const WAS_INFORMED_BY: &str = "http://btp.works/chronicleoperation/ns#WasInformedBy";
	const INFORMING_ACTIVITY_NAME: &str =
		"http://btp.works/chronicleoperation/ns#InformingActivityName";
	const GENERATED: &str = "http://btp.works/chronicleoperation/ns#Generated";
	const CREATE_NAMESPACE: &str = "http://btp.works/chronicleoperation/ns#CreateNamespace";
	const NAMESPACE_NAME: &str = "http://btp.works/chronicleoperation/ns#namespaceName";
	const NAMESPACE_UUID: &str = "http://btp.works/chronicleoperation/ns#namespaceUuid";
	const AGENT_EXISTS: &str = "http://btp.works/chronicleoperation/ns#AgentExists";
	const AGENT_NAME: &str = "http://btp.works/chronicleoperation/ns#agentName";
	const AGENT_UUID: &str = "http://btp.works/chronicleoperation/ns#agentUuid";
	const AGENT_ACTS_ON_BEHALF_OF: &str =
		"http://btp.works/chronicleoperation/ns#AgentActsOnBehalfOf";
	const DELEGATE_ID: &str = "http://btp.works/chronicleoperation/ns#delegateId";
	const RESPONSIBLE_ID: &str = "http://btp.works/chronicleoperation/ns#responsibleId";

	impl AsRef<str> for ChronicleOperations {
		fn as_ref(&self) -> &'static str {
			match self {
				ChronicleOperations::ActivityExists => &ACTIVITY_EXISTS,
				ChronicleOperations::ActivityName => &ACTIVITY_NAME,
				ChronicleOperations::StartActivity => &START_ACTIVITY,
				ChronicleOperations::StartActivityTime => &START_ACTIVITY_TIME,
				ChronicleOperations::EndActivity => &END_ACTIVITY,
				ChronicleOperations::EndActivityTime => &END_ACTIVITY_TIME,
				ChronicleOperations::WasAssociatedWith => &WAS_ASSOCIATED_WITH,
				ChronicleOperations::WasAttributedTo => &WAS_ATTRIBUTED_TO,
				ChronicleOperations::ActivityUses => &ACTIVITY_USES,
				ChronicleOperations::EntityName => &ENTITY_NAME,
				ChronicleOperations::Locator => &LOCATOR,
				ChronicleOperations::Role => &ROLE,
				ChronicleOperations::EntityExists => &ENTITY_EXISTS,
				ChronicleOperations::WasGeneratedBy => &WAS_GENERATED_BY,
				ChronicleOperations::EntityDerive => &ENTITY_DERIVE,
				ChronicleOperations::DerivationType => &DERIVATION_TYPE,
				ChronicleOperations::UsedEntityName => &USED_ENTITY_NAME,
				ChronicleOperations::SetAttributes => &SET_ATTRIBUTES,
				ChronicleOperations::Attributes => &ATTRIBUTES,
				ChronicleOperations::Attribute => &ATTRIBUTE,
				ChronicleOperations::DomaintypeId => &DOMAINTYPE_ID,
				ChronicleOperations::WasInformedBy => &WAS_INFORMED_BY,
				ChronicleOperations::InformingActivityName => &INFORMING_ACTIVITY_NAME,
				ChronicleOperations::Generated => &GENERATED,
				ChronicleOperations::CreateNamespace => &CREATE_NAMESPACE,
				ChronicleOperations::NamespaceName => &NAMESPACE_NAME,
				ChronicleOperations::NamespaceUuid => &NAMESPACE_UUID,
				ChronicleOperations::AgentExists => &AGENT_EXISTS,
				ChronicleOperations::AgentName => &AGENT_NAME,
				ChronicleOperations::AgentUuid => &AGENT_UUID,
				ChronicleOperations::AgentActsOnBehalfOf => &AGENT_ACTS_ON_BEHALF_OF,
				ChronicleOperations::DelegateId => &DELEGATE_ID,
				ChronicleOperations::ResponsibleId => &RESPONSIBLE_ID,
			}
		}
	}

	#[cfg(feature = "json-ld")]
	impl Into<iri_string::types::IriAbsoluteString> for ChronicleOperations {
		fn into(self) -> IriAbsoluteString {
			UriAbsoluteString::try_from(self.as_str().to_string())
				.unwrap()
				.try_into()
				.unwrap()
		}
	}

	impl ChronicleOperations {
		pub fn as_str(&self) -> &str {
			self.as_ref()
		}
	}

	impl core::fmt::Display for ChronicleOperations {
		fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
			write!(f, "{}", self.as_str())
		}
	}
}

pub use chronicle_operations::ChronicleOperations;

mod prov {
	use iri_string::types::{IriAbsoluteString, UriAbsoluteString};

	use crate::prov::FromCompact;

	#[derive(Clone, Copy, PartialEq, Eq, Hash)]
	pub enum Prov {
		Agent,
		Entity,
		Activity,
		WasAssociatedWith,
		QualifiedAssociation,
		QualifiedAttribution,
		Association,
		Attribution,
		Responsible,
		WasGeneratedBy,
		Used,
		WasAttributedTo,
		StartedAtTime,
		EndedAtTime,
		WasDerivedFrom,
		HadPrimarySource,
		WasQuotedFrom,
		WasRevisionOf,
		ActedOnBehalfOf,
		QualifiedDelegation,
		Delegation,
		Delegate,
		HadRole,
		HadActivity,
		HadEntity,
		WasInformedBy,
		Generated,
	}

	const AGENT: &str = "http://www.w3.org/ns/prov#Agent";
	const ENTITY: &str = "http://www.w3.org/ns/prov#Entity";
	const ACTIVITY: &str = "http://www.w3.org/ns/prov#Activity";
	const WAS_ASSOCIATED_WITH: &str = "http://www.w3.org/ns/prov#wasAssociatedWith";
	const QUALIFIED_ASSOCIATION: &str = "http://www.w3.org/ns/prov#qualifiedAssociation";
	const QUALIFIED_ATTRIBUTION: &str = "http://www.w3.org/ns/prov#qualifiedAttribution";
	const ASSOCIATION: &str = "http://www.w3.org/ns/prov#Association";
	const ATTRIBUTION: &str = "http://www.w3.org/ns/prov#Attribution";
	const RESPONSIBLE: &str = "http://www.w3.org/ns/prov#agent";
	const WAS_GENERATED_BY: &str = "http://www.w3.org/ns/prov#wasGeneratedBy";
	const USED: &str = "http://www.w3.org/ns/prov#used";
	const WAS_ATTRIBUTED_TO: &str = "http://www.w3.org/ns/prov#wasAttributedTo";
	const STARTED_AT_TIME: &str = "http://www.w3.org/ns/prov#startedAtTime";
	const ENDED_AT_TIME: &str = "http://www.w3.org/ns/prov#endedAtTime";
	const WAS_DERIVED_FROM: &str = "http://www.w3.org/ns/prov#wasDerivedFrom";
	const HAD_PRIMARY_SOURCE: &str = "http://www.w3.org/ns/prov#hadPrimarySource";
	const WAS_QUOTED_FROM: &str = "http://www.w3.org/ns/prov#wasQuotedFrom";
	const WAS_REVISION_OF: &str = "http://www.w3.org/ns/prov#wasRevisionOf";
	const ACTED_ON_BEHALF_OF: &str = "http://www.w3.org/ns/prov#actedOnBehalfOf";
	const QUALIFIED_DELEGATION: &str = "http://www.w3.org/ns/prov#qualifiedDelegation";
	const DELEGATION: &str = "http://www.w3.org/ns/prov#Delegation";
	const DELEGATE: &str = "http://www.w3.org/ns/prov#agent";
	const HAD_ROLE: &str = "http://www.w3.org/ns/prov#hadRole";
	const HAD_ACTIVITY: &str = "http://www.w3.org/ns/prov#hadActivity";
	const HAD_ENTITY: &str = "http://www.w3.org/ns/prov#hadEntity";
	const WAS_INFORMED_BY: &str = "http://www.w3.org/ns/prov#wasInformedBy";
	const GENERATED: &str = "http://www.w3.org/ns/prov#generated";

	impl AsRef<str> for Prov {
		fn as_ref(&self) -> &'static str {
			match self {
				Prov::Agent => &AGENT,
				Prov::Entity => &ENTITY,
				Prov::Activity => &ACTIVITY,
				Prov::WasAssociatedWith => &WAS_ASSOCIATED_WITH,
				Prov::QualifiedAssociation => &QUALIFIED_ASSOCIATION,
				Prov::QualifiedAttribution => &QUALIFIED_ATTRIBUTION,
				Prov::Association => &ASSOCIATION,
				Prov::Attribution => &ATTRIBUTION,
				Prov::Responsible => &RESPONSIBLE,
				Prov::WasGeneratedBy => &WAS_GENERATED_BY,
				Prov::Used => &USED,
				Prov::WasAttributedTo => &WAS_ATTRIBUTED_TO,
				Prov::StartedAtTime => &STARTED_AT_TIME,
				Prov::EndedAtTime => &ENDED_AT_TIME,
				Prov::WasDerivedFrom => &WAS_DERIVED_FROM,
				Prov::HadPrimarySource => &HAD_PRIMARY_SOURCE,
				Prov::WasQuotedFrom => &WAS_QUOTED_FROM,
				Prov::WasRevisionOf => &WAS_REVISION_OF,
				Prov::ActedOnBehalfOf => &ACTED_ON_BEHALF_OF,
				Prov::QualifiedDelegation => &QUALIFIED_DELEGATION,
				Prov::Delegation => &DELEGATION,
				Prov::Delegate => &DELEGATE,
				Prov::HadRole => &HAD_ROLE,
				Prov::HadActivity => &HAD_ACTIVITY,
				Prov::HadEntity => &HAD_ENTITY,
				Prov::WasInformedBy => &WAS_INFORMED_BY,
				Prov::Generated => &GENERATED,
			}
		}
	}

	#[cfg(feature = "json-ld")]
	impl Into<iri_string::types::IriAbsoluteString> for Prov {
		fn into(self) -> IriAbsoluteString {
			UriAbsoluteString::try_from(self.as_str().to_string())
				.unwrap()
				.try_into()
				.unwrap()
		}
	}

	impl Prov {
		pub fn as_str(&self) -> &str {
			self.as_ref()
		}
	}

	impl core::fmt::Display for Prov {
		fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
			write!(f, "{}", self.as_str())
		}
	}
}

pub use prov::Prov;

mod chronicle {
	use iri_string::types::{IriAbsoluteString, UriAbsoluteString};
	#[cfg(not(feature = "std"))]
	use parity_scale_codec::alloc::string::String;
	#[cfg(not(feature = "std"))]
	use scale_info::prelude::{borrow::ToOwned, string::ToString, *};
	use uuid::Uuid;

	use crate::prov::{ActivityId, AgentId, EntityId, ExternalId, ExternalIdPart, Role};

	#[derive(Clone, Copy, PartialEq, Eq, Hash)]
	pub enum Chronicle {
		Namespace,
		HasNamespace,
		Value,
	}

	const NAMESPACE: &str = "chronicle:namespace";
	const HAS_NAMESPACE: &str = "chronicle:hasNamespace";
	const VALUE: &str = "chronicle:value";

	impl AsRef<str> for Chronicle {
		fn as_ref(&self) -> &'static str {
			match self {
				Chronicle::Namespace => &NAMESPACE,
				Chronicle::HasNamespace => &HAS_NAMESPACE,
				Chronicle::Value => &VALUE,
			}
		}
	}

	#[cfg(feature = "json-ld")]
	impl Into<iri_string::types::IriAbsoluteString> for Chronicle {
		fn into(self) -> IriAbsoluteString {
			UriAbsoluteString::try_from(self.as_str().to_string())
				.unwrap()
				.try_into()
				.unwrap()
		}
	}

	impl Chronicle {
		pub fn as_str(&self) -> &str {
			self.as_ref()
		}
	}

	impl core::fmt::Display for Chronicle {
		fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
			write!(f, "{}", self.as_str())
		}
	}

	/// Operations to format specific Iri kinds, using percentage encoding to ensure they are infallible
	impl Chronicle {
		pub const LONG_PREFIX: &'static str = "http://btp.works/chronicle/ns#";
		pub const PREFIX: &'static str = "chronicle:";

		pub fn namespace(external_id: &ExternalId, id: &Uuid) -> UriAbsoluteString {
			format!("{}ns:{}:{}", Self::PREFIX, external_id.as_str(), id)
				.try_into()
				.unwrap()
		}

		pub fn agent(external_id: &ExternalId) -> UriAbsoluteString {
			format!("{}agent:{}", Self::PREFIX, external_id.as_str()).try_into().unwrap()
		}

		pub fn activity(external_id: &ExternalId) -> UriAbsoluteString {
			format!("{}activity:{}", Self::PREFIX, external_id.as_str()).try_into().unwrap()
		}

		pub fn entity(external_id: &ExternalId) -> UriAbsoluteString {
			format!("{}entity:{}", Self::PREFIX, external_id.as_str()).try_into().unwrap()
		}

		pub fn domaintype(external_id: &ExternalId) -> UriAbsoluteString {
			format!("{}domaintype:{}", Self::PREFIX, external_id.as_str())
				.try_into()
				.unwrap()
		}

		pub fn association(
			agent: &AgentId,
			activity: &ActivityId,
			role: &Option<Role>,
		) -> UriAbsoluteString {
			format!(
				"{}association:{}:{}:role={}",
				Self::PREFIX,
				agent.external_id_part().as_str(),
				activity.external_id_part().as_ref(),
				(&role.as_ref().map(|x| x.as_str()).unwrap_or_else(|| "")),
			)
			.try_into()
			.unwrap()
		}

		pub fn delegation(
			delegate: &AgentId,
			responsible: &AgentId,
			activity: &Option<ActivityId>,
			role: &Option<Role>,
		) -> UriAbsoluteString {
			format!(
				"{}delegation:{}:{}:role={}:activity={}",
				Self::PREFIX,
				delegate.external_id_part().as_str(),
				responsible.external_id_part().as_str(),
				role.as_ref().map(|x| x.as_str()).unwrap_or(""),
				activity.as_ref().map(|x| x.external_id_part().as_str()).unwrap_or(""),
			)
			.try_into()
			.unwrap()
		}

		pub fn attribution(
			agent: &AgentId,
			entity: &EntityId,
			role: &Option<Role>,
		) -> UriAbsoluteString {
			format!(
				"{}attribution:{}:{}:role={}",
				Self::PREFIX,
				agent.external_id_part().as_str(),
				entity.external_id_part().as_ref(),
				(&role.as_ref().map(|x| x.to_string()).unwrap_or_else(|| "".to_owned())),
			)
			.try_into()
			.unwrap()
		}
	}
}

pub use chronicle::*;

/// As these operations are meant to be infallible, prop test them to ensure
#[cfg(test)]
#[allow(clippy::useless_conversion)]
mod test {
	use crate::prov::{ActivityId, AgentId, EntityId, ExternalId, NamespaceId};

	use super::Chronicle;
	use proptest::prelude::*;
	use uuid::Uuid;

	proptest! {
	#![proptest_config(ProptestConfig {
			max_shrink_iters: std::u32::MAX, verbose: 0, .. ProptestConfig::default()
	})]
		#[test]
		fn namespace(external_id in ".*") {
			NamespaceId::try_from(
				Chronicle::namespace(&ExternalId::from(external_id), &Uuid::new_v4())
			).unwrap();
		}

		#[test]
		fn agent(external_id in ".*") {
			AgentId::try_from(
				Chronicle::agent(&ExternalId::from(external_id))
			).unwrap();
		}

		#[test]
		fn entity(external_id in ".*") {
			EntityId::try_from(
			 Chronicle::entity(&ExternalId::from(external_id))
			).unwrap();
		}

		#[test]
		fn activity(external_id in ".*") {
			ActivityId::try_from(
				Chronicle::activity(&ExternalId::from(external_id))
			).unwrap();
		}
	}
}
