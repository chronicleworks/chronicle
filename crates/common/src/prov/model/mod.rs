mod contradiction;
pub use contradiction::Contradiction;
pub mod transaction;
use scale_info::prelude::sync::Arc;
use scale_info::{build::Fields, Path, Type, TypeInfo};
pub use transaction::ChronicleTransaction;

use chrono::{DateTime, NaiveDateTime, Utc};
use iref::IriBuf;
use json_ld::NoLoader;
use lazy_static::lazy_static;
use locspan::Meta;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use rdf_types::{vocabulary::no_vocabulary_mut, BlankIdBuf};
use serde::Serialize;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    convert::Infallible,
    fmt::{Debug, Display},
};
use tokio::task::JoinError;
use tracing::{instrument, trace};
use uuid::Uuid;

use crate::{
    attributes::{Attribute, Attributes},
    identity::IdentityError,
    opa::OpaExecutorError,
    prov::operations::WasAttributedTo,
};

use super::operations::TimeWrapper;
use super::UuidWrapper;
use super::{
    id,
    operations::{
        ActivityExists, ActivityUses, ActsOnBehalfOf, AgentExists, ChronicleOperation,
        CreateNamespace, DerivationType, EndActivity, EntityDerive, EntityExists, SetAttributes,
        StartActivity, WasAssociatedWith, WasGeneratedBy, WasInformedBy,
    },
    ActivityId, AgentId, AssociationId, AttributionId, ChronicleIri, DelegationId, DomaintypeId,
    EntityId, ExternalId, ExternalIdPart, NamespaceId, Role, UuidPart,
};

