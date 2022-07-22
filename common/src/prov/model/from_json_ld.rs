use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use futures::TryFutureExt;
use iref::{AsIri, Iri, IriBuf, IriRefBuf};
use json::JsonValue;
use json_ld::{
    syntax::Term, util::AsJson, Document, Indexed, JsonContext, NoLoader, Node, Reference,
};

use crate::{
    attributes::{Attribute, Attributes},
    prov::{
        operations::{
            ActivityExists, ActivityUses, ActsOnBehalfOf, AgentExists, ChronicleOperation,
            CreateNamespace, DerivationType, EndActivity, EntityDerive, EntityExists,
            EntityHasEvidence, RegisterKey, SetAttributes, StartActivity, WasAssociatedWith,
            WasGeneratedBy,
        },
        vocab::{Chronicle, ChronicleOperations, Prov},
        ActivityId, AgentId, DomaintypeId, EntityId, EvidenceId, IdentityId, NamePart, NamespaceId,
        Role, UuidPart,
    },
};

use super::{
    Activity, Agent, Attachment, Entity, ExpandedJson, Identity, ProcessorError, ProvModel,
};

fn extract_reference_ids(iri: &dyn AsIri, node: &Node) -> Result<Vec<IriBuf>, ProcessorError> {
    let ids: Result<Vec<_>, _> = node
        .get(&Reference::Id(iri.as_iri().into()))
        .map(|o| {
            o.id().ok_or_else(|| ProcessorError::MissingId {
                object: node.as_json(),
            })
        })
        .map(|id| {
            id.and_then(|id| {
                id.as_iri().ok_or_else(|| ProcessorError::MissingId {
                    object: node.as_json(),
                })
            })
        })
        .map(|id| id.map(|id| id.to_owned()))
        .collect();

    ids
}

fn extract_scalar_prop<'a>(
    iri: &dyn AsIri,
    node: &'a Node,
) -> Result<&'a Indexed<json_ld::object::Object>, ProcessorError> {
    node.get_any(&Reference::Id(iri.as_iri().into()))
        .ok_or_else(|| ProcessorError::MissingProperty {
            iri: iri.as_iri().as_str().to_string(),
            object: node.as_json(),
        })
}

fn extract_namespace(agent: &Node) -> Result<NamespaceId, ProcessorError> {
    Ok(NamespaceId::try_from(Iri::from_str(
        extract_scalar_prop(&Chronicle::HasNamespace, agent)?
            .id()
            .ok_or(ProcessorError::MissingId {
                object: agent.as_json(),
            })?
            .as_str(),
    )?)?)
}

impl ProvModel {
    pub async fn apply_json_ld_bytes(self, buf: &[u8]) -> Result<Self, ProcessorError> {
        self.apply_json_ld(json::parse(std::str::from_utf8(buf)?)?)
            .await
    }

    /// Take a Json-Ld input document, assuming it is in compact form, expand it and apply the state to the prov model
    /// Replace @context with our resource context
    /// We rely on reified @types, so subclassing must also include supertypes
    pub async fn apply_json_ld(mut self, mut json: JsonValue) -> Result<Self, ProcessorError> {
        json.remove("@context");
        json.insert("@context", crate::context::PROV.clone()).ok();

        let output = json
            .expand::<JsonContext, _>(&mut NoLoader)
            .map_err(|e| ProcessorError::Expansion {
                inner: e.to_string(),
            })
            .await?;

        for o in output {
            let o = o
                .try_cast::<Node>()
                .map_err(|_| ProcessorError::NotANode {})?
                .into_inner();
            if o.has_type(&Reference::Id(Chronicle::Namespace.as_iri().into())) {
                self.apply_node_as_namespace(&o)?;
            }
            if o.has_type(&Reference::Id(Prov::Agent.as_iri().into())) {
                self.apply_node_as_agent(&o)?;
            } else if o.has_type(&Reference::Id(Prov::Activity.as_iri().into())) {
                self.apply_node_as_activity(&o)?;
            } else if o.has_type(&Reference::Id(Prov::Entity.as_iri().into())) {
                self.apply_node_as_entity(&o)?;
            } else if o.has_type(&Reference::Id(Chronicle::Identity.as_iri().into())) {
                self.apply_node_as_identity(&o)?;
            } else if o.has_type(&Reference::Id(Chronicle::HasEvidence.as_iri().into())) {
                self.apply_node_as_attachment(&o)?;
            } else if o.has_type(&Reference::Id(Prov::Delegation.as_iri().into())) {
                self.apply_node_as_delegation(&o)?;
            } else if o.has_type(&Reference::Id(Prov::Association.as_iri().into())) {
                self.apply_node_as_association(&o)?;
            }
        }

        Ok(self)
    }

