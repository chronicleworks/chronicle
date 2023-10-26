use chrono::{DateTime, Utc};

use futures::{future::BoxFuture, FutureExt};
use iref::{Iri, IriBuf, IriRef};
use iri_string::types::{IriAbsoluteString, UriAbsoluteStr, UriAbsoluteString};
use json_ld::{
	syntax::IntoJsonWithContextMeta, Indexed, Loader, Node, Profile, RemoteDocument, Term,
};
use locspan::Meta;
use mime::Mime;
use rdf_types::{vocabulary::no_vocabulary_mut, BlankIdBuf, IriVocabularyMut};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use tracing::{error, instrument, trace};

#[cfg(not(feature = "std"))]
use parity_scale_codec::{alloc::string::String, alloc::vec::Vec, prelude::*};

use crate::{
	attributes::{Attribute, Attributes},
	prov::{
		operations::{
			ActivityExists, ActivityUses, ActsOnBehalfOf, AgentExists, ChronicleOperation,
			CreateNamespace, DerivationType, EndActivity, EntityDerive, EntityExists,
			SetAttributes, StartActivity, WasAssociatedWith, WasAttributedTo, WasGeneratedBy,
			WasInformedBy,
		},
		vocab::{Chronicle, ChronicleOperations, Prov},
		ActivityId, AgentId, DomaintypeId, EntityId, ExternalIdPart, NamespaceId, Role, UuidPart,
	},
};

use crate::prov::{Activity, Agent, Entity, ProcessorError, ProvModel};

pub struct ContextLoader;

impl Loader<IriBuf, ()> for ContextLoader {
	type Error = ();
	type Output = json_ld::syntax::Value<Self::Error>;

	// This is only used to load the context, so we can just return it directly
	fn load_with<'b>(
		&'b mut self,
		vocabulary: &'b mut (impl Sync + Send + IriVocabularyMut<Iri = IriBuf>),
		url: IriBuf,
	) -> BoxFuture<Result<RemoteDocument<IriBuf, Self::Error, Self::Output>, Self::Error>>
	where
		IriBuf: 'b,
	{
		use hashbrown::HashSet;
		use std::str::FromStr;
		let mut profiles = HashSet::new();
		profiles.insert(Profile::new(url.as_iri(), vocabulary));
		trace!("Loading context from {}", url);
		async move {
			let json = json!({
			 "@context": crate::context::PROV.clone()
			});
			let value = json_ld::syntax::Value::from_serde_json(json, |_| ());
			Ok(json_ld::RemoteDocument::new_full(
				Some(url),
				Some(Mime::from_str("application/json").unwrap()),
				None,
				profiles,
				value,
			))
		}
		.boxed()
	}
}

fn as_json(node: &Node<IriBuf, BlankIdBuf, ()>) -> serde_json::Value {
	node.clone().into_json_meta_with((), no_vocabulary_mut()).into_value().into()
}

// Convert with coercion from our vocab iris, this is safe as sourced from constants
fn id_from_iri_string<I: Into<IriAbsoluteString>>(iri: I) -> json_ld::Id {
	json_ld::Id::Valid(json_ld::ValidId::Iri(IriBuf::try_from(iri.into().to_string()).unwrap()))
}

fn extract_reference_ids<I: Into<IriAbsoluteString>>(
	iri: I,
	node: &Node<IriBuf, BlankIdBuf, ()>,
) -> Result<Vec<IriBuf>, ProcessorError> {
	let ids: Result<Vec<_>, _> = node
		.get(&id_from_iri_string(iri))
		.map(|o| o.id().ok_or_else(|| ProcessorError::MissingId { object: as_json(node) }))
		.map(|id| {
			id.and_then(|id| {
				id.as_iri().ok_or_else(|| ProcessorError::MissingId { object: as_json(node) })
			})
		})
		.map(|id| id.map(|id| id.to_owned()))
		.collect();

	ids
}

fn extract_scalar_prop<'a, I: Into<IriAbsoluteString> + Clone>(
	iri: I,
	node: &'a Node<IriBuf, BlankIdBuf, ()>,
) -> Result<&'a Indexed<json_ld::object::Object<IriBuf, BlankIdBuf, ()>, ()>, ProcessorError> {
	if let Some(object) = node.get_any(&id_from_iri_string(iri.clone())) {
		Ok(object)
	} else {
		Err(ProcessorError::MissingProperty { iri: iri.into().to_string(), object: as_json(node) })
	}
}

