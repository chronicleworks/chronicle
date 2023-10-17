use iref::{AsIri, Iri};
use serde_json::{json, Value};

use crate::{
    attributes::{Attribute, Attributes},
    prov::{
        operations::{ChronicleOperation, CreateNamespace, DerivationType},
        vocab::{Chronicle, ChronicleOperations, Prov},
        ChronicleIri, ExternalIdPart, FromCompact, UuidPart,
    },
};

use super::{ExpandedJson, ProvModel};
use crate::prov::operations::*;
pub trait ToJson {
    fn to_json(&self) -> ExpandedJson;
}

impl ToJson for ProvModel {
    /// Write the model out as a JSON-LD document in expanded form
    fn to_json(&self) -> ExpandedJson {
        let mut doc = Vec::new();

        for (id, ns) in self.namespaces.iter() {
            doc.push(json!({
                "@id": (*id.de_compact()),
                "@type": [Iri::from(Chronicle::Namespace).as_str()],
                "http://btp.works/chronicle/ns#externalId": [{
                    "@value": ns.external_id.as_str(),
                }]
            }))
        }

        for ((_, id), agent) in self.agents.iter() {
            let mut typ = vec![Iri::from(Prov::Agent).to_string()];
            if let Some(x) = agent.domaintypeid.as_ref() {
                typ.push(x.de_compact())
            }

            if let Value::Object(mut agentdoc) = json!({
                "@id": (*id.de_compact()),
                "@type": typ,
                "http://btp.works/chronicle/ns#externalId": [{
                   "@value": agent.external_id.as_str(),
                }]
            }) {
                let agent_key = (agent.namespaceid.clone(), agent.id.clone());

                if let Some(delegation) = self
                    .acted_on_behalf_of
                    .get(&(agent.namespaceid.to_owned(), id.to_owned()))
                {
                    let mut ids = Vec::new();
                    let mut qualified_ids = Vec::new();

                    for delegation in delegation.iter() {
                        ids.push(json!({"@id": delegation.responsible_id.de_compact()}));
                        qualified_ids.push(json!({"@id": delegation.id.de_compact()}));
                    }

                    agentdoc.insert(
                        Iri::from(Prov::ActedOnBehalfOf).to_string(),
                        Value::Array(ids),
                    );

                    agentdoc.insert(
                        Iri::from(Prov::QualifiedDelegation).to_string(),
                        Value::Array(qualified_ids),
                    );
                }

                let mut values = Vec::new();

                values.push(json!({
                    "@id": Value::String(agent.namespaceid.de_compact()),
                }));

                agentdoc.insert(
                    Iri::from(Chronicle::HasNamespace).to_string(),
                    Value::Array(values),
                );

                Self::write_attributes(&mut agentdoc, agent.attributes.values());

                doc.push(Value::Object(agentdoc));
            }
        }

        for (_, associations) in self.association.iter() {
            for association in associations {
                if let Value::Object(mut associationdoc) = json!({
                    "@id": association.id.de_compact(),
                    "@type": [Iri::from(Prov::Association).as_str()],
                }) {
                    let mut values = Vec::new();

                    values.push(json!({
                        "@id": Value::String(association.agent_id.de_compact()),
                    }));

                    associationdoc.insert(
                        Iri::from(Prov::Responsible).to_string(),
                        Value::Array(values),
                    );

                    associationdoc.insert(
                        Iri::from(Prov::HadActivity).to_string(),
                        Value::Array(vec![json!({
                            "@id": Value::String(association.activity_id.de_compact()),
                        })]),
                    );

                    if let Some(role) = &association.role {
                        associationdoc.insert(
                            Iri::from(Prov::HadRole).to_string(),
                            json!([{ "@value": role.to_string()}]),
                        );
                    }

                    let mut values = Vec::new();

                    values.push(json!({
                        "@id": Value::String(association.namespace_id.de_compact()),
                    }));

                    associationdoc.insert(
                        Iri::from(Chronicle::HasNamespace).to_string(),
                        Value::Array(values),
                    );

                    doc.push(Value::Object(associationdoc));
                }
            }
        }

        for (_, attributions) in self.attribution.iter() {
            for attribution in attributions {
                if let Value::Object(mut attribution_doc) = json!({
                    "@id": attribution.id.de_compact(),
                    "@type": [Iri::from(Prov::Attribution).as_str()],
                }) {
                    let mut values = Vec::new();

                    values.push(json!({
                        "@id": Value::String(attribution.agent_id.de_compact()),
                    }));

                    attribution_doc.insert(
                        Iri::from(Prov::Responsible).to_string(),
                        Value::Array(values),
                    );

                    attribution_doc.insert(
                        Iri::from(Prov::HadEntity).to_string(),
                        Value::Array(vec![json!({
                            "@id": Value::String(attribution.entity_id.de_compact()),
                        })]),
                    );

                    if let Some(role) = &attribution.role {
                        attribution_doc.insert(
                            Iri::from(Prov::HadRole).to_string(),
                            json!([{ "@value": role.to_string()}]),
                        );
                    }

                    let mut values = Vec::new();

                    values.push(json!({
                        "@id": Value::String(attribution.namespace_id.de_compact()),
                    }));

                    attribution_doc.insert(
                        Iri::from(Chronicle::HasNamespace).to_string(),
                        Value::Array(values),
                    );

                    doc.push(Value::Object(attribution_doc));
                }
            }
        }

        for (_, delegations) in self.delegation.iter() {
            for delegation in delegations {
                if let Value::Object(mut delegationdoc) = json!({
                    "@id": delegation.id.de_compact(),
                    "@type": [Iri::from(Prov::Delegation).as_str()],
                }) {
                    if let Some(activity_id) = &delegation.activity_id {
                        delegationdoc.insert(
                            Iri::from(Prov::HadActivity).to_string(),
                            Value::Array(vec![json!({
                                "@id": Value::String(activity_id.de_compact()),
                            })]),
                        );
                    }

                    if let Some(role) = &delegation.role {
                        delegationdoc.insert(
                            Iri::from(Prov::HadRole).to_string(),
                            json!([{ "@value": role.to_string()}]),
                        );
                    }

                    let mut responsible_ids = Vec::new();
                    responsible_ids.push(
                        json!({ "@id": Value::String(delegation.responsible_id.de_compact())}),
                    );

                    delegationdoc.insert(
                        Iri::from(Prov::ActedOnBehalfOf).to_string(),
                        Value::Array(responsible_ids),
                    );

                    let mut delegate_ids = Vec::new();
                    delegate_ids
                        .push(json!({ "@id": Value::String(delegation.delegate_id.de_compact())}));

                    delegationdoc.insert(
                        Iri::from(Prov::Delegate).to_string(),
                        Value::Array(delegate_ids),
                    );

                    let mut values = Vec::new();

                    values.push(json!({
                        "@id": Value::String(delegation.namespace_id.de_compact()),
                    }));

                    delegationdoc.insert(
                        Iri::from(Chronicle::HasNamespace).to_string(),
                        Value::Array(values),
                    );

                    doc.push(Value::Object(delegationdoc));
                }
            }
        }

        for ((namespace, id), activity) in self.activities.iter() {
            let mut typ = vec![Iri::from(Prov::Activity).de_compact()];
            if let Some(x) = activity.domaintypeid.as_ref() {
                typ.push(x.de_compact())
            }

            if let Value::Object(mut activitydoc) = json!({
                "@id": (*id.de_compact()),
                "@type": typ,
                "http://btp.works/chronicle/ns#externalId": [{
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

                    activitydoc.insert(
                        Iri::from(Prov::WasAssociatedWith).de_compact(),
                        Value::Array(ids),
                    );

                    activitydoc.insert(
                        Iri::from(Prov::QualifiedAssociation).de_compact(),
                        Value::Array(qualified_ids),
                    );
                }

                if let Some(usage) = self.usage.get(&(namespace.to_owned(), id.to_owned())) {
                    let mut ids = Vec::new();

                    for usage in usage.iter() {
                        ids.push(json!({"@id": usage.entity_id.de_compact()}));
                    }

                    activitydoc.insert(Iri::from(Prov::Used).de_compact(), Value::Array(ids));
                }

                let mut values = Vec::new();

                values.push(json!({
                    "@id": Value::String(activity.namespaceid.de_compact()),
                }));

                activitydoc.insert(
                    Iri::from(Chronicle::HasNamespace).to_string(),
                    Value::Array(values),
                );

                if let Some(activities) = self
                    .was_informed_by
                    .get(&(namespace.to_owned(), id.to_owned()))
                {
                    let mut values = Vec::new();

                    for (_, activity) in activities {
                        values.push(json!({
                            "@id": Value::String(activity.de_compact()),
                        }));
                    }
                    activitydoc.insert(
                        Iri::from(Prov::WasInformedBy).to_string(),
                        Value::Array(values),
                    );
                }

                Self::write_attributes(&mut activitydoc, activity.attributes.values());

                doc.push(Value::Object(activitydoc));
            }
        }

        for ((namespace, id), entity) in self.entities.iter() {
            let mut typ = vec![Iri::from(Prov::Entity).de_compact()];
            if let Some(x) = entity.domaintypeid.as_ref() {
                typ.push(x.de_compact())
            }

            if let Value::Object(mut entitydoc) = json!({
                "@id": (*id.de_compact()),
                "@type": typ,
                "http://btp.works/chronicle/ns#externalId": [{
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
                        entitydoc.insert(
                            Iri::from(Prov::WasDerivedFrom).to_string(),
                            Value::Array(derived_ids),
                        );
                    }
                    if !primary_ids.is_empty() {
                        entitydoc.insert(
                            Iri::from(Prov::HadPrimarySource).to_string(),
                            Value::Array(primary_ids),
                        );
                    }
                    if !quotation_ids.is_empty() {
                        entitydoc.insert(
                            Iri::from(Prov::WasQuotedFrom).to_string(),
                            Value::Array(quotation_ids),
                        );
                    }
                    if !revision_ids.is_empty() {
                        entitydoc.insert(
                            Iri::from(Prov::WasRevisionOf).to_string(),
                            Value::Array(revision_ids),
                        );
                    }
                }

                if let Some(generation) =
                    self.generation.get(&(namespace.to_owned(), id.to_owned()))
                {
                    let mut ids = Vec::new();

                    for generation in generation.iter() {
                        ids.push(json!({"@id": generation.activity_id.de_compact()}));
                    }

                    entitydoc.insert(
                        Iri::from(Prov::WasGeneratedBy).to_string(),
                        Value::Array(ids),
                    );
                }

                let entity_key = (entity.namespaceid.clone(), entity.id.clone());

                if let Some(attributions) = self.attribution.get(&entity_key) {
                    let mut ids = Vec::new();

                    let mut qualified_ids = Vec::new();
                    for attribution in attributions.iter() {
                        ids.push(json!({"@id": attribution.agent_id.de_compact()}));
                        qualified_ids.push(json!({"@id": attribution.id.de_compact()}));
                    }

                    entitydoc.insert(
                        Iri::from(Prov::WasAttributedTo).de_compact(),
                        Value::Array(ids),
                    );

                    entitydoc.insert(
                        Iri::from(Prov::QualifiedAttribution).de_compact(),
                        Value::Array(qualified_ids),
                    );
                }

                let mut values = Vec::new();

                values.push(json!({
                    "@id": Value::String(entity.namespaceid.de_compact()),
                }));

                entitydoc.insert(
                    Iri::from(Chronicle::HasNamespace).to_string(),
                    Value::Array(values),
                );

                Self::write_attributes(&mut entitydoc, entity.attributes.values());

                doc.push(Value::Object(entitydoc));
            }
        }

        ExpandedJson(Value::Array(doc))
    }
}

impl ProvModel {
    fn write_attributes<'a, I: Iterator<Item = &'a Attribute>>(
        doc: &mut serde_json::Map<String, Value>,
        attributes: I,
    ) {
        let mut attribute_node = serde_json::Map::new();

        for attribute in attributes {
            attribute_node.insert(attribute.typ.clone(), attribute.value.clone());
        }

        doc.insert(
            Chronicle::Value.as_iri().to_string(),
            json!([{"@value" : Value::Object(attribute_node), "@type": "@json"}]),
        );
    }
}

impl ToJson for ChronicleOperation {
    fn to_json(&self) -> ExpandedJson {
        let mut operation: Vec<Value> = Vec::new();

        let o = match self {
            ChronicleOperation::CreateNamespace(CreateNamespace { id, .. }) => {
                let mut o = Value::new_operation(ChronicleOperations::CreateNamespace);

                o.has_value(
                    OperationValue::string(id.external_id_part()),
                    ChronicleOperations::NamespaceName,
                );

                o.has_value(
                    OperationValue::string(id.uuid_part()),
                    ChronicleOperations::NamespaceUuid,
                );

                o
            }
            ChronicleOperation::AgentExists(AgentExists {
                namespace,
                external_id,
            }) => {
                let mut o = Value::new_operation(ChronicleOperations::AgentExists);

                o.has_value(
                    OperationValue::string(namespace.external_id_part()),
                    ChronicleOperations::NamespaceName,
                );

                o.has_value(
                    OperationValue::string(namespace.uuid_part()),
                    ChronicleOperations::NamespaceUuid,
                );

                o.has_value(
                    OperationValue::string(external_id),
                    ChronicleOperations::AgentName,
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
                let mut o = Value::new_operation(ChronicleOperations::AgentActsOnBehalfOf);

                o.has_value(
                    OperationValue::string(namespace.external_id_part()),
                    ChronicleOperations::NamespaceName,
                );

                o.has_value(
                    OperationValue::string(namespace.uuid_part()),
                    ChronicleOperations::NamespaceUuid,
                );

                o.has_value(
                    OperationValue::string(delegate_id.external_id_part()),
                    ChronicleOperations::DelegateId,
                );

                o.has_value(
                    OperationValue::string(responsible_id.external_id_part()),
                    ChronicleOperations::ResponsibleId,
                );

                if let Some(role) = role {
                    o.has_value(OperationValue::string(role), ChronicleOperations::Role);
                }

                if let Some(activity_id) = activity_id {
                    o.has_value(
                        OperationValue::string(activity_id.external_id_part()),
                        ChronicleOperations::ActivityName,
                    );
                }

                o
            }
            ChronicleOperation::ActivityExists(ActivityExists {
                namespace,
                external_id,
            }) => {
                let mut o = Value::new_operation(ChronicleOperations::ActivityExists);

                o.has_value(
                    OperationValue::string(namespace.external_id_part()),
                    ChronicleOperations::NamespaceName,
                );

                o.has_value(
                    OperationValue::string(namespace.uuid_part()),
                    ChronicleOperations::NamespaceUuid,
                );

                o.has_value(
                    OperationValue::string(external_id),
                    ChronicleOperations::ActivityName,
                );

                o
            }
            ChronicleOperation::StartActivity(StartActivity {
                namespace,
                id,
                time,
            }) => {
                let mut o = Value::new_operation(ChronicleOperations::StartActivity);

                o.has_value(
                    OperationValue::string(namespace.external_id_part()),
                    ChronicleOperations::NamespaceName,
                );

                o.has_value(
                    OperationValue::string(namespace.uuid_part()),
                    ChronicleOperations::NamespaceUuid,
                );

                o.has_value(
                    OperationValue::string(id.external_id_part()),
                    ChronicleOperations::ActivityName,
                );

                o.has_value(
                    OperationValue::string(time.to_rfc3339()),
                    ChronicleOperations::StartActivityTime,
                );

                o
            }
            ChronicleOperation::EndActivity(EndActivity {
                namespace,
                id,
                time,
            }) => {
                let mut o = Value::new_operation(ChronicleOperations::EndActivity);

                o.has_value(
                    OperationValue::string(namespace.external_id_part()),
                    ChronicleOperations::NamespaceName,
                );

                o.has_value(
                    OperationValue::string(namespace.uuid_part()),
                    ChronicleOperations::NamespaceUuid,
                );

                o.has_value(
                    OperationValue::string(id.external_id_part()),
                    ChronicleOperations::ActivityName,
                );

                o.has_value(
                    OperationValue::string(time.to_rfc3339()),
                    ChronicleOperations::EndActivityTime,
                );

                o
            }
            ChronicleOperation::ActivityUses(ActivityUses {
                namespace,
                id,
                activity,
            }) => {
                let mut o = Value::new_operation(ChronicleOperations::ActivityUses);

                o.has_value(
                    OperationValue::string(namespace.external_id_part()),
                    ChronicleOperations::NamespaceName,
                );

                o.has_value(
                    OperationValue::string(namespace.uuid_part()),
                    ChronicleOperations::NamespaceUuid,
                );

                o.has_value(
                    OperationValue::string(id.external_id_part()),
                    ChronicleOperations::EntityName,
                );

                o.has_value(
                    OperationValue::string(activity.external_id_part()),
                    ChronicleOperations::ActivityName,
                );

                o
            }
            ChronicleOperation::EntityExists(EntityExists {
                namespace,
                external_id,
            }) => {
                let mut o = Value::new_operation(ChronicleOperations::EntityExists);

                o.has_value(
                    OperationValue::string(namespace.external_id_part()),
                    ChronicleOperations::NamespaceName,
                );

                o.has_value(
                    OperationValue::string(namespace.uuid_part()),
                    ChronicleOperations::NamespaceUuid,
                );

                o.has_value(
                    OperationValue::string(external_id),
                    ChronicleOperations::EntityName,
                );

                o
            }
            ChronicleOperation::WasGeneratedBy(WasGeneratedBy {
                namespace,
                id,
                activity,
            }) => {
                let mut o = Value::new_operation(ChronicleOperations::WasGeneratedBy);

                o.has_value(
                    OperationValue::string(namespace.external_id_part()),
                    ChronicleOperations::NamespaceName,
                );

                o.has_value(
                    OperationValue::string(namespace.uuid_part()),
                    ChronicleOperations::NamespaceUuid,
                );

                o.has_value(
                    OperationValue::string(id.external_id_part()),
                    ChronicleOperations::EntityName,
                );

                o.has_value(
                    OperationValue::string(activity.external_id_part()),
                    ChronicleOperations::ActivityName,
                );

                o
            }
            ChronicleOperation::WasInformedBy(WasInformedBy {
                namespace,
                activity,
                informing_activity,
            }) => {
                let mut o = Value::new_operation(ChronicleOperations::WasInformedBy);

                o.has_value(
                    OperationValue::string(namespace.external_id_part()),
                    ChronicleOperations::NamespaceName,
                );

                o.has_value(
                    OperationValue::string(namespace.uuid_part()),
                    ChronicleOperations::NamespaceUuid,
                );

                o.has_value(
                    OperationValue::string(activity.external_id_part()),
                    ChronicleOperations::ActivityName,
                );

                o.has_value(
                    OperationValue::string(informing_activity.external_id_part()),
                    ChronicleOperations::InformingActivityName,
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
                let mut o = Value::new_operation(ChronicleOperations::EntityDerive);

                o.has_value(
                    OperationValue::string(namespace.external_id_part()),
                    ChronicleOperations::NamespaceName,
                );

                o.has_value(
                    OperationValue::string(namespace.uuid_part()),
                    ChronicleOperations::NamespaceUuid,
                );

                o.has_value(
                    OperationValue::string(id.external_id_part()),
                    ChronicleOperations::EntityName,
                );

                o.has_value(
                    OperationValue::string(used_id.external_id_part()),
                    ChronicleOperations::UsedEntityName,
                );

                if let Some(activity) = activity_id {
                    o.has_value(
                        OperationValue::string(activity.external_id_part()),
                        ChronicleOperations::ActivityName,
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
                let mut o = Value::new_operation(ChronicleOperations::SetAttributes);

                o.has_value(
                    OperationValue::string(namespace.external_id_part()),
                    ChronicleOperations::NamespaceName,
                );

                o.has_value(
                    OperationValue::string(namespace.uuid_part()),
                    ChronicleOperations::NamespaceUuid,
                );

                o.has_value(
                    OperationValue::string(id.external_id_part()),
                    ChronicleOperations::EntityName,
                );

                if let Some(domaintypeid) = &attributes.typ {
                    let id = OperationValue::string(domaintypeid.external_id_part());
                    o.has_value(id, ChronicleOperations::DomaintypeId);
                }

                o.attributes_object(attributes);

                o
            }
            ChronicleOperation::SetAttributes(SetAttributes::Activity {
                namespace,
                id,
                attributes,
            }) => {
                let mut o = Value::new_operation(ChronicleOperations::SetAttributes);

                o.has_value(
                    OperationValue::string(namespace.external_id_part()),
                    ChronicleOperations::NamespaceName,
                );

                o.has_value(
                    OperationValue::string(namespace.uuid_part()),
                    ChronicleOperations::NamespaceUuid,
                );

                o.has_value(
                    OperationValue::string(id.external_id_part()),
                    ChronicleOperations::ActivityName,
                );

                if let Some(domaintypeid) = &attributes.typ {
                    let id = OperationValue::string(domaintypeid.external_id_part());
                    o.has_value(id, ChronicleOperations::DomaintypeId);
                }

                o.attributes_object(attributes);

                o
            }
            ChronicleOperation::SetAttributes(SetAttributes::Agent {
                namespace,
                id,
                attributes,
            }) => {
                let mut o = Value::new_operation(ChronicleOperations::SetAttributes);

                o.has_value(
                    OperationValue::string(namespace.external_id_part()),
                    ChronicleOperations::NamespaceName,
                );

                o.has_value(
                    OperationValue::string(namespace.uuid_part()),
                    ChronicleOperations::NamespaceUuid,
                );

                o.has_value(
                    OperationValue::string(id.external_id_part()),
                    ChronicleOperations::AgentName,
                );

                if let Some(domaintypeid) = &attributes.typ {
                    let id = OperationValue::string(domaintypeid.external_id_part());
                    o.has_value(id, ChronicleOperations::DomaintypeId);
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
                let mut o = Value::new_operation(ChronicleOperations::WasAssociatedWith);

                o.has_value(
                    OperationValue::string(namespace.external_id_part()),
                    ChronicleOperations::NamespaceName,
                );

                o.has_value(
                    OperationValue::string(namespace.uuid_part()),
                    ChronicleOperations::NamespaceUuid,
                );

                o.has_value(
                    OperationValue::string(activity_id.external_id_part()),
                    ChronicleOperations::ActivityName,
                );

                o.has_value(
                    OperationValue::string(agent_id.external_id_part()),
                    ChronicleOperations::AgentName,
                );

                if let Some(role) = role {
                    o.has_value(OperationValue::string(role), ChronicleOperations::Role);
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
                let mut o = Value::new_operation(ChronicleOperations::WasAttributedTo);

                o.has_value(
                    OperationValue::string(namespace.external_id_part()),
                    ChronicleOperations::NamespaceName,
                );

                o.has_value(
                    OperationValue::string(namespace.uuid_part()),
                    ChronicleOperations::NamespaceUuid,
                );

                o.has_value(
                    OperationValue::string(entity_id.external_id_part()),
                    ChronicleOperations::EntityName,
                );

                o.has_value(
                    OperationValue::string(agent_id.external_id_part()),
                    ChronicleOperations::AgentName,
                );

                if let Some(role) = role {
                    o.has_value(OperationValue::string(role), ChronicleOperations::Role);
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
    fn new_operation(op: ChronicleOperations) -> Self;
    fn new_type(id: OperationValue, op: ChronicleOperations) -> Self;
    fn new_value(id: OperationValue) -> Self;
    fn new_id(id: OperationValue) -> Self;
    fn has_value(&mut self, value: OperationValue, op: ChronicleOperations);
    fn has_id(&mut self, id: OperationValue, op: ChronicleOperations);
    fn attributes_object(&mut self, attributes: &Attributes);
    fn derivation(&mut self, typ: &DerivationType);
}

impl Operate for Value {
    fn new_type(id: OperationValue, op: ChronicleOperations) -> Self {
        json!({
            "@id": id.0,
            "@type": [iref::Iri::from(op).as_str()],
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

    fn has_value(&mut self, value: OperationValue, op: ChronicleOperations) {
        if let Value::Object(map) = self {
            let key = iref::Iri::from(op).to_string();
            let mut values: Vec<Value> = Vec::new();
            let object = Self::new_value(value);
            values.push(object);
            map.insert(key, Value::Array(values));
        } else {
            panic!("use on JSON objects only");
        }
    }

    fn has_id(&mut self, id: OperationValue, op: ChronicleOperations) {
        if let Value::Object(map) = self {
            let key = iref::Iri::from(op).to_string();
            let mut value: Vec<Value> = Vec::new();
            let object = Self::new_id(id);
            value.push(object);
            map.insert(key, Value::Array(value));
        } else {
            panic!("use on JSON objects only");
        }
    }

    fn new_operation(op: ChronicleOperations) -> Self {
        let id = OperationValue::string("_:n1");
        Self::new_type(id, op)
    }

    fn attributes_object(&mut self, attributes: &Attributes) {
        if let Value::Object(map) = self {
            let mut attribute_node = serde_json::Map::new();
            for attribute in attributes.attributes.values() {
                attribute_node.insert(attribute.typ.clone(), attribute.value.clone());
            }
            map.insert(
                iref::Iri::from(ChronicleOperations::Attributes).to_string(),
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

        self.has_value(id, ChronicleOperations::DerivationType);
    }
}