pub mod to_json_ld;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProcessorError {
    #[error("Invalid address")]
    Address,
    #[error("Json Ld Error {0}")]
    Compaction(#[from] CompactionError),
    #[error("Contradiction {0}")]
    Contradiction(#[from] Contradiction),
    #[error("Json Ld Error {inner}")]
    Expansion { inner: String },
    #[error("IdentityError {0}")]
    Identity(#[from] IdentityError),
    #[error("Invalid IRI {0}")]
    IRef(#[from] iref::Error),
    #[error("Not a Chronicle IRI {0}")]
    NotAChronicleIri(#[from] id::ParseIriError),
    #[error("Missing @id {object:?}")]
    MissingId { object: serde_json::Value },
    #[error("Missing property {iri}:{object:?}")]
    MissingProperty {
        iri: String,
        object: serde_json::Value,
    },
    #[error("Json LD object is not a node {0}")]
    NotANode(serde_json::Value),
    #[error("Chronicle value is not a JSON object")]
    NotAnObject,
    #[error("OpaExecutorError: {0}")]
    OpaExecutor(#[from] OpaExecutorError),
    #[error("Malformed JSON {0}")]
    SerdeJson(#[from] serde_json::Error),
    #[error("Unparsable date/time {0}")]
    SubmissionFormat(#[from] PayloadError),
    #[error("Submission body format: {0}")]
    Time(#[from] chrono::ParseError),
    #[error("Tokio Error {0}")]
    Tokio(#[from] JoinError),
    #[error("State is not valid utf8 {0}")]
    Utf8(#[from] std::str::Utf8Error),
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

#[derive(Serialize, Deserialize, Encode, Decode, PartialEq, Eq, Debug, Clone)]
pub struct ChronicleTransactionId(String);

impl Display for ChronicleTransactionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl From<Uuid> for ChronicleTransactionId {
    fn from(u: Uuid) -> Self {
        Self(u.to_string())
    }
}

impl From<&str> for ChronicleTransactionId {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

impl ChronicleTransactionId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Encode, Decode, TypeInfo, PartialEq, Eq, Serialize, Deserialize)]
pub struct Namespace {
    pub id: NamespaceId,
    pub uuid: UuidWrapper,
    pub external_id: ExternalId,
}

impl Namespace {
    pub fn new(id: NamespaceId, uuid: Uuid, external_id: &ExternalId) -> Self {
        Self {
            id,
            uuid: uuid.into(),
            external_id: external_id.to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Encode, Decode, TypeInfo)]
pub struct Agent {
    pub id: AgentId,
    pub namespaceid: NamespaceId,
    pub external_id: ExternalId,
    pub domaintypeid: Option<DomaintypeId>,
    pub attributes: BTreeMap<String, Attribute>,
}

impl Agent {
    pub fn has_attributes(self, attributes: Attributes) -> Self {
        let Self {
            id,
            namespaceid,
            external_id,
            ..
        } = self;

        Self {
            id,
            namespaceid,
            external_id,
            domaintypeid: attributes.typ,
            attributes: attributes.attributes,
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

#[derive(Debug, Clone, Encode, Decode, TypeInfo, PartialEq, Eq, Serialize, Deserialize)]
pub struct Activity {
    pub id: ActivityId,
    pub namespaceid: NamespaceId,
    pub external_id: ExternalId,
    pub domaintypeid: Option<DomaintypeId>,
    pub attributes: BTreeMap<String, Attribute>,
    pub started: Option<TimeWrapper>,
    pub ended: Option<TimeWrapper>,
}

impl Activity {
    pub fn has_attributes(self, attributes: Attributes) -> Self {
        let Self {
            id,
            namespaceid,
            external_id,
            started,
            ended,
            ..
        } = self;
        Self {
            id,
            namespaceid,
            external_id,
            started,
            ended,
            domaintypeid: attributes.typ,
            attributes: attributes.attributes,
        }
    }

    // Create a prototypical agent from its IRI, we can only determine external_id
    pub fn exists(namespaceid: NamespaceId, id: ActivityId) -> Self {
        Self {
            namespaceid,
            external_id: id.external_id_part().to_owned(),
            id,
            started: None,
            ended: None,
            domaintypeid: None,
            attributes: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Encode, Decode, TypeInfo)]
pub struct Entity {
    pub id: EntityId,
    pub namespaceid: NamespaceId,
    pub external_id: ExternalId,
    pub domaintypeid: Option<DomaintypeId>,
    pub attributes: BTreeMap<String, Attribute>,
}

impl Entity {
    pub fn has_attributes(self, attributes: Attributes) -> Self {
        let Self {
            id,
            namespaceid,
            external_id,
            ..
        } = self;
        Self {
            id,
            namespaceid,
            external_id,
            domaintypeid: attributes.typ,
            attributes: attributes.attributes,
        }
    }

    pub fn exists(namespaceid: NamespaceId, id: EntityId) -> Self {
        Self {
            external_id: id.external_id_part().to_owned(),
            id,
            namespaceid,
            domaintypeid: None,
            attributes: BTreeMap::new(),
        }
    }
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    Encode,
    Decode,
    TypeInfo,
)]
pub struct Derivation {
    pub generated_id: EntityId,
    pub used_id: EntityId,
    pub activity_id: Option<ActivityId>,
    pub typ: DerivationType,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    Encode,
    Decode,
    TypeInfo,
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

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    Encode,
    Decode,
    TypeInfo,
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

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    Encode,
    Decode,
    TypeInfo,
)]
pub struct Usage {
    pub activity_id: ActivityId,
    pub entity_id: EntityId,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    Encode,
    Decode,
    TypeInfo,
)]
pub struct Generation {
    pub activity_id: ActivityId,
    pub generated_id: EntityId,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    Encode,
    Decode,
    TypeInfo,
)]
pub struct GeneratedEntity {
    pub entity_id: EntityId,
    pub generated_id: ActivityId,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
    Encode,
    Decode,
    TypeInfo,
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

#[derive(Debug, Default, Clone, PartialEq, Eq, Encode, Decode, TypeInfo)]
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

impl MaxEncodedLen for ProvModel {
    fn max_encoded_len() -> usize {
        64 * 1024usize
    }
}

impl ProvModel {
    /// Merge the supplied ProvModel into this one
    pub fn combine(&mut self, other: &ProvModel) {
        self.namespaces.extend(other.namespaces.clone());
        self.agents.extend(other.agents.clone());
        self.acted_on_behalf_of
            .extend(other.acted_on_behalf_of.clone());
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
        let derivation_set = Arc::make_mut(
            self.derivation
                .entry((namespace_id, id.clone()))
                .or_default(),
        );

        derivation_set.insert(Derivation {
            typ,
            generated_id: id,
            used_id,
            activity_id,
        });
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
            self.association
                .entry((namespace_id.clone(), activity_id.clone()))
                .or_default(),
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
            self.generation
                .entry((namespace_id.clone(), generated_id.clone()))
                .or_default(),
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
            self.generated
                .entry((namespace_id.clone(), generated_id.clone()))
                .or_default(),
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
            self.usage
                .entry((namespace_id.clone(), activity_id.clone()))
                .or_default(),
        );

        usage_set.insert(Usage {
            activity_id: activity_id.clone(),
            entity_id: entity_id.clone(),
        });
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
            self.attribution
                .entry((namespace_id.clone(), entity_id.clone()))
                .or_default(),
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
                uuid: uuid.to_owned().into(),
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
        self.agents
            .get(&(ns.clone(), agent.clone()))
            .map(|arc| arc.as_ref())
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
        self.entities
            .get(&(ns.clone(), entity.clone()))
            .map(|arc| arc.as_ref())
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
        self.activities
            .get(&(ns.clone(), activity.clone()))
            .map(|arc| arc.as_ref())
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
    /// If a statement requires a subject or object that does not currently exist in the model, then we create it
    /// If an operation contradicts a previous statement, then we record the
    /// contradiction, but attempt to apply as much of the operation as possible
    #[instrument(skip(self,tx), level = "trace", name="apply_chronicle_operation", fields(op = ?tx, model= ?self), ret(Debug))]
    pub fn apply(&mut self, tx: &ChronicleOperation) -> Result<(), Contradiction> {
        let tx = tx.to_owned();
        match tx {
            ChronicleOperation::CreateNamespace(CreateNamespace {
                id,
                external_id: _,
                uuid: _,
            }) => {
                self.namespace_context(&id);
                Ok(())
            }
            ChronicleOperation::AgentExists(AgentExists {
                namespace,
                external_id,
                ..
            }) => {
                self.namespace_context(&namespace);
                self.agent_context(&namespace, &AgentId::from_external_id(&external_id));

                Ok(())
            }
            ChronicleOperation::AgentActsOnBehalfOf(ActsOnBehalfOf {
                id: _,
                namespace,
                delegate_id,
                activity_id,
                role,
                responsible_id,
            }) => {
                self.namespace_context(&namespace);
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
            }
            ChronicleOperation::ActivityExists(ActivityExists {
                namespace,
                external_id,
                ..
            }) => {
                self.namespace_context(&namespace);
                self.activity_context(&namespace, &ActivityId::from_external_id(&external_id));

                Ok(())
            }
            ChronicleOperation::StartActivity(StartActivity {
                namespace,
                id,
                time,
            }) => {
                self.namespace_context(&namespace);
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
                        ));
                    }
                    (_, Some(TimeWrapper(ended))) if ended < time.0 => {
                        return Err(Contradiction::invalid_range(
                            id.into(),
                            namespace,
                            time.0,
                            ended,
                        ));
                    }
                    _ => {}
                };

                self.modify_activity(&namespace, &id, move |activity| {
                    activity.started = Some(time);
                });

                Ok(())
            }
            ChronicleOperation::EndActivity(EndActivity {
                namespace,
                id,
                time,
            }) => {
                self.namespace_context(&namespace);
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
                        ));
                    }
                    (Some(TimeWrapper(started)), _) if started > time.0 => {
                        return Err(Contradiction::invalid_range(
                            id.into(),
                            namespace,
                            started,
                            time.0,
                        ));
                    }
                    _ => {}
                };

                self.modify_activity(&namespace, &id, move |activity| {
                    activity.ended = Some(time);
                });

                Ok(())
            }
            ChronicleOperation::WasAssociatedWith(WasAssociatedWith {
                id: _,
                role,
                namespace,
                activity_id,
                agent_id,
            }) => {
                self.namespace_context(&namespace);
                self.agent_context(&namespace, &agent_id);
                self.activity_context(&namespace, &activity_id);
                self.qualified_association(&namespace, &activity_id, &agent_id, role);

                Ok(())
            }
            ChronicleOperation::WasAttributedTo(WasAttributedTo {
                id: _,
                role,
                namespace,
                entity_id,
                agent_id,
            }) => {
                self.namespace_context(&namespace);
                self.agent_context(&namespace, &agent_id);
                self.entity_context(&namespace, &entity_id);
                self.qualified_attribution(&namespace, &entity_id, &agent_id, role);

                Ok(())
            }
            ChronicleOperation::ActivityUses(ActivityUses {
                namespace,
                id,
                activity,
            }) => {
                self.namespace_context(&namespace);

                self.activity_context(&namespace, &activity);
                self.entity_context(&namespace, &id);

                self.used(namespace, &activity, &id);

                Ok(())
            }
            ChronicleOperation::EntityExists(EntityExists {
                namespace,
                external_id,
                ..
            }) => {
                self.namespace_context(&namespace);
                self.entity_context(&namespace, &EntityId::from_external_id(&external_id));
                Ok(())
            }
            ChronicleOperation::WasGeneratedBy(WasGeneratedBy {
                namespace,
                id,
                activity,
            }) => {
                self.namespace_context(&namespace);

                self.entity_context(&namespace, &id);
                self.activity_context(&namespace, &activity);

                self.was_generated_by(namespace, &id, &activity);

                Ok(())
            }
            ChronicleOperation::WasInformedBy(WasInformedBy {
                namespace,
                activity,
                informing_activity,
            }) => {
                self.namespace_context(&namespace);
                self.activity_context(&namespace, &activity);
                self.activity_context(&namespace, &informing_activity);

                self.was_informed_by(namespace, &activity, &informing_activity);

                Ok(())
            }
            ChronicleOperation::EntityDerive(EntityDerive {
                namespace,
                id,
                typ,
                used_id,
                activity_id,
            }) => {
                self.namespace_context(&namespace);

                self.entity_context(&namespace, &id);
                self.entity_context(&namespace, &used_id);

                if let Some(activity_id) = &activity_id {
                    self.activity_context(&namespace, activity_id);
                }

                self.was_derived_from(namespace, typ, used_id, id, activity_id);

                Ok(())
            }
            ChronicleOperation::SetAttributes(SetAttributes::Entity {
                namespace,
                id,
                attributes,
            }) => {
                self.namespace_context(&namespace);
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
                    entity.domaintypeid = attributes.typ.clone();
                    entity.attributes = attributes.attributes;
                });

                Ok(())
            }
            ChronicleOperation::SetAttributes(SetAttributes::Activity {
                namespace,
                id,
                attributes,
            }) => {
                self.namespace_context(&namespace);
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
                    activity.domaintypeid = attributes.typ.clone();
                    activity.attributes = attributes.attributes;
                });

                Ok(())
            }
            ChronicleOperation::SetAttributes(SetAttributes::Agent {
                namespace,
                id,
                attributes,
            }) => {
                self.namespace_context(&namespace);
                self.agent_context(&namespace, &id);

                if let Some(current) = self
                    .agents
                    .get(&(namespace.clone(), id.clone()))
                    .map(|agent| &agent.attributes)
                {
                    Self::validate_attribute_changes(
                        &id.clone().into(),
                        &namespace,
                        current,
                        &attributes,
                    )?;
                };

                self.modify_agent(&namespace, &id, move |agent| {
                    agent.domaintypeid = attributes.typ.clone();
                    agent.attributes = attributes.attributes;
                });

                Ok(())
            }
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
            .attributes
            .iter()
            .filter_map(|(current_name, current_value)| {
                if let Some(attempted_value) = current.get(current_name) {
                    if current_value != attempted_value {
                        Some((
                            current_name.clone(),
                            current_value.clone(),
                            attempted_value.clone(),
                        ))
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

    pub(crate) fn add_agent(&mut self, agent: Agent) {
        self.agents
            .insert((agent.namespaceid.clone(), agent.id.clone()), agent.into());
    }

    pub(crate) fn add_activity(&mut self, activity: Activity) {
        self.activities.insert(
            (activity.namespaceid.clone(), activity.id.clone()),
            activity.into(),
        );
    }

    pub(crate) fn add_entity(&mut self, entity: Entity) {
        self.entities.insert(
            (entity.namespaceid.clone(), entity.id.clone()),
            entity.into(),
        );
    }
}

custom_error::custom_error! {pub CompactionError
    JsonLd{inner: String}                  = "JSON-LD: {inner}", //TODO: contribute Send to the upstream JsonLD error type
    Join{source : JoinError}               = "Tokio: {source}",
    Serde{source: serde_json::Error}       = "Serde conversion: {source}",
    InvalidExpanded{message: String}       = "Expanded document invalid: {message}",
    NoObject{document: Value}              = "Compacted document not a JSON object: {document}",
}
pub struct ExpandedJson(pub serde_json::Value);

fn construct_context_definition<M>(
    json: &serde_json::Value,
    metadata: M,
) -> json_ld::syntax::context::Definition<M>
where
    M: Clone + Debug,
{
    use json_ld::syntax::{
        context::{
            definition::{Bindings, Version},
            Definition, TermDefinition,
        },
        Entry, Nullable, TryFromJson,
    };
    if let Value::Object(map) = json {
        match map.get("@version") {
            None => {}
            Some(Value::Number(version)) if version.as_f64() == Some(1.1) => {}
            Some(json_version) => panic!("unexpected JSON-LD context @version: {json_version}"),
        };
        let mut bindings = Bindings::new();
        for (key, value) in map {
            if key == "@version" {
                // already handled above
            } else if let Some('@') = key.chars().next() {
                panic!("unexpected JSON-LD context key: {key}");
            } else {
                let value =
                    json_ld::syntax::Value::from_serde_json(value.clone(), |_| metadata.clone());
                let term: Meta<TermDefinition<M>, M> = TryFromJson::try_from_json(value)
                    .expect("failed to convert {value} to term binding");
                bindings.insert(
                    Meta(key.clone().into(), metadata.clone()),
                    Meta(Nullable::Some(term.value().clone()), metadata.clone()),
                );
            }
        }
        Definition {
            base: None,
            import: None,
            language: None,
            direction: None,
            propagate: None,
            protected: None,
            type_: None,
            version: Some(Entry::new(
                metadata.clone(),
                Meta::new(Version::V1_1, metadata),
            )),
            vocab: None,
            bindings,
        }
    } else {
        panic!("failed to convert JSON to LD context: {json:?}");
    }
}

lazy_static! {
    static ref JSON_LD_CONTEXT_DEFS: json_ld::syntax::context::Definition<()> =
        construct_context_definition(&crate::context::PROV, ());
}

impl ExpandedJson {
    async fn compact_unordered(self) -> Result<CompactedJson, CompactionError> {
        use json_ld::{
            syntax::context, Compact, ExpandedDocument, Process, ProcessingMode, TryFromJson,
        };

        let vocabulary = no_vocabulary_mut();
        let mut loader: NoLoader<IriBuf, (), json_ld::syntax::Value> = NoLoader::new();

        // process context
        let value = context::Value::One(Meta::new(
            context::Context::Definition(JSON_LD_CONTEXT_DEFS.clone()),
            (),
        ));
        let context_meta = Meta::new(value, ());
        let processed_context = context_meta
            .process(vocabulary, &mut loader, None)
            .await
            .map_err(|e| CompactionError::JsonLd {
                inner: format!("{:?}", e),
            })?;

        // compact document

        let expanded_meta = json_ld::syntax::Value::from_serde_json(self.0, |_| ());

        let expanded_doc: Meta<ExpandedDocument<IriBuf, BlankIdBuf, ()>, ()> =
            TryFromJson::try_from_json_in(vocabulary, expanded_meta).map_err(|e| {
                CompactionError::InvalidExpanded {
                    message: format!("{:?}", e.into_value()),
                }
            })?;

        let output = expanded_doc
            .compact_full(
                vocabulary,
                processed_context.as_ref(),
                &mut loader,
                json_ld::compaction::Options {
                    processing_mode: ProcessingMode::JsonLd1_1,
                    compact_to_relative: true,
                    compact_arrays: true,
                    ordered: true,
                },
            )
            .await
            .map_err(|e| CompactionError::JsonLd {
                inner: e.to_string(),
            })?;

        // Sort @graph

        // reference context

        let json: Value = output.into_value().into();

        if let Value::Object(mut map) = json {
            map.insert(
                "@context".to_string(),
                Value::String("https://btp.works/chr/1.0/c.jsonld".to_string()),
            );
            Ok(CompactedJson(Value::Object(map)))
        } else {
            Err(CompactionError::NoObject { document: json })
        }
    }

    // Sort @graph by json value, as they are unstable and we need deterministic output
    pub async fn compact(self) -> Result<Value, CompactionError> {
        let mut v: serde_json::Value =
            serde_json::from_str(&self.compact_unordered().await?.0.to_string())?;

        if let Some(v) = v.pointer_mut("/@graph").and_then(|p| p.as_array_mut()) {
            v.sort_by_cached_key(|v| v.to_string());
        }

        Ok(v)
    }

    pub async fn compact_stable_order(self) -> Result<Value, CompactionError> {
        self.compact().await
    }
}
pub mod from_json_ld;

pub struct CompactedJson(pub serde_json::Value);

impl std::ops::Deref for CompactedJson {
    type Target = serde_json::Value;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl CompactedJson {
    pub fn pretty(&self) -> String {
        serde_json::to_string_pretty(&self.0).unwrap()
    }
}

/// Property testing of prov models created and round tripped via JSON / LD
#[cfg(test)]
pub mod proptest;
