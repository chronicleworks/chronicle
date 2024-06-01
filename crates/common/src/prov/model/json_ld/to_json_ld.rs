use serde_json::{json, Value};

#[cfg(not(feature = "std"))]
use parity_scale_codec::{
    alloc::string::{String, ToString},
    alloc::vec::Vec,
};

use super::ExpandedJson;
use crate::{
    attributes::{Attribute, Attributes},
    prov::{
        operations::{ChronicleOperation, CreateNamespace, DerivationType, *},
        vocab::{self, Chronicle, Prov},
        ChronicleIri, ExternalIdPart, FromCompact, ProvModel, UuidPart,
    },
};

pub trait ToJson {
    fn to_json(&self) -> ExpandedJson;
}

impl ToJson for ProvModel {
    fn to_json(&self) -> ExpandedJson {
        let mut doc = Vec::new();

        for (id, ns) in self.namespaces.iter() {
            doc.push(json!({
				"@id": (*id.de_compact()),
				"@type": [Chronicle::Namespace.as_str()],
				"http://chronicle.works/chronicle/ns#externalId": [{
					"@value": ns.external_id.as_str(),
				}]
			}))
        }

        for ((_, id), agent) in self.agents.iter() {
            let mut typ = vec![Prov::Agent.to_string()];
            if let Some(x) = agent.domaintypeid.as_ref() {
                typ.push(x.de_compact())
            }

            if let Value::Object(mut agentdoc) = json!({
				"@id": (*id.de_compact()),
				"@type": typ,
				"http://chronicle.works/chronicle/ns#externalId": [{
				   "@value": agent.external_id.as_str(),
				}]
			}) {
                if let Some(delegation) =
                    self.acted_on_behalf_of.get(&(agent.namespaceid.to_owned(), id.to_owned()))
                {
                    let mut ids = Vec::new();
                    let mut qualified_ids = Vec::new();

                    for delegation in delegation.iter() {
                        ids.push(json!({"@id": delegation.responsible_id.de_compact()}));
                        qualified_ids.push(json!({"@id": delegation.id.de_compact()}));
                    }

                    agentdoc.insert(Prov::ActedOnBehalfOf.to_string(), Value::Array(ids));

                    agentdoc
                        .insert(Prov::QualifiedDelegation.to_string(), Value::Array(qualified_ids));
                }

                let mut values = Vec::new();

                values.push(json!({
					"@id": Value::String(agent.namespaceid.de_compact()),
				}));

                agentdoc.insert(Chronicle::HasNamespace.to_string(), Value::Array(values));

                Self::write_attributes(&mut agentdoc, agent.attributes.iter());

                doc.push(Value::Object(agentdoc));
            }
        }

        for (_, associations) in self.association.iter() {
            for association in (*associations).iter() {
                if let Value::Object(mut associationdoc) = json!({
					"@id": association.id.de_compact(),
					"@type": [Prov::Association.as_str()],
				}) {
                    let mut values = Vec::new();

                    values.push(json!({
						"@id": Value::String(association.agent_id.de_compact()),
					}));

                    associationdoc.insert(Prov::Responsible.to_string(), Value::Array(values));

                    associationdoc.insert(
                        Prov::HadActivity.to_string(),
                        Value::Array(vec![json!({
							"@id": Value::String(association.activity_id.de_compact()),
						})]),
                    );

                    if let Some(role) = &association.role {
                        associationdoc.insert(
                            Prov::HadRole.to_string(),
                            json!([{ "@value": role.to_string()}]),
                        );
                    }

                    let mut values = Vec::new();

                    values.push(json!({
						"@id": Value::String(association.namespace_id.de_compact()),
					}));

                    associationdoc
                        .insert(Chronicle::HasNamespace.to_string(), Value::Array(values));

                    doc.push(Value::Object(associationdoc));
                }
            }
        }

        for (_, attributions) in self.attribution.iter() {
            for attribution in (*attributions).iter() {
                if let Value::Object(mut attribution_doc) = json!({
					"@id": attribution.id.de_compact(),
					"@type": [Prov::Attribution.as_str()],
				}) {
                    let mut values = Vec::new();

                    values.push(json!({
						"@id": Value::String(attribution.agent_id.de_compact()),
					}));

                    attribution_doc.insert(Prov::Responsible.to_string(), Value::Array(values));

                    attribution_doc.insert(
                        Prov::HadEntity.to_string(),
                        Value::Array(vec![json!({
							"@id": Value::String(attribution.entity_id.de_compact()),
						})]),
                    );

                    if let Some(role) = &attribution.role {
                        attribution_doc.insert(
                            Prov::HadRole.to_string(),
                            json!([{ "@value": role.to_string()}]),
                        );
                    }

                    let mut values = Vec::new();

                    values.push(json!({
						"@id": Value::String(attribution.namespace_id.de_compact()),
					}));

                    attribution_doc
                        .insert(Chronicle::HasNamespace.to_string(), Value::Array(values));

                    doc.push(Value::Object(attribution_doc));
                }
            }
        }

        for (_, delegations) in self.delegation.iter() {
            for delegation in (*delegations).iter() {
                if let Value::Object(mut delegationdoc) = json!({
					"@id": delegation.id.de_compact(),
					"@type": [Prov::Delegation.as_str()],
				}) {
                    if let Some(activity_id) = &delegation.activity_id {
                        delegationdoc.insert(
                            Prov::HadActivity.to_string(),
                            Value::Array(vec![json!({
								"@id": Value::String(activity_id.de_compact()),
							})]),
                        );
                    }

                    if let Some(role) = &delegation.role {
                        delegationdoc.insert(
                            Prov::HadRole.to_string(),
                            json!([{ "@value": role.to_string()}]),
                        );
                    }

                    let mut responsible_ids = Vec::new();
                    responsible_ids.push(
                        json!({ "@id": Value::String(delegation.responsible_id.de_compact())}),
                    );

                    delegationdoc
                        .insert(Prov::ActedOnBehalfOf.to_string(), Value::Array(responsible_ids));

                    let mut delegate_ids = Vec::new();
                    delegate_ids
                        .push(json!({ "@id": Value::String(delegation.delegate_id.de_compact())}));

                    delegationdoc.insert(Prov::Delegate.to_string(), Value::Array(delegate_ids));

                    let mut values = Vec::new();

                    values.push(json!({
						"@id": Value::String(delegation.namespace_id.de_compact()),
					}));

                    delegationdoc.insert(Chronicle::HasNamespace.to_string(), Value::Array(values));

                    doc.push(Value::Object(delegationdoc));
                }
            }
        }

        for ((namespace, id), activity) in self.activities.iter() {
            let mut typ = vec![Prov::Activity.de_compact()];
            if let Some(x) = activity.domaintype_id.as_ref() {
                typ.push(x.de_compact())
            }

            if let Value::Object(mut activitydoc) = json!({
				"@id": (*id.de_compact()),
				"@type": typ,
				"http://chronicle.works/chronicle/ns#externalId": [{
				   "@value": activity.external_id.as_str(),
				}]
			}) {
                if let Some(time) = activity.started {
                    let mut values = Vec::new();
                    values.push(json!({"@value": time.to_rfc3339()}));

                    activitydoc.insert(
                        "http://www.w3.org/ns/prov#startedAtTime".to_string(),
                        Value::Array(values),
                    );
                }

                if let Some(time) = activity.ended {
                    let mut values = Vec::new();
                    values.push(json!({"@value": time.to_rfc3339()}));

                    activitydoc.insert(
                        "http://www.w3.org/ns/prov#endedAtTime".to_string(),
                        Value::Array(values),
                    );
                }

                if let Some(asoc) = self.association.get(&(namespace.to_owned(), id.to_owned())) {
                    let mut ids = Vec::new();

                    let mut qualified_ids = Vec::new();
                    for asoc in asoc.iter() {
                        ids.push(json!({"@id": asoc.agent_id.de_compact()}));
                        qualified_ids.push(json!({"@id": asoc.id.de_compact()}));
                    }

                    activitydoc.insert(Prov::WasAssociatedWith.de_compact(), Value::Array(ids));

                    activitydoc.insert(
                        Prov::QualifiedAssociation.de_compact(),
                        Value::Array(qualified_ids),
                    );
                }

                if let Some(usage) = self.usage.get(&(namespace.to_owned(), id.to_owned())) {
                    let mut ids = Vec::new();

                    for usage in usage.iter() {
                        ids.push(json!({"@id": usage.entity_id.de_compact()}));
                    }

                    activitydoc.insert(Prov::Used.de_compact(), Value::Array(ids));
                }

                let mut values = Vec::new();

                values.push(json!({
					"@id": Value::String(activity.namespace_id.de_compact()),
				}));

                activitydoc.insert(Chronicle::HasNamespace.to_string(), Value::Array(values));

                if let Some(activities) =
                    self.was_informed_by.get(&(namespace.to_owned(), id.to_owned()))
                {
                    let mut values = Vec::new();

                    for (_, activity) in (*activities).iter() {
                        values.push(json!({
							"@id": Value::String(activity.de_compact()),
						}));
                    }
                    activitydoc.insert(Prov::WasInformedBy.to_string(), Value::Array(values));
                }

                Self::write_attributes(&mut activitydoc, activity.attributes.iter());

                doc.push(Value::Object(activitydoc));
            }
        }

        for ((namespace, id), entity) in self.entities.iter() {
            let mut typ = vec![Prov::Entity.de_compact()];
            if let Some(x) = entity.domaintypeid.as_ref() {
                typ.push(x.de_compact())
            }

            if let Value::Object(mut entitydoc) = json!({
				"@id": (*id.de_compact()),
				"@type": typ,
				"http://chronicle.works/chronicle/ns#externalId": [{
				   "@value": entity.external_id.as_str()
				}]
			}) {
                if let Some(derivation) =
                    self.derivation.get(&(namespace.to_owned(), id.to_owned()))
                {
                    let mut derived_ids = Vec::new();
                    let mut primary_ids = Vec::new();
                    let mut quotation_ids = Vec::new();
                    let mut revision_ids = Vec::new();

                    for derivation in derivation.iter() {
                        let id = json!({"@id": derivation.used_id.de_compact()});
                        match derivation.typ {
                            DerivationType::PrimarySource => primary_ids.push(id),
                            DerivationType::Quotation => quotation_ids.push(id),
                            DerivationType::Revision => revision_ids.push(id),
                            DerivationType::None => derived_ids.push(id),
                        }
                    }
                    if !derived_ids.is_empty() {
                        entitydoc
                            .insert(Prov::WasDerivedFrom.to_string(), Value::Array(derived_ids));
                    }
                    if !primary_ids.is_empty() {
                        entitydoc
                            .insert(Prov::HadPrimarySource.to_string(), Value::Array(primary_ids));
                    }
                    if !quotation_ids.is_empty() {
                        entitydoc
                            .insert(Prov::WasQuotedFrom.to_string(), Value::Array(quotation_ids));
                    }
                    if !revision_ids.is_empty() {
                        entitydoc
                            .insert(Prov::WasRevisionOf.to_string(), Value::Array(revision_ids));
                    }
                }

                if let Some(generation) =
                    self.generation.get(&(namespace.to_owned(), id.to_owned()))
                {
                    let mut ids = Vec::new();

                    for generation in generation.iter() {
                        ids.push(json!({"@id": generation.activity_id.de_compact()}));
                    }

                    entitydoc.insert(Prov::WasGeneratedBy.to_string(), Value::Array(ids));
                }

                let entity_key = (entity.namespace_id.clone(), entity.id.clone());

                if let Some(attributions) = self.attribution.get(&entity_key) {
                    let mut ids = Vec::new();

                    let mut qualified_ids = Vec::new();
                    for attribution in attributions.iter() {
                        ids.push(json!({"@id": attribution.agent_id.de_compact()}));
                        qualified_ids.push(json!({"@id": attribution.id.de_compact()}));
                    }

                    entitydoc.insert(Prov::WasAttributedTo.de_compact(), Value::Array(ids));

                    entitydoc.insert(
                        Prov::QualifiedAttribution.de_compact(),
                        Value::Array(qualified_ids),
                    );
                }

                let mut values = Vec::new();

                values.push(json!({
					"@id": Value::String(entity.namespace_id.de_compact()),
				}));

                entitydoc.insert(Chronicle::HasNamespace.to_string(), Value::Array(values));

                Self::write_attributes(&mut entitydoc, entity.attributes.iter());

                doc.push(Value::Object(entitydoc));
            }
        }

        ExpandedJson(Value::Array(doc))
    }
}

