mod contradiction;
pub use contradiction::Contradiction;

#[cfg(feature = "json-ld")]
pub mod json_ld;
#[cfg(test)]
#[cfg(feature = "json-ld")]
#[cfg(feature = "std")]
mod proptest;

use core::{convert::Infallible, fmt::Debug};
#[cfg(not(feature = "std"))]
use parity_scale_codec::{
	alloc::collections::{BTreeMap, BTreeSet},
	alloc::string::String,
	alloc::vec::Vec,
};
#[cfg(not(feature = "std"))]
use scale_info::{
	prelude::borrow::ToOwned, prelude::string::ToString, prelude::sync::Arc, prelude::*,
};
use serde::Serialize;
#[cfg(feature = "std")]
use std::{
	collections::{BTreeMap, BTreeSet},
	sync::Arc,
};

#[cfg(feature = "std")]
use thiserror::Error;
#[cfg(not(feature = "std"))]
use thiserror_no_std::Error;

use tracing::{instrument, trace};
use uuid::Uuid;

use crate::{
	attributes::{Attribute, Attributes},
	identity::IdentityError,
	prov::operations::WasAttributedTo,
};

use super::{
	id,
	operations::{
		ActivityExists, ActivityUses, ActsOnBehalfOf, AgentExists, ChronicleOperation,
		CreateNamespace, DerivationType, EndActivity, EntityDerive, EntityExists, SetAttributes,
		StartActivity, TimeWrapper, WasAssociatedWith, WasGeneratedBy, WasInformedBy,
	},
	ActivityId, AgentId, AssociationId, AttributionId, ChronicleIri, DelegationId, DomaintypeId,
	EntityId, ExternalId, ExternalIdPart, NamespaceId, Role, UuidPart,
};

#[cfg(feature = "json-ld")]
#[derive(Error, Debug)]
pub enum ProcessorError {
	#[error("Invalid address")]
	Address,
	#[error("Json Ld Error {0}")]
	Compaction(
		#[from]
		#[source]
		json_ld::CompactionError,
	),
	#[error("Contradiction {0}")]
	Contradiction(Contradiction),
	#[error("Json Ld Error {inner}")]
	Expansion { inner: String },
	#[error("IdentityError {0}")]
	Identity(
		#[from]
		#[source]
		IdentityError,
	),
	#[error("Invalid IRI {0}")]
	IRef(
		#[from]
		#[source]
		iref::Error,
	),
	#[error("Not a Chronicle IRI {0}")]
	NotAChronicleIri(
		#[from]
		#[source]
		id::ParseIriError,
	),
	#[error("Missing @id {object:?}")]
	MissingId { object: serde_json::Value },
	#[error("Missing property {iri}:{object:?}")]
	MissingProperty { iri: String, object: serde_json::Value },
	#[error("Json LD object is not a node {0}")]
	NotANode(serde_json::Value),
	#[error("Chronicle value is not a JSON object")]
	NotAnObject,

	#[error("Missing activity")]
	MissingActivity,
	#[error("OpaExecutorError: {0}")]
	OpaExecutor(
		#[from]
		#[source]
		anyhow::Error,
	),
	#[error("Malformed JSON {0}")]
	SerdeJson(
		#[from]
		#[source]
		serde_json::Error,
	),

	#[error("Submission {0}")]
	SubmissionFormat(
		#[from]
		#[source]
		PayloadError,
	),
	#[error("Submission body format: {0}")]
	Time(
		#[from]
		#[source]
		chrono::ParseError,
	),
	#[error("Tokio Error")]
	Tokio,
	#[error("State is not valid utf8 {0}")]
	Utf8(
		#[from]
		#[source]
		core::str::Utf8Error,
	),
}

#[cfg(not(feature = "json-ld"))]
#[derive(Error, Debug)]
pub enum ProcessorError {
	#[error("Invalid address")]
	Address,
	#[error("Contradiction {0}")]
	Contradiction(Contradiction),
	#[error("IdentityError {0}")]
	Identity(
		#[from]
		#[source]
		IdentityError,
	),
	#[error("Not a Chronicle IRI {0}")]
	NotAChronicleIri(
		#[from]
		#[source]
		id::ParseIriError,
	),
	#[error("Missing @id {object:?}")]
	MissingId { object: serde_json::Value },
	#[error("Missing property {iri}:{object:?}")]
	MissingProperty { iri: String, object: serde_json::Value },
	#[error("Json LD object is not a node {0}")]
	NotANode(serde_json::Value),
	#[error("Chronicle value is not a JSON object")]
	NotAnObject,
	#[error("OpaExecutorError: {0}")]
	OpaExecutor(
		#[from]
		#[source]
		anyhow::Error,
	),
	#[error("Malformed JSON {0}")]
	SerdeJson(
		#[from]
		#[source]
		serde_json::Error,
	),
	#[error("Unparsable date/time {0}")]
	SubmissionFormat(
		#[from]
		#[source]
		PayloadError,
	),
	#[error("Submission body format: {0}")]
	Time(
		#[from]
		#[source]
		chrono::ParseError,
	),
	#[error("Tokio Error")]
	Tokio,
	#[error("State is not valid utf8 {0}")]
	Utf8(
		#[from]
		#[source]
		core::str::Utf8Error,
	),
}

