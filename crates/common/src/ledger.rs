use scale_info::TypeInfo;
use tracing::instrument;

use crate::{
	identity::SignedIdentity,
	prov::{
		operations::{
			ActivityExists, ActivityUses, ActsOnBehalfOf, AgentExists, ChronicleOperation,
			CreateNamespace, EndActivity, EntityDerive, EntityExists, SetAttributes, StartActivity,
			WasAssociatedWith, WasAttributedTo, WasGeneratedBy, WasInformedBy,
		},
		ActivityId, AgentId, ChronicleIri, ChronicleTransactionId, Contradiction, EntityId,
		NamespaceId, ParseIriError, ProcessorError, ProvModel,
	},
};

#[cfg(not(feature = "std"))]
use core::str::FromStr;
#[cfg(not(feature = "std"))]
use parity_scale_codec::{
	alloc::boxed::Box, alloc::collections::btree_map::Entry, alloc::collections::BTreeMap,
	alloc::collections::BTreeSet, alloc::string::String, alloc::sync::Arc, alloc::vec::Vec,
};
#[cfg(not(feature = "std"))]
use scale_info::prelude::*;
#[cfg(feature = "std")]
use std::{
	boxed::Box, collections::btree_map::Entry, collections::BTreeMap, collections::BTreeSet,
	str::FromStr, sync::Arc,
};

#[derive(Debug, Clone)]
pub enum SubmissionError {
	Communication { source: Arc<anyhow::Error>, tx_id: ChronicleTransactionId },
	Processor { source: Arc<ProcessorError>, tx_id: ChronicleTransactionId },
	Contradiction { source: Contradiction, tx_id: ChronicleTransactionId },
}

impl SubmissionError {
	pub fn tx_id(&self) -> &ChronicleTransactionId {
		match self {
			SubmissionError::Communication { tx_id, .. } => tx_id,
			SubmissionError::Processor { tx_id, .. } => tx_id,
			SubmissionError::Contradiction { tx_id, .. } => tx_id,
		}
	}

	pub fn processor(tx_id: &ChronicleTransactionId, source: ProcessorError) -> SubmissionError {
		SubmissionError::Processor { source: Arc::new(source), tx_id: tx_id.clone() }
	}

	pub fn contradiction(tx_id: &ChronicleTransactionId, source: Contradiction) -> SubmissionError {
		SubmissionError::Contradiction { source, tx_id: tx_id.clone() }
	}

	pub fn communication(tx_id: &ChronicleTransactionId, source: anyhow::Error) -> SubmissionError {
		SubmissionError::Communication { source: Arc::new(source), tx_id: tx_id.clone() }
	}
}

#[derive(Debug)]
pub enum SubscriptionError {
	Implementation { source: anyhow::Error },
}

impl core::fmt::Display for SubscriptionError {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		match self {
			Self::Implementation { .. } => write!(f, "Subscription error"),
		}
	}
}

impl core::fmt::Display for SubmissionError {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		match self {
			Self::Communication { source, .. } => write!(f, "Ledger error {source} "),
			Self::Processor { source, .. } => write!(f, "Processor error {source} "),
			Self::Contradiction { source, .. } => write!(f, "Contradiction: {source}"),
		}
	}
}

pub type SubmitResult = Result<ChronicleTransactionId, SubmissionError>;

#[derive(Debug, Clone)]
pub struct Commit {
	pub tx_id: ChronicleTransactionId,
	pub block_id: String,
	pub delta: Box<ProvModel>,
}

impl Commit {
	pub fn new(tx_id: ChronicleTransactionId, block_id: String, delta: Box<ProvModel>) -> Self {
		Commit { tx_id, block_id, delta }
	}
}

pub type CommitResult = Result<Commit, (ChronicleTransactionId, Contradiction)>;

#[derive(Debug, Clone)]
pub enum SubmissionStage {
	Submitted(SubmitResult),
	Committed(Commit, Box<SignedIdentity>),
	NotCommitted((ChronicleTransactionId, Contradiction, Box<SignedIdentity>)),
}

