mod chronicle_operations {

	#[derive(Clone, Copy, PartialEq, Eq, Hash)]
	pub enum ChronicleOperation {
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

	const ACTIVITY_EXISTS: &str = "http://chronicle.works/chronicleoperations/ns#ActivityExists";
	const ACTIVITY_NAME: &str = "http://chronicle.works/chronicleoperations/ns#ActivityName";
	const START_ACTIVITY: &str = "http://chronicle.works/chronicleoperations/ns#StartActivity";
	const START_ACTIVITY_TIME: &str =
		"http://chronicle.works/chronicleoperations/ns#StartActivityTime";
	const END_ACTIVITY: &str = "http://chronicle.works/chronicleoperations/ns#EndActivity";
	const END_ACTIVITY_TIME: &str = "http://chronicle.works/chronicleoperations/ns#EndActivityTime";
	const WAS_ASSOCIATED_WITH: &str =
		"http://chronicle.works/chronicleoperations/ns#WasAssociatedWith";
	const WAS_ATTRIBUTED_TO: &str = "http://chronicle.works/chronicleoperations/ns#WasAttributedTo";
	const ACTIVITY_USES: &str = "http://chronicle.works/chronicleoperations/ns#ActivityUses";
	const ENTITY_NAME: &str = "http://chronicle.works/chronicleoperations/ns#EntityName";
	const LOCATOR: &str = "http://chronicle.works/chronicleoperations/ns#Locator";
	const ROLE: &str = "http://chronicle.works/chronicleoperations/ns#Role";
	const ENTITY_EXISTS: &str = "http://chronicle.works/chronicleoperations/ns#EntityExists";
	const WAS_GENERATED_BY: &str = "http://chronicle.works/chronicleoperations/ns#WasGeneratedBy";
	const ENTITY_DERIVE: &str = "http://chronicle.works/chronicleoperations/ns#EntityDerive";
	const DERIVATION_TYPE: &str = "http://chronicle.works/chronicleoperations/ns#DerivationType";
	const USED_ENTITY_NAME: &str = "http://chronicle.works/chronicleoperations/ns#UsedEntityName";
	const SET_ATTRIBUTES: &str = "http://chronicle.works/chronicleoperations/ns#SetAttributes";
	const ATTRIBUTES: &str = "http://chronicle.works/chronicleoperations/ns#Attributes";
	const ATTRIBUTE: &str = "http://chronicle.works/chronicleoperations/ns#Attribute";
	const DOMAINTYPE_ID: &str = "http://chronicle.works/chronicleoperations/ns#DomaintypeId";
	const WAS_INFORMED_BY: &str = "http://chronicle.works/chronicleoperations/ns#WasInformedBy";
	const INFORMING_ACTIVITY_NAME: &str =
		"http://chronicle.works/chronicleoperations/ns#InformingActivityName";
	const GENERATED: &str = "http://chronicle.works/chronicleoperations/ns#Generated";
	const CREATE_NAMESPACE: &str = "http://chronicle.works/chronicleoperations/ns#CreateNamespace";
	const NAMESPACE_NAME: &str = "http://chronicle.works/chronicleoperations/ns#namespaceName";
	const NAMESPACE_UUID: &str = "http://chronicle.works/chronicleoperations/ns#namespaceUuid";
	const AGENT_EXISTS: &str = "http://chronicle.works/chronicleoperations/ns#AgentExists";
	const AGENT_NAME: &str = "http://chronicle.works/chronicleoperations/ns#agentName";
	const AGENT_UUID: &str = "http://chronicle.works/chronicleoperations/ns#agentUuid";
	const AGENT_ACTS_ON_BEHALF_OF: &str =
		"http://chronicle.works/chronicleoperations/ns#AgentActsOnBehalfOf";
	const DELEGATE_ID: &str = "http://chronicle.works/chronicleoperations/ns#delegateId";
	const RESPONSIBLE_ID: &str = "http://chronicle.works/chronicleoperations/ns#responsibleId";