#[derive(Error, Debug)]
pub enum PayloadError {
	#[error("No list of Chronicle operations")]
	OpsNotAList,
	#[error("Not a JSON object")]
	NotAnObject,
	#[error("No version number")]
	VersionMissing,
	#[error("Unknown version number")]
	VersionUnknown,
}

impl From<Infallible> for ProcessorError {
	fn from(_: Infallible) -> Self {
		unreachable!()
	}
}

#[derive(Error, Debug)]
pub enum ChronicleTransactionIdError {
	#[error("Invalid transaction id {id}")]
	InvalidTransactionId { id: String },
}

#[cfg_attr(
	feature = "parity-encoding",
	derive(scale_info::TypeInfo, parity_scale_codec::Encode, parity_scale_codec::Decode)
)]
#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone, Copy, Default)]
pub struct ChronicleTransactionId([u8; 16]);

impl core::ops::Deref for ChronicleTransactionId {
	type Target = [u8; 16];

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl core::fmt::Display for ChronicleTransactionId {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.write_str(&hex::encode(self.0))
	}
}

impl From<Uuid> for ChronicleTransactionId {
	fn from(u: Uuid) -> Self {
		Self(u.into_bytes())
	}
}

impl From<[u8; 16]> for ChronicleTransactionId {
	fn from(u: [u8; 16]) -> Self {
		Self(u)
	}
}

impl core::convert::TryFrom<String> for ChronicleTransactionId {
	type Error = hex::FromHexError;

	fn try_from(s: String) -> Result<Self, Self::Error> {
		Self::try_from(s.as_str())
	}
}

impl core::convert::TryFrom<&str> for ChronicleTransactionId {
	type Error = hex::FromHexError;

	fn try_from(s: &str) -> Result<Self, Self::Error> {
		let bytes = hex::decode(s)?;
		let mut array = [0; 16];
		array.copy_from_slice(&bytes[0..16]);
		Ok(Self(array))
	}
}

#[cfg_attr(
	feature = "parity-encoding",
	derive(scale_info::TypeInfo, parity_scale_codec::Encode, parity_scale_codec::Decode)
)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Namespace {
	pub id: NamespaceId,
	pub uuid: [u8; 16],
	pub external_id: ExternalId,
}

impl Namespace {
	pub fn new(id: NamespaceId, uuid: Uuid, external_id: &ExternalId) -> Self {
		Self { id, uuid: uuid.into_bytes(), external_id: external_id.to_owned() }
	}
}

#[cfg_attr(
	feature = "parity-encoding",
	derive(scale_info::TypeInfo, parity_scale_codec::Encode, parity_scale_codec::Decode)
)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Agent {
	pub id: AgentId,
	pub namespaceid: NamespaceId,
	pub external_id: ExternalId,
	pub domaintypeid: Option<DomaintypeId>,
	pub attributes: BTreeMap<String, Attribute>,
}

impl Agent {
	pub fn has_attributes(self, attributes: Attributes) -> Self {
		let Self { id, namespaceid, external_id, .. } = self;

		Self {
			id,
			namespaceid,
			external_id,
			domaintypeid: attributes.get_typ().clone(),
			attributes: attributes.get_items().iter().cloned().collect(),
		}
	}

	// Create a prototypical agent from its IRI, we can only determine external_id
	pub fn exists(namespaceid: NamespaceId, id: AgentId) -> Self {
		Self {
			namespaceid,
			external_id: id.external_id_part().to_owned(),
			id,
			domaintypeid: None,
			attributes: BTreeMap::new(),
		}
	}
}

#[cfg_attr(
	feature = "parity-encoding",
	derive(scale_info::TypeInfo, parity_scale_codec::Encode, parity_scale_codec::Decode)
)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Activity {
	pub id: ActivityId,
	pub namespace_id: NamespaceId,
	pub external_id: ExternalId,
	pub domaintype_id: Option<DomaintypeId>,
	pub attributes: BTreeMap<String, Attribute>,
	pub started: Option<TimeWrapper>,
	pub ended: Option<TimeWrapper>,
}