impl SubmissionStage {
	pub fn submitted_error(r: &SubmissionError) -> Self {
		SubmissionStage::Submitted(Err(r.clone()))
	}

	pub fn submitted(r: &ChronicleTransactionId) -> Self {
		SubmissionStage::Submitted(Ok(r.clone()))
	}

	pub fn committed(commit: Commit, identity: SignedIdentity) -> Self {
		SubmissionStage::Committed(commit, identity.into())
	}

	pub fn not_committed(
		tx: ChronicleTransactionId,
		contradiction: Contradiction,
		identity: SignedIdentity,
	) -> Self {
		SubmissionStage::NotCommitted((tx, contradiction, identity.into()))
	}

	pub fn tx_id(&self) -> &ChronicleTransactionId {
		match self {
			Self::Submitted(tx_id) => match tx_id {
				Ok(tx_id) => tx_id,
				Err(e) => e.tx_id(),
			},
			Self::Committed(commit, _) => &commit.tx_id,
			Self::NotCommitted((tx_id, _, _)) => tx_id,
		}
	}
}

#[derive(
	parity_scale_codec::Encode,
	parity_scale_codec::Decode,
	TypeInfo,
	PartialEq,
	Eq,
	PartialOrd,
	Ord,
	Debug,
	Clone,
)]
pub struct LedgerAddress {
	// Namespaces do not have a namespace
	namespace: Option<NamespaceId>,
	resource: ChronicleIri,
}

impl parity_scale_codec::MaxEncodedLen for LedgerAddress {
	fn max_encoded_len() -> usize {
		2048usize
	}
}

impl core::fmt::Display for LedgerAddress {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		if let Some(namespace) = &self.namespace {
			write!(f, "{}:{}", namespace, self.resource)
		} else {
			write!(f, "{}", self.resource)
		}
	}
}

pub trait NameSpacePart {
	fn namespace_part(&self) -> Option<NamespaceId>;
}

impl NameSpacePart for LedgerAddress {
	fn namespace_part(&self) -> Option<NamespaceId> {
		self.namespace.clone()
	}
}

pub trait ResourcePart {
	fn resource_part(&self) -> ChronicleIri;
}

impl ResourcePart for LedgerAddress {
	fn resource_part(&self) -> ChronicleIri {
		self.resource.clone()
	}
}

impl LedgerAddress {
	fn from_ld(ns: Option<&str>, resource: &str) -> Result<Self, ParseIriError> {
		Ok(Self {
			namespace: if let Some(ns) = ns {
				Some(ChronicleIri::from_str(ns)?.namespace()?)
			} else {
				None
			},
			resource: ChronicleIri::from_str(resource)?,
		})
	}

	fn namespace(ns: &NamespaceId) -> Self {
		Self { namespace: None, resource: ns.clone().into() }
	}

	fn in_namespace(ns: &NamespaceId, resource: impl Into<ChronicleIri>) -> Self {
		Self { namespace: Some(ns.clone()), resource: resource.into() }
	}
}

// Split a ProvModel into a snapshot list of its components - Namespaces, Entities, Activities and
// Agents
pub trait ProvSnapshot {
	fn to_snapshot(&self) -> Vec<((Option<NamespaceId>, ChronicleIri), ProvModel)>;
}