impl ProvModel {
    fn write_attributes<'a, I: Iterator<Item=&'a Attribute>>(
        doc: &mut serde_json::Map<String, Value>,
        attributes: I,
    ) {
        let mut attribute_node = serde_json::Map::new();

        for attribute in attributes {
            attribute_node.insert(attribute.typ.clone(), attribute.value.0.clone());
        }

        doc.insert(
            Chronicle::Value.to_string(),
            json!([{"@value" : Value::Object(attribute_node), "@type": "@json"}]),
        );
    }
}

impl ToJson for ChronicleOperation {
    fn to_json(&self) -> ExpandedJson {
        let mut operation: Vec<Value> = Vec::new();

        let o = match self {
            ChronicleOperation::CreateNamespace(CreateNamespace { id, .. }) => {
                let mut o = Value::new_operation(vocab::ChronicleOperation::CreateNamespace);

                o.has_value(
                    OperationValue::string(id.external_id_part()),
                    vocab::ChronicleOperation::NamespaceName,
                );

                o.has_value(
                    OperationValue::string(id.uuid_part()),
                    vocab::ChronicleOperation::NamespaceUuid,
                );

                o
            }
            ChronicleOperation::AgentExists(AgentExists { namespace, id }) => {
                let mut o = Value::new_operation(vocab::ChronicleOperation::AgentExists);

                o.has_value(
                    OperationValue::string(namespace.external_id_part()),
                    vocab::ChronicleOperation::NamespaceName,
                );

                o.has_value(
                    OperationValue::string(namespace.uuid_part()),
                    vocab::ChronicleOperation::NamespaceUuid,
                );

                o.has_value(
                    OperationValue::string(id.external_id_part()),
                    vocab::ChronicleOperation::AgentName,
                );

                o
            }
            ChronicleOperation::AgentActsOnBehalfOf(ActsOnBehalfOf {
                                                        namespace,
                                                        id: _, // This is derivable from components
                                                        delegate_id,
                                                        activity_id,
                                                        role,
                                                        responsible_id,
                                                    }) => {
                let mut o = Value::new_operation(vocab::ChronicleOperation::AgentActsOnBehalfOf);

                o.has_value(
                    OperationValue::string(namespace.external_id_part()),
                    vocab::ChronicleOperation::NamespaceName,
                );

                o.has_value(
                    OperationValue::string(namespace.uuid_part()),
                    vocab::ChronicleOperation::NamespaceUuid,
                );

                o.has_value(
                    OperationValue::string(delegate_id.external_id_part()),
                    vocab::ChronicleOperation::DelegateId,
                );

                o.has_value(
                    OperationValue::string(responsible_id.external_id_part()),
                    vocab::ChronicleOperation::ResponsibleId,
                );

                if let Some(role) = role {
                    o.has_value(OperationValue::string(role), vocab::ChronicleOperation::Role);
                }

                if let Some(activity_id) = activity_id {
                    o.has_value(
                        OperationValue::string(activity_id.external_id_part()),
                        vocab::ChronicleOperation::ActivityName,
                    );
                }

                o
            }
            ChronicleOperation::ActivityExists(ActivityExists { namespace, id }) => {
                let mut o = Value::new_operation(vocab::ChronicleOperation::ActivityExists);

                o.has_value(
                    OperationValue::string(namespace.external_id_part()),
                    vocab::ChronicleOperation::NamespaceName,
                );

                o.has_value(
                    OperationValue::string(namespace.uuid_part()),
                    vocab::ChronicleOperation::NamespaceUuid,
                );

                o.has_value(
                    OperationValue::string(id.external_id_part()),
                    vocab::ChronicleOperation::ActivityName,
                );

                o
            }
            ChronicleOperation::StartActivity(StartActivity { namespace, id, time }) => {
                let mut o = Value::new_operation(vocab::ChronicleOperation::StartActivity);

                o.has_value(
                    OperationValue::string(namespace.external_id_part()),
                    vocab::ChronicleOperation::NamespaceName,
                );

                o.has_value(
                    OperationValue::string(namespace.uuid_part()),
                    vocab::ChronicleOperation::NamespaceUuid,
                );

                o.has_value(
                    OperationValue::string(id.external_id_part()),
                    vocab::ChronicleOperation::ActivityName,
                );

                o.has_value(
                    OperationValue::string(time.to_rfc3339()),
                    vocab::ChronicleOperation::StartActivityTime,
                );

                o
            }
            ChronicleOperation::EndActivity(EndActivity { namespace, id, time }) => {
                let mut o = Value::new_operation(vocab::ChronicleOperation::EndActivity);

                o.has_value(
                    OperationValue::string(namespace.external_id_part()),
                    vocab::ChronicleOperation::NamespaceName,
                );

                o.has_value(
                    OperationValue::string(namespace.uuid_part()),
                    vocab::ChronicleOperation::NamespaceUuid,
                );

                o.has_value(
                    OperationValue::string(id.external_id_part()),
                    vocab::ChronicleOperation::ActivityName,
                );

                o.has_value(
                    OperationValue::string(time.to_rfc3339()),
                    vocab::ChronicleOperation::EndActivityTime,
                );

                o
            }
            ChronicleOperation::ActivityUses(ActivityUses { namespace, id, activity }) => {
                let mut o = Value::new_operation(vocab::ChronicleOperation::ActivityUses);

                o.has_value(
                    OperationValue::string(namespace.external_id_part()),
                    vocab::ChronicleOperation::NamespaceName,
                );

                o.has_value(
                    OperationValue::string(namespace.uuid_part()),
                    vocab::ChronicleOperation::NamespaceUuid,
                );

                o.has_value(
                    OperationValue::string(id.external_id_part()),
                    vocab::ChronicleOperation::EntityName,
                );

                o.has_value(
                    OperationValue::string(activity.external_id_part()),
                    vocab::ChronicleOperation::ActivityName,
                );

                o
            }
            ChronicleOperation::EntityExists(EntityExists { namespace, id }) => {
                let mut o = Value::new_operation(vocab::ChronicleOperation::EntityExists);

                o.has_value(
                    OperationValue::string(namespace.external_id_part()),
                    vocab::ChronicleOperation::NamespaceName,
                );

                o.has_value(
                    OperationValue::string(namespace.uuid_part()),
                    vocab::ChronicleOperation::NamespaceUuid,
                );

                o.has_value(
                    OperationValue::string(id.external_id_part()),
                    vocab::ChronicleOperation::EntityName,
                );

                o
            }
            ChronicleOperation::WasGeneratedBy(WasGeneratedBy { namespace, id, activity }) => {
                let mut o = Value::new_operation(vocab::ChronicleOperation::WasGeneratedBy);

                o.has_value(
                    OperationValue::string(namespace.external_id_part()),
                    vocab::ChronicleOperation::NamespaceName,
                );

                o.has_value(
                    OperationValue::string(namespace.uuid_part()),
                    vocab::ChronicleOperation::NamespaceUuid,
                );

                o.has_value(
                    OperationValue::string(id.external_id_part()),
                    vocab::ChronicleOperation::EntityName,
                );

                o.has_value(
                    OperationValue::string(activity.external_id_part()),
                    vocab::ChronicleOperation::ActivityName,
                );

                o
            }
            ChronicleOperation::WasInformedBy(WasInformedBy {
                                                  namespace,
                                                  activity,
                                                  informing_activity,
                                              }) => {
                let mut o = Value::new_operation(vocab::ChronicleOperation::WasInformedBy);

                o.has_value(
                    OperationValue::string(namespace.external_id_part()),
                    vocab::ChronicleOperation::NamespaceName,
                );

                o.has_value(
                    OperationValue::string(namespace.uuid_part()),
                    vocab::ChronicleOperation::NamespaceUuid,
                );

                o.has_value(
                    OperationValue::string(activity.external_id_part()),
                    vocab::ChronicleOperation::ActivityName,
                );

                o.has_value(
                    OperationValue::string(informing_activity.external_id_part()),
                    vocab::ChronicleOperation::InformingActivityName,
                );

                o
            }
            ChronicleOperation::EntityDerive(EntityDerive {
                                                 namespace,
                                                 id,
                                                 used_id,
                                                 activity_id,
                                                 typ,
                                             }) => {
                let mut o = Value::new_operation(vocab::ChronicleOperation::EntityDerive);

                o.has_value(
                    OperationValue::string(namespace.external_id_part()),
                    vocab::ChronicleOperation::NamespaceName,
                );

                o.has_value(
                    OperationValue::string(namespace.uuid_part()),
                    vocab::ChronicleOperation::NamespaceUuid,
                );

                o.has_value(
                    OperationValue::string(id.external_id_part()),
                    vocab::ChronicleOperation::EntityName,
                );

                o.has_value(
                    OperationValue::string(used_id.external_id_part()),
                    vocab::ChronicleOperation::UsedEntityName,
                );

                if let Some(activity) = activity_id {
                    o.has_value(
                        OperationValue::string(activity.external_id_part()),
                        vocab::ChronicleOperation::ActivityName,
                    );
                }

                if *typ != DerivationType::None {
                    o.derivation(typ);
                }

                o
            }
            ChronicleOperation::SetAttributes(SetAttributes::Entity {
                                                  namespace,
                                                  id,
                                                  attributes,
                                              }) => {
                let mut o = Value::new_operation(vocab::ChronicleOperation::SetAttributes);

                o.has_value(
                    OperationValue::string(namespace.external_id_part()),
                    vocab::ChronicleOperation::NamespaceName,
                );

                o.has_value(
                    OperationValue::string(namespace.uuid_part()),
                    vocab::ChronicleOperation::NamespaceUuid,
                );

                o.has_value(
                    OperationValue::string(id.external_id_part()),
                    vocab::ChronicleOperation::EntityName,
                );

                if let Some(domaintypeid) = attributes.get_typ() {
                    let id = OperationValue::string(domaintypeid.external_id_part());
                    o.has_value(id, vocab::ChronicleOperation::DomaintypeId);
                }

                o.attributes_object(attributes);

                o
            }
            ChronicleOperation::SetAttributes(SetAttributes::Activity {
                                                  namespace,
                                                  id,
                                                  attributes,
                                              }) => {
                let mut o = Value::new_operation(vocab::ChronicleOperation::SetAttributes);

                o.has_value(
                    OperationValue::string(namespace.external_id_part()),
                    vocab::ChronicleOperation::NamespaceName,
                );

                o.has_value(
                    OperationValue::string(namespace.uuid_part()),
                    vocab::ChronicleOperation::NamespaceUuid,
                );

                o.has_value(
                    OperationValue::string(id.external_id_part()),
                    vocab::ChronicleOperation::ActivityName,
                );

                if let Some(domaintypeid) = attributes.get_typ() {
                    let id = OperationValue::string(domaintypeid.external_id_part());
                    o.has_value(id, vocab::ChronicleOperation::DomaintypeId);
                }

                o.attributes_object(attributes);

                o
            }
            ChronicleOperation::SetAttributes(SetAttributes::Agent {
                                                  namespace,
                                                  id,
                                                  attributes,
                                              }) => {
                let mut o = Value::new_operation(vocab::ChronicleOperation::SetAttributes);

                o.has_value(
                    OperationValue::string(namespace.external_id_part()),
                    vocab::ChronicleOperation::NamespaceName,
                );

                o.has_value(
                    OperationValue::string(namespace.uuid_part()),
                    vocab::ChronicleOperation::NamespaceUuid,
                );

                o.has_value(
                    OperationValue::string(id.external_id_part()),
                    vocab::ChronicleOperation::AgentName,
                );

                if let Some(domaintypeid) = attributes.get_typ() {
                    let id = OperationValue::string(domaintypeid.external_id_part());
                    o.has_value(id, vocab::ChronicleOperation::DomaintypeId);
                }

                o.attributes_object(attributes);

                o
            }
            ChronicleOperation::WasAssociatedWith(WasAssociatedWith {
                                                      id: _,
                                                      role,
                                                      namespace,
                                                      activity_id,
                                                      agent_id,
                                                  }) => {
                let mut o = Value::new_operation(vocab::ChronicleOperation::WasAssociatedWith);

                o.has_value(
                    OperationValue::string(namespace.external_id_part()),
                    vocab::ChronicleOperation::NamespaceName,
                );

                o.has_value(
                    OperationValue::string(namespace.uuid_part()),
                    vocab::ChronicleOperation::NamespaceUuid,
                );

                o.has_value(
                    OperationValue::string(activity_id.external_id_part()),
                    vocab::ChronicleOperation::ActivityName,
                );

                o.has_value(
                    OperationValue::string(agent_id.external_id_part()),
                    vocab::ChronicleOperation::AgentName,
                );

                if let Some(role) = role {
                    o.has_value(OperationValue::string(role), vocab::ChronicleOperation::Role);
                }

                o
            }
            ChronicleOperation::WasAttributedTo(WasAttributedTo {
                                                    id: _,
                                                    role,
                                                    namespace,
                                                    entity_id,
                                                    agent_id,
                                                }) => {
                let mut o = Value::new_operation(vocab::ChronicleOperation::WasAttributedTo);

                o.has_value(
                    OperationValue::string(namespace.external_id_part()),
                    vocab::ChronicleOperation::NamespaceName,
                );

                o.has_value(
                    OperationValue::string(namespace.uuid_part()),
                    vocab::ChronicleOperation::NamespaceUuid,
                );

                o.has_value(
                    OperationValue::string(entity_id.external_id_part()),
                    vocab::ChronicleOperation::EntityName,
                );

                o.has_value(
                    OperationValue::string(agent_id.external_id_part()),
                    vocab::ChronicleOperation::AgentName,
                );

                if let Some(role) = role {
                    o.has_value(OperationValue::string(role), vocab::ChronicleOperation::Role);
                }

                o
            }
        };
        operation.push(o);
        super::ExpandedJson(operation.into())
    }
}