impl Activity {
	pub fn has_attributes(self, attributes: Attributes) -> Self {
		let Self { id, namespace_id, external_id, started, ended, .. } = self;
		Self {
			id,
			namespace_id,
			external_id,
			started,
			ended,
			domaintype_id: attributes.get_typ().clone(),
			attributes: attributes.get_items().iter().cloned().collect(),
		}
	}

	// Create a prototypical agent from its IRI, we can only determine external_id
	pub fn exists(namespace_id: NamespaceId, id: ActivityId) -> Self {
		Self {
			namespace_id,
			external_id: id.external_id_part().to_owned(),
			id,
			started: None,
			ended: None,
			domaintype_id: None,
			attributes: BTreeMap::new(),
		}
	}
}

#[cfg_attr(
	feature = "parity-encoding",
	derive(scale_info::TypeInfo, parity_scale_codec::Encode, parity_scale_codec::Decode)
)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entity {
	pub id: EntityId,
	pub namespace_id: NamespaceId,
	pub external_id: ExternalId,
	pub domaintypeid: Option<DomaintypeId>,
	pub attributes: BTreeMap<String, Attribute>,
}

impl Entity {
	pub fn has_attributes(self, attributes: Attributes) -> Self {
		let Self { id, namespace_id: namespaceid, external_id, .. } = self;
		Self {
			id,
			namespace_id: namespaceid,
			external_id,
			domaintypeid: attributes.get_typ().clone(),
			attributes: attributes.get_items().iter().cloned().collect(),
		}
	}

