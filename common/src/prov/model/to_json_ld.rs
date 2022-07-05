use std::collections::BTreeMap;

use iref::{AsIri, Iri};
use json::{object, JsonValue};
use serde_json::Value;

use crate::{
    attributes::{Attribute, Attributes},
    prov::{
        operations::{ChronicleOperation, CreateNamespace, DerivationType},
        vocab::{Chronicle, ChronicleOperations, Prov},
        NamePart, UuidPart,
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
        let mut doc = json::Array::new();

        for (id, ns) in self.namespaces.iter() {
            doc.push(object! {
                "@id": (*id.to_string()),
                "@type": Iri::from(Chronicle::Namespace).as_str(),
                "http://www.w3.org/2000/01/rdf-schema#label": [{
                    "@value": ns.name.as_str(),
                }]
            })
        }

        for ((ns, id), identity) in self.identities.iter() {
            doc.push(object! {
                "@id": (*id.to_string()),
                "@type": Iri::from(Chronicle::Identity).as_str(),
                "http://blockchaintp.com/chronicle/ns#publicKey": [{
                    "@value": identity.public_key.to_string(),
                }],
                "http://blockchaintp.com/chronicle/ns#hasNamespace": [{
                    "@id": ns.to_string()
                }],
            })
        }

        for ((ns, id), attachment) in self.attachments.iter() {
            let mut attachmentdoc = object! {
                "@id": (*id.to_string()),
                "@type": Iri::from(Chronicle::HasAttachment).as_str(),
                "http://blockchaintp.com/chronicle/ns#entitySignature": attachment.signature.to_string(),
                "http://blockchaintp.com/chronicle/ns#signedAtTime": attachment.signature_time.to_rfc3339(),
                "http://blockchaintp.com/chronicle/ns#signedBy": {
                    "@id": attachment.signer.to_string()
                },
                "http://blockchaintp.com/chronicle/ns#hasNamespace": [{
                    "@id": ns.to_string()
                }],
            };

            if let Some(locator) = attachment.locator.as_ref() {
                let mut values = json::Array::new();

                values.push(object! {
                    "@value": JsonValue::String(locator.to_owned()),
                });

                attachmentdoc
                    .insert(Iri::from(Chronicle::Locator).as_str(), values)
                    .ok();
            }

            doc.push(attachmentdoc);
        }

        for ((_, id), agent) in self.agents.iter() {
            let mut typ = vec![Iri::from(Prov::Agent).to_string()];
            if let Some(x) = agent.domaintypeid.as_ref() {
                typ.push(x.to_string())
            }

            let mut agentdoc = object! {
                "@id": (*id.to_string()),
                "@type": typ,
                "http://www.w3.org/2000/01/rdf-schema#label": [{
                   "@value": agent.name.as_str(),
                }]
            };

            let agent_key = (agent.namespaceid.clone(), agent.id.clone());

            if let Some((_, identity)) = self.has_identity.get(&agent_key) {
                agentdoc
                    .insert(
                        Iri::from(Chronicle::HasIdentity).as_str(),
                        object! {"@id": identity.to_string()},
                    )
                    .ok();
            }

            if let Some(identities) = self.had_identity.get(&agent_key) {
                let mut values = json::Array::new();

                for (_, id) in identities {
                    values.push(object! { "@id": id.to_string()});
                }
                agentdoc
                    .insert(Iri::from(Chronicle::HadIdentity).as_str(), values)
                    .ok();
            }

            if let Some(delegation) = self
                .delegation
                .get(&(agent.namespaceid.to_owned(), id.to_owned()))
            {
                let mut ids = json::Array::new();

                for delegation in delegation.iter() {
                    ids.push(object! {"@id": delegation.delegate_id.to_string()});
                }

                agentdoc
                    .insert(Iri::from(Prov::ActedOnBehalfOf).as_str(), ids)
                    .ok();
            }

            let mut values = json::Array::new();

            values.push(object! {
                "@id": JsonValue::String(agent.namespaceid.to_string()),
            });

            agentdoc
                .insert(Iri::from(Chronicle::HasNamespace).as_str(), values)
                .ok();

            Self::write_attributes(&mut agentdoc, agent.attributes.values());

            doc.push(agentdoc);
        }

        for ((namespace, id), activity) in self.activities.iter() {
            let mut typ = vec![Iri::from(Prov::Activity).to_string()];
            if let Some(x) = activity.domaintypeid.as_ref() {
                typ.push(x.to_string())
            }

            let mut activitydoc = object! {
                "@id": (*id.to_string()),
                "@type": typ,
                "http://www.w3.org/2000/01/rdf-schema#label": [{
                   "@value": activity.name.as_str(),
                }]
            };

            if let Some(time) = activity.started {
                let mut values = json::Array::new();
                values.push(object! {"@value": time.to_rfc3339()});

                activitydoc
                    .insert("http://www.w3.org/ns/prov#startedAtTime", values)
                    .ok();
            }

            if let Some(time) = activity.ended {
                let mut values = json::Array::new();
                values.push(object! {"@value": time.to_rfc3339()});

                activitydoc
                    .insert("http://www.w3.org/ns/prov#endedAtTime", values)
                    .ok();
            }

            if let Some(asoc) = self.association.get(&(namespace.to_owned(), id.to_owned())) {
                let mut ids = json::Array::new();

                for asoc in asoc.iter() {
                    ids.push(object! {"@id": asoc.agent_id.to_string()});
                }

                activitydoc
                    .insert(&Iri::from(Prov::WasAssociatedWith).to_string(), ids)
                    .ok();
            }

            if let Some(useage) = self.useage.get(&(namespace.to_owned(), id.to_owned())) {
                let mut ids = json::Array::new();

                for useage in useage.iter() {
                    ids.push(object! {"@id": useage.entity_id.to_string()});
                }

                activitydoc
                    .insert(&Iri::from(Prov::Used).to_string(), ids)
                    .ok();
            }

            let mut values = json::Array::new();

            values.push(object! {
                "@id": JsonValue::String(activity.namespaceid.to_string()),
            });

            activitydoc
                .insert(Iri::from(Chronicle::HasNamespace).as_str(), values)
                .ok();

            Self::write_attributes(&mut activitydoc, activity.attributes.values());

            doc.push(activitydoc);
        }

        for ((namespace, id), entity) in self.entities.iter() {
            let mut typ = vec![Iri::from(Prov::Entity).to_string()];
            if let Some(x) = entity.domaintypeid.as_ref() {
                typ.push(x.to_string())
            }

            let mut entitydoc = object! {
                "@id": (*id.to_string()),
                "@type": typ,
                "http://www.w3.org/2000/01/rdf-schema#label": [{
                   "@value": entity.name.as_str()
                }]
            };

            if let Some(derivation) = self.derivation.get(&(namespace.to_owned(), id.to_owned())) {
                let mut derived_ids = json::Array::new();
                let mut primary_ids = json::Array::new();
                let mut quotation_ids = json::Array::new();
                let mut revision_ids = json::Array::new();

                for derivation in derivation.iter() {
                    let id = object! {"@id": derivation.used_id.to_string()};
                    match derivation.typ {
                        Some(DerivationType::PrimarySource) => primary_ids.push(id),
                        Some(DerivationType::Quotation) => quotation_ids.push(id),
                        Some(DerivationType::Revision) => revision_ids.push(id),
                        _ => derived_ids.push(id),
                    }
                }
                if !derived_ids.is_empty() {
                    entitydoc
                        .insert(Iri::from(Prov::WasDerivedFrom).as_str(), derived_ids)
                        .ok();
                }
                if !primary_ids.is_empty() {
                    entitydoc
                        .insert(Iri::from(Prov::HadPrimarySource).as_str(), primary_ids)
                        .ok();
                }
                if !quotation_ids.is_empty() {
                    entitydoc
                        .insert(Iri::from(Prov::WasQuotedFrom).as_str(), quotation_ids)
                        .ok();
                }
                if !revision_ids.is_empty() {
                    entitydoc
                        .insert(Iri::from(Prov::WasRevisionOf).as_str(), revision_ids)
                        .ok();
                }
            }

            if let Some(generation) = self.generation.get(&(namespace.to_owned(), id.to_owned())) {
                let mut ids = json::Array::new();

                for generation in generation.iter() {
                    ids.push(object! {"@id": generation.activity_id.to_string()});
                }

                entitydoc
                    .insert(Iri::from(Prov::WasGeneratedBy).as_str(), ids)
                    .ok();
            }

            let entity_key = (entity.namespaceid.clone(), entity.id.clone());

            if let Some((_, identity)) = self.has_attachment.get(&entity_key) {
                entitydoc
                    .insert(
                        Iri::from(Chronicle::HasAttachment).as_str(),
                        object! {"@id": identity.to_string()},
                    )
                    .ok();
            }

            if let Some(identities) = self.had_attachment.get(&entity_key) {
                let mut values = json::Array::new();

                for (_, id) in identities {
                    values.push(object! { "@id": id.to_string()});
                }
                entitydoc
                    .insert(Iri::from(Chronicle::HadAttachment).as_str(), values)
                    .ok();
            }

            let mut values = json::Array::new();

            values.push(object! {
                "@id": JsonValue::String(entity.namespaceid.to_string()),
            });

            entitydoc
                .insert(Iri::from(Chronicle::HasNamespace).as_str(), values)
                .ok();

            Self::write_attributes(&mut entitydoc, entity.attributes.values());

            doc.push(entitydoc);
        }

        ExpandedJson(doc.into())
    }
}