fn extract_namespace(agent: &Node<IriBuf, BlankIdBuf, ()>) -> Result<NamespaceId, ProcessorError> {
	Ok(NamespaceId::try_from(
		extract_scalar_prop(Chronicle::HasNamespace, agent)?
			.id()
			.ok_or(ProcessorError::MissingId { object: as_json(agent) })?
			.to_string(),
	)?)
}

impl ProvModel {
	pub async fn apply_json_ld_str(&mut self, buf: &str) -> Result<(), ProcessorError> {
		self.apply_json_ld(serde_json::from_str(buf)?).await?;

		Ok(())
	}

	pub async fn apply_json_ld_bytes(&mut self, buf: &[u8]) -> Result<(), ProcessorError> {
		self.apply_json_ld(serde_json::from_slice(buf)?).await?;

		Ok(())
	}

	/// Take a Json-Ld input document, assuming it is in compact form, expand it and apply the state
	/// to the prov model Replace @context with our resource context
	/// We rely on reified @types, so subclassing must also include supertypes
	#[instrument(level = "trace", skip(self, json))]
	pub async fn apply_json_ld(&mut self, json: serde_json::Value) -> Result<(), ProcessorError> {
		if let serde_json::Value::Object(mut map) = json {
			map.insert(
				"@context".to_string(),
				serde_json::Value::String("https://btp.works/chr/1.0/c.jsonld".to_string()),
			);
			let json = serde_json::Value::Object(map);

			trace!(to_apply_compact=%serde_json::to_string_pretty(&json)?);

			use json_ld::Expand;
			let output = json_ld::syntax::Value::from_serde_json(json.clone(), |_| ())
				.expand(&mut ContextLoader)
				.await
				.map_err(|e| ProcessorError::Expansion { inner: format!("{e:?}") })?;

			for o in output.into_value().into_objects() {
				let o =
					o.value().inner().as_node().ok_or(ProcessorError::NotANode(json.clone()))?;

				if o.has_type(&id_from_iri_string(Chronicle::Namespace)) {
					self.apply_node_as_namespace(o)?;
				}
				if o.has_type(&id_from_iri_string(Prov::Agent)) {
					self.apply_node_as_agent(o)?;
				} else if o.has_type(&id_from_iri_string(Prov::Activity)) {
					self.apply_node_as_activity(o)?;
				} else if o.has_type(&id_from_iri_string(Prov::Entity)) {
					self.apply_node_as_entity(o)?;
				} else if o.has_type(&id_from_iri_string(Prov::Delegation)) {
					self.apply_node_as_delegation(o)?;
				} else if o.has_type(&id_from_iri_string(Prov::Association)) {
					self.apply_node_as_association(o)?;
				} else if o.has_type(&id_from_iri_string(Prov::Attribution)) {
					self.apply_node_as_attribution(o)?;
				}
			}
			Ok(())
		} else {
			Err(ProcessorError::NotAnObject)
		}
	}

	/// Extract the types and find the first that is not prov::, as we currently only alow zero or
	/// one domain types this should be sufficient
	fn extract_attributes(
		node: &Node<IriBuf, BlankIdBuf, ()>,
	) -> Result<Attributes, ProcessorError> {
		let typ = node
			.types()
			.iter()
			.filter_map(|x| x.as_iri())
			.find(|x| x.as_str().contains("domaintype"))
			.map(|iri| Ok::<_, ProcessorError>(DomaintypeId::try_from(iri.to_string())?))
			.transpose();

		if let serde_json::Value::Object(map) = as_json(node) {
			if let Some(serde_json::Value::Array(array)) = map.get(Chronicle::Value.as_str()) {
				if array.len() == 1 {
					let o = array.get(0).unwrap();
					let serde_object = &o["@value"];

					if let serde_json::Value::Object(object) = serde_object {
						let attributes = object
							.into_iter()
							.map(|(typ, value)| {
								(
									typ.clone(),
									Attribute { typ: typ.clone(), value: value.clone().into() },
								)
							})
							.collect();

						return Ok(Attributes { typ: typ?, attributes });
					}
				}
			}
		}

		Err(ProcessorError::NotAnObject)
	}