    /// Extract the types and find the first that is not prov::, as we currently only alow zero or one domain types
    /// this should be sufficient
    fn extract_attributes(node: &Node) -> Result<Attributes, ProcessorError> {
        let typ = node
            .types()
            .iter()
            .filter_map(|x| x.as_iri())
            .find(|x| x.as_str().contains("domaintype"))
            .map(|iri| Ok::<_, ProcessorError>(DomaintypeId::try_from(iri.as_iri())?))
            .transpose();

        Ok(Attributes {
            typ: typ?,
            attributes: node
                .get(&Reference::Id(Chronicle::Value.as_iri().into()))
                .map(|o| {
                    let serde_object = serde_json::from_str(&*o.as_json()["@value"].to_string())?;

                    if let serde_json::Value::Object(object) = serde_object {
                        Ok(object
                            .into_iter()
                            .map(|(typ, value)| Attribute { typ, value })
                            .collect::<Vec<_>>())
                    } else {
                        Err(ProcessorError::NotAnObject {})
                    }
                })
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .flatten()
                .map(|attr| (attr.typ.clone(), attr))
                .collect(),
        })
    }

    fn apply_node_as_namespace(&mut self, ns: &Node) -> Result<(), ProcessorError> {
        let ns = ns.id().ok_or_else(|| ProcessorError::MissingId {
            object: ns.as_json(),
        })?;

        self.namespace_context(&NamespaceId::try_from(Iri::from_str(ns.as_str())?)?);

        Ok(())
    }

    fn apply_node_as_delegation(&mut self, delegation: &Node) -> Result<(), ProcessorError> {
        let namespace_id = extract_namespace(delegation)?;
        self.namespace_context(&namespace_id);

        let role = extract_scalar_prop(&Prov::HadRole, delegation)
            .ok()
            .and_then(|x| x.as_str().map(Role::from));

        let responsible_id = extract_reference_ids(&Prov::Responsible, delegation)?
            .into_iter()
            .next()
            .ok_or_else(|| ProcessorError::MissingProperty {
                object: delegation.as_json(),
                iri: Prov::Responsible.as_iri().to_string(),
            })
            .and_then(|x| Ok(AgentId::try_from(x.as_iri())?))?;

        let delegate_id = extract_reference_ids(&Prov::ActedOnBehalfOf, delegation)?
            .into_iter()
            .next()
            .ok_or_else(|| ProcessorError::MissingProperty {
                object: delegation.as_json(),
                iri: Prov::ActedOnBehalfOf.as_iri().to_string(),
            })
            .and_then(|x| Ok(AgentId::try_from(x.as_iri())?))?;

        let activity_id = extract_reference_ids(&Prov::HadActivity, delegation)?
            .into_iter()
            .next()
            .map(|x| ActivityId::try_from(x.as_iri()))
            .transpose()?;

        self.qualified_delegation(
            &namespace_id,
            &responsible_id,
            &delegate_id,
            activity_id,
            role,
        );
        Ok(())
    }