impl ProvModel {
    fn we_need_to_update_the_ld_library_to_a_version_that_supports_serde(
        json: &serde_json::Value,
    ) -> JsonValue {
        json::parse(&json.to_string()).unwrap()
    }

    fn write_attributes<'a, I: Iterator<Item = &'a Attribute>>(doc: &mut JsonValue, attributes: I) {
        let mut attribute_node = object! {};

        for attribute in attributes {
            attribute_node
                .insert(
                    &*attribute.typ,
                    Self::we_need_to_update_the_ld_library_to_a_version_that_supports_serde(
                        &attribute.value,
                    ),
                )
                .ok();
        }

        doc.insert(
            &Chronicle::Value.as_iri().to_string(),
            object! {"@value" : attribute_node, "@type": "@json"},
        )
        .ok();
    }
}

impl ToJson for ChronicleOperation {
    fn to_json(&self) -> ExpandedJson {
        let mut operation: Vec<JsonValue> = json::Array::new();

        let o = match self {
            ChronicleOperation::CreateNamespace(CreateNamespace { id, .. }) => {
                let mut o = JsonValue::new_operation(ChronicleOperations::CreateNamespace);

                o.operate(
                    OperationsId::from_id(id.name_part()),
                    ChronicleOperations::NamespaceName,
                );

                o.operate(
                    OperationsId::from_id(id.uuid_part()),
                    ChronicleOperations::NamespaceUuid,
                );

                o
            }
            ChronicleOperation::CreateAgent(CreateAgent { namespace, name }) => {
                let mut o = JsonValue::new_operation(ChronicleOperations::CreateAgent);

                o.operate(
                    OperationsId::from_id(namespace.name_part()),
                    ChronicleOperations::NamespaceName,
                );

                o.operate(
                    OperationsId::from_id(namespace.uuid_part()),
                    ChronicleOperations::NamespaceUuid,
                );

                o.operate(OperationsId::from_id(name), ChronicleOperations::AgentName);

                o
            }
            ChronicleOperation::AgentActsOnBehalfOf(ActsOnBehalfOf {
                namespace,
                id,
                delegate_id,
                activity_id,
            }) => {
                let mut o = JsonValue::new_operation(ChronicleOperations::AgentActsOnBehalfOf);

                o.operate(
                    OperationsId::from_id(namespace.name_part()),
                    ChronicleOperations::NamespaceName,
                );

                o.operate(
                    OperationsId::from_id(namespace.uuid_part()),
                    ChronicleOperations::NamespaceUuid,
                );

                o.operate(
                    OperationsId::from_id(id.name_part()),
                    ChronicleOperations::AgentName,
                );

                o.operate(
                    OperationsId::from_id(delegate_id.name_part()),
                    ChronicleOperations::DelegateId,
                );

                if let Some(activity_id) = activity_id {
                    o.operate(
                        OperationsId::from_id(activity_id.name_part()),
                        ChronicleOperations::ActivityName,
                    );
                }

                o
            }
            ChronicleOperation::RegisterKey(RegisterKey {
                namespace,
                id,
                publickey,
            }) => {
                let mut o = JsonValue::new_operation(ChronicleOperations::RegisterKey);

                o.operate(
                    OperationsId::from_id(namespace.name_part()),
                    ChronicleOperations::NamespaceName,
                );

                o.operate(
                    OperationsId::from_id(namespace.uuid_part()),
                    ChronicleOperations::NamespaceUuid,
                );

                o.operate(
                    OperationsId::from_id(id.name_part()),
                    ChronicleOperations::AgentName,
                );

                o.operate(
                    OperationsId::from_id(publickey.to_owned()),
                    ChronicleOperations::PublicKey,
                );

                o
            }
            ChronicleOperation::CreateActivity(CreateActivity { namespace, name }) => {
                let mut o = JsonValue::new_operation(ChronicleOperations::CreateActivity);

                o.operate(
                    OperationsId::from_id(namespace.name_part()),
                    ChronicleOperations::NamespaceName,
                );

                o.operate(
                    OperationsId::from_id(namespace.uuid_part()),
                    ChronicleOperations::NamespaceUuid,
                );

                o.operate(
                    OperationsId::from_id(name),
                    ChronicleOperations::ActivityName,
                );

                o
            }
            ChronicleOperation::StartActivity(StartActivity {
                namespace,
                id,
                agent,
                time,
            }) => {
                let mut o = JsonValue::new_operation(ChronicleOperations::StartActivity);

                o.operate(
                    OperationsId::from_id(namespace.name_part()),
                    ChronicleOperations::NamespaceName,
                );

                o.operate(
                    OperationsId::from_id(namespace.uuid_part()),
                    ChronicleOperations::NamespaceUuid,
                );

                o.operate(
                    OperationsId::from_id(agent.name_part()),
                    ChronicleOperations::AgentName,
                );

                o.operate(
                    OperationsId::from_id(id.name_part()),
                    ChronicleOperations::ActivityName,
                );

                o.operate(
                    OperationsId::from_id(time.to_rfc3339()),
                    ChronicleOperations::StartActivityTime,
                );

                o
            }
            ChronicleOperation::EndActivity(EndActivity {
                namespace,
                id,
                agent,
                time,
            }) => {
                let mut o = JsonValue::new_operation(ChronicleOperations::EndActivity);

                o.operate(
                    OperationsId::from_id(namespace.name_part()),
                    ChronicleOperations::NamespaceName,
                );

                o.operate(
                    OperationsId::from_id(namespace.uuid_part()),
                    ChronicleOperations::NamespaceUuid,
                );

                o.operate(
                    OperationsId::from_id(id.name_part()),
                    ChronicleOperations::ActivityName,
                );

                o.operate(
                    OperationsId::from_id(agent.name_part()),
                    ChronicleOperations::AgentName,
                );

                o.operate(
                    OperationsId::from_id(time.to_rfc3339()),
                    ChronicleOperations::EndActivityTime,
                );

                o
            }
            ChronicleOperation::ActivityUses(ActivityUses {
                namespace,
                id,
                activity,
            }) => {
                let mut o = JsonValue::new_operation(ChronicleOperations::ActivityUses);

                o.operate(
                    OperationsId::from_id(namespace.name_part()),
                    ChronicleOperations::NamespaceName,
                );

                o.operate(
                    OperationsId::from_id(namespace.uuid_part()),
                    ChronicleOperations::NamespaceUuid,
                );

                o.operate(
                    OperationsId::from_id(id.name_part()),
                    ChronicleOperations::EntityName,
                );

                o.operate(
                    OperationsId::from_id(activity.name_part()),
                    ChronicleOperations::ActivityName,
                );

                o
            }
            ChronicleOperation::CreateEntity(CreateEntity { namespace, name }) => {
                let mut o = JsonValue::new_operation(ChronicleOperations::CreateEntity);

                o.operate(
                    OperationsId::from_id(namespace.name_part()),
                    ChronicleOperations::NamespaceName,
                );

                o.operate(
                    OperationsId::from_id(namespace.uuid_part()),
                    ChronicleOperations::NamespaceUuid,
                );

                o.operate(OperationsId::from_id(name), ChronicleOperations::EntityName);

                o
            }
            ChronicleOperation::GenerateEntity(GenerateEntity {
                namespace,
                id,
                activity,
            }) => {
                let mut o = JsonValue::new_operation(ChronicleOperations::GenerateEntity);

                o.operate(
                    OperationsId::from_id(namespace.name_part()),
                    ChronicleOperations::NamespaceName,
                );

                o.operate(
                    OperationsId::from_id(namespace.uuid_part()),
                    ChronicleOperations::NamespaceUuid,
                );

                o.operate(
                    OperationsId::from_id(id.name_part()),
                    ChronicleOperations::EntityName,
                );

                o.operate(
                    OperationsId::from_id(activity.name_part()),
                    ChronicleOperations::ActivityName,
                );

                o
            }
            ChronicleOperation::EntityAttach(EntityAttach {
                namespace,
                identityid: _,
                id,
                locator: _,
                agent,
                signature: _,
                signature_time: _,
            }) => {
                let mut o = JsonValue::new_operation(ChronicleOperations::EntityAttach);

                o.operate(
                    OperationsId::from_id(namespace.name_part()),
                    ChronicleOperations::NamespaceName,
                );

                o.operate(
                    OperationsId::from_id(namespace.uuid_part()),
                    ChronicleOperations::NamespaceUuid,
                );

                o.operate(
                    OperationsId::from_id(id.name_part()),
                    ChronicleOperations::EntityName,
                );

                o.operate(
                    OperationsId::from_id(agent.name_part()),
                    ChronicleOperations::AgentName,
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
                let mut o = JsonValue::new_operation(ChronicleOperations::EntityDerive);

                o.operate(
                    OperationsId::from_id(namespace.name_part()),
                    ChronicleOperations::NamespaceName,
                );

                o.operate(
                    OperationsId::from_id(namespace.uuid_part()),
                    ChronicleOperations::NamespaceUuid,
                );

                o.operate(
                    OperationsId::from_id(id.name_part()),
                    ChronicleOperations::EntityName,
                );

                o.operate(
                    OperationsId::from_id(used_id.name_part()),
                    ChronicleOperations::UsedEntityName,
                );

                if let Some(activity) = activity_id {
                    o.operate(
                        OperationsId::from_id(activity.name_part()),
                        ChronicleOperations::ActivityName,
                    );
                }

                if let Some(typ) = typ {
                    o.derivation(typ);
                }

                o
            }
            ChronicleOperation::SetAttributes(SetAttributes::Entity {
                namespace,
                id,
                attributes,
            }) => {
                let mut o = JsonValue::new_operation(ChronicleOperations::SetAttributes);

                o.operate(
                    OperationsId::from_id(namespace.name_part()),
                    ChronicleOperations::NamespaceName,
                );

                o.operate(
                    OperationsId::from_id(namespace.uuid_part()),
                    ChronicleOperations::NamespaceUuid,
                );

                o.operate(
                    OperationsId::from_id(id.name_part()),
                    ChronicleOperations::EntityName,
                );

                o.attributes_object(OperationsId::from_id(id.name_part()), attributes);

                o
            }
            ChronicleOperation::SetAttributes(SetAttributes::Activity {
                namespace,
                id,
                attributes,
            }) => {
                let mut o = JsonValue::new_operation(ChronicleOperations::SetAttributes);

                o.operate(
                    OperationsId::from_id(namespace.name_part()),
                    ChronicleOperations::NamespaceName,
                );

                o.operate(
                    OperationsId::from_id(namespace.uuid_part()),
                    ChronicleOperations::NamespaceUuid,
                );

                o.operate(
                    OperationsId::from_id(id.name_part()),
                    ChronicleOperations::ActivityName,
                );

                o.attributes_object(OperationsId::from_id(id.name_part()), attributes);

                o
            }
            ChronicleOperation::SetAttributes(SetAttributes::Agent {
                namespace,
                id,
                attributes,
            }) => {
                let mut o = JsonValue::new_operation(ChronicleOperations::SetAttributes);

                o.operate(
                    OperationsId::from_id(namespace.name_part()),
                    ChronicleOperations::NamespaceName,
                );

                o.operate(
                    OperationsId::from_id(namespace.uuid_part()),
                    ChronicleOperations::NamespaceUuid,
                );

                o.operate(
                    OperationsId::from_id(id.name_part()),
                    ChronicleOperations::AgentName,
                );

                o.attributes_object(OperationsId::from_id(id.name_part()), attributes);

                o
            }
        };
        operation.push(o);
        super::ExpandedJson(operation.into())
    }
}

struct OperationsId(String);

impl OperationsId {
    fn from_id(id: impl std::fmt::Display) -> Self {
        OperationsId(id.to_string())
    }
}

trait Operate {
    fn new_operation(op: ChronicleOperations) -> Self;
    fn new_type(id: OperationsId, op: ChronicleOperations) -> Self;
    fn new_value(id: OperationsId) -> Self;
    fn operate(&mut self, id: OperationsId, op: ChronicleOperations);
    fn attributes_object(&mut self, id: OperationsId, attributes: &Attributes);
    fn new_attribute(typ: String, val: Value) -> Self;
    fn derivation(&mut self, typ: &DerivationType);
}

impl Operate for JsonValue {
    fn new_type(id: OperationsId, op: ChronicleOperations) -> Self {
        object! {
            "@id": id.0,
            "@type": iref::Iri::from(op).as_str(),
        }
    }