	fn apply_node_as_namespace(
		&mut self,
		ns: &Node<IriBuf, BlankIdBuf, ()>,
	) -> Result<(), ProcessorError> {
		let ns = ns.id().ok_or_else(|| ProcessorError::MissingId { object: as_json(ns) })?;

		self.namespace_context(&NamespaceId::try_from(ns.to_string())?);

		Ok(())
	}

	fn apply_node_as_delegation(
		&mut self,
		delegation: &Node<IriBuf, BlankIdBuf, ()>,
	) -> Result<(), ProcessorError> {
		let namespace_id = extract_namespace(delegation)?;
		self.namespace_context(&namespace_id);

		let role = extract_scalar_prop(Prov::HadRole, delegation)
			.ok()
			.and_then(|x| x.as_str().map(Role::from));

		let responsible_id = extract_reference_ids(Prov::ActedOnBehalfOf, delegation)?
			.into_iter()
			.next()
			.ok_or_else(|| ProcessorError::MissingProperty {
				object: as_json(delegation),
				iri: Prov::ActedOnBehalfOf.to_string(),
			})
			.and_then(|x| Ok(AgentId::try_from(x.to_string())?))?;

		let delegate_id = extract_reference_ids(Prov::Delegate, delegation)?
			.into_iter()
			.next()
			.ok_or_else(|| ProcessorError::MissingProperty {
				object: as_json(delegation),
				iri: Prov::Delegate.to_string(),
			})
			.and_then(|x| Ok(AgentId::try_from(x.to_string())?))?;

		let activity_id = extract_reference_ids(Prov::HadActivity, delegation)?
			.into_iter()
			.next()
			.map(|x| ActivityId::try_from(x.to_string()))
			.transpose()?;

		self.qualified_delegation(&namespace_id, &responsible_id, &delegate_id, activity_id, role);
		Ok(())
	}

	fn apply_node_as_association(
		&mut self,
		association: &Node<IriBuf, BlankIdBuf, ()>,
	) -> Result<(), ProcessorError> {
		let namespace_id = extract_namespace(association)?;
		self.namespace_context(&namespace_id);

		let role = extract_scalar_prop(Prov::HadRole, association)
			.ok()
			.and_then(|x| x.as_str().map(Role::from));

		let agent_id = extract_reference_ids(Prov::Responsible, association)?
			.into_iter()
			.next()
			.ok_or_else(|| ProcessorError::MissingProperty {
				object: as_json(association),
				iri: Prov::Responsible.to_string(),
			})
			.and_then(|x| Ok(AgentId::try_from(x.to_string())?))?;

		let activity_id = extract_reference_ids(Prov::HadActivity, association)?
			.into_iter()
			.next()
			.ok_or_else(|| ProcessorError::MissingProperty {
				object: as_json(association),
				iri: Prov::HadActivity.to_string(),
			})
			.and_then(|x| Ok(ActivityId::try_from(x.to_string())?))?;

		self.qualified_association(&namespace_id, &activity_id, &agent_id, role);

		Ok(())
	}

	fn apply_node_as_attribution(
		&mut self,
		attribution: &Node<IriBuf, BlankIdBuf, ()>,
	) -> Result<(), ProcessorError> {
		let namespace_id = extract_namespace(attribution)?;
		self.namespace_context(&namespace_id);

		let role = extract_scalar_prop(Prov::HadRole, attribution)
			.ok()
			.and_then(|x| x.as_str().map(Role::from));

		let agent_id = extract_reference_ids(Prov::Responsible, attribution)?
			.into_iter()
			.next()
			.ok_or_else(|| ProcessorError::MissingProperty {
				object: as_json(attribution),
				iri: Prov::Responsible.to_string(),
			})
			.and_then(|x| Ok(AgentId::try_from(x.to_string())?))?;

		let entity_id = extract_reference_ids(Prov::HadEntity, attribution)?
			.into_iter()
			.next()
			.ok_or_else(|| ProcessorError::MissingProperty {
				object: as_json(attribution),
				iri: Prov::HadEntity.to_string(),
			})
			.and_then(|x| Ok(EntityId::try_from(x.to_string())?))?;

		self.qualified_attribution(&namespace_id, &entity_id, &agent_id, role);

		Ok(())
	}