	impl AsRef<str> for ChronicleOperation {
		fn as_ref(&self) -> &'static str {
			match self {
				ChronicleOperation::ActivityExists => ACTIVITY_EXISTS,
				ChronicleOperation::ActivityName => ACTIVITY_NAME,
				ChronicleOperation::StartActivity => START_ACTIVITY,
				ChronicleOperation::StartActivityTime => START_ACTIVITY_TIME,
				ChronicleOperation::EndActivity => END_ACTIVITY,
				ChronicleOperation::EndActivityTime => END_ACTIVITY_TIME,
				ChronicleOperation::WasAssociatedWith => WAS_ASSOCIATED_WITH,
				ChronicleOperation::WasAttributedTo => WAS_ATTRIBUTED_TO,
				ChronicleOperation::ActivityUses => ACTIVITY_USES,
				ChronicleOperation::EntityName => ENTITY_NAME,
				ChronicleOperation::Locator => LOCATOR,
				ChronicleOperation::Role => ROLE,
				ChronicleOperation::EntityExists => ENTITY_EXISTS,
				ChronicleOperation::WasGeneratedBy => WAS_GENERATED_BY,
				ChronicleOperation::EntityDerive => ENTITY_DERIVE,
				ChronicleOperation::DerivationType => DERIVATION_TYPE,
				ChronicleOperation::UsedEntityName => USED_ENTITY_NAME,
				ChronicleOperation::SetAttributes => SET_ATTRIBUTES,
				ChronicleOperation::Attributes => ATTRIBUTES,
				ChronicleOperation::Attribute => ATTRIBUTE,
				ChronicleOperation::DomaintypeId => DOMAINTYPE_ID,
				ChronicleOperation::WasInformedBy => WAS_INFORMED_BY,
				ChronicleOperation::InformingActivityName => INFORMING_ACTIVITY_NAME,
				ChronicleOperation::Generated => GENERATED,
				ChronicleOperation::CreateNamespace => CREATE_NAMESPACE,
				ChronicleOperation::NamespaceName => NAMESPACE_NAME,
				ChronicleOperation::NamespaceUuid => NAMESPACE_UUID,
				ChronicleOperation::AgentExists => AGENT_EXISTS,
				ChronicleOperation::AgentName => AGENT_NAME,
				ChronicleOperation::AgentUuid => AGENT_UUID,
				ChronicleOperation::AgentActsOnBehalfOf => AGENT_ACTS_ON_BEHALF_OF,
				ChronicleOperation::DelegateId => DELEGATE_ID,
				ChronicleOperation::ResponsibleId => RESPONSIBLE_ID,
			}
		}
	}

	#[cfg(feature = "json-ld")]
	impl From<ChronicleOperation> for iri_string::types::IriString {
		fn from(val: ChronicleOperation) -> Self {
			use iri_string::types::UriString;
			UriString::try_from(val.as_str().to_string()).unwrap().into()
		}
	}

	impl ChronicleOperation {
		pub fn as_str(&self) -> &str {
			self.as_ref()
		}
	}

	impl core::fmt::Display for ChronicleOperation {
		fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
			write!(f, "{}", self.as_str())
		}
	}
}

pub use chronicle_operations::ChronicleOperation;