    fn new_value(id: OperationsId) -> Self {
        object! {
            "@value": id.0
        }
    }

    fn operate(&mut self, id: OperationsId, op: ChronicleOperations) {
        let key = iref::Iri::from(op).to_string();
        let mut value: Vec<JsonValue> = json::Array::new();
        let object = Self::new_value(id);
        value.push(object);
        self.insert(&key, value).ok();
    }

    fn new_operation(op: ChronicleOperations) -> Self {
        let id = OperationsId::from_id("_:n1");
        Self::new_type(id, op)
    }

    fn attributes_object(&mut self, id: OperationsId, attributes: &Attributes) {
        let key = iref::Iri::from(ChronicleOperations::Attributes).to_string();

        let mut attributes_object = Self::new_type(id, ChronicleOperations::Attributes);

        if let Some(domaintypeid) = &attributes.typ {
            let id = OperationsId::from_id(domaintypeid.name_part());
            attributes_object.operate(id, ChronicleOperations::DomaintypeId);
        }

        let value: Vec<JsonValue> = vec![attributes_object];

        self.insert(&key, value).ok();

        if !attributes.attributes.is_empty() {
            let mut attribute_objects: Vec<JsonValue> = json::Array::new();
            let mut ordered_map: BTreeMap<String, Attribute> = BTreeMap::new();
            for (k, v) in &attributes.attributes {
                ordered_map.insert(k.clone(), v.clone());
            }
            #[allow(clippy::for_kv_map)]
            for (_, attr) in ordered_map {
                let object = Self::new_attribute(attr.typ.clone(), attr.value.clone());
                attribute_objects.push(object);
            }
            self.insert(&key, attribute_objects).ok();
        }
    }