	fn apply_node_as_agent(
		&mut self,
		agent: &Node<IriBuf, BlankIdBuf, ()>,
	) -> Result<(), ProcessorError> {
		let id = AgentId::try_from(
			agent
				.id()
				.ok_or_else(|| ProcessorError::MissingId { object: as_json(agent) })?
				.to_string(),
		)?;

		let namespaceid = extract_namespace(agent)?;
		self.namespace_context(&namespaceid);

		let attributes = Self::extract_attributes(agent)?;

		let agent = Agent::exists(namespaceid, id).has_attributes(attributes);

		self.add_agent(agent);

		Ok(())
	}

	fn apply_node_as_activity(
		&mut self,
		activity: &Node<IriBuf, BlankIdBuf, ()>,
	) -> Result<(), ProcessorError> {
		let id = ActivityId::try_from(
			activity
				.id()
				.ok_or_else(|| ProcessorError::MissingId { object: as_json(activity) })?
				.to_string(),
		)?;

		let namespaceid = extract_namespace(activity)?;
		self.namespace_context(&namespaceid);

		let started = extract_scalar_prop(Prov::StartedAtTime, activity)
			.ok()
			.and_then(|x| x.as_str().map(DateTime::parse_from_rfc3339));

		let ended = extract_scalar_prop(Prov::EndedAtTime, activity)
			.ok()
			.and_then(|x| x.as_str().map(DateTime::parse_from_rfc3339));

		let used = extract_reference_ids(Prov::Used, activity)?
			.into_iter()
			.map(|id| EntityId::try_from(id.to_string()))
			.collect::<Result<Vec<_>, _>>()?;

		let was_informed_by = extract_reference_ids(Prov::WasInformedBy, activity)?
			.into_iter()
			.map(|id| ActivityId::try_from(id.to_string()))
			.collect::<Result<Vec<_>, _>>()?;

		let attributes = Self::extract_attributes(activity)?;

		let mut activity = Activity::exists(namespaceid.clone(), id).has_attributes(attributes);

		if let Some(started) = started {
			activity.started = Some(DateTime::<Utc>::from(started?).into());
		}

		if let Some(ended) = ended {
			activity.ended = Some(DateTime::<Utc>::from(ended?).into());
		}

		for entity in used {
			self.used(namespaceid.clone(), &activity.id, &entity);
		}

		for informing_activity in was_informed_by {
			self.was_informed_by(namespaceid.clone(), &activity.id, &informing_activity);
		}

		self.add_activity(activity);

		Ok(())
	}

	fn apply_node_as_entity(
		&mut self,
		entity: &Node<IriBuf, BlankIdBuf, ()>,
	) -> Result<(), ProcessorError> {
		let id = EntityId::try_from(
			entity
				.id()
				.ok_or_else(|| ProcessorError::MissingId { object: as_json(entity) })?
				.to_string(),
		)?;

		let namespaceid = extract_namespace(entity)?;
		self.namespace_context(&namespaceid);

		let generatedby = extract_reference_ids(Prov::WasGeneratedBy, entity)?
			.into_iter()
			.map(|id| ActivityId::try_from(id.to_string()))
			.collect::<Result<Vec<_>, _>>()?;

		for derived in extract_reference_ids(Prov::WasDerivedFrom, entity)?
			.into_iter()
			.map(|id| EntityId::try_from(id.to_string()))
		{
			self.was_derived_from(
				namespaceid.clone(),
				DerivationType::None,
				derived?,
				id.clone(),
				None,
			);
		}

		for derived in extract_reference_ids(Prov::WasQuotedFrom, entity)?
			.into_iter()
			.map(|id| EntityId::try_from(id.to_string()))
		{
			self.was_derived_from(
				namespaceid.clone(),
				DerivationType::quotation(),
				derived?,
				id.clone(),
				None,
			);
		}

		for derived in extract_reference_ids(Prov::WasRevisionOf, entity)?
			.into_iter()
			.map(|id| EntityId::try_from(id.to_string()))
		{
			self.was_derived_from(
				namespaceid.clone(),
				DerivationType::revision(),
				derived?,
				id.clone(),
				None,
			);
		}

		for derived in extract_reference_ids(Prov::HadPrimarySource, entity)?
			.into_iter()
			.map(|id| EntityId::try_from(id.to_string()))
		{
			self.was_derived_from(
				namespaceid.clone(),
				DerivationType::primary_source(),
				derived?,
				id.clone(),
				None,
			);
		}

		for activity in generatedby {
			self.was_generated_by(namespaceid.clone(), &id, &activity);
		}

		let attributes = Self::extract_attributes(entity)?;
		self.add_entity(Entity::exists(namespaceid, id).has_attributes(attributes));

		Ok(())
	}
}