mod prov {

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
				Prov::Agent => AGENT,
				Prov::Entity => ENTITY,
				Prov::Activity => ACTIVITY,
				Prov::WasAssociatedWith => WAS_ASSOCIATED_WITH,
				Prov::QualifiedAssociation => QUALIFIED_ASSOCIATION,
				Prov::QualifiedAttribution => QUALIFIED_ATTRIBUTION,
				Prov::Association => ASSOCIATION,
				Prov::Attribution => ATTRIBUTION,
				Prov::Responsible => RESPONSIBLE,
				Prov::WasGeneratedBy => WAS_GENERATED_BY,
				Prov::Used => USED,
				Prov::WasAttributedTo => WAS_ATTRIBUTED_TO,
				Prov::StartedAtTime => STARTED_AT_TIME,
				Prov::EndedAtTime => ENDED_AT_TIME,
				Prov::WasDerivedFrom => WAS_DERIVED_FROM,
				Prov::HadPrimarySource => HAD_PRIMARY_SOURCE,
				Prov::WasQuotedFrom => WAS_QUOTED_FROM,
				Prov::WasRevisionOf => WAS_REVISION_OF,
				Prov::ActedOnBehalfOf => ACTED_ON_BEHALF_OF,
				Prov::QualifiedDelegation => QUALIFIED_DELEGATION,
				Prov::Delegation => DELEGATION,
				Prov::Delegate => DELEGATE,
				Prov::HadRole => HAD_ROLE,
				Prov::HadActivity => HAD_ACTIVITY,
				Prov::HadEntity => HAD_ENTITY,
				Prov::WasInformedBy => WAS_INFORMED_BY,
				Prov::Generated => GENERATED,
			}
		}
	}

	#[cfg(feature = "json-ld")]
	impl From<Prov> for iri_string::types::IriString {
		fn from(val: Prov) -> Self {
			use iri_string::types::UriString;
			UriString::try_from(val.as_str().to_string()).unwrap().into()
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
	use core::str::FromStr;

	use iri_string::types::UriString;
	#[cfg(not(feature = "std"))]
	use parity_scale_codec::alloc::string::String;
	use percent_encoding::NON_ALPHANUMERIC;
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

	const NAMESPACE: &str = "http://chronicle.works/chronicle/ns#Namespace";
	const HAS_NAMESPACE: &str = "http://chronicle.works/chronicle/ns#hasNamespace";
	const VALUE: &str = "http://chronicle.works/chronicle/ns#Value";

	impl AsRef<str> for Chronicle {
		fn as_ref(&self) -> &'static str {
			match self {
				Chronicle::Namespace => NAMESPACE,
				Chronicle::HasNamespace => HAS_NAMESPACE,
				Chronicle::Value => VALUE,
			}
		}
	}

	#[cfg(feature = "json-ld")]
	impl From<Chronicle> for iri_string::types::IriString {
		fn from(val: Chronicle) -> Self {
			UriString::try_from(val.as_str().to_string()).unwrap().into()
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

	lazy_static::lazy_static! {
		static ref ENCODE_SET: percent_encoding::AsciiSet =
			percent_encoding::NON_ALPHANUMERIC
				.remove(b'_')
				.remove(b'-')
				.remove(b'.');
	}

	/// Operations to format specific Iri kinds, using percentage encoding to ensure they are
	/// infallible. This module provides functionality to create various types of IRIs with
	/// percent encoding applied to external IDs where necessary.
	impl Chronicle {
		pub const LEGACY_PREFIXES: &'static [&'static str] =
			&["http://btp.works/chronicle/ns#", "http://blockchaintp.com/chronicle/ns#"];
		pub const LONG_PREFIX: &'static str = "http://chronicle.works/chronicle/ns#";
		pub const PREFIX: &'static str = "chronicle";

		/// Encodes the given external ID using percent-encoding to ensure it is a valid Chronicle CURIE
		fn encode_external_id(external_id: &ExternalId) -> String {
			percent_encoding::utf8_percent_encode(external_id.as_str(), &ENCODE_SET).to_string()
		}

		/// Constructs a namespace IRI using a given external ID and UUID.
		pub fn namespace(
			external_id: &ExternalId,
			id: &Uuid,
		) -> Result<UriString, iri_string::validate::Error> {
			let encoded_external_id = Self::encode_external_id(external_id);
			UriString::from_str(&format!("{}:ns:{}:{}", Self::PREFIX, encoded_external_id, id))
		}

		/// Constructs an agent IRI using a given external ID.
		pub fn agent(
			external_id: &ExternalId,
		) -> Result<UriString, iri_string::types::CreationError<String>> {
			let encoded_external_id = Self::encode_external_id(external_id);
			format!("{}:agent:{}", Self::PREFIX, encoded_external_id).try_into()
		}

		/// Constructs an activity IRI using a given external ID.
		pub fn activity(
			external_id: &ExternalId,
		) -> Result<UriString, iri_string::types::CreationError<String>> {
			let encoded_external_id = Self::encode_external_id(external_id);
			format!("{}:activity:{}", Self::PREFIX, encoded_external_id).try_into()
		}

		/// Constructs an entity IRI using a given external ID.
		pub fn entity(
			external_id: &ExternalId,
		) -> Result<UriString, iri_string::types::CreationError<String>> {
			let encoded_external_id = Self::encode_external_id(external_id);
			format!("{}:entity:{}", Self::PREFIX, encoded_external_id).try_into()
		}

		/// Constructs a domaintype IRI using a given external ID.
		pub fn domaintype(
			external_id: &ExternalId,
		) -> Result<UriString, iri_string::types::CreationError<String>> {
			let encoded_external_id = Self::encode_external_id(external_id);
			format!("{}:domaintype:{}", Self::PREFIX, encoded_external_id).try_into()
		}

		/// Constructs an association IRI using given agent and activity IDs, and an optional role.
		pub fn association(
			agent: &AgentId,
			activity: &ActivityId,
			role: &Option<Role>,
		) -> Result<UriString, iri_string::types::CreationError<String>> {
			let encoded_agent_id = Self::encode_external_id(agent.external_id_part());
			let encoded_activity_id = Self::encode_external_id(activity.external_id_part());
			let encoded_role = role
				.as_ref()
				.map(|r| Self::encode_external_id(&ExternalId::from(r.as_str())))
				.unwrap_or_else(|| "".to_owned());
			format!(
				"{}:association:{}:{}:role={}",
				Self::PREFIX,
				encoded_agent_id,
				encoded_activity_id,
				encoded_role,
			)
			.try_into()
		}

		/// Constructs a delegation IRI using given delegate and responsible agent IDs, and optional activity and role.
		#[tracing::instrument(
			name = "delegation_iri_creation",
			skip(delegate, responsible, activity, role)
		)]
		pub fn delegation(
			delegate: &AgentId,
			responsible: &AgentId,
			activity: &Option<ActivityId>,
			role: &Option<Role>,
		) -> Result<UriString, iri_string::types::CreationError<String>> {
			let encoded_delegate_id = Self::encode_external_id(delegate.external_id_part());
			let encoded_responsible_id = Self::encode_external_id(responsible.external_id_part());
			let encoded_activity_id = activity
				.as_ref()
				.map(|a| Self::encode_external_id(a.external_id_part()))
				.unwrap_or_default();
			let encoded_role = role
				.as_ref()
				.map(|r| Self::encode_external_id(&ExternalId::from(r.as_str())))
				.unwrap_or_else(|| "".to_owned());
			format!(
				"{}:delegation:{}:{}:role={}:activity={}",
				Self::PREFIX,
				encoded_delegate_id,
				encoded_responsible_id,
				encoded_role,
				encoded_activity_id,
			)
			.try_into()
		}

		/// Constructs an attribution IRI using given agent and entity IDs, and an optional role.
		#[tracing::instrument(name = "attribution_iri_creation", skip(agent, entity, role))]
		pub fn attribution(
			agent: &AgentId,
			entity: &EntityId,
			role: &Option<Role>,
		) -> Result<UriString, iri_string::types::CreationError<String>> {
			let encoded_agent_id = Self::encode_external_id(agent.external_id_part());
			let encoded_entity_id = Self::encode_external_id(entity.external_id_part());
			let encoded_role = role
				.as_ref()
				.map(|r| Self::encode_external_id(&ExternalId::from(r.as_str())))
				.unwrap_or_else(|| "".to_owned());
			format!(
				"{}:attribution:{}:{}:role={}",
				Self::PREFIX,
				encoded_agent_id,
				encoded_entity_id,
				encoded_role,
			)
			.try_into()
		}
	}
}