    fn new_attribute(typ: String, val: Value) -> Self {
        object! {
            "@type": typ,
            "@primitive_type": val.to_string(),
        }
    }

    fn derivation(&mut self, typ: &DerivationType) {
        let typ = match typ {
            DerivationType::Revision => "Revision",
            DerivationType::Quotation => "Quotation",
            DerivationType::PrimarySource => "PrimarySource",
        };
        let id = OperationsId::from_id(typ);

        self.operate(id, ChronicleOperations::DerivationType);
    }
}

#[cfg(test)]
mod test {
    use std::collections::BTreeMap;

    use serde_json::json;
    use uuid::Uuid;

    use crate::{
        attributes::{Attribute, Attributes},
        prov::{
            operations::{
                ActivityUses, ActsOnBehalfOf, CreateActivity, CreateEntity, CreateNamespace,
                EntityAttach, EntityDerive, GenerateEntity, RegisterKey, SetAttributes,
                StartActivity,
            },
            to_json_ld::ToJson,
            ActivityId, AgentId, DomaintypeId, EntityId, NamePart, NamespaceId,
        },
    };

    use super::{ChronicleOperation, DerivationType};

    fn uuid() -> Uuid {
        let bytes = [
            0xa1, 0xa2, 0xa3, 0xa4, 0xb1, 0xb2, 0xc1, 0xc2, 0xd1, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6,
            0xd7, 0xd8,
        ];
        Uuid::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn test_create_namespace() {
        let name = "testns";
        let id = NamespaceId::from_name(name, uuid());

        let op = ChronicleOperation::CreateNamespace(CreateNamespace::new(id, name, uuid()));
        let x = op.to_json();
        let x: serde_json::Value = serde_json::from_str(&x.0.to_string()).unwrap();
        insta::assert_json_snapshot!(&x, @r###"
        [
          {
            "@id": "_:n1",
            "@type": "http://blockchaintp.com/chronicleoperations/ns#CreateNamespace",
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceName": [
              {
                "@value": "testns"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceUuid": [
              {
                "@value": "a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8"
              }
            ]
          }
        ]
        "###);
    }

    #[tokio::test]
    async fn test_create_agent() {
        let uuid = uuid();
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid);
        let name: crate::prov::Name =
            crate::prov::NamePart::name_part(&crate::prov::AgentId::from_name("test_agent"))
                .clone();
        let op: ChronicleOperation =
            super::ChronicleOperation::CreateAgent(crate::prov::operations::CreateAgent {
                namespace,
                name,
            });
        let x = op.to_json();
        let x: serde_json::Value = serde_json::from_str(&x.0.to_string()).unwrap();
        insta::assert_json_snapshot!(&x, @r###"
        [
          {
            "@id": "_:n1",
            "@type": "http://blockchaintp.com/chronicleoperations/ns#CreateAgent",
            "http://blockchaintp.com/chronicleoperations/ns#AgentName": [
              {
                "@value": "test_agent"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceName": [
              {
                "@value": "testns"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceUuid": [
              {
                "@value": "a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8"
              }
            ]
          }
        ]
        "###);
    }

    #[tokio::test]
    async fn test_agent_acts_on_behalf_of() {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid());
        let id = AgentId::from_name("test_agent");
        let delegate_id = AgentId::from_name("test_delegate");
        let activity_id = Some(ActivityId::from_name("test_activity"));

        let op: ChronicleOperation = ChronicleOperation::AgentActsOnBehalfOf(ActsOnBehalfOf {
            namespace,
            id,
            delegate_id,
            activity_id,
        });
        let x = op.to_json();
        let x: serde_json::Value = serde_json::from_str(&x.0.to_string()).unwrap();
        insta::assert_json_snapshot!(&x, @r###"
        [
          {
            "@id": "_:n1",
            "@type": "http://blockchaintp.com/chronicleoperations/ns#AgentActsOnBehalfOf",
            "http://blockchaintp.com/chronicleoperations/ns#ActivityName": [
              {
                "@value": "test_activity"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#AgentName": [
              {
                "@value": "test_agent"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#DelegateId": [
              {
                "@value": "test_delegate"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceName": [
              {
                "@value": "testns"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceUuid": [
              {
                "@value": "a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8"
              }
            ]
          }
        ]
        "###);
    }

    #[tokio::test]
    async fn test_register_key() {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid());
        let id = crate::prov::AgentId::from_name("test_agent");
        let publickey =
            "02197db854d8c6a488d4a0ef3ef1fcb0c06d66478fae9e87a237172cf6f6f7de23".to_string();

        let op: ChronicleOperation = ChronicleOperation::RegisterKey(RegisterKey {
            namespace,
            id,
            publickey,
        });

        let x = op.to_json();
        let x: serde_json::Value = serde_json::from_str(&x.0.to_string()).unwrap();
        insta::assert_json_snapshot!(&x, @r###"
        [
          {
            "@id": "_:n1",
            "@type": "http://blockchaintp.com/chronicleoperations/ns#RegisterKey",
            "http://blockchaintp.com/chronicleoperations/ns#AgentName": [
              {
                "@value": "test_agent"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceName": [
              {
                "@value": "testns"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceUuid": [
              {
                "@value": "a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#PublicKey": [
              {
                "@value": "02197db854d8c6a488d4a0ef3ef1fcb0c06d66478fae9e87a237172cf6f6f7de23"
              }
            ]
          }
        ]
        "###);
    }

    #[tokio::test]
    async fn test_create_activity() {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid());
        let name = NamePart::name_part(&ActivityId::from_name("test_activity")).to_owned();

        let op: ChronicleOperation =
            ChronicleOperation::CreateActivity(CreateActivity { namespace, name });

        let x = op.to_json();
        let x: serde_json::Value = serde_json::from_str(&x.0.to_string()).unwrap();
        insta::assert_json_snapshot!(&x, @r###"
        [
          {
            "@id": "_:n1",
            "@type": "http://blockchaintp.com/chronicleoperations/ns#CreateActivity",
            "http://blockchaintp.com/chronicleoperations/ns#ActivityName": [
              {
                "@value": "test_activity"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceName": [
              {
                "@value": "testns"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceUuid": [
              {
                "@value": "a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8"
              }
            ]
          }
        ]
        "###);
    }

    #[tokio::test]
    async fn start_activity() {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid());
        let id = ActivityId::from_name("test_activity");
        let agent = AgentId::from_name("test_agent");
        let time = chrono::DateTime::<chrono::Utc>::from_utc(
            chrono::NaiveDateTime::from_timestamp(61, 0),
            chrono::Utc,
        );
        let op: ChronicleOperation = ChronicleOperation::StartActivity(StartActivity {
            namespace,
            id,
            agent,
            time,
        });

        let x = op.to_json();
        let x: serde_json::Value = serde_json::from_str(&x.0.to_string()).unwrap();
        insta::assert_json_snapshot!(&x, @r###"
        [
          {
            "@id": "_:n1",
            "@type": "http://blockchaintp.com/chronicleoperations/ns#StartActivity",
            "http://blockchaintp.com/chronicleoperations/ns#ActivityName": [
              {
                "@value": "test_activity"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#AgentName": [
              {
                "@value": "test_agent"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceName": [
              {
                "@value": "testns"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceUuid": [
              {
                "@value": "a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#StartActivityTime": [
              {
                "@value": "1970-01-01T00:01:01+00:00"
              }
            ]
          }
        ]
        "###);
    }

    #[tokio::test]
    async fn test_end_activity() {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid());
        let id = ActivityId::from_name("test_activity");
        let agent = crate::prov::AgentId::from_name("test_agent");
        let time = chrono::DateTime::<chrono::Utc>::from_utc(
            chrono::NaiveDateTime::from_timestamp(61, 0),
            chrono::Utc,
        );
        let op: ChronicleOperation =
            super::ChronicleOperation::EndActivity(crate::prov::operations::EndActivity {
                namespace,
                id,
                agent,
                time,
            });

        let x = op.to_json();
        let x: serde_json::Value = serde_json::from_str(&x.0.to_string()).unwrap();
        insta::assert_json_snapshot!(&x, @r###"
        [
          {
            "@id": "_:n1",
            "@type": "http://blockchaintp.com/chronicleoperations/ns#EndActivity",
            "http://blockchaintp.com/chronicleoperations/ns#ActivityName": [
              {
                "@value": "test_activity"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#AgentName": [
              {
                "@value": "test_agent"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#EndActivityTime": [
              {
                "@value": "1970-01-01T00:01:01+00:00"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceName": [
              {
                "@value": "testns"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceUuid": [
              {
                "@value": "a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8"
              }
            ]
          }
        ]
        "###);
    }

    #[tokio::test]
    async fn test_activity_uses() {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid());
        let id = EntityId::from_name("test_entity");
        let activity = ActivityId::from_name("test_activity");
        let op: ChronicleOperation = ChronicleOperation::ActivityUses(ActivityUses {
            namespace,
            id,
            activity,
        });

        let x = op.to_json();
        let x: serde_json::Value = serde_json::from_str(&x.0.to_string()).unwrap();
        insta::assert_json_snapshot!(&x, @r###"
        [
          {
            "@id": "_:n1",
            "@type": "http://blockchaintp.com/chronicleoperations/ns#ActivityUses",
            "http://blockchaintp.com/chronicleoperations/ns#ActivityName": [
              {
                "@value": "test_activity"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#EntityName": [
              {
                "@value": "test_entity"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceName": [
              {
                "@value": "testns"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceUuid": [
              {
                "@value": "a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8"
              }
            ]
          }
        ]
        "###);
    }

    #[tokio::test]
    async fn test_create_entity() {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid());
        let id = NamePart::name_part(&EntityId::from_name("test_entity")).to_owned();
        let operation: ChronicleOperation = ChronicleOperation::CreateEntity(CreateEntity {
            namespace,
            name: id,
        });

        let x = operation.to_json();
        let x: serde_json::Value = serde_json::from_str(&x.0.to_string()).unwrap();
        insta::assert_json_snapshot!(&x, @r###"
        [
          {
            "@id": "_:n1",
            "@type": "http://blockchaintp.com/chronicleoperations/ns#CreateEntity",
            "http://blockchaintp.com/chronicleoperations/ns#EntityName": [
              {
                "@value": "test_entity"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceName": [
              {
                "@value": "testns"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceUuid": [
              {
                "@value": "a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8"
              }
            ]
          }
        ]
        "###);
    }

    #[tokio::test]
    async fn test_generate_entity() {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid());
        let id = EntityId::from_name("test_entity");
        let activity = ActivityId::from_name("test_activity");
        let operation: ChronicleOperation = ChronicleOperation::GenerateEntity(GenerateEntity {
            namespace,
            id,
            activity,
        });

        let x = operation.to_json();
        let x: serde_json::Value = serde_json::from_str(&x.0.to_string()).unwrap();
        insta::assert_json_snapshot!(&x, @r###"
        [
          {
            "@id": "_:n1",
            "@type": "http://blockchaintp.com/chronicleoperations/ns#GenerateEntity",
            "http://blockchaintp.com/chronicleoperations/ns#ActivityName": [
              {
                "@value": "test_activity"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#EntityName": [
              {
                "@value": "test_entity"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceName": [
              {
                "@value": "testns"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceUuid": [
              {
                "@value": "a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8"
              }
            ]
          }
        ]
        "###);
    }

    #[tokio::test]
    async fn test_entity_attach() {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid());
        let id = EntityId::from_name("test_entity");
        let agent = AgentId::from_name("test_agent");
        let operation: ChronicleOperation = ChronicleOperation::EntityAttach(EntityAttach {
            namespace,
            identityid: None,
            id,
            locator: None,
            agent,
            signature: None,
            signature_time: None,
        });

        let x = operation.to_json();
        let x: serde_json::Value = serde_json::from_str(&x.0.to_string()).unwrap();
        insta::assert_json_snapshot!(&x, @r###"
        [
          {
            "@id": "_:n1",
            "@type": "http://blockchaintp.com/chronicleoperations/ns#EntityAttach",
            "http://blockchaintp.com/chronicleoperations/ns#AgentName": [
              {
                "@value": "test_agent"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#EntityName": [
              {
                "@value": "test_entity"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceName": [
              {
                "@value": "testns"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceUuid": [
              {
                "@value": "a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8"
              }
            ]
          }
        ]
        "###);
    }

    #[tokio::test]
    async fn test_entity_derive() {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid());
        let id = EntityId::from_name("test_entity");
        let used_id = EntityId::from_name("test_used_entity");
        let activity_id = Some(ActivityId::from_name("test_activity"));
        let typ = Some(DerivationType::Revision);
        let operation: ChronicleOperation = ChronicleOperation::EntityDerive(EntityDerive {
            namespace,
            id,
            used_id,
            activity_id,
            typ,
        });

        let x = operation.to_json();
        let x: serde_json::Value = serde_json::from_str(&x.0.to_string()).unwrap();
        insta::assert_json_snapshot!(&x, @r###"
        [
          {
            "@id": "_:n1",
            "@type": "http://blockchaintp.com/chronicleoperations/ns#EntityDerive",
            "http://blockchaintp.com/chronicleoperations/ns#ActivityName": [
              {
                "@value": "test_activity"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#DerivationType": [
              {
                "@value": "Revision"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#EntityName": [
              {
                "@value": "test_entity"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceName": [
              {
                "@value": "testns"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceUuid": [
              {
                "@value": "a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#UsedEntityName": [
              {
                "@value": "test_used_entity"
              }
            ]
          }
        ]
        "###);
    }

    #[tokio::test]
    async fn test_set_attributes_entity() {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid());
        let id = EntityId::from_name("test_entity");
        let domain = DomaintypeId::from_name("test_domain");
        let attributes = Attributes {
            typ: Some(domain),
            attributes: BTreeMap::new(),
        };
        let operation: ChronicleOperation =
            ChronicleOperation::SetAttributes(SetAttributes::Entity {
                namespace,
                id,
                attributes,
            });
        let x = operation.to_json();
        let x: serde_json::Value = serde_json::from_str(&x.0.to_string()).unwrap();
        insta::assert_json_snapshot!(&x, @r###"
        [
          {
            "@id": "_:n1",
            "@type": "http://blockchaintp.com/chronicleoperations/ns#SetAttributes",
            "http://blockchaintp.com/chronicleoperations/ns#Attributes": [
              {
                "@id": "test_entity",
                "@type": "http://blockchaintp.com/chronicleoperations/ns#Attributes",
                "http://blockchaintp.com/chronicleoperations/ns#DomaintypeId": [
                  {
                    "@value": "test_domain"
                  }
                ]
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#EntityName": [
              {
                "@value": "test_entity"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceName": [
              {
                "@value": "testns"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceUuid": [
              {
                "@value": "a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8"
              }
            ]
          }
        ]
        "###);
    }

    #[tokio::test]
    async fn test_set_attributes_entity_multiple_attributes() {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid());
        let id = EntityId::from_name("test_entity");
        let domain = DomaintypeId::from_name("test_domain");
        let attrs = {
            let mut h: BTreeMap<String, Attribute> = BTreeMap::new();

            let attr = Attribute {
                typ: "Bool".to_string(),
                value: json!("Bool"),
            };
            h.insert("bool_attribute".to_string(), attr);

            let attr = Attribute {
                typ: "String".to_string(),
                value: json!("String"),
            };
            h.insert("string_attribute".to_string(), attr);

            let attr = Attribute {
                typ: "Int".to_string(),
                value: json!("Int"),
            };
            h.insert("int_attribute".to_string(), attr);

            h
        };

        let attributes = Attributes {
            typ: Some(domain),
            attributes: attrs,
        };
        let operation: ChronicleOperation =
            ChronicleOperation::SetAttributes(SetAttributes::Entity {
                namespace,
                id,
                attributes,
            });
        let x = operation.to_json();
        let x: serde_json::Value = serde_json::from_str(&x.0.to_string()).unwrap();
        insta::assert_json_snapshot!(&x, @r###"
        [
          {
            "@id": "_:n1",
            "@type": "http://blockchaintp.com/chronicleoperations/ns#SetAttributes",
            "http://blockchaintp.com/chronicleoperations/ns#Attributes": [
              {
                "@primitive_type": "\"Bool\"",
                "@type": "Bool"
              },
              {
                "@primitive_type": "\"Int\"",
                "@type": "Int"
              },
              {
                "@primitive_type": "\"String\"",
                "@type": "String"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#EntityName": [
              {
                "@value": "test_entity"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceName": [
              {
                "@value": "testns"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceUuid": [
              {
                "@value": "a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8"
              }
            ]
          }
        ]
        "###);
    }
}