struct OperationValue(String);

impl OperationValue {
    fn string(value: impl ToString) -> Self {
        OperationValue(value.to_string())
    }

    #[allow(dead_code)]
    fn identity(id: ChronicleIri) -> Self {
        OperationValue(id.to_string())
    }
}

trait Operate {
    fn new_operation(op: vocab::ChronicleOperation) -> Self;
    fn new_type(id: OperationValue, op: vocab::ChronicleOperation) -> Self;
    fn new_value(id: OperationValue) -> Self;
    fn new_id(id: OperationValue) -> Self;
    fn has_value(&mut self, value: OperationValue, op: vocab::ChronicleOperation);
    fn has_id(&mut self, id: OperationValue, op: vocab::ChronicleOperation);
    fn attributes_object(&mut self, attributes: &Attributes);
    fn derivation(&mut self, typ: &DerivationType);
}

impl Operate for Value {
    fn new_type(id: OperationValue, op: vocab::ChronicleOperation) -> Self {
        json!({
			"@id": id.0,
			"@type": [op.as_str()],
		})
    }

    fn new_value(id: OperationValue) -> Self {
        json!({
			"@value": id.0
		})
    }

    fn new_id(id: OperationValue) -> Self {
        json!({
			"@id": id.0
		})
    }

    fn has_value(&mut self, value: OperationValue, op: vocab::ChronicleOperation) {
        if let Value::Object(map) = self {
            let key = op.to_string();
            let mut values: Vec<Value> = Vec::new();
            let object = Self::new_value(value);
            values.push(object);
            map.insert(key, Value::Array(values));
        } else {
            panic!("use on JSON objects only");
        }
    }