impl ProvSnapshot for ProvModel {
	fn to_snapshot(&self) -> Vec<((Option<NamespaceId>, ChronicleIri), ProvModel)> {
		let mut snapshot = Vec::new();

		for (namespace_id, namespace) in &self.namespaces {
			snapshot.push((
				(None, namespace_id.clone().into()),
				ProvModel {
					namespaces: vec![(namespace_id.clone(), namespace.clone())]
						.into_iter()
						.collect(),
					..Default::default()
				},
			));
		}

		for ((ns, agent_id), agent) in &self.agents {
			let mut delegation = BTreeMap::new();
			if let Some(delegation_set) = self.delegation.get(&(ns.clone(), agent_id.clone())) {
				delegation.insert((ns.clone(), agent_id.clone()), delegation_set.clone());
			}
			let mut acted_on_behalf_of = BTreeMap::new();
			if let Some(acted_on_behalf_of_set) =
				self.acted_on_behalf_of.get(&(ns.clone(), agent_id.clone()))
			{
				acted_on_behalf_of
					.insert((ns.clone(), agent_id.clone()), acted_on_behalf_of_set.clone());
			}
			snapshot.push((
				(Some(ns.clone()), agent_id.clone().into()),
				ProvModel {
					agents: vec![((ns.clone(), agent_id.clone()), agent.clone())]
						.into_iter()
						.collect(),
					delegation,
					acted_on_behalf_of,
					..Default::default()
				},
			));
		}

		for ((ns, activity_id), activity) in &self.activities {
			let mut was_informed_by = BTreeMap::new();
			if let Some(was_informed_by_set) =
				self.was_informed_by.get(&(ns.clone(), activity_id.clone()))
			{
				was_informed_by
					.insert((ns.clone(), activity_id.clone()), was_informed_by_set.clone());
			}
			let mut generated = BTreeMap::new();
			if let Some(generated_set) = self.generated.get(&(ns.clone(), activity_id.clone())) {
				generated.insert((ns.clone(), activity_id.clone()), generated_set.clone());
			}
			let mut usage = BTreeMap::new();
			if let Some(usage_set) = self.usage.get(&(ns.clone(), activity_id.clone())) {
				usage.insert((ns.clone(), activity_id.clone()), usage_set.clone());
			}
			let mut association = BTreeMap::new();
			if let Some(association_set) = self.association.get(&(ns.clone(), activity_id.clone()))
			{
				association.insert((ns.clone(), activity_id.clone()), association_set.clone());
			}

			snapshot.push((
				(Some(ns.clone()), activity_id.clone().into()),
				ProvModel {
					activities: vec![((ns.clone(), activity_id.clone()), activity.clone())]
						.into_iter()
						.collect(),
					was_informed_by,
					usage,
					generated,
					association,
					..Default::default()
				},
			));
		}

		for ((ns, entity_id), entity) in &self.entities {
			let mut derivation = BTreeMap::new();
			if let Some(derivation_set) = self.derivation.get(&(ns.clone(), entity_id.clone())) {
				derivation.insert((ns.clone(), entity_id.clone()), derivation_set.clone());
			}
			let mut generation = BTreeMap::new();
			if let Some(generation_set) = self.generation.get(&(ns.clone(), entity_id.clone())) {
				generation.insert((ns.clone(), entity_id.clone()), generation_set.clone());
			}
			let mut attribution = BTreeMap::new();
			if let Some(attribution_set) = self.attribution.get(&(ns.clone(), entity_id.clone())) {
				attribution.insert((ns.clone(), entity_id.clone()), attribution_set.clone());
			}
			snapshot.push((
				(Some(ns.clone()), entity_id.clone().into()),
				ProvModel {
					entities: vec![((ns.clone(), entity_id.clone()), entity.clone())]
						.into_iter()
						.collect(),
					derivation,
					generation,
					attribution,
					..Default::default()
				},
			));
		}

		snapshot
	}
}

#[derive(Debug, Clone)]
pub struct StateInput {
	data: ProvModel,
}

impl StateInput {
	pub fn new(data: ProvModel) -> Self {
		Self { data }
	}

	pub fn data(&self) -> &ProvModel {
		&self.data
	}
}

#[derive(Debug)]
pub struct StateOutput {
	pub address: LedgerAddress,
	pub data: ProvModel,
}

impl StateOutput {
	pub fn new(address: LedgerAddress, data: ProvModel) -> Self {
		Self { address, data }
	}

	pub fn address(&self) -> &LedgerAddress {
		&self.address
	}

	pub fn data(&self) -> &ProvModel {
		&self.data
	}
}

#[derive(Debug, Clone)]
pub struct Version {
	pub(crate) version: u32,
	pub(crate) value: Option<ProvModel>,
}

impl Version {
	pub fn write(&mut self, value: Option<ProvModel>) {
		if value != self.value {
			self.version += 1;
			self.value = value
		}
	}
}

/// Hold a cache of `LedgerWriter::submit` input and output address data
pub struct OperationState {
	state: BTreeMap<LedgerAddress, Version>,
}