	pub fn exists(namespaceid: NamespaceId, id: EntityId) -> Self {
		Self {
			external_id: id.external_id_part().to_owned(),
			id,
			namespace_id: namespaceid,
			domaintypeid: None,
			attributes: BTreeMap::new(),
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[cfg_attr(
	feature = "parity-encoding",
	derive(scale_info::TypeInfo, parity_scale_codec::Encode, parity_scale_codec::Decode)
)]
pub struct Derivation {
	pub generated_id: EntityId,
	pub used_id: EntityId,
	pub activity_id: Option<ActivityId>,
	pub typ: DerivationType,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[cfg_attr(
	feature = "parity-encoding",
	derive(scale_info::TypeInfo, parity_scale_codec::Encode, parity_scale_codec::Decode)
)]
pub struct Delegation {
	pub namespace_id: NamespaceId,
	pub id: DelegationId,
	pub delegate_id: AgentId,
	pub responsible_id: AgentId,
	pub activity_id: Option<ActivityId>,
	pub role: Option<Role>,
}

impl Delegation {
	pub fn new(
		namespace_id: &NamespaceId,
		delegate_id: &AgentId,
		responsible_id: &AgentId,
		activity_id: Option<&ActivityId>,
		role: Option<Role>,
	) -> Self {
		Self {
			namespace_id: namespace_id.clone(),
			id: DelegationId::from_component_ids(
				delegate_id,
				responsible_id,
				activity_id,
				role.as_ref(),
			),
			delegate_id: delegate_id.clone(),
			responsible_id: responsible_id.clone(),
			activity_id: activity_id.cloned(),
			role,
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[cfg_attr(
	feature = "parity-encoding",
	derive(scale_info::TypeInfo, parity_scale_codec::Encode, parity_scale_codec::Decode)
)]

pub struct Association {
	pub namespace_id: NamespaceId,
	pub id: AssociationId,
	pub agent_id: AgentId,
	pub activity_id: ActivityId,
	pub role: Option<Role>,
}

impl Association {
	pub fn new(
		namespace_id: &NamespaceId,
		agent_id: &AgentId,
		activity_id: &ActivityId,
		role: Option<Role>,
	) -> Self {
		Self {
			namespace_id: namespace_id.clone(),
			id: AssociationId::from_component_ids(agent_id, activity_id, role.as_ref()),
			agent_id: agent_id.clone(),
			activity_id: activity_id.clone(),
			role,
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[cfg_attr(
	feature = "parity-encoding",
	derive(scale_info::TypeInfo, parity_scale_codec::Encode, parity_scale_codec::Decode)
)]
pub struct Usage {
	pub activity_id: ActivityId,
	pub entity_id: EntityId,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[cfg_attr(
	feature = "parity-encoding",
	derive(scale_info::TypeInfo, parity_scale_codec::Encode, parity_scale_codec::Decode)
)]
pub struct Generation {
	pub activity_id: ActivityId,
	pub generated_id: EntityId,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(
	feature = "parity-encoding",
	derive(scale_info::TypeInfo, parity_scale_codec::Encode, parity_scale_codec::Decode)
)]
pub struct GeneratedEntity {
	pub entity_id: EntityId,
	pub generated_id: ActivityId,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[cfg_attr(
	feature = "parity-encoding",
	derive(scale_info::TypeInfo, parity_scale_codec::Encode, parity_scale_codec::Decode)
)]
pub struct Attribution {
	pub namespace_id: NamespaceId,
	pub id: AttributionId,
	pub agent_id: AgentId,
	pub entity_id: EntityId,
	pub role: Option<Role>,
}

impl Attribution {
	pub fn new(
		namespace_id: &NamespaceId,
		agent_id: &AgentId,
		entity_id: &EntityId,
		role: Option<Role>,
	) -> Self {
		Self {
			namespace_id: namespace_id.clone(),
			id: AttributionId::from_component_ids(agent_id, entity_id, role.as_ref()),
			agent_id: agent_id.clone(),
			entity_id: entity_id.clone(),
			role,
		}
	}
}

type NamespacedId<T> = (NamespaceId, T);
type NamespacedAgent = NamespacedId<AgentId>;
type NamespacedEntity = NamespacedId<EntityId>;
type NamespacedActivity = NamespacedId<ActivityId>;

#[cfg_attr(
	feature = "parity-encoding",
	derive(scale_info::TypeInfo, parity_scale_codec::Encode, parity_scale_codec::Decode)
)]
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProvModel {
	pub namespaces: BTreeMap<NamespaceId, Arc<Namespace>>,
	pub agents: BTreeMap<NamespacedAgent, Arc<Agent>>,
	pub acted_on_behalf_of: BTreeMap<NamespacedAgent, Arc<BTreeSet<Delegation>>>,
	pub delegation: BTreeMap<NamespacedAgent, Arc<BTreeSet<Delegation>>>,
	pub entities: BTreeMap<NamespacedEntity, Arc<Entity>>,
	pub derivation: BTreeMap<NamespacedEntity, Arc<BTreeSet<Derivation>>>,
	pub generation: BTreeMap<NamespacedEntity, Arc<BTreeSet<Generation>>>,
	pub attribution: BTreeMap<NamespacedEntity, Arc<BTreeSet<Attribution>>>,
	pub activities: BTreeMap<NamespacedActivity, Arc<Activity>>,
	pub was_informed_by: BTreeMap<NamespacedActivity, Arc<BTreeSet<NamespacedActivity>>>,
	pub generated: BTreeMap<NamespacedActivity, Arc<BTreeSet<GeneratedEntity>>>,
	pub association: BTreeMap<NamespacedActivity, Arc<BTreeSet<Association>>>,
	pub usage: BTreeMap<NamespacedActivity, Arc<BTreeSet<Usage>>>,
}

#[cfg(feature = "parity-encoding")]
pub mod provmodel_protocol {
	use super::*;
	#[derive(
		scale_info::TypeInfo,
		parity_scale_codec::Encode,
		parity_scale_codec::Decode,
		Debug,
		Default,
		Clone,
		Serialize,
		Deserialize,
		PartialEq,
		Eq,
	)]
	pub struct ProvModelV1 {
		pub namespaces: BTreeMap<NamespaceId, Arc<Namespace>>, /* We need NamespaceIdV1 /
		                                                        * NamespaceV1 etc, recursively
		                                                        * until there are only primitive
		                                                        * types */
		pub agents: BTreeMap<NamespacedAgent, Arc<Agent>>,
		pub acted_on_behalf_of: BTreeMap<NamespacedAgent, Arc<BTreeSet<Delegation>>>,
		pub delegation: BTreeMap<NamespacedAgent, Arc<BTreeSet<Delegation>>>,
		pub entities: BTreeMap<NamespacedEntity, Arc<Entity>>,
		pub derivation: BTreeMap<NamespacedEntity, Arc<BTreeSet<Derivation>>>,
		pub generation: BTreeMap<NamespacedEntity, Arc<BTreeSet<Generation>>>,
		pub attribution: BTreeMap<NamespacedEntity, Arc<BTreeSet<Attribution>>>,
		pub activities: BTreeMap<NamespacedActivity, Arc<Activity>>,
		pub was_informed_by: BTreeMap<NamespacedActivity, Arc<BTreeSet<NamespacedActivity>>>,
		pub generated: BTreeMap<NamespacedActivity, Arc<BTreeSet<GeneratedEntity>>>,
		pub association: BTreeMap<NamespacedActivity, Arc<BTreeSet<Association>>>,
		pub usage: BTreeMap<NamespacedActivity, Arc<BTreeSet<Usage>>>,
	}

	impl From<ProvModelV1> for ProvModel {
		fn from(value: ProvModelV1) -> Self {
			ProvModel {
				namespaces: value.namespaces,
				agents: value.agents,
				acted_on_behalf_of: value.acted_on_behalf_of,
				delegation: value.delegation,
				entities: value.entities,
				derivation: value.derivation,
				generation: value.generation,
				attribution: value.attribution,
				activities: value.activities,
				was_informed_by: value.was_informed_by,
				generated: value.generated,
				association: value.association,
				usage: value.usage,
			}
		}
	}
}

#[cfg(feature = "parity-encoding")]
// TODO: We can make these structures reasonably bounded (and copy ids with interning) - though JSON
// attributes may need some handwaving
impl parity_scale_codec::MaxEncodedLen for ProvModel {
	fn max_encoded_len() -> usize {
		64 * 1024usize
	}
}

impl ProvModel {
	/// Merge the supplied ProvModel into this one
	pub fn combine(&mut self, other: &ProvModel) {
		self.namespaces.extend(other.namespaces.clone());
		self.agents.extend(other.agents.clone());
		self.acted_on_behalf_of.extend(other.acted_on_behalf_of.clone());
		self.delegation.extend(other.delegation.clone());
		self.entities.extend(other.entities.clone());
		self.derivation.extend(other.derivation.clone());
		self.generation.extend(other.generation.clone());
		self.attribution.extend(other.attribution.clone());
		self.activities.extend(other.activities.clone());
		self.was_informed_by.extend(other.was_informed_by.clone());
		self.generated.extend(other.generated.clone());
		self.association.extend(other.association.clone());
		self.usage.extend(other.usage.clone());
	}

	/// Apply a sequence of `ChronicleTransaction` to an empty model, then return it
	pub fn from_tx<'a, I>(tx: I) -> Result<Self, Contradiction>
	where
		I: IntoIterator<Item = &'a ChronicleOperation>,
	{
		let mut model = Self::default();
		for tx in tx {
			model.apply(tx)?;
		}

		Ok(model)
	}

	/// Append a derivation to the model
	pub fn was_derived_from(
		&mut self,
		namespace_id: NamespaceId,
		typ: DerivationType,
		used_id: EntityId,
		id: EntityId,
		activity_id: Option<ActivityId>,
	) {
		let derivation_set =
			Arc::make_mut(self.derivation.entry((namespace_id, id.clone())).or_default());

		derivation_set.insert(Derivation { typ, generated_id: id, used_id, activity_id });
	}

	/// Append a delegation to the model
	pub fn qualified_delegation(
		&mut self,
		namespace_id: &NamespaceId,
		responsible_id: &AgentId,
		delegate_id: &AgentId,
		activity_id: Option<ActivityId>,
		role: Option<Role>,
	) {
		let delegation = Delegation {
			namespace_id: namespace_id.clone(),
			id: DelegationId::from_component_ids(
				delegate_id,
				responsible_id,
				activity_id.as_ref(),
				role.as_ref(),
			),
			responsible_id: responsible_id.clone(),
			delegate_id: delegate_id.clone(),
			activity_id,
			role,
		};

		let delegation_set = Arc::make_mut(
			self.delegation
				.entry((namespace_id.clone(), responsible_id.clone()))
				.or_default(),
		);
		delegation_set.insert(delegation.clone());

		let acted_on_behalf_of_set = Arc::make_mut(
			self.acted_on_behalf_of
				.entry((namespace_id.clone(), responsible_id.clone()))
				.or_default(),
		);

		acted_on_behalf_of_set.insert(delegation);
	}

	pub fn qualified_association(
		&mut self,
		namespace_id: &NamespaceId,
		activity_id: &ActivityId,
		agent_id: &AgentId,
		role: Option<Role>,
	) {
		let association_set = Arc::make_mut(
			self.association.entry((namespace_id.clone(), activity_id.clone())).or_default(),
		);

		association_set.insert(Association {
			namespace_id: namespace_id.clone(),
			id: AssociationId::from_component_ids(agent_id, activity_id, role.as_ref()),
			agent_id: agent_id.clone(),
			activity_id: activity_id.clone(),
			role,
		});
	}

	pub fn was_generated_by(
		&mut self,
		namespace_id: NamespaceId,
		generated_id: &EntityId,
		activity_id: &ActivityId,
	) {
		let generation_set = Arc::make_mut(
			self.generation.entry((namespace_id.clone(), generated_id.clone())).or_default(),
		);
		generation_set.insert(Generation {
			activity_id: activity_id.clone(),
			generated_id: generated_id.clone(),
		});
	}

	pub fn generated(
		&mut self,
		namespace_id: NamespaceId,
		generated_id: &ActivityId,
		entity_id: &EntityId,
	) {
		let generated_set = Arc::make_mut(
			self.generated.entry((namespace_id.clone(), generated_id.clone())).or_default(),
		);

		generated_set.insert(GeneratedEntity {
			entity_id: entity_id.clone(),
			generated_id: generated_id.clone(),
		});
	}

	pub fn used(
		&mut self,
		namespace_id: NamespaceId,
		activity_id: &ActivityId,
		entity_id: &EntityId,
	) {
		let usage_set = Arc::make_mut(
			self.usage.entry((namespace_id.clone(), activity_id.clone())).or_default(),
		);

		usage_set.insert(Usage { activity_id: activity_id.clone(), entity_id: entity_id.clone() });
	}

	pub fn was_informed_by(
		&mut self,
		namespace_id: NamespaceId,
		activity_id: &ActivityId,
		informing_activity_id: &ActivityId,
	) {
		let was_informed_by_set = Arc::make_mut(
			self.was_informed_by
				.entry((namespace_id.clone(), activity_id.clone()))
				.or_default(),
		);

		was_informed_by_set.insert((namespace_id, informing_activity_id.clone()));
	}

	pub fn qualified_attribution(
		&mut self,
		namespace_id: &NamespaceId,
		entity_id: &EntityId,
		agent_id: &AgentId,
		role: Option<Role>,
	) {
		let attribution_set = Arc::make_mut(
			self.attribution.entry((namespace_id.clone(), entity_id.clone())).or_default(),
		);

		attribution_set.insert(Attribution {
			namespace_id: namespace_id.clone(),
			id: AttributionId::from_component_ids(agent_id, entity_id, role.as_ref()),
			agent_id: agent_id.clone(),
			entity_id: entity_id.clone(),
			role,
		});
	}

	/// Ensure we have the referenced namespace in our model
	pub fn namespace_context(&mut self, ns: &NamespaceId) {
		let (namespace_name, uuid) = (ns.external_id_part(), ns.uuid_part());

		self.namespaces.insert(
			ns.clone(),
			Namespace {
				id: ns.clone(),
				uuid: uuid.into_bytes(),
				external_id: namespace_name.to_owned(),
			}
			.into(),
		);
	}

	/// Ensure we have the referenced agent in our model, so that open world
	/// assumptions can be made
	pub fn agent_context(&mut self, ns: &NamespaceId, agent: &AgentId) {
		self.agents
			.entry((ns.clone(), agent.clone()))
			.or_insert_with(|| Agent::exists(ns.clone(), agent.clone()).into());
	}

	pub fn get_agent(&mut self, ns: &NamespaceId, agent: &AgentId) -> Option<&Agent> {
		self.agents.get(&(ns.clone(), agent.clone())).map(|arc| arc.as_ref())
	}

	pub fn modify_agent<F: FnOnce(&mut Agent) + 'static>(
		&mut self,
		ns: &NamespaceId,
		agent: &AgentId,
		f: F,
	) {
		if let Some(arc) = self.agents.get_mut(&(ns.clone(), agent.clone())) {
			let agent: &mut Agent = Arc::make_mut(arc);
			f(agent);
		}
	}

	/// Ensure we have the referenced entity in our model, so that open world
	/// assumptions can be made
	pub fn entity_context(&mut self, ns: &NamespaceId, entity: &EntityId) {
		self.entities
			.entry((ns.clone(), entity.clone()))
			.or_insert_with(|| Entity::exists(ns.clone(), entity.clone()).into());
	}

	pub fn get_entity(&mut self, ns: &NamespaceId, entity: &EntityId) -> Option<&Entity> {
		self.entities.get(&(ns.clone(), entity.clone())).map(|arc| arc.as_ref())
	}

	pub fn modify_entity<F: FnOnce(&mut Entity) + 'static>(
		&mut self,
		ns: &NamespaceId,
		entity: &EntityId,
		f: F,
	) {
		if let Some(arc) = self.entities.get_mut(&(ns.clone(), entity.clone())) {
			let entity: &mut Entity = Arc::make_mut(arc);
			f(entity);
		}
	}

	/// Ensure we have the referenced activity in our model, so that open world
	/// assumptions can be made
	pub fn activity_context(&mut self, ns: &NamespaceId, activity: &ActivityId) {
		self.activities
			.entry((ns.clone(), activity.clone()))
			.or_insert_with(|| Activity::exists(ns.clone(), activity.clone()).into());
	}

	pub fn get_activity(&mut self, ns: &NamespaceId, activity: &ActivityId) -> Option<&Activity> {
		self.activities.get(&(ns.clone(), activity.clone())).map(|arc| arc.as_ref())
	}

	pub fn modify_activity<F: FnOnce(&mut Activity) + 'static>(
		&mut self,
		ns: &NamespaceId,
		activity: &ActivityId,
		f: F,
	) {
		if let Some(arc) = self.activities.get_mut(&(ns.clone(), activity.clone())) {
			let activity: &mut Activity = Arc::make_mut(arc);
			f(activity);
		}
	}

	/// Transform a sequence of `ChronicleOperation` events into a provenance model,
	/// If a statement requires a subject or object that does not currently exist in the model, then
	/// we create it If an operation contradicts a previous statement, then we record the
	/// contradiction, but attempt to apply as much of the operation as possible
	#[instrument(skip(self,tx), level = "trace", name="apply_chronicle_operation", fields(op = ?tx, model= ?self), ret(Debug))]
	pub fn apply(&mut self, tx: &ChronicleOperation) -> Result<(), Contradiction> {
		let tx = tx.to_owned();
		match tx {
			ChronicleOperation::CreateNamespace(CreateNamespace { id }) => {
				self.namespace_context(&id);
				Ok(())
			},
			ChronicleOperation::AgentExists(AgentExists { namespace, id, .. }) => {
				self.agent_context(&namespace, &id);

				Ok(())
			},
			ChronicleOperation::AgentActsOnBehalfOf(ActsOnBehalfOf {
				id: _,
				namespace,
				delegate_id,
				activity_id,
				role,
				responsible_id,
			}) => {
				self.agent_context(&namespace, &delegate_id);
				self.agent_context(&namespace, &responsible_id);

				if let Some(activity_id) = activity_id.clone() {
					self.activity_context(&namespace, &activity_id);
				}

				self.qualified_delegation(
					&namespace,
					&responsible_id,
					&delegate_id,
					activity_id,
					role,
				);

				Ok(())
			},
			ChronicleOperation::ActivityExists(ActivityExists { namespace, id, .. }) => {
				self.activity_context(&namespace, &id);

				Ok(())
			},
			ChronicleOperation::StartActivity(StartActivity { namespace, id, time }) => {
				self.activity_context(&namespace, &id);

				let activity = self.get_activity(&namespace, &id);

				trace!(check_start_contradiction = ?time, existing_time=?activity.and_then(|activity| activity.started));
				match (
					activity.and_then(|activity| activity.started),
					activity.and_then(|activity| activity.ended),
				) {
					(Some(TimeWrapper(started)), _) if started != time.0 => {
						return Err(Contradiction::start_date_alteration(
							id.into(),
							namespace,
							started,
							time.0,
						))
					},
					(_, Some(TimeWrapper(ended))) if ended < time.0 => {
						return Err(Contradiction::invalid_range(
							id.into(),
							namespace,
							time.0,
							ended,
						))
					},
					_ => {},
				};

				self.modify_activity(&namespace, &id, move |activity| {
					activity.started = Some(time);
				});

				Ok(())
			},
			ChronicleOperation::EndActivity(EndActivity { namespace, id, time }) => {
				self.activity_context(&namespace, &id);

				let activity = self.get_activity(&namespace, &id);

				trace!(check_end_contradiction = ?time, existing_time=?activity.and_then(|activity| activity.ended));
				match (
					activity.and_then(|activity| activity.started),
					activity.and_then(|activity| activity.ended),
				) {
					(_, Some(TimeWrapper(ended))) if ended != time.0 => {
						return Err(Contradiction::end_date_alteration(
							id.into(),
							namespace,
							ended,
							time.0,
						))
					},
					(Some(TimeWrapper(started)), _) if started > time.0 => {
						return Err(Contradiction::invalid_range(
							id.into(),
							namespace,
							started,
							time.0,
						))
					},
					_ => {},
				};

				self.modify_activity(&namespace, &id, move |activity| {
					activity.ended = Some(time);
				});

				Ok(())
			},
			ChronicleOperation::WasAssociatedWith(WasAssociatedWith {
				id: _,
				role,
				namespace,
				activity_id,
				agent_id,
			}) => {
				self.agent_context(&namespace, &agent_id);
				self.activity_context(&namespace, &activity_id);
				self.qualified_association(&namespace, &activity_id, &agent_id, role);

				Ok(())
			},
			ChronicleOperation::WasAttributedTo(WasAttributedTo {
				id: _,
				role,
				namespace,
				entity_id,
				agent_id,
			}) => {
				self.agent_context(&namespace, &agent_id);
				self.entity_context(&namespace, &entity_id);
				self.qualified_attribution(&namespace, &entity_id, &agent_id, role);

				Ok(())
			},
			ChronicleOperation::ActivityUses(ActivityUses { namespace, id, activity }) => {
				self.activity_context(&namespace, &activity);
				self.entity_context(&namespace, &id);

				self.used(namespace, &activity, &id);

				Ok(())
			},
			ChronicleOperation::EntityExists(EntityExists { namespace, id, .. }) => {
				self.entity_context(&namespace, &id);
				Ok(())
			},
			ChronicleOperation::WasGeneratedBy(WasGeneratedBy { namespace, id, activity }) => {
				self.entity_context(&namespace, &id);
				self.activity_context(&namespace, &activity);

				self.was_generated_by(namespace, &id, &activity);

				Ok(())
			},
			ChronicleOperation::WasInformedBy(WasInformedBy {
				namespace,
				activity,
				informing_activity,
			}) => {
				self.activity_context(&namespace, &activity);
				self.activity_context(&namespace, &informing_activity);

				self.was_informed_by(namespace, &activity, &informing_activity);

				Ok(())
			},
			ChronicleOperation::EntityDerive(EntityDerive {
				namespace,
				id,
				typ,
				used_id,
				activity_id,
			}) => {
				self.entity_context(&namespace, &id);
				self.entity_context(&namespace, &used_id);

				if let Some(activity_id) = &activity_id {
					self.activity_context(&namespace, activity_id);
				}

				self.was_derived_from(namespace, typ, used_id, id, activity_id);

				Ok(())
			},
			ChronicleOperation::SetAttributes(SetAttributes::Entity {
				namespace,
				id,
				attributes,
			}) => {
				self.entity_context(&namespace, &id);

				if let Some(current) = self
					.entities
					.get(&(namespace.clone(), id.clone()))
					.map(|entity| &entity.attributes)
				{
					Self::validate_attribute_changes(
						&id.clone().into(),
						&namespace,
						current,
						&attributes,
					)?;
				};

				self.modify_entity(&namespace, &id, move |entity| {
					entity.domaintypeid = attributes.get_typ().clone();
					entity.attributes = attributes.get_items().iter().cloned().collect();
				});

				Ok(())
			},
			ChronicleOperation::SetAttributes(SetAttributes::Activity {
				namespace,
				id,
				attributes,
			}) => {
				self.activity_context(&namespace, &id);

				if let Some(current) = self
					.activities
					.get(&(namespace.clone(), id.clone()))
					.map(|activity| &activity.attributes)
				{
					Self::validate_attribute_changes(
						&id.clone().into(),
						&namespace,
						current,
						&attributes,
					)?;
				};

				self.modify_activity(&namespace, &id, move |activity| {
					activity.domaintype_id = attributes.get_typ().clone();
					activity.attributes = attributes.get_items().iter().cloned().collect();
				});

				Ok(())
			},
			ChronicleOperation::SetAttributes(SetAttributes::Agent {
				namespace,
				id,
				attributes,
			}) => {
				self.agent_context(&namespace, &id);

				if let Some(current) =
					self.agents.get(&(namespace.clone(), id.clone())).map(|agent| &agent.attributes)
				{
					Self::validate_attribute_changes(
						&id.clone().into(),
						&namespace,
						current,
						&attributes,
					)?;
				};

				self.modify_agent(&namespace, &id, move |agent| {
					agent.domaintypeid = attributes.get_typ().clone();
					agent.attributes = attributes.get_items().iter().cloned().collect();
				});

				Ok(())
			},
		}
	}

	/// Allow additional attributes, but changing an existing attribute is not allowed
	#[instrument(level = "trace", ret(Debug))]
	fn validate_attribute_changes(
		id: &ChronicleIri,
		namespace: &NamespaceId,
		current: &BTreeMap<String, Attribute>,
		attempted: &Attributes,
	) -> Result<(), Contradiction> {
		let contradictions = attempted
			.get_items()
			.iter()
			.filter_map(|(current_name, current_value)| {
				if let Some(attempted_value) = current.get(current_name) {
					if current_value != attempted_value {
						Some((current_name.clone(), current_value.clone(), attempted_value.clone()))
					} else {
						None
					}
				} else {
					None
				}
			})
			.collect::<Vec<_>>();

		if contradictions.is_empty() {
			Ok(())
		} else {
			Err(Contradiction::attribute_value_change(
				id.clone(),
				namespace.clone(),
				contradictions,
			))
		}
	}

	#[cfg(feature = "json-ld")]
	pub(crate) fn add_agent(&mut self, agent: Agent) {
		self.agents.insert((agent.namespaceid.clone(), agent.id.clone()), agent.into());
	}

	#[cfg(feature = "json-ld")]
	pub(crate) fn add_activity(&mut self, activity: Activity) {
		self.activities
			.insert((activity.namespace_id.clone(), activity.id.clone()), activity.into());
	}

	#[cfg(feature = "json-ld")]
	pub(crate) fn add_entity(&mut self, entity: Entity) {
		self.entities
			.insert((entity.namespace_id.clone(), entity.id.clone()), entity.into());
	}
}