trait Operation {
	fn namespace(&self) -> NamespaceId;
	fn agent(&self) -> AgentId;
	fn delegate(&self) -> AgentId;
	fn responsible(&self) -> AgentId;
	fn optional_activity(&self) -> Option<ActivityId>;
	fn activity(&self) -> ActivityId;
	fn optional_role(&self) -> Option<Role>;
	fn start_time(&self) -> String;
	fn locator(&self) -> Option<String>;
	fn end_time(&self) -> String;
	fn entity(&self) -> EntityId;
	fn used_entity(&self) -> EntityId;
	fn derivation(&self) -> DerivationType;
	fn domain(&self) -> Option<DomaintypeId>;
	fn attributes(&self) -> BTreeMap<String, Attribute>;
	fn informing_activity(&self) -> ActivityId;
}

impl Operation for Node<IriBuf, BlankIdBuf, ()> {
	fn namespace(&self) -> NamespaceId {
		let mut uuid_objects = self.get(&id_from_iri_string(ChronicleOperations::NamespaceUuid));
		let uuid = uuid_objects.next().unwrap().as_str().unwrap();
		let mut name_objects = self.get(&id_from_iri_string(ChronicleOperations::NamespaceName));
		let external_id = name_objects.next().unwrap().as_str().unwrap();
		let uuid = uuid::Uuid::parse_str(uuid).unwrap();
		NamespaceId::from_external_id(external_id, uuid)
	}

	fn agent(&self) -> AgentId {
		let mut name_objects = self.get(&id_from_iri_string(ChronicleOperations::AgentName));
		let external_id = name_objects.next().unwrap().as_str().unwrap();
		AgentId::from_external_id(external_id)
	}

	fn delegate(&self) -> AgentId {
		let mut name_objects = self.get(&id_from_iri_string(ChronicleOperations::DelegateId));
		let external_id = name_objects.next().unwrap().as_str().unwrap();
		AgentId::from_external_id(external_id)
	}

	fn optional_activity(&self) -> Option<ActivityId> {
		let mut name_objects = self.get(&id_from_iri_string(ChronicleOperations::ActivityName));
		let object = match name_objects.next() {
			Some(object) => object,
			None => return None,
		};
		Some(ActivityId::from_external_id(object.as_str().unwrap()))
	}

	fn start_time(&self) -> String {
		let mut objects = self.get(&id_from_iri_string(ChronicleOperations::StartActivityTime));
		let time = objects.next().unwrap().as_str().unwrap();
		time.to_owned()
	}

	fn end_time(&self) -> String {
		let mut objects = self.get(&id_from_iri_string(ChronicleOperations::EndActivityTime));
		let time = objects.next().unwrap().as_str().unwrap();
		time.to_owned()
	}

	fn entity(&self) -> EntityId {
		let mut name_objects = self.get(&id_from_iri_string(ChronicleOperations::EntityName));
		let external_id = name_objects.next().unwrap().as_str().unwrap();
		EntityId::from_external_id(external_id)
	}

	fn used_entity(&self) -> EntityId {
		let mut name_objects = self.get(&id_from_iri_string(ChronicleOperations::UsedEntityName));
		let external_id = name_objects.next().unwrap().as_str().unwrap();
		EntityId::from_external_id(external_id)
	}

	fn derivation(&self) -> DerivationType {
		let mut objects = self.get(&id_from_iri_string(ChronicleOperations::DerivationType));
		let derivation = match objects.next() {
			Some(object) => object.as_str().unwrap(),
			None => return DerivationType::None,
		};

		match derivation {
			"Revision" => DerivationType::Revision,
			"Quotation" => DerivationType::Quotation,
			"PrimarySource" => DerivationType::PrimarySource,
			_ => unreachable!(),
		}
	}

	fn domain(&self) -> Option<DomaintypeId> {
		let mut objects = self.get(&id_from_iri_string(ChronicleOperations::DomaintypeId));
		let d = match objects.next() {
			Some(object) => object.as_str().unwrap(),
			None => return None,
		};
		Some(DomaintypeId::from_external_id(d))
	}