    fn has_id(&mut self, id: OperationValue, op: vocab::ChronicleOperation) {
        if let Value::Object(map) = self {
            let key = op.to_string();
            let mut value: Vec<Value> = Vec::new();
            let object = Self::new_id(id);
            value.push(object);
            map.insert(key, Value::Array(value));
        } else {
            panic!("use on JSON objects only");
        }
    }

    fn new_operation(op: vocab::ChronicleOperation) -> Self {
        let id = OperationValue::string("_:n1");
        Self::new_type(id, op)
    }

    fn attributes_object(&mut self, attributes: &Attributes) {
        if let Value::Object(map) = self {
            let mut attribute_node = serde_json::Map::new();
            for attribute in attributes.get_values() {
                attribute_node.insert(attribute.typ.clone(), attribute.value.0.clone());
            }
            map.insert(
                vocab::ChronicleOperation::Attributes.to_string(),
                json!([{"@value" : attribute_node, "@type": "@json"}]),
            );
        } else {
            panic!("use on JSON objects only");
        }
    }

    fn derivation(&mut self, typ: &DerivationType) {
        let typ = match typ {
            DerivationType::None => panic!("should never handle a None derivation type"),
            DerivationType::Revision => "Revision",
            DerivationType::Quotation => "Quotation",
            DerivationType::PrimarySource => "PrimarySource",
        };
        let id = OperationValue::string(typ);

        self.has_value(id, vocab::ChronicleOperation::DerivationType);
    }
}