pub use chronicle::*;

/// As these operations are meant to be infallible, prop test them to ensure
#[cfg(test)]
#[allow(clippy::useless_conversion)]
mod test {
	use crate::prov::{
		ActivityId, AgentId, AssociationId, AttributionId, DelegationId, DomaintypeId, EntityId,
		ExternalId, ExternalIdPart, NamespaceId, Role,
	};

	use super::Chronicle;
	use proptest::prelude::*;
	use uuid::Uuid;

	proptest! {
	#![proptest_config(ProptestConfig {
			max_shrink_iters: std::u32::MAX, verbose: 0, .. ProptestConfig::default()
	})]
		#[test]
		fn namespace(external_id in ".+") {
			let result = Chronicle::namespace(&ExternalId::from(external_id.clone()), &Uuid::new_v4()).unwrap();
			let id = NamespaceId::try_from(result).unwrap();
			assert_eq!(id.external_id_part().as_str(), external_id);
		}

		#[test]
		fn agent(external_id in ".+") {
			let result = Chronicle::agent(&ExternalId::from(external_id.clone())).unwrap();
			let id = AgentId::try_from(result).unwrap();

			assert_eq!(id.external_id_part().as_str(), external_id);
		}

		#[test]
		fn entity(external_id in ".+") {
			let result = Chronicle::entity(&ExternalId::from(external_id.clone())).unwrap();
			let id = EntityId::try_from(result).unwrap();


			assert_eq!(id.external_id_part().as_str(), external_id);
		}

		#[test]
		fn activity(external_id in ".+") {
			let result = Chronicle::activity(&ExternalId::from(external_id.clone())).unwrap();
			let id = ActivityId::try_from(result).unwrap();

			assert_eq!(id.external_id_part().as_str(), external_id);
		}

		#[test]
		fn domaintype(external_id in ".+") {
			let result = Chronicle::domaintype(&ExternalId::from(external_id.clone())).unwrap();
			let id = DomaintypeId::try_from(result).unwrap();

			assert_eq!(id.external_id_part().as_str(), external_id);
		}

		#[test]
		fn attribution(agent_id in ".+", entity_id in ".+", role in proptest::option::of(".+")) {
			let agent = AgentId::from_external_id(agent_id.clone());
			let entity = EntityId::from_external_id(entity_id.clone());
			let role_option = role.map(Role::from);
			let result = Chronicle::attribution(&agent, &entity, &role_option).unwrap();
			let id = AttributionId::try_from(result).unwrap();

			assert_eq!(id.entity().external_id_part().as_str(), entity_id);
			assert_eq!(id.agent().external_id_part().as_str(), agent_id);
		}

		#[test]
		fn delegation(delegate_id in ".+", responsible_id in ".+", activity_id in proptest::option::of(".+"), role in proptest::option::of(".+")) {
			let delegate = AgentId::from_external_id(delegate_id);
			let responsible = AgentId::from_external_id(responsible_id);
			let activity_option = activity_id.map(ActivityId::from_external_id);
			let role_option = role.map(Role::from);
			let result = Chronicle::delegation(&delegate, &responsible, &activity_option, &role_option).unwrap();
			DelegationId::try_from(result).unwrap();
		}

		#[test]
		fn association(agent_id in ".+", activity_id in ".+", role in proptest::option::of(".+")) {
			let agent = AgentId::from_external_id(agent_id);
			let activity = ActivityId::from_external_id(activity_id);
			let role_option = role.map(Role::from);
			let result = Chronicle::association(&agent, &activity, &role_option).unwrap();
			AssociationId::try_from(result).unwrap();
		}
	}
}