	fn attributes(&self) -> BTreeMap<String, Attribute> {
		self.get(&id_from_iri_string(ChronicleOperations::Attributes))
			.map(|o| {
				let serde_object =
					if let Some(json_ld::object::Value::Json(Meta(json, _))) = o.as_value() {
						json.clone().into()
					} else {
						serde_json::from_str(&as_json(o.as_node().unwrap())["@value"].to_string())
							.unwrap()
					};

				if let serde_json::Value::Object(object) = serde_object {
					Ok(object
						.into_iter()
						.map(|(typ, value)| Attribute { typ, value: value.into() })
						.collect::<Vec<_>>())
				} else {
					Err(ProcessorError::NotAnObject {})
				}
			})
			.collect::<Result<Vec<_>, _>>()
			.unwrap()
			.into_iter()
			.flatten()
			.map(|attr| (attr.typ.clone(), attr))
			.collect()
	}

	fn responsible(&self) -> AgentId {
		let mut name_objects = self.get(&id_from_iri_string(ChronicleOperations::ResponsibleId));
		let external_id = name_objects.next().unwrap().as_str().unwrap();
		AgentId::from_external_id(external_id)
	}

	fn optional_role(&self) -> Option<Role> {
		let mut name_objects = self.get(&id_from_iri_string(ChronicleOperations::Role));
		let object = match name_objects.next() {
			Some(object) => object,
			None => return None,
		};
		Some(Role::from(object.as_str().unwrap()))
	}

	fn activity(&self) -> ActivityId {
		let mut name_objects = self.get(&id_from_iri_string(ChronicleOperations::ActivityName));
		let external_id = name_objects.next().unwrap().as_str().unwrap();
		ActivityId::from_external_id(external_id)
	}

	fn locator(&self) -> Option<String> {
		let mut objects = self.get(&id_from_iri_string(ChronicleOperations::Locator));

		let locator = match objects.next() {
			Some(object) => object,
			None => return None,
		};

		Some(locator.as_str().unwrap().to_owned())
	}

	fn informing_activity(&self) -> ActivityId {
		let mut name_objects =
			self.get(&id_from_iri_string(ChronicleOperations::InformingActivityName));
		let external_id = name_objects.next().unwrap().as_str().unwrap();
		ActivityId::from_external_id(external_id)
	}
}

