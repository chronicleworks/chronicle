use iref::{AsIri, Iri};
use json::{object, JsonValue};

use crate::{
    attributes::Attribute,
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
    fn to_json(&self) -> super::ExpandedJson {
        let mut operation: Vec<JsonValue> = Vec::new();

        let o = match self {
            ChronicleOperation::CreateNamespace(CreateNamespace { id, .. }) => {
                let mut o = json::object! {
                    "@id": "_:n1",
                    "@type": iref::Iri::from(ChronicleOperations::CreateNamespace).as_str(),
                };
                o.operate(
                    id.name_part().to_string(),
                    ChronicleOperations::NamespaceName,
                );

                o.operate(
                    id.uuid_part().to_string(),
                    ChronicleOperations::NamespaceUuid,
                );

                o
            }
            ChronicleOperation::CreateAgent(CreateAgent { namespace, name }) => {
                let mut o = json::object! {
                    "@id": "_:n1",
                    "@type": iref::Iri::from(ChronicleOperations::CreateAgent).as_str(),
                };

                o.operate(
                    namespace.name_part().to_string(),
                    ChronicleOperations::NamespaceName,
                );

                o.operate(
                    namespace.uuid_part().to_string(),
                    ChronicleOperations::NamespaceUuid,
                );

                o.operate(name.to_string(), ChronicleOperations::AgentName);

                o
            }
            ChronicleOperation::AgentActsOnBehalfOf(ActsOnBehalfOf {
                namespace,
                id,
                delegate_id,
                activity_id,
            }) => {
                let mut o = json::object! {
                    "@id": "_:n1",
                    "@type": iref::Iri::from(ChronicleOperations::AgentActsOnBehalfOf).as_str(),
                };

                o.operate(
                    namespace.name_part().to_string(),
                    ChronicleOperations::NamespaceName,
                );

                o.operate(
                    namespace.uuid_part().to_string(),
                    ChronicleOperations::NamespaceUuid,
                );

                o.operate(id.name_part().to_string(), ChronicleOperations::AgentName);

                let key = iref::Iri::from(ChronicleOperations::DelegateId).to_string();
                let mut value: Vec<JsonValue> = Vec::new();
                let object = json::object! {
                    "@value": delegate_id.name_part().to_string()
                };
                value.push(object);
                o.insert(&key, value).ok();

                if let Some(activity_id) = activity_id {
                    o.operate(
                        activity_id.name_part().to_string(),
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
                let mut o = json::object! {
                    "@id": "_:n1",
                    "@type": iref::Iri::from(ChronicleOperations::RegisterKey).as_str(),
                };

                o.operate(
                    namespace.name_part().to_string(),
                    ChronicleOperations::NamespaceName,
                );

                o.operate(
                    namespace.uuid_part().to_string(),
                    ChronicleOperations::NamespaceUuid,
                );

                o.operate(id.name_part().to_string(), ChronicleOperations::AgentName);

                let key = iref::Iri::from(ChronicleOperations::PublicKey).to_string();
                let mut value: Vec<JsonValue> = Vec::new();
                let object = json::object! {
                    "@value": publickey.to_owned()
                };
                value.push(object);
                o.insert(&key, value).ok();

                o
            }
            ChronicleOperation::CreateActivity(CreateActivity { namespace, name }) => {
                let mut o = json::object! {
                    "@id": "_:n1",
                    "@type": iref::Iri::from(ChronicleOperations::CreateActivity).as_str(),
                };

                o.operate(
                    namespace.name_part().to_string(),
                    ChronicleOperations::NamespaceName,
                );

                o.operate(
                    namespace.uuid_part().to_string(),
                    ChronicleOperations::NamespaceUuid,
                );

                o.operate(name.to_string(), ChronicleOperations::ActivityName);

                o
            }
            ChronicleOperation::StartActivity(StartActivity {
                namespace,
                id,
                agent,
                time,
            }) => {
                let mut o = json::object! {
                    "@id": "_:n1",
                    "@type": iref::Iri::from(ChronicleOperations::StartActivity).as_str(),
                };

                o.operate(
                    namespace.name_part().to_string(),
                    ChronicleOperations::NamespaceName,
                );

                o.operate(
                    namespace.uuid_part().to_string(),
                    ChronicleOperations::NamespaceUuid,
                );

                o.operate(
                    agent.name_part().to_string(),
                    ChronicleOperations::AgentName,
                );

                o.operate(
                    id.name_part().to_string(),
                    ChronicleOperations::ActivityName,
                );

                o.operate(time.to_rfc3339(), ChronicleOperations::StartActivityTime);

                o
            }
            ChronicleOperation::EndActivity(EndActivity {
                namespace,
                id,
                agent,
                time,
            }) => {
                let mut o = json::object! {
                    "@id": "_:n1",
                    "@type": iref::Iri::from(ChronicleOperations::EndActivity).as_str(),
                };

                o.operate(
                    namespace.name_part().to_string(),
                    ChronicleOperations::NamespaceName,
                );

                o.operate(
                    namespace.uuid_part().to_string(),
                    ChronicleOperations::NamespaceUuid,
                );

                o.operate(
                    id.name_part().to_string(),
                    ChronicleOperations::ActivityName,
                );

                o.operate(
                    agent.name_part().to_string(),
                    ChronicleOperations::AgentName,
                );

                o.operate(time.to_rfc3339(), ChronicleOperations::EndActivityTime);

                o
            }
            ChronicleOperation::ActivityUses(ActivityUses {
                namespace,
                id,
                activity,
            }) => {
                let mut o = json::object! {
                    "@id": "_:n1",
                    "@type": iref::Iri::from(ChronicleOperations::ActivityUses).as_str(),
                };

                o.operate(
                    namespace.name_part().to_string(),
                    ChronicleOperations::NamespaceName,
                );

                o.operate(
                    namespace.uuid_part().to_string(),
                    ChronicleOperations::NamespaceUuid,
                );

                o.operate(id.name_part().to_string(), ChronicleOperations::EntityName);

                o.operate(
                    activity.name_part().to_string(),
                    ChronicleOperations::ActivityName,
                );

                o
            }
            ChronicleOperation::CreateEntity(CreateEntity { namespace, name }) => {
                let mut o = json::object! {
                    "@id": "_:n1",
                    "@type": iref::Iri::from(ChronicleOperations::CreateEntity).as_str(),
                };

                o.operate(
                    namespace.name_part().to_string(),
                    ChronicleOperations::NamespaceName,
                );

                o.operate(
                    namespace.uuid_part().to_string(),
                    ChronicleOperations::NamespaceUuid,
                );

                o.operate(name.to_string(), ChronicleOperations::EntityName);

                o
            }
            ChronicleOperation::GenerateEntity(GenerateEntity {
                namespace,
                id,
                activity,
            }) => {
                let mut o = json::object! {
                    "@id": "_:n1",
                    "@type": iref::Iri::from(ChronicleOperations::GenerateEntity).as_str(),
                };

                o.operate(
                    namespace.name_part().to_string(),
                    ChronicleOperations::NamespaceName,
                );

                o.operate(
                    namespace.uuid_part().to_string(),
                    ChronicleOperations::NamespaceUuid,
                );

                o.operate(id.name_part().to_string(), ChronicleOperations::EntityName);

                o.operate(
                    activity.name_part().to_string(),
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
                let mut o = json::object! {
                    "@id": "_:n1",
                    "@type": iref::Iri::from(ChronicleOperations::EntityAttach).as_str(),
                };

                o.operate(
                    namespace.name_part().to_string(),
                    ChronicleOperations::NamespaceName,
                );

                o.operate(
                    namespace.uuid_part().to_string(),
                    ChronicleOperations::NamespaceUuid,
                );

                o.operate(id.name_part().to_string(), ChronicleOperations::EntityName);

                o.operate(
                    agent.name_part().to_string(),
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
                let mut o = json::object! {
                    "@id": "_:n1",
                    "@type": iref::Iri::from(ChronicleOperations::EntityDerive).as_str(),
                };

                o.operate(
                    namespace.name_part().to_string(),
                    ChronicleOperations::NamespaceName,
                );

                o.operate(
                    namespace.uuid_part().to_string(),
                    ChronicleOperations::NamespaceUuid,
                );

                o.operate(id.name_part().to_string(), ChronicleOperations::EntityName);

                o.operate(
                    used_id.name_part().to_string(),
                    ChronicleOperations::EntityName,
                );

                if let Some(activity) = activity_id {
                    o.operate(
                        activity.name_part().to_string(),
                        ChronicleOperations::ActivityName,
                    );
                }

                if let Some(typ) = typ {
                    let typ = match typ {
                        DerivationType::Revision => "Revision",
                        DerivationType::Quotation => "Quotation",
                        DerivationType::PrimarySource => "PrimarySource",
                    };

                    let key = iref::Iri::from(ChronicleOperations::DerivationType).to_string();
                    let mut value: Vec<JsonValue> = Vec::new();
                    let object = json::object! {
                        "@value": typ,
                    };
                    value.push(object);
                    o.insert(&key, value).ok();
                }

                o
            }
            ChronicleOperation::SetAttributes(SetAttributes::Entity {
                namespace,
                id,
                attributes,
            }) => {
                let mut o = json::object! {
                    "@id": "_:n1",
                    "@type": iref::Iri::from(ChronicleOperations::SetAttributes).as_str(),
                };

                o.operate(
                    namespace.name_part().to_string(),
                    ChronicleOperations::NamespaceName,
                );

                o.operate(
                    namespace.uuid_part().to_string(),
                    ChronicleOperations::NamespaceUuid,
                );

                o.operate(id.name_part().to_string(), ChronicleOperations::EntityName);

                let mut attributes_object = json::object! {
                    "@id": id.name_part().to_string(),
                    "@type": iref::Iri::from(ChronicleOperations::Attributes).as_str(),
                };

                if let Some(domaintypeid) = &attributes.typ {
                    let key = iref::Iri::from(ChronicleOperations::DomaintypeId).to_string();
                    let mut value: Vec<JsonValue> = Vec::new();
                    let object = json::object! {
                        "@value": domaintypeid.name_part().to_string(),
                    };
                    value.push(object);
                    attributes_object.insert(&key, value).ok();
                }

                let key = iref::Iri::from(ChronicleOperations::Attributes).to_string();
                let value: Vec<JsonValue> = vec![attributes_object];

                // let mut v = Vec::new();
                // for (key, value) in attributes.attributes {
                //     v.push(json::object! { })
                // }

                // let object = json::object! {
                //     "@value": domaintypeid.name_part().to_string(),
                // };

                o.insert(&key, value).ok();
                // attributes are JSON objects in structure
                o
            }
            ChronicleOperation::SetAttributes(SetAttributes::Activity {
                namespace,
                id,
                attributes,
            }) => {
                let mut o = json::object! {
                    "@id": "_:n1",
                    "@type": iref::Iri::from(ChronicleOperations::SetAttributes).as_str(),
                };

                o.operate(
                    namespace.name_part().to_string(),
                    ChronicleOperations::NamespaceName,
                );

                o.operate(
                    namespace.uuid_part().to_string(),
                    ChronicleOperations::NamespaceUuid,
                );

                o.operate(
                    id.name_part().to_string(),
                    ChronicleOperations::ActivityName,
                );

                let mut attributes_object = json::object! {
                    "@id": id.name_part().to_string(),
                    "@type": iref::Iri::from(ChronicleOperations::Attributes).as_str(),
                };

                if let Some(domaintypeid) = &attributes.typ {
                    let key = iref::Iri::from(ChronicleOperations::DomaintypeId).to_string();
                    let mut value: Vec<JsonValue> = Vec::new();
                    let object = json::object! {
                        "@value": domaintypeid.name_part().to_string(),
                    };
                    value.push(object);
                    attributes_object.insert(&key, value).ok();
                }

                let key = iref::Iri::from(ChronicleOperations::Attributes).to_string();
                let value: Vec<JsonValue> = vec![attributes_object];

                // let mut v = Vec::new();
                // for (key, value) in attributes.attributes {
                //     v.push(json::object! { })
                // }

                // let object = json::object! {
                //     "@value": domaintypeid.name_part().to_string(),
                // };

                o.insert(&key, value).ok();
                // attributes are JSON objects in structure
                o
            }
            ChronicleOperation::SetAttributes(SetAttributes::Agent {
                namespace,
                id,
                attributes,
            }) => {
                let mut o = json::object! {
                    "@id": "_:n1",
                    "@type": iref::Iri::from(ChronicleOperations::SetAttributes).as_str(),
                };

                o.operate(
                    namespace.name_part().to_string(),
                    ChronicleOperations::NamespaceName,
                );

                o.operate(
                    namespace.uuid_part().to_string(),
                    ChronicleOperations::NamespaceUuid,
                );

                o.operate(id.name_part().to_string(), ChronicleOperations::AgentName);

                let mut attributes_object = json::object! {
                    "@id": id.name_part().to_string(),
                    "@type": iref::Iri::from(ChronicleOperations::Attributes).as_str(),
                };

                if let Some(domaintypeid) = &attributes.typ {
                    let key = iref::Iri::from(ChronicleOperations::DomaintypeId).to_string();
                    let mut value: Vec<JsonValue> = Vec::new();
                    let object = json::object! {
                        "@value": domaintypeid.name_part().to_string(),
                    };
                    value.push(object);
                    attributes_object.insert(&key, value).ok();
                }

                let key = iref::Iri::from(ChronicleOperations::Attributes).to_string();
                let value: Vec<JsonValue> = vec![attributes_object];

                // let mut v = Vec::new();
                // for (key, value) in attributes.attributes {
                //     v.push(json::object! { })
                // }

                // let object = json::object! {
                //     "@value": domaintypeid.name_part().to_string(),
                // };

                o.insert(&key, value).ok();
                // attributes are JSON objects in structure
                o
            }
        };
        operation.push(o);
        super::ExpandedJson(operation.into())
    }
}

trait Operate {
    fn operate(&mut self, id: String, op: ChronicleOperations);
}

impl Operate for JsonValue {
    fn operate(&mut self, id: String, op: ChronicleOperations) {
        let key = iref::Iri::from(op).to_string();
        let mut value: Vec<JsonValue> = Vec::new();
        let object = json::object! {
            "@value": id
        };
        value.push(object);
        self.insert(&key, value).ok();
    }

    // fn namespacename(id: &NamespaceId) -> (String, Vec<JsonValue>) {
    //     let key = iref::Iri::from(ChronicleOperations::NamespaceName).to_string();
    //     let mut value: Vec<JsonValue> = Vec::new();
    //     let object = json::object! {
    //         "@value": id.name_part().to_string()
    //     };
    //     value.push(object);
    //     (key, value)
    // }

    // fn namespaceuuid(id: &NamespaceId) -> (String, Vec<JsonValue>) {
    //     let key = iref::Iri::from(ChronicleOperations::NamespaceUuid).to_string();
    //     let mut value: Vec<JsonValue> = Vec::new();
    //     let object = json::object! {
    //         "@value": id.uuid_part().to_string()
    //     };
    //     value.push(object);
    //     (key, value)
    // }

    // fn agentname(name: &Name) -> (String, Vec<JsonValue>) {
    //     let key = iref::Iri::from(ChronicleOperations::AgentName).to_string();
    //     let mut value: Vec<JsonValue> = Vec::new();
    //     let object = json::object! {
    //         "@value": name.to_string()
    //     };
    //     value.push(object);
    //     (key, value)
    // }

    // fn activityname(name: &Name) -> (String, Vec<JsonValue>) {
    //     let key = iref::Iri::from(ChronicleOperations::ActivityName).to_string();
    //     let mut value: Vec<JsonValue> = Vec::new();
    //     let object = json::object! {
    //         "@value": name.to_string()
    //     };
    //     value.push(object);
    //     (key, value)
    // }

    // fn operation_time(time: &DateTime<Utc>) -> (String, Vec<JsonValue>) {
    //     let key = iref::Iri::from(ChronicleOperations::StartActivityTime).to_string();
    //     let mut value: Vec<JsonValue> = Vec::new();
    //     let object = json::object! {
    //         "@value": time.to_rfc3339()
    //     };
    //     value.push(object);
    //     (key, value)
    // }

    // fn entity_name(id: &Name) -> (String, Vec<JsonValue>) {
    //     let key = iref::Iri::from(ChronicleOperations::EntityName).to_string();
    //     let mut value: Vec<JsonValue> = Vec::new();
    //     let object = json::object! {
    //         "@value": id.to_string()
    //     };
    //     value.push(object);
    //     (key, value)
    // }
}

#[cfg(test)]
mod test {
    use crate::prov::{
        operations::CreateNamespace, to_json_ld::ToJson, ActivityId, AgentId, DomaintypeId,
        EntityId, IdentityId, NamespaceId,
    };

    use super::{ChronicleOperation, DerivationType};

    #[tokio::test]
    async fn test_create_namespace() {
        let name = "testns";
        let uuid = uuid::Uuid::new_v4();
        let id = NamespaceId::from_name(name, uuid);

        let op = ChronicleOperation::CreateNamespace(CreateNamespace::new(id, name, uuid));
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
                "@value": "33b1983a-3525-4451-b262-6a8c9a70b309"
              }
            ]
          }
        ]
        "###);
    }

    #[tokio::test]
    async fn test_create_agent() {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid::Uuid::new_v4());
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
                "@value": "151c66ab-8e7e-47f3-8f79-316f0945fc90"
              }
            ]
          }
        ]
        "###);
    }

    #[tokio::test]
    async fn test_agent_acts_on_behalf_of() {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid::Uuid::new_v4());
        let id = crate::prov::AgentId::from_name("test_agent");
        let delegate_id = AgentId::from_name("test_delegate");
        let activity_id = Some(ActivityId::from_name("test_activity"));

        let op: ChronicleOperation = super::ChronicleOperation::AgentActsOnBehalfOf(
            crate::prov::operations::ActsOnBehalfOf {
                namespace,
                id,
                delegate_id,
                activity_id,
            },
        );
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
                "@value": "4549da83-632f-4692-9cd3-daf349f74333"
              }
            ]
          }
        ]
        "###);
    }

    #[tokio::test]
    async fn test_register_key() {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid::Uuid::new_v4());
        let id = crate::prov::AgentId::from_name("test_agent");
        let publickey =
            "02197db854d8c6a488d4a0ef3ef1fcb0c06d66478fae9e87a237172cf6f6f7de23".to_string();

        let op: ChronicleOperation =
            super::ChronicleOperation::RegisterKey(crate::prov::operations::RegisterKey {
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
                "@value": "dc330483-0741-40e8-baa7-7e23509150e8"
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
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid::Uuid::new_v4());
        let name =
            crate::prov::NamePart::name_part(&ActivityId::from_name("test_activity")).to_owned();

        let op: ChronicleOperation =
            super::ChronicleOperation::CreateActivity(crate::prov::operations::CreateActivity {
                namespace,
                name,
            });

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
                "@value": "37d3298c-db82-4e04-be38-a910f9a66865"
              }
            ]
          }
        ]
        "###);
    }

    #[tokio::test]
    async fn start_activity() {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid::Uuid::new_v4());
        let id = ActivityId::from_name("test_activity");
        let agent = crate::prov::AgentId::from_name("test_agent");
        let time = chrono::DateTime::<chrono::Utc>::from_utc(
            chrono::NaiveDateTime::from_timestamp(61, 0),
            chrono::Utc,
        );
        let op: ChronicleOperation =
            super::ChronicleOperation::StartActivity(crate::prov::operations::StartActivity {
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
                "@value": "a556926a-7277-456c-96ab-b2aedca40466"
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
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid::Uuid::new_v4());
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
                "@value": "a2f927c8-32c1-46e9-b5b4-9b3753d0d991"
              }
            ]
          }
        ]
        "###);
    }

    #[tokio::test]
    async fn test_activity_uses() {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid::Uuid::new_v4());
        let id = EntityId::from_name("test_entity");
        let activity = ActivityId::from_name("test_activity");
        let op: ChronicleOperation =
            super::ChronicleOperation::ActivityUses(crate::prov::operations::ActivityUses {
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
                "@value": "1698fb67-2662-4591-b457-60bbe7a38788"
              }
            ]
          }
        ]
        "###);
    }

    #[tokio::test]
    async fn test_create_entity() {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid::Uuid::new_v4());
        let id = crate::prov::NamePart::name_part(&EntityId::from_name("test_entity")).to_owned();
        let op: ChronicleOperation =
            super::ChronicleOperation::CreateEntity(crate::prov::operations::CreateEntity {
                namespace,
                name: id,
            });

        let x = op.to_json();
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
                "@value": "3ca8843a-1611-40c7-a39f-e76fbdef0475"
              }
            ]
          }
        ]
        "###);
    }

    #[tokio::test]
    async fn test_generate_entity() {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid::Uuid::new_v4());
        let id = EntityId::from_name("test_entity");
        let activity = ActivityId::from_name("test_activity");
        let op: ChronicleOperation =
            super::ChronicleOperation::GenerateEntity(crate::prov::operations::GenerateEntity {
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
                "@value": "88e38a73-e236-4e06-892d-fb5aec5301b8"
              }
            ]
          }
        ]
        "###);
    }

    #[tokio::test]
    async fn test_entity_attach() {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid::Uuid::new_v4());
        let id = EntityId::from_name("test_entity");
        let agent = crate::prov::AgentId::from_name("test_agent");
        let public_key =
            "02197db854d8c6a488d4a0ef3ef1fcb0c06d66478fae9e87a237172cf6f6f7de23".to_string();
        let time = chrono::DateTime::<chrono::Utc>::from_utc(
            chrono::NaiveDateTime::from_timestamp(61, 0),
            chrono::Utc,
        );
        let op: ChronicleOperation =
            super::ChronicleOperation::EntityAttach(crate::prov::operations::EntityAttach {
                namespace,
                identityid: IdentityId::from_name("name", public_key),
                id,
                locator: Some(String::from("nothing")),
                agent,
                signature: String::from("string"),
                signature_time: time,
            });

        let x = op.to_json();
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
                "@value": "824d6d43-d20a-4da2-bdf0-a5df708c9628"
              }
            ]
          }
        ]
        "###);
    }

    #[tokio::test]
    async fn test_entity_derive() {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid::Uuid::new_v4());
        let id = EntityId::from_name("test_entity");
        let used_id = EntityId::from_name("test_used_entity");
        let activity_id = Some(ActivityId::from_name("test_activity"));
        let typ = Some(DerivationType::Revision);
        let op: ChronicleOperation =
            super::ChronicleOperation::EntityDerive(crate::prov::operations::EntityDerive {
                namespace,
                id,
                used_id,
                activity_id,
                typ,
            });

        let x = op.to_json();
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
                "@value": "test_used_entity"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceName": [
              {
                "@value": "testns"
              }
            ],
            "http://blockchaintp.com/chronicleoperations/ns#NamespaceUuid": [
              {
                "@value": "e81c4d54-9b29-4e06-9b21-e225df3eaddb"
              }
            ]
          }
        ]
        "###);
    }

    #[tokio::test]
    async fn test_set_attributes_entity() {
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid::Uuid::new_v4());
        let id = EntityId::from_name("test_entity");
        let domain = DomaintypeId::from_name("test_domain");
        let attributes = crate::attributes::Attributes {
            typ: Some(domain),
            attributes: std::collections::HashMap::new(),
        };
        let op: ChronicleOperation = super::ChronicleOperation::SetAttributes(
            crate::prov::operations::SetAttributes::Entity {
                namespace,
                id,
                attributes,
            },
        );
        let x = op.to_json();
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
                "@value": "2af37707-fd59-443b-8f5a-4d7f5f643a27"
              }
            ]
          }
        ]
        "###);
    }
}
