use iref::{AsIri, Iri};
use json::{object, JsonValue};

use crate::{
    attributes::{Attribute, Attributes},
    prov::{
        operations::{ChronicleOperation, CreateNamespace, DerivationType},
        vocab::{Chronicle, ChronicleOperations, Prov},
        ChronicleIri, ExternalIdPart, UuidPart,
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
                "http://blockchaintp.com/chronicle/ns#externalId": [{
                    "@value": ns.external_id.as_str(),
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
                "@type": Iri::from(Chronicle::HasEvidence).as_str(),
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
                "http://blockchaintp.com/chronicle/ns#externalId": [{
                   "@value": agent.external_id.as_str(),
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
                let mut qualified_ids = json::Array::new();

                for delegation in delegation.iter() {
                    ids.push(object! {"@id": delegation.delegate_id.to_string()});
                    qualified_ids.push(object! {"@id": delegation.id.to_string()});
                }

                agentdoc
                    .insert(Iri::from(Prov::ActedOnBehalfOf).as_str(), ids)
                    .ok();

                agentdoc
                    .insert(Iri::from(Prov::QualifiedDelegation).as_str(), qualified_ids)
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

        for (_, associations) in self.association.iter() {
            for association in associations {
                let mut associationdoc = object! {
                    "@id": association.id.to_string(),
                    "@type": Iri::from(Prov::Association).as_str(),
                };

                let mut values = json::Array::new();

                values.push(object! {
                    "@id": JsonValue::String(association.agent_id.to_string()),
                });

                associationdoc
                    .insert(Iri::from(Prov::Responsible).as_str(), values)
                    .ok();

                associationdoc
                    .insert(
                        Iri::from(Prov::HadActivity).as_str(),
                        vec![object! {
                            "@id": JsonValue::String(association.activity_id.to_string()),
                        }],
                    )
                    .ok();

                if let Some(role) = &association.role {
                    associationdoc
                        .insert(
                            Iri::from(Prov::HadRole).as_str(),
                            vec![JsonValue::String(role.to_string())],
                        )
                        .ok();
                }

                let mut values = json::Array::new();

                values.push(object! {
                    "@id": JsonValue::String(association.namespace_id.to_string()),
                });

                associationdoc
                    .insert(Iri::from(Chronicle::HasNamespace).as_str(), values)
                    .ok();

                doc.push(associationdoc);
            }
        }

        for (_, delegations) in self.delegation.iter() {
            for delegation in delegations {
                let mut delegationdoc = object! {
                    "@id": delegation.id.to_string(),
                    "@type": Iri::from(Prov::Delegation).as_str(),
                };

                if let Some(activity_id) = &delegation.activity_id {
                    delegationdoc
                        .insert(
                            Iri::from(Prov::HadActivity).as_str(),
                            vec![object! {
                                "@id": JsonValue::String(activity_id.to_string()),
                            }],
                        )
                        .ok();
                }

                if let Some(role) = &delegation.role {
                    delegationdoc
                        .insert(
                            Iri::from(Prov::HadRole).as_str(),
                            vec![JsonValue::String(role.to_string())],
                        )
                        .ok();
                }

                let mut responsible_ids = json::Array::new();
                responsible_ids.push(
                    object! { "@id": JsonValue::String(delegation.responsible_id.to_string())},
                );

                delegationdoc
                    .insert(Iri::from(Prov::Responsible).as_str(), responsible_ids)
                    .ok();

                let mut delegate_ids = json::Array::new();
                delegate_ids
                    .push(object! { "@id": JsonValue::String(delegation.delegate_id.to_string())});

                delegationdoc
                    .insert(Iri::from(Prov::ActedOnBehalfOf).as_str(), delegate_ids)
                    .ok();

                let mut values = json::Array::new();

                values.push(object! {
                    "@id": JsonValue::String(delegation.namespace_id.to_string()),
                });

                delegationdoc
                    .insert(Iri::from(Chronicle::HasNamespace).as_str(), values)
                    .ok();

                doc.push(delegationdoc);
            }
        }

        for ((namespace, id), activity) in self.activities.iter() {
            let mut typ = vec![Iri::from(Prov::Activity).to_string()];
            if let Some(x) = activity.domaintypeid.as_ref() {
                typ.push(x.to_string())
            }

            let mut activitydoc = object! {
                "@id": (*id.to_string()),
                "@type": typ,
                "http://blockchaintp.com/chronicle/ns#externalId": [{
                   "@value": activity.external_id.as_str(),
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

                let mut qualified_ids = json::Array::new();
                for asoc in asoc.iter() {
                    ids.push(object! {"@id": asoc.agent_id.to_string()});
                    qualified_ids.push(object! {"@id": asoc.id.to_string()});
                }

                activitydoc
                    .insert(&Iri::from(Prov::WasAssociatedWith).to_string(), ids)
                    .ok();

                activitydoc
                    .insert(
                        &Iri::from(Prov::QualifiedAssociation).to_string(),
                        qualified_ids,
                    )
                    .ok();
            }

            if let Some(usage) = self.usage.get(&(namespace.to_owned(), id.to_owned())) {
                let mut ids = json::Array::new();

                for usage in usage.iter() {
                    ids.push(object! {"@id": usage.entity_id.to_string()});
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

            if let Some(activities) = self
                .was_informed_by
                .get(&(namespace.to_owned(), id.to_owned()))
            {
                let mut values = json::Array::new();

                for (_, activity) in activities {
                    values.push(object! {
                        "@id": JsonValue::String(activity.to_string()),
                    });
                }
                activitydoc
                    .insert(Iri::from(Prov::WasInformedBy).as_str(), values)
                    .ok();
            }

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
                "http://blockchaintp.com/chronicle/ns#externalId": [{
                   "@value": entity.external_id.as_str()
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

            if let Some((_, identity)) = self.has_evidence.get(&entity_key) {
                entitydoc
                    .insert(
                        Iri::from(Chronicle::HasEvidence).as_str(),
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
                    .insert(Iri::from(Chronicle::HadEvidence).as_str(), values)
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

fn we_need_to_update_the_ld_library_to_a_version_that_supports_serde(
    json: &serde_json::Value,
) -> JsonValue {
    json::parse(&json.to_string()).unwrap()
}

impl ProvModel {
    fn write_attributes<'a, I: Iterator<Item = &'a Attribute>>(doc: &mut JsonValue, attributes: I) {
        let mut attribute_node = object! {};

        for attribute in attributes {
            attribute_node
                .insert(
                    &attribute.typ,
                    we_need_to_update_the_ld_library_to_a_version_that_supports_serde(
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
                let mut o = JsonValue::new_operation(ChronicleOperations::AgentExists);

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
                let mut o = JsonValue::new_operation(ChronicleOperations::AgentActsOnBehalfOf);

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
            ChronicleOperation::RegisterKey(RegisterKey {
                namespace,
                id,
                publickey,
            }) => {
                let mut o = JsonValue::new_operation(ChronicleOperations::RegisterKey);

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

                o.has_value(
                    OperationValue::string(publickey.to_owned()),
                    ChronicleOperations::PublicKey,
                );

                o
            }
            ChronicleOperation::ActivityExists(ActivityExists {
                namespace,
                external_id,
            }) => {
                let mut o = JsonValue::new_operation(ChronicleOperations::ActivityExists);

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
                let mut o = JsonValue::new_operation(ChronicleOperations::StartActivity);

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
                let mut o = JsonValue::new_operation(ChronicleOperations::EndActivity);

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
                let mut o = JsonValue::new_operation(ChronicleOperations::ActivityUses);

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
                let mut o = JsonValue::new_operation(ChronicleOperations::EntityExists);

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
                let mut o = JsonValue::new_operation(ChronicleOperations::WasGeneratedBy);

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
                let mut o = JsonValue::new_operation(ChronicleOperations::WasInformedBy);

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
            ChronicleOperation::EntityHasEvidence(EntityHasEvidence {
                namespace,
                identityid,
                id,
                locator,
                agent,
                signature,
                signature_time,
            }) => {
                let mut o = JsonValue::new_operation(ChronicleOperations::EntityHasEvidence);

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
                    OperationValue::string(agent.external_id_part()),
                    ChronicleOperations::AgentName,
                );

                if let Some(locator) = locator {
                    o.has_value(
                        OperationValue::string(locator),
                        ChronicleOperations::Locator,
                    );
                }

                if let Some(signature) = signature {
                    o.has_value(
                        OperationValue::string(signature),
                        ChronicleOperations::Signature,
                    );
                }

                if let Some(signature_time) = signature_time {
                    o.has_value(
                        OperationValue::string(signature_time.to_rfc3339()),
                        ChronicleOperations::SignatureTime,
                    );
                }

                if let Some(identity_id) = identityid {
                    o.has_id(
                        OperationValue::identity(identity_id.clone().into()),
                        ChronicleOperations::Identity,
                    );
                }

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
                let mut o = JsonValue::new_operation(ChronicleOperations::SetAttributes);

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
                let mut o = JsonValue::new_operation(ChronicleOperations::SetAttributes);

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
                let mut o = JsonValue::new_operation(ChronicleOperations::WasAssociatedWith);

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

impl Operate for JsonValue {
    fn new_type(id: OperationValue, op: ChronicleOperations) -> Self {
        object! {
            "@id": id.0,
            "@type": iref::Iri::from(op).as_str(),
        }
    }

    fn new_value(id: OperationValue) -> Self {
        object! {
            "@value": id.0
        }
    }

    fn new_id(id: OperationValue) -> Self {
        object! {
            "@id": id.0
        }
    }

    fn has_value(&mut self, value: OperationValue, op: ChronicleOperations) {
        let key = iref::Iri::from(op).to_string();
        let mut values: Vec<JsonValue> = json::Array::new();
        let object = Self::new_value(value);
        values.push(object);
        self.insert(&key, values).ok();
    }

    fn has_id(&mut self, id: OperationValue, op: ChronicleOperations) {
        let key = iref::Iri::from(op).to_string();
        let mut value: Vec<JsonValue> = json::Array::new();
        let object = Self::new_id(id);
        value.push(object);
        self.insert(&key, value).ok();
    }

    fn new_operation(op: ChronicleOperations) -> Self {
        let id = OperationValue::string("_:n1");
        Self::new_type(id, op)
    }

    fn attributes_object(&mut self, attributes: &Attributes) {
        let mut attribute_node = object! {};
        for attribute in attributes.attributes.values() {
            attribute_node
                .insert(
                    &attribute.typ,
                    we_need_to_update_the_ld_library_to_a_version_that_supports_serde(
                        &attribute.value,
                    ),
                )
                .ok();
        }

        self.insert(
            &iref::Iri::from(ChronicleOperations::Attributes).to_string(),
            object! {"@value" : attribute_node, "@type": "@json"},
        )
        .ok();
    }

    fn derivation(&mut self, typ: &DerivationType) {
        let typ = match typ {
            DerivationType::Revision => "Revision",
            DerivationType::Quotation => "Quotation",
            DerivationType::PrimarySource => "PrimarySource",
        };
        let id = OperationValue::string(typ);

        self.has_value(id, ChronicleOperations::DerivationType);
    }
}