impl Default for OperationState {
	fn default() -> Self {
		Self::new()
	}
}

impl OperationState {
	pub fn new() -> Self {
		Self { state: BTreeMap::new() }
	}

	pub fn update_state_from_output(&mut self, output: impl Iterator<Item = StateOutput>) {
		self.update_state(output.map(|output| (output.address, Some(output.data))))
	}
	/// Load input values into `OperationState`
	pub fn update_state(
		&mut self,
		input: impl Iterator<Item = (LedgerAddress, Option<ProvModel>)>,
	) {
		input.for_each(|(address, value)| {
			let entry = self.state.entry(address);
			if let Entry::Vacant(e) = entry {
				e.insert(Version { version: 0, value });
			} else if let Entry::Occupied(mut e) = entry {
				e.get_mut().write(value);
			}
		});
	}

	/// Return the input data held in `OperationState`
	/// as a vector of `StateInput`s
	pub fn input(&self) -> Vec<StateInput> {
		self.state
			.values()
			.cloned()
			.filter_map(|v| v.value.map(StateInput::new))
			.collect()
	}

	/// Check if the data associated with an address has changed in processing
	/// while outputting a stream of dirty `StateOutput`s
	pub fn dirty(self) -> impl Iterator<Item = StateOutput> {
		self.state
			.into_iter()
			.filter_map(|(addr, data)| {
				if data.version > 0 {
					data.value.map(|value| (StateOutput::new(addr, value)))
				} else {
					None
				}
			})
			.collect::<Vec<_>>()
			.into_iter()
	}

	/// Return the input data held in `OperationState` for `addresses` as a vector of `StateInput`s
	pub fn opa_context(&self, addresses: BTreeSet<LedgerAddress>) -> Vec<StateInput> {
		self.state
			.iter()
			.filter(|(addr, _data)| addresses.iter().any(|a| &a == addr))
			.map(|(_, data)| data.clone())
			.filter_map(|v| v.value.map(StateInput::new))
			.collect()
	}
}