    fn apply_node_as_association(&mut self, association: &Node) -> Result<(), ProcessorError> {
        let namespace_id = extract_namespace(association)?;
        self.namespace_context(&namespace_id);

        let role = extract_scalar_prop(&Prov::HadRole, association)
            .ok()
            .and_then(|x| x.as_str().map(Role::from));

        let agent_id = extract_reference_ids(&Prov::Responsible, association)?
            .into_iter()
            .next()
            .ok_or_else(|| ProcessorError::MissingProperty {
                object: association.as_json(),
                iri: Prov::Responsible.as_iri().to_string(),
            })
            .and_then(|x| Ok(AgentId::try_from(x.as_iri())?))?;

        let activity_id = extract_reference_ids(&Prov::HadActivity, association)?
            .into_iter()
            .next()
            .ok_or_else(|| ProcessorError::MissingProperty {
                object: association.as_json(),
                iri: Prov::HadActivity.as_iri().to_string(),
            })
            .and_then(|x| Ok(ActivityId::try_from(x.as_iri())?))?;

        self.qualified_association(&namespace_id, &activity_id, &agent_id, role);

        Ok(())
    }

    fn apply_node_as_agent(&mut self, agent: &Node) -> Result<(), ProcessorError> {
        let id = AgentId::try_from(Iri::from_str(
            agent
                .id()
                .ok_or_else(|| ProcessorError::MissingId {
                    object: agent.as_json(),
                })?
                .as_str(),
        )?)?;

        let namespaceid = extract_namespace(agent)?;
        self.namespace_context(&namespaceid);

        let attributes = Self::extract_attributes(agent)?;

        for identity in extract_reference_ids(&Chronicle::HasIdentity, agent)?
            .into_iter()
            .map(|id| IdentityId::try_from(id.as_iri()))
        {
            self.has_identity(namespaceid.clone(), &id, &identity?);
        }

        for identity in extract_reference_ids(&Chronicle::HadIdentity, agent)?
            .into_iter()
            .map(|id| IdentityId::try_from(id.as_iri()))
        {
            self.had_identity(namespaceid.clone(), &id, &identity?);
        }

        let agent = Agent::exists(namespaceid, id).has_attributes(attributes);

        self.add_agent(agent);

        Ok(())
    }

    fn apply_node_as_activity(&mut self, activity: &Node) -> Result<(), ProcessorError> {
        let id = ActivityId::try_from(Iri::from_str(
            activity
                .id()
                .ok_or_else(|| ProcessorError::MissingId {
                    object: activity.as_json(),
                })?
                .as_str(),
        )?)?;

        let namespaceid = extract_namespace(activity)?;
        self.namespace_context(&namespaceid);

        let started = extract_scalar_prop(&Prov::StartedAtTime, activity)
            .ok()
            .and_then(|x| x.as_str().map(DateTime::parse_from_rfc3339));

        let ended = extract_scalar_prop(&Prov::EndedAtTime, activity)
            .ok()
            .and_then(|x| x.as_str().map(DateTime::parse_from_rfc3339));

        let used = extract_reference_ids(&Prov::Used, activity)?
            .into_iter()
            .map(|id| EntityId::try_from(id.as_iri()))
            .collect::<Result<Vec<_>, _>>()?;

        let attributes = Self::extract_attributes(activity)?;

        let mut activity = Activity::exists(namespaceid.clone(), id).has_attributes(attributes);

        if let Some(started) = started {
            activity.started = Some(DateTime::<Utc>::from(started?));
        }

        if let Some(ended) = ended {
            activity.ended = Some(DateTime::<Utc>::from(ended?));
        }

        for entity in used {
            self.used(namespaceid.clone(), &activity.id, &entity);
        }

        self.add_activity(activity);

        Ok(())
    }