impl ChronicleOperation {
	pub async fn from_json(json: &Value) -> Result<Self, ProcessorError> {
		use json_ld::Expand;

		let mut output = json_ld::syntax::Value::from_serde_json(json.clone(), |_| ())
			.expand(&mut ContextLoader)
			.await
			.map_err(|e| ProcessorError::Expansion { inner: format!("{e:?}") })?;

		output.canonicalize();
		if let Some(object) = output.into_value().into_objects().into_iter().next() {
			let o =
				object.value().inner().as_node().ok_or(ProcessorError::NotANode(json.clone()))?;
			if o.has_type(&id_from_iri_string(ChronicleOperations::CreateNamespace)) {
				let namespace = o.namespace();
				let external_id = namespace.external_id_part().to_owned();
				let uuid = namespace.uuid_part().to_owned();
				Ok(ChronicleOperation::CreateNamespace(CreateNamespace {
					id: namespace,
					external_id,
					uuid: uuid.into(),
				}))
			} else if o.has_type(&id_from_iri_string(ChronicleOperations::AgentExists)) {
				let namespace = o.namespace();
				let agent = o.agent();
				let external_id = agent.external_id_part();
				Ok(ChronicleOperation::AgentExists(AgentExists {
					namespace,
					external_id: external_id.into(),
				}))
			} else if o.has_type(&id_from_iri_string(ChronicleOperations::AgentActsOnBehalfOf)) {
				let namespace = o.namespace();
				let delegate_id = o.delegate();
				let responsible_id = o.responsible();
				let activity_id = o.optional_activity();

				Ok(ChronicleOperation::AgentActsOnBehalfOf(ActsOnBehalfOf::new(
					&namespace,
					&responsible_id,
					&delegate_id,
					activity_id.as_ref(),
					o.optional_role(),
				)))
			} else if o.has_type(&id_from_iri_string(ChronicleOperations::ActivityExists)) {
				let namespace = o.namespace();
				let activity_id = o.optional_activity().unwrap();
				let external_id = activity_id.external_id_part().to_owned();
				Ok(ChronicleOperation::ActivityExists(ActivityExists { namespace, external_id }))
			} else if o.has_type(&id_from_iri_string(ChronicleOperations::StartActivity)) {
				let namespace = o.namespace();
				let id = o.optional_activity().unwrap();
				let time: DateTime<Utc> = o.start_time().parse().unwrap();
				Ok(ChronicleOperation::StartActivity(StartActivity {
					namespace,
					id,
					time: time.into(),
				}))
			} else if o.has_type(&id_from_iri_string(ChronicleOperations::EndActivity)) {
				let namespace = o.namespace();
				let id = o.optional_activity().unwrap();
				let time: DateTime<Utc> = o.end_time().parse().unwrap();
				Ok(ChronicleOperation::EndActivity(EndActivity {
					namespace,
					id,
					time: time.into(),
				}))
			} else if o.has_type(&id_from_iri_string(ChronicleOperations::ActivityUses)) {
				let namespace = o.namespace();
				let id = o.entity();
				let activity = o.optional_activity().unwrap();
				Ok(ChronicleOperation::ActivityUses(ActivityUses { namespace, id, activity }))
			} else if o.has_type(&id_from_iri_string(ChronicleOperations::EntityExists)) {
				let namespace = o.namespace();
				let entity = o.entity();
				let id = entity.external_id_part().into();
				Ok(ChronicleOperation::EntityExists(EntityExists { namespace, external_id: id }))
			} else if o.has_type(&id_from_iri_string(ChronicleOperations::WasGeneratedBy)) {
				let namespace = o.namespace();
				let id = o.entity();
				let activity = o.optional_activity().unwrap();
				Ok(ChronicleOperation::WasGeneratedBy(WasGeneratedBy { namespace, id, activity }))
			} else if o.has_type(&id_from_iri_string(ChronicleOperations::EntityDerive)) {
				let namespace = o.namespace();
				let id = o.entity();
				let used_id = o.used_entity();
				let activity_id = o.optional_activity();
				let typ = o.derivation();
				Ok(ChronicleOperation::EntityDerive(EntityDerive {
					namespace,
					id,
					used_id,
					activity_id,
					typ,
				}))
			} else if o.has_type(&id_from_iri_string(ChronicleOperations::SetAttributes)) {
				let namespace = o.namespace();
				let domain = o.domain();

				let attrs = o.attributes();

				let attributes = Attributes { typ: domain, attributes: attrs };
				let actor: SetAttributes = {
					if o.has_key(&Term::Id(id_from_iri_string(ChronicleOperations::EntityName))) {
						let id = o.entity();
						SetAttributes::Entity { namespace, id, attributes }
					} else if o
						.has_key(&Term::Id(id_from_iri_string(ChronicleOperations::AgentName)))
					{
						let id = o.agent();
						SetAttributes::Agent { namespace, id, attributes }
					} else {
						let id = o.optional_activity().unwrap();
						SetAttributes::Activity { namespace, id, attributes }
					}
				};

				Ok(ChronicleOperation::SetAttributes(actor))
			} else if o.has_type(&id_from_iri_string(ChronicleOperations::WasAssociatedWith)) {
				Ok(ChronicleOperation::WasAssociatedWith(WasAssociatedWith::new(
					&o.namespace(),
					&o.activity(),
					&o.agent(),
					o.optional_role(),
				)))
			} else if o.has_type(&id_from_iri_string(ChronicleOperations::WasAttributedTo)) {
				Ok(ChronicleOperation::WasAttributedTo(WasAttributedTo::new(
					&o.namespace(),
					&o.entity(),
					&o.agent(),
					o.optional_role(),
				)))
			} else if o.has_type(&id_from_iri_string(ChronicleOperations::WasInformedBy)) {
				let namespace = o.namespace();
				let activity = o.activity();
				let informing_activity = o.informing_activity();
				Ok(ChronicleOperation::WasInformedBy(WasInformedBy {
					namespace,
					activity,
					informing_activity,
				}))
			} else {
				error!("Unknown operation: {:?}", o.type_entry());
				unreachable!()
			}
		} else {
			Err(ProcessorError::NotANode(json.clone()))
		}
	}
}