impl ChronicleOperation {
	/// Compute dependencies for a chronicle operation, input and output addresses are always
	/// symmetric
	pub fn dependencies(&self) -> Vec<LedgerAddress> {
		match self {
			ChronicleOperation::CreateNamespace(CreateNamespace { id, .. }) => {
				vec![LedgerAddress::namespace(id)]
			},
			ChronicleOperation::AgentExists(AgentExists { namespace, external_id, .. }) => {
				vec![
					LedgerAddress::namespace(namespace),
					LedgerAddress::in_namespace(namespace, AgentId::from_external_id(external_id)),
				]
			},
			ChronicleOperation::ActivityExists(ActivityExists {
				namespace, external_id, ..
			}) => {
				vec![
					LedgerAddress::namespace(namespace),
					LedgerAddress::in_namespace(
						namespace,
						ActivityId::from_external_id(external_id),
					),
				]
			},
			ChronicleOperation::StartActivity(StartActivity { namespace, id, .. }) => {
				vec![
					LedgerAddress::namespace(namespace),
					LedgerAddress::in_namespace(namespace, id.clone()),
				]
			},
			ChronicleOperation::WasAssociatedWith(WasAssociatedWith {
				id,
				namespace,
				activity_id,
				agent_id,
				..
			}) => vec![
				LedgerAddress::namespace(namespace),
				LedgerAddress::in_namespace(namespace, id.clone()),
				LedgerAddress::in_namespace(namespace, activity_id.clone()),
				LedgerAddress::in_namespace(namespace, agent_id.clone()),
			],
			ChronicleOperation::WasAttributedTo(WasAttributedTo {
				id,
				namespace,
				entity_id,
				agent_id,
				..
			}) => vec![
				LedgerAddress::namespace(namespace),
				LedgerAddress::in_namespace(namespace, id.clone()),
				LedgerAddress::in_namespace(namespace, entity_id.clone()),
				LedgerAddress::in_namespace(namespace, agent_id.clone()),
			],
			ChronicleOperation::EndActivity(EndActivity { namespace, id, .. }) => {
				vec![
					LedgerAddress::namespace(namespace),
					LedgerAddress::in_namespace(namespace, id.clone()),
				]
			},
			ChronicleOperation::ActivityUses(ActivityUses { namespace, id, activity }) => {
				vec![
					LedgerAddress::namespace(namespace),
					LedgerAddress::in_namespace(namespace, activity.clone()),
					LedgerAddress::in_namespace(namespace, id.clone()),
				]
			},
			ChronicleOperation::EntityExists(EntityExists { namespace, external_id }) => {
				vec![
					LedgerAddress::namespace(namespace),
					LedgerAddress::in_namespace(namespace, EntityId::from_external_id(external_id)),
				]
			},
			ChronicleOperation::WasGeneratedBy(WasGeneratedBy { namespace, id, activity }) => vec![
				LedgerAddress::namespace(namespace),
				LedgerAddress::in_namespace(namespace, activity.clone()),
				LedgerAddress::in_namespace(namespace, id.clone()),
			],
			ChronicleOperation::WasInformedBy(WasInformedBy {
				namespace,
				activity,
				informing_activity,
			}) => {
				vec![
					LedgerAddress::namespace(namespace),
					LedgerAddress::in_namespace(namespace, activity.clone()),
					LedgerAddress::in_namespace(namespace, informing_activity.clone()),
				]
			},
			ChronicleOperation::AgentActsOnBehalfOf(ActsOnBehalfOf {
				namespace,
				id,
				delegate_id,
				activity_id,
				responsible_id,
				..
			}) => vec![
				Some(LedgerAddress::namespace(namespace)),
				activity_id
					.as_ref()
					.map(|activity_id| LedgerAddress::in_namespace(namespace, activity_id.clone())),
				Some(LedgerAddress::in_namespace(namespace, delegate_id.clone())),
				Some(LedgerAddress::in_namespace(namespace, responsible_id.clone())),
				Some(LedgerAddress::in_namespace(namespace, id.clone())),
			]
			.into_iter()
			.flatten()
			.collect(),
			ChronicleOperation::EntityDerive(EntityDerive {
				namespace,
				id,
				used_id,
				activity_id,
				..
			}) => vec![
				Some(LedgerAddress::namespace(namespace)),
				activity_id
					.as_ref()
					.map(|activity_id| LedgerAddress::in_namespace(namespace, activity_id.clone())),
				Some(LedgerAddress::in_namespace(namespace, used_id.clone())),
				Some(LedgerAddress::in_namespace(namespace, id.clone())),
			]
			.into_iter()
			.flatten()
			.collect(),
			ChronicleOperation::SetAttributes(SetAttributes::Agent { id, namespace, .. }) => {
				vec![
					LedgerAddress::namespace(namespace),
					LedgerAddress::in_namespace(namespace, id.clone()),
				]
			},
			ChronicleOperation::SetAttributes(SetAttributes::Entity { id, namespace, .. }) => {
				vec![
					LedgerAddress::namespace(namespace),
					LedgerAddress::in_namespace(namespace, id.clone()),
				]
			},
			ChronicleOperation::SetAttributes(SetAttributes::Activity {
				id, namespace, ..
			}) => {
				vec![
					LedgerAddress::namespace(namespace),
					LedgerAddress::in_namespace(namespace, id.clone()),
				]
			},
		}
	}

	/// Apply an operation's input states to the prov model
	/// Take input states and apply them to the prov model, then apply transaction,
	/// then return a snapshot of output state for diff calculation
	#[instrument(level = "debug", skip(self, model, input))]
	pub fn process(
		&self,
		mut model: ProvModel,
		input: Vec<StateInput>,
	) -> Result<(Vec<StateOutput>, ProvModel), ProcessorError> {
		for input in input.iter() {
			model.combine(input.data())
		}
		model.apply(self).map_err(ProcessorError::Contradiction)?;
		Ok((
			model
				.to_snapshot()
				.into_iter()
				.map(|((namespace, resource), prov)| {
					StateOutput::new(LedgerAddress { namespace, resource }, prov)
				})
				.collect::<Vec<_>>(),
			model,
		))
	}
}