    fn apply_node_as_identity(&mut self, identity: &Node) -> Result<(), ProcessorError> {
        let namespaceid = extract_namespace(identity)?;

        let id = IdentityId::try_from(Iri::from_str(
            identity
                .id()
                .ok_or_else(|| ProcessorError::MissingId {
                    object: identity.as_json(),
                })?
                .as_str(),
        )?)?;

        let public_key = extract_scalar_prop(&Chronicle::PublicKey, identity)
            .ok()
            .and_then(|x| x.as_str().map(|x| x.to_string()))
            .ok_or_else(|| ProcessorError::MissingProperty {
                iri: Chronicle::PublicKey.as_iri().to_string(),
                object: identity.as_json(),
            })?;

        self.add_identity(Identity {
            id,
            namespaceid,
            public_key,
        });

        Ok(())
    }

    fn apply_node_as_attachment(&mut self, attachment: &Node) -> Result<(), ProcessorError> {
        let namespaceid = extract_namespace(attachment)?;

        let id = EvidenceId::try_from(Iri::from_str(
            attachment
                .id()
                .ok_or_else(|| ProcessorError::MissingId {
                    object: attachment.as_json(),
                })?
                .as_str(),
        )?)?;

        let signer = extract_reference_ids(&Chronicle::SignedBy, attachment)?
            .into_iter()
            .next()
            .ok_or_else(|| ProcessorError::MissingId {
                object: attachment.as_json(),
            })
            .map(|id| IdentityId::try_from(id.as_iri()))??;

        let signature = extract_scalar_prop(&Chronicle::Signature, attachment)
            .ok()
            .and_then(|x| x.as_str())
            .ok_or_else(|| ProcessorError::MissingProperty {
                iri: Chronicle::Signature.as_iri().to_string(),
                object: attachment.as_json(),
            })?
            .to_owned();

        let signature_time = extract_scalar_prop(&Chronicle::SignedAtTime, attachment)
            .ok()
            .and_then(|x| x.as_str().map(DateTime::parse_from_rfc3339))
            .ok_or_else(|| ProcessorError::MissingProperty {
                iri: Chronicle::SignedAtTime.as_iri().to_string(),
                object: attachment.as_json(),
            })??;

        let locator = extract_scalar_prop(&Chronicle::Locator, attachment)
            .ok()
            .and_then(|x| x.as_str());

        self.add_attachment(Attachment {
            namespaceid,
            id,
            signature,
            signer,
            locator: locator.map(|x| x.to_owned()),
            signature_time: signature_time.into(),
        });

        Ok(())
    }

