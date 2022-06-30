use iref::{AsIri, Iri};
use json::{object, JsonValue};

use crate::{
    attributes::Attribute,
    prov::{
        operations::DerivationType,
        vocab::{Chronicle, Prov},
    },
};

use super::{ExpandedJson, ProvModel};

trait ToJson {
    fn to_json(&self) -> ExpandedJson;
}

impl ProvModel {
    /// Write the model out as a JSON-LD document in expanded form
    pub fn to_json(&self) -> ExpandedJson {
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