    fn apply_node_as_entity(&mut self, entity: &Node) -> Result<(), ProcessorError> {
        let id = EntityId::try_from(Iri::from_str(
            entity
                .id()
                .ok_or_else(|| ProcessorError::MissingId {
                    object: entity.as_json(),
                })?
                .as_str(),
        )?)?;

        let namespaceid = extract_namespace(entity)?;
        self.namespace_context(&namespaceid);

        let generatedby = extract_reference_ids(&Prov::WasGeneratedBy, entity)?
            .into_iter()
            .map(|id| ActivityId::try_from(id.as_iri()))
            .collect::<Result<Vec<_>, _>>()?;

        for attachment in extract_reference_ids(&Chronicle::HasEvidence, entity)?
            .into_iter()
            .map(|id| EvidenceId::try_from(id.as_iri()))
        {
            self.has_attachment(namespaceid.clone(), id.clone(), &attachment?);
        }

        for attachment in extract_reference_ids(&Chronicle::HadEvidence, entity)?
            .into_iter()
            .map(|id| EvidenceId::try_from(id.as_iri()))
        {
            self.had_attachment(namespaceid.clone(), id.clone(), &attachment?);
        }

        for derived in extract_reference_ids(&Prov::WasDerivedFrom, entity)?
            .into_iter()
            .map(|id| EntityId::try_from(id.as_iri()))
        {
            self.was_derived_from(namespaceid.clone(), None, derived?, id.clone(), None);
        }

        for derived in extract_reference_ids(&Prov::WasQuotedFrom, entity)?
            .into_iter()
            .map(|id| EntityId::try_from(id.as_iri()))
        {
            self.was_derived_from(
                namespaceid.clone(),
                Some(DerivationType::quotation()),
                derived?,
                id.clone(),
                None,
            );
        }

        for derived in extract_reference_ids(&Prov::WasRevisionOf, entity)?
            .into_iter()
            .map(|id| EntityId::try_from(id.as_iri()))
        {
            self.was_derived_from(
                namespaceid.clone(),
                Some(DerivationType::revision()),
                derived?,
                id.clone(),
                None,
            );
        }

        for derived in extract_reference_ids(&Prov::HadPrimarySource, entity)?
            .into_iter()
            .map(|id| EntityId::try_from(id.as_iri()))
        {
            self.was_derived_from(
                namespaceid.clone(),
                Some(DerivationType::primary_source()),
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
    fn operation_namespace(&self) -> NamespaceId;
    fn operation_agent(&self) -> AgentId;
    fn operation_delegate(&self) -> AgentId;
    fn operation_responsible(&self) -> AgentId;
    fn operation_optional_activity(&self) -> Option<ActivityId>;
    fn operation_activity(&self) -> ActivityId;
    fn operation_optional_role(&self) -> Option<Role>;
    fn operation_identity(&self) -> Option<IdentityId>;
    fn operation_key(&self) -> String;
    fn operation_start_time(&self) -> String;
    fn operation_locator(&self) -> Option<String>;
    fn operation_signature(&self) -> Option<String>;
    fn operation_signature_time(&self) -> Option<String>;
    fn operation_end_time(&self) -> String;
    fn operation_entity(&self) -> EntityId;
    fn operation_used_entity(&self) -> EntityId;
    fn operation_derivation(&self) -> Option<DerivationType>;
    fn operation_domain(&self) -> Option<DomaintypeId>;
    fn operation_attributes(&self) -> BTreeMap<String, Attribute>;
}

impl Operation for Node {
    fn operation_namespace(&self) -> NamespaceId {
        let mut uuid_objects = self.get(&Reference::Id(
            ChronicleOperations::NamespaceUuid.as_iri().into(),
        ));
        let uuid = uuid_objects.next().unwrap().as_str().unwrap();
        let mut name_objects = self.get(&Reference::Id(
            ChronicleOperations::NamespaceName.as_iri().into(),
        ));
        let name = name_objects.next().unwrap().as_str().unwrap();
        let uuid = uuid::Uuid::parse_str(uuid).unwrap();
        NamespaceId::from_name(name, uuid)
    }

    fn operation_agent(&self) -> AgentId {
        let mut name_objects = self.get(&Reference::Id(
            ChronicleOperations::AgentName.as_iri().into(),
        ));
        let name = name_objects.next().unwrap().as_str().unwrap();
        AgentId::from_name(name)
    }

    fn operation_delegate(&self) -> AgentId {
        let mut name_objects = self.get(&Reference::Id(
            ChronicleOperations::DelegateId.as_iri().into(),
        ));
        let name = name_objects.next().unwrap().as_str().unwrap();
        AgentId::from_name(name)
    }

    fn operation_optional_activity(&self) -> Option<ActivityId> {
        let mut name_objects = self.get(&Reference::Id(
            ChronicleOperations::ActivityName.as_iri().into(),
        ));
        let object = match name_objects.next() {
            Some(object) => object,
            None => return None,
        };
        Some(ActivityId::from_name(object.as_str().unwrap()))
    }

    fn operation_key(&self) -> String {
        let mut objects = self.get(&Reference::Id(
            ChronicleOperations::PublicKey.as_iri().into(),
        ));
        String::from(objects.next().unwrap().as_str().unwrap())
    }

    fn operation_start_time(&self) -> String {
        let mut objects = self.get(&Reference::Id(
            ChronicleOperations::StartActivityTime.as_iri().into(),
        ));
        let time = objects.next().unwrap().as_str().unwrap();
        time.to_owned()
    }

    fn operation_end_time(&self) -> String {
        let mut objects = self.get(&Reference::Id(
            ChronicleOperations::EndActivityTime.as_iri().into(),
        ));
        let time = objects.next().unwrap().as_str().unwrap();
        time.to_owned()
    }

    fn operation_entity(&self) -> EntityId {
        let mut name_objects = self.get(&Reference::Id(
            ChronicleOperations::EntityName.as_iri().into(),
        ));
        let name = name_objects.next().unwrap().as_str().unwrap();
        EntityId::from_name(name)
    }

    fn operation_used_entity(&self) -> EntityId {
        let mut name_objects = self.get(&Reference::Id(
            ChronicleOperations::UsedEntityName.as_iri().into(),
        ));
        let name = name_objects.next().unwrap().as_str().unwrap();
        EntityId::from_name(name)
    }

    fn operation_derivation(&self) -> Option<DerivationType> {
        let mut objects = self.get(&Reference::Id(
            ChronicleOperations::DerivationType.as_iri().into(),
        ));
        let derivation = match objects.next() {
            Some(object) => object.as_str().unwrap(),
            None => return None,
        };

        let d = match derivation {
            "Revision" => DerivationType::Revision,
            "Quotation" => DerivationType::Quotation,
            "PrimarySource" => DerivationType::PrimarySource,
            _ => unreachable!(),
        };
        Some(d)
    }

    fn operation_domain(&self) -> Option<DomaintypeId> {
        let mut objects = self.get(&Reference::Id(
            ChronicleOperations::DomaintypeId.as_iri().into(),
        ));
        let d = match objects.next() {
            Some(object) => object.as_str().unwrap(),
            None => return None,
        };
        Some(DomaintypeId::from_name(d))
    }

    fn operation_attributes(&self) -> BTreeMap<String, Attribute> {
        self.get(&Reference::Id(
            ChronicleOperations::Attributes.as_iri().into(),
        ))
        .map(|o| {
            let serde_object = serde_json::from_str(&*o.as_json()["@value"].to_string()).unwrap();

            if let serde_json::Value::Object(object) = serde_object {
                Ok(object
                    .into_iter()
                    .map(|(typ, value)| Attribute { typ, value })
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

    fn operation_responsible(&self) -> AgentId {
        let mut name_objects = self.get(&Reference::Id(
            ChronicleOperations::ResponsibleId.as_iri().into(),
        ));
        let name = name_objects.next().unwrap().as_str().unwrap();
        AgentId::from_name(name)
    }

    fn operation_optional_role(&self) -> Option<Role> {
        let mut name_objects = self.get(&Reference::Id(ChronicleOperations::Role.as_iri().into()));
        let object = match name_objects.next() {
            Some(object) => object,
            None => return None,
        };
        Some(Role::from(object.as_str().unwrap()))
    }

    fn operation_activity(&self) -> ActivityId {
        let mut name_objects = self.get(&Reference::Id(
            ChronicleOperations::ActivityName.as_iri().into(),
        ));
        let name = name_objects.next().unwrap().as_str().unwrap();
        ActivityId::from_name(name)
    }

    fn operation_identity(&self) -> Option<IdentityId> {
        let mut id_objects = self.get(&Reference::Id(
            ChronicleOperations::Identity.as_iri().into(),
        ));
        let id = match id_objects.next() {
            Some(id) => id,
            None => return None,
        };
        Some(
            IdentityId::try_from(
                IriRefBuf::from_string(id.as_str().unwrap().to_owned())
                    .unwrap()
                    .as_iri()
                    .unwrap(),
            )
            .unwrap(),
        )
    }

    fn operation_locator(&self) -> Option<String> {
        let mut objects = self.get(&Reference::Id(ChronicleOperations::Locator.as_iri().into()));

        let locator = match objects.next() {
            Some(object) => object,
            None => return None,
        };

        Some(locator.as_str().unwrap().to_owned())
    }

    fn operation_signature(&self) -> Option<String> {
        let mut objects = self.get(&Reference::Id(
            ChronicleOperations::Signature.as_iri().into(),
        ));
        let signature = match objects.next() {
            Some(object) => object,
            None => return None,
        };

        Some(signature.as_str().unwrap().to_owned())
    }

    fn operation_signature_time(&self) -> Option<String> {
        let mut objects = self.get(&Reference::Id(
            ChronicleOperations::SignatureTime.as_iri().into(),
        ));
        let time = match objects.next() {
            Some(object) => object,
            None => return None,
        };

        Some(time.as_str().unwrap().to_owned())
    }
}

impl ChronicleOperation {
    pub async fn from_json(ExpandedJson(json): ExpandedJson) -> Result<Self, ProcessorError> {
        let output = json
            .expand::<JsonContext, _>(&mut NoLoader)
            .map_err(|e| ProcessorError::Expansion {
                inner: e.to_string(),
            })
            .await?;
        if let Some(object) = output.into_iter().next() {
            let o = object
                .try_cast::<Node>()
                .map_err(|_| ProcessorError::NotANode {})?
                .into_inner();
            if o.has_type(&Reference::Id(
                ChronicleOperations::CreateNamespace.as_iri().into(),
            )) {
                let namespace = o.operation_namespace();
                let name = namespace.name_part().to_owned();
                let uuid = namespace.uuid_part().to_owned();
                Ok(ChronicleOperation::CreateNamespace(CreateNamespace {
                    id: namespace,
                    name,
                    uuid,
                }))
            } else if o.has_type(&Reference::Id(
                ChronicleOperations::AgentExists.as_iri().into(),
            )) {
                let namespace = o.operation_namespace();
                let agent = o.operation_agent();
                let name = agent.name_part();
                Ok(ChronicleOperation::AgentExists(AgentExists {
                    namespace,
                    name: name.into(),
                }))
            } else if o.has_type(&Reference::Id(
                ChronicleOperations::AgentActsOnBehalfOf.as_iri().into(),
            )) {
                let namespace = o.operation_namespace();
                let delegate_id = o.operation_delegate();
                let responsible_id = o.operation_responsible();
                let activity_id = o.operation_optional_activity();

                Ok(ChronicleOperation::AgentActsOnBehalfOf(
                    ActsOnBehalfOf::new(
                        &namespace,
                        &responsible_id,
                        &delegate_id,
                        activity_id.as_ref(),
                        o.operation_optional_role(),
                    ),
                ))
            } else if o.has_type(&Reference::Id(
                ChronicleOperations::RegisterKey.as_iri().into(),
            )) {
                let namespace = o.operation_namespace();
                let id = o.operation_agent();
                let publickey = o.operation_key();
                Ok(ChronicleOperation::RegisterKey(RegisterKey {
                    namespace,
                    id,
                    publickey,
                }))
            } else if o.has_type(&Reference::Id(
                ChronicleOperations::ActivityExists.as_iri().into(),
            )) {
                let namespace = o.operation_namespace();
                let activity_id = o.operation_optional_activity().unwrap();
                let name = activity_id.name_part().to_owned();
                Ok(ChronicleOperation::ActivityExists(ActivityExists {
                    namespace,
                    name,
                }))
            } else if o.has_type(&Reference::Id(
                ChronicleOperations::StartActivity.as_iri().into(),
            )) {
                let namespace = o.operation_namespace();
                let id = o.operation_optional_activity().unwrap();
                let time: DateTime<Utc> = o.operation_start_time().parse().unwrap();
                Ok(ChronicleOperation::StartActivity(StartActivity {
                    namespace,
                    id,
                    time,
                }))
            } else if o.has_type(&Reference::Id(
                ChronicleOperations::EndActivity.as_iri().into(),
            )) {
                let namespace = o.operation_namespace();
                let id = o.operation_optional_activity().unwrap();
                let time: DateTime<Utc> = o.operation_end_time().parse().unwrap();
                Ok(ChronicleOperation::EndActivity(EndActivity {
                    namespace,
                    id,
                    time,
                }))
            } else if o.has_type(&Reference::Id(
                ChronicleOperations::ActivityUses.as_iri().into(),
            )) {
                let namespace = o.operation_namespace();
                let id = o.operation_entity();
                let activity = o.operation_optional_activity().unwrap();
                Ok(ChronicleOperation::ActivityUses(ActivityUses {
                    namespace,
                    id,
                    activity,
                }))
            } else if o.has_type(&Reference::Id(
                ChronicleOperations::EntityExists.as_iri().into(),
            )) {
                let namespace = o.operation_namespace();
                let entity = o.operation_entity();
                let id = entity.name_part().into();
                Ok(ChronicleOperation::EntityExists(EntityExists {
                    namespace,
                    name: id,
                }))
            } else if o.has_type(&Reference::Id(
                ChronicleOperations::WasGeneratedBy.as_iri().into(),
            )) {
                let namespace = o.operation_namespace();
                let id = o.operation_entity();
                let activity = o.operation_optional_activity().unwrap();
                Ok(ChronicleOperation::WasGeneratedBy(WasGeneratedBy {
                    namespace,
                    id,
                    activity,
                }))
            } else if o.has_type(&Reference::Id(
                ChronicleOperations::EntityHasEvidence.as_iri().into(),
            )) {
                let namespace = o.operation_namespace();
                let id = o.operation_entity();
                let agent = o.operation_agent();
                let signature_time = o.operation_signature_time().map(|t| t.parse().unwrap());
                Ok(ChronicleOperation::EntityHasEvidence(EntityHasEvidence {
                    namespace,
                    identityid: o.operation_identity(),
                    id,
                    locator: o.operation_locator(),
                    agent,
                    signature: o.operation_signature(),
                    signature_time,
                }))
            } else if o.has_type(&Reference::Id(
                ChronicleOperations::EntityDerive.as_iri().into(),
            )) {
                let namespace = o.operation_namespace();
                let id = o.operation_entity();
                let used_id = o.operation_used_entity();
                let activity_id = o.operation_optional_activity();
                let typ = o.operation_derivation();
                Ok(ChronicleOperation::EntityDerive(EntityDerive {
                    namespace,
                    id,
                    used_id,
                    activity_id,
                    typ,
                }))
            } else if o.has_type(&Reference::Id(
                ChronicleOperations::SetAttributes.as_iri().into(),
            )) {
                let namespace = o.operation_namespace();
                let domain = o.operation_domain();

                let attrs = o.operation_attributes();

                let attributes = Attributes {
                    typ: domain,
                    attributes: attrs,
                };
                let actor: SetAttributes = {
                    if o.has_key(&Term::Ref(Reference::Id(
                        ChronicleOperations::EntityName.as_iri().into(),
                    ))) {
                        let id = o.operation_entity();
                        SetAttributes::Entity {
                            namespace,
                            id,
                            attributes,
                        }
                    } else if o.has_key(&Term::Ref(Reference::Id(
                        ChronicleOperations::AgentName.as_iri().into(),
                    ))) {
                        let id = o.operation_agent();
                        SetAttributes::Agent {
                            namespace,
                            id,
                            attributes,
                        }
                    } else {
                        let id = o.operation_optional_activity().unwrap();
                        SetAttributes::Activity {
                            namespace,
                            id,
                            attributes,
                        }
                    }
                };

                Ok(ChronicleOperation::SetAttributes(actor))
            } else if o.has_type(&Reference::Id(
                ChronicleOperations::WasAssociatedWith.as_iri().into(),
            )) {
                Ok(ChronicleOperation::WasAssociatedWith(
                    WasAssociatedWith::new(
                        &o.operation_namespace(),
                        &o.operation_activity(),
                        &o.operation_agent(),
                        o.operation_optional_role(),
                    ),
                ))
            } else {
                unreachable!()
            }
        } else {
            Err(ProcessorError::NotANode {})
        }
    }
}
