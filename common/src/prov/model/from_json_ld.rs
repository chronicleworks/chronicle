use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use futures::TryFutureExt;
use iref::{AsIri, Iri, IriBuf};
use json::JsonValue;
use json_ld::{
    syntax::Term, util::AsJson, Document, Indexed, JsonContext, NoLoader, Node, Reference,
};

use crate::{
    attributes::{Attribute, Attributes},
    prov::{
        operations::{
            ActivityUses, ActsOnBehalfOf, ChronicleOperation, CreateActivity, CreateAgent,
            CreateEntity, CreateNamespace, DerivationType, EndActivity, EntityAttach, EntityDerive,
            GenerateEntity, RegisterKey, SetAttributes, StartActivity,
        },
        vocab::{Chronicle, ChronicleOperations, Prov},
        ActivityId, AgentId, AttachmentId, DomaintypeId, EntityId, IdentityId, NamePart,
        NamespaceId, UuidPart,
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
            } else if o.has_type(&Reference::Id(Chronicle::HasAttachment.as_iri().into())) {
                self.apply_node_as_attachment(&o)?;
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

        for delegated in extract_reference_ids(&Prov::ActedOnBehalfOf, agent)?
            .into_iter()
            .map(|id| AgentId::try_from(id.as_iri()))
        {
            self.acted_on_behalf_of(namespaceid.clone(), id.clone(), delegated?, None);
        }

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

        let wasassociatedwith = extract_reference_ids(&Prov::WasAssociatedWith, activity)?
            .into_iter()
            .map(|id| AgentId::try_from(id.as_iri()))
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

        for agent in wasassociatedwith {
            self.was_associated_with(&namespaceid, &activity.id, &agent);
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

        let id = AttachmentId::try_from(Iri::from_str(
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

        for attachment in extract_reference_ids(&Chronicle::HasAttachment, entity)?
            .into_iter()
            .map(|id| AttachmentId::try_from(id.as_iri()))
        {
            self.has_attachment(namespaceid.clone(), id.clone(), &attachment?);
        }

        for attachment in extract_reference_ids(&Chronicle::HadAttachment, entity)?
            .into_iter()
            .map(|id| AttachmentId::try_from(id.as_iri()))
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

impl ChronicleOperation {
    pub async fn from_json(ExpandedJson(json): ExpandedJson) -> Result<Self, ProcessorError> {
        let output = json
            .expand::<JsonContext, _>(&mut NoLoader)
            .map_err(|e| ProcessorError::Expansion {
                inner: e.to_string(),
            })
            .await?;
        assert!(output.len() == 1);
        if let Some(object) = output.into_iter().next() {
            let o = object
                .try_cast::<Node>()
                .map_err(|_| ProcessorError::NotANode {})?
                .into_inner();
            let id = o.id().unwrap().as_str();
            assert!(id == "_:n1");
            if o.has_type(&Reference::Id(
                ChronicleOperations::CreateNamespace.as_iri().into(),
            )) {
                let namespace = operation_namespace(&o);
                let name = namespace.name_part().to_owned();
                let uuid = namespace.uuid_part().to_owned();
                Ok(ChronicleOperation::CreateNamespace(CreateNamespace {
                    id: namespace,
                    name,
                    uuid,
                }))
            } else if o.has_type(&Reference::Id(
                ChronicleOperations::CreateAgent.as_iri().into(),
            )) {
                let namespace = operation_namespace(&o);
                let agent = operation_agent(&o);
                let name = agent.name_part();
                Ok(ChronicleOperation::CreateAgent(CreateAgent {
                    namespace,
                    name: name.into(),
                }))
            } else if o.has_type(&Reference::Id(
                ChronicleOperations::AgentActsOnBehalfOf.as_iri().into(),
            )) {
                let namespace = operation_namespace(&o);
                let id = operation_agent(&o);
                let delegate_id = operation_delegate(&o);
                let activity_id = operation_activity(&o);
                Ok(ChronicleOperation::AgentActsOnBehalfOf(ActsOnBehalfOf {
                    namespace,
                    id,
                    delegate_id,
                    activity_id,
                }))
            } else if o.has_type(&Reference::Id(
                ChronicleOperations::RegisterKey.as_iri().into(),
            )) {
                let namespace = operation_namespace(&o);
                let id = operation_agent(&o);
                let publickey = operation_key(&o);
                Ok(ChronicleOperation::RegisterKey(RegisterKey {
                    namespace,
                    id,
                    publickey,
                }))
            } else if o.has_type(&Reference::Id(
                ChronicleOperations::CreateActivity.as_iri().into(),
            )) {
                let namespace = operation_namespace(&o);
                let activity_id = operation_activity(&o).unwrap();
                let name = activity_id.name_part().to_owned();
                Ok(ChronicleOperation::CreateActivity(CreateActivity {
                    namespace,
                    name,
                }))
            } else if o.has_type(&Reference::Id(
                ChronicleOperations::StartActivity.as_iri().into(),
            )) {
                let namespace = operation_namespace(&o);
                let id = operation_activity(&o).unwrap();
                let agent = operation_agent(&o);
                let time: DateTime<Utc> = operation_start_time(&o).parse().unwrap();
                Ok(ChronicleOperation::StartActivity(StartActivity {
                    namespace,
                    id,
                    agent,
                    time,
                }))
            } else if o.has_type(&Reference::Id(
                ChronicleOperations::EndActivity.as_iri().into(),
            )) {
                let namespace = operation_namespace(&o);
                let id = operation_activity(&o).unwrap();
                let agent = operation_agent(&o);
                let time: DateTime<Utc> = operation_end_time(&o).parse().unwrap();
                Ok(ChronicleOperation::EndActivity(EndActivity {
                    namespace,
                    id,
                    agent,
                    time,
                }))
            } else if o.has_type(&Reference::Id(
                ChronicleOperations::ActivityUses.as_iri().into(),
            )) {
                let namespace = operation_namespace(&o);
                let id = operation_entity(&o);
                let activity = operation_activity(&o).unwrap();
                Ok(ChronicleOperation::ActivityUses(ActivityUses {
                    namespace,
                    id,
                    activity,
                }))
            } else if o.has_type(&Reference::Id(
                ChronicleOperations::CreateEntity.as_iri().into(),
            )) {
                let namespace = operation_namespace(&o);
                let entity = operation_entity(&o);
                let id = entity.name_part().into();
                Ok(ChronicleOperation::CreateEntity(CreateEntity {
                    namespace,
                    name: id,
                }))
            } else if o.has_type(&Reference::Id(
                ChronicleOperations::GenerateEntity.as_iri().into(),
            )) {
                let namespace = operation_namespace(&o);
                let id = operation_entity(&o);
                let activity = operation_activity(&o).unwrap();
                Ok(ChronicleOperation::GenerateEntity(GenerateEntity {
                    namespace,
                    id,
                    activity,
                }))
            } else if o.has_type(&Reference::Id(
                ChronicleOperations::EntityAttach.as_iri().into(),
            )) {
                let namespace = operation_namespace(&o);
                let id = operation_entity(&o);
                let agent = operation_agent(&o);
                Ok(ChronicleOperation::EntityAttach(EntityAttach {
                    namespace,
                    identityid: None,
                    id,
                    locator: None,
                    agent,
                    signature: None,
                    signature_time: None,
                }))
            } else if o.has_type(&Reference::Id(
                ChronicleOperations::EntityDerive.as_iri().into(),
            )) {
                let namespace = operation_namespace(&o);
                let id = operation_entity(&o);
                let used_id = operation_used_entity(&o);
                let activity_id = operation_activity(&o);
                let typ = operation_derivation(&o);
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
                let namespace = operation_namespace(&o);
                let domain = operation_domain(&o);

                let attrs = operation_attributes(&o);

                let attributes = Attributes {
                    typ: domain,
                    attributes: attrs,
                };
                let actor: SetAttributes = {
                    if o.has_key(&Term::Ref(Reference::Id(
                        ChronicleOperations::EntityName.as_iri().into(),
                    ))) {
                        let id = operation_entity(&o);
                        SetAttributes::Entity {
                            namespace,
                            id,
                            attributes,
                        }
                    } else if o.has_key(&Term::Ref(Reference::Id(
                        ChronicleOperations::AgentName.as_iri().into(),
                    ))) {
                        let id = operation_agent(&o);
                        SetAttributes::Agent {
                            namespace,
                            id,
                            attributes,
                        }
                    } else {
                        let id = operation_activity(&o).unwrap();
                        SetAttributes::Activity {
                            namespace,
                            id,
                            attributes,
                        }
                    }
                };

                Ok(ChronicleOperation::SetAttributes(actor))
            } else {
                unreachable!()
            }
        } else {
            Err(ProcessorError::NotANode {})
        }
    }
}

fn operation_namespace(o: &Node) -> NamespaceId {
    let mut uuid_objects = o.get(&Reference::Id(
        ChronicleOperations::NamespaceUuid.as_iri().into(),
    ));
    let uuid = uuid_objects.next().unwrap().as_str().unwrap();
    let mut name_objects = o.get(&Reference::Id(
        ChronicleOperations::NamespaceName.as_iri().into(),
    ));
    let name = name_objects.next().unwrap().as_str().unwrap();
    let uuid = uuid::Uuid::parse_str(uuid).unwrap();
    NamespaceId::from_name(name, uuid)
}

fn operation_agent(o: &Node) -> AgentId {
    let mut name_objects = o.get(&Reference::Id(
        ChronicleOperations::AgentName.as_iri().into(),
    ));
    let name = name_objects.next().unwrap().as_str().unwrap();
    AgentId::from_name(name)
}

fn operation_delegate(o: &Node) -> AgentId {
    let mut name_objects = o.get(&Reference::Id(
        ChronicleOperations::DelegateId.as_iri().into(),
    ));
    let name = name_objects.next().unwrap().as_str().unwrap();
    AgentId::from_name(name)
}

fn operation_activity(o: &Node) -> Option<ActivityId> {
    let mut name_objects = o.get(&Reference::Id(
        ChronicleOperations::ActivityName.as_iri().into(),
    ));
    let object = match name_objects.next() {
        Some(object) => object,
        None => return None,
    };
    Some(ActivityId::from_name(object.as_str().unwrap()))
}

fn operation_key(o: &Node) -> String {
    let mut objects = o.get(&Reference::Id(
        ChronicleOperations::PublicKey.as_iri().into(),
    ));
    String::from(objects.next().unwrap().as_str().unwrap())
}

fn operation_start_time(o: &Node) -> String {
    let mut objects = o.get(&Reference::Id(
        ChronicleOperations::StartActivityTime.as_iri().into(),
    ));
    let time = objects.next().unwrap().as_str().unwrap();
    eprint!("time!: {}", time);
    time.to_owned()
}

fn operation_end_time(o: &Node) -> String {
    let mut objects = o.get(&Reference::Id(
        ChronicleOperations::EndActivityTime.as_iri().into(),
    ));
    let time = objects.next().unwrap().as_str().unwrap();
    eprint!("time!: {}", time);
    time.to_owned()
}

fn operation_entity(o: &Node) -> EntityId {
    let mut name_objects = o.get(&Reference::Id(
        ChronicleOperations::EntityName.as_iri().into(),
    ));
    let name = name_objects.next().unwrap().as_str().unwrap();
    EntityId::from_name(name)
}

fn operation_used_entity(o: &Node) -> EntityId {
    let mut name_objects = o.get(&Reference::Id(
        ChronicleOperations::UsedEntityName.as_iri().into(),
    ));
    let name = name_objects.next().unwrap().as_str().unwrap();
    EntityId::from_name(name)
}

fn operation_derivation(o: &Node) -> Option<DerivationType> {
    let mut objects = o.get(&Reference::Id(
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

fn operation_domain(o: &Node) -> Option<DomaintypeId> {
    let mut objects = o.get(&Reference::Id(
        ChronicleOperations::DomaintypeId.as_iri().into(),
    ));
    let d = match objects.next() {
        Some(object) => object.as_str().unwrap(),
        None => return None,
    };
    Some(DomaintypeId::from_name(d))
}

fn operation_attributes(o: &Node) -> BTreeMap<String, Attribute> {
    let objects = o.get(&Reference::Id(
        ChronicleOperations::Attributes.as_iri().into(),
    ));
    let mut a: BTreeMap<String, Attribute> = BTreeMap::new();
    for o in objects {
        let j = o.as_json();
        let x = j["@type"][0].to_string();
        let value = serde_json::json!(x);
        let attr = Attribute {
            typ: x.clone(),
            value,
        };
        a.insert(format!("{}_attribute", x.to_lowercase()), attr);
    }
    a
}

#[cfg(test)]
mod test {
    use std::collections::BTreeMap;

    use uuid::Uuid;

    use crate::{
        attributes::{Attribute, Attributes},
        prov::{
            operations::{
                ActivityUses, ActsOnBehalfOf, ChronicleOperation, CreateActivity, CreateAgent,
                CreateEntity, CreateNamespace, DerivationType, EntityAttach, EntityDerive,
                GenerateEntity, RegisterKey, SetAttributes, StartActivity,
            },
            to_json_ld::ToJson,
            ActivityId, AgentId, DomaintypeId, EntityId, Name, NamePart, NamespaceId,
            ProcessorError,
        },
    };

    #[tokio::test]
    async fn test_set_attributes_activity_multiple_attributes() -> Result<(), ProcessorError> {
        let uuid =
            Uuid::parse_str("a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8").map_err(|e| eprintln!("{}", e));
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid.unwrap());
        let id = ActivityId::from_name("test_activity");
        let domain = DomaintypeId::from_name("test_domain");

        let attrs = {
            let mut h: BTreeMap<String, Attribute> = BTreeMap::new();

            let attr = Attribute {
                typ: "Bool".to_string(),
                value: serde_json::json!("Bool"),
            };
            h.insert("bool_attribute".to_string(), attr);

            let attr = Attribute {
                typ: "String".to_string(),
                value: serde_json::json!("String"),
            };
            h.insert("string_attribute".to_string(), attr);

            let attr = Attribute {
                typ: "Int".to_string(),
                value: serde_json::json!("Int"),
            };
            h.insert("int_attribute".to_string(), attr);

            h
        };
        let attributes = Attributes {
            typ: Some(domain),
            attributes: attrs,
        };
        let operation: ChronicleOperation =
            ChronicleOperation::SetAttributes(SetAttributes::Activity {
                namespace,
                id,
                attributes,
            });

        let serialized_operation = operation.to_json();

        let deserialized_operation = ChronicleOperation::from_json(serialized_operation).await?;

        assert!(
            ChronicleOperation::from_json(deserialized_operation.to_json()).await?
                == deserialized_operation
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_set_attributes_agent_empty_attributes() -> Result<(), ProcessorError> {
        let uuid =
            Uuid::parse_str("a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8").map_err(|e| eprintln!("{}", e));
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid.unwrap());
        let id = AgentId::from_name("test_agent");
        let domain = DomaintypeId::from_name("test_domain");
        let attributes = Attributes {
            typ: Some(domain),
            attributes: BTreeMap::new(),
        };
        let operation: ChronicleOperation =
            ChronicleOperation::SetAttributes(SetAttributes::Agent {
                namespace,
                id,
                attributes,
            });

        let serialized_operation = operation.to_json();
        let deserialized_operation = ChronicleOperation::from_json(serialized_operation).await?;
        assert!(
            ChronicleOperation::from_json(deserialized_operation.to_json()).await?
                == deserialized_operation
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_set_attributes_entity_empty_attributes() -> Result<(), ProcessorError> {
        let uuid =
            Uuid::parse_str("a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8").map_err(|e| eprintln!("{}", e));
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid.unwrap());
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

        let serialized_operation = operation.to_json();
        let deserialized_operation = ChronicleOperation::from_json(serialized_operation).await?;
        assert!(
            ChronicleOperation::from_json(deserialized_operation.to_json()).await?
                == deserialized_operation
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_entity_derive() -> Result<(), ProcessorError> {
        let uuid =
            Uuid::parse_str("a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8").map_err(|e| eprintln!("{}", e));
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid.unwrap());
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

        let serialized_operation = operation.to_json();
        let deserialized_operation = ChronicleOperation::from_json(serialized_operation).await?;
        assert!(
            ChronicleOperation::from_json(deserialized_operation.to_json()).await?
                == deserialized_operation
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_entity_attach() -> Result<(), ProcessorError> {
        let uuid =
            Uuid::parse_str("a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8").map_err(|e| eprintln!("{}", e));
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid.unwrap());

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

        let serialized_operation = operation.to_json();
        let deserialized_operation = ChronicleOperation::from_json(serialized_operation).await?;
        assert!(
            ChronicleOperation::from_json(deserialized_operation.to_json()).await?
                == deserialized_operation
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_generate_entity() -> Result<(), ProcessorError> {
        let uuid =
            Uuid::parse_str("a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8").map_err(|e| eprintln!("{}", e));
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid.unwrap());

        let id = EntityId::from_name("test_entity");
        let activity = ActivityId::from_name("test_activity");
        let operation: ChronicleOperation =
            super::ChronicleOperation::GenerateEntity(GenerateEntity {
                namespace,
                id,
                activity,
            });

        let serialized_operation = operation.to_json();
        let deserialized_operation = ChronicleOperation::from_json(serialized_operation).await?;
        assert!(
            ChronicleOperation::from_json(deserialized_operation.to_json()).await?
                == deserialized_operation
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_create_entity() -> Result<(), ProcessorError> {
        let uuid =
            Uuid::parse_str("a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8").map_err(|e| eprintln!("{}", e));
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid.unwrap());

        let id = NamePart::name_part(&EntityId::from_name("test_entity")).to_owned();
        let operation: ChronicleOperation = ChronicleOperation::CreateEntity(CreateEntity {
            namespace,
            name: id,
        });

        let serialized_operation = operation.to_json();
        let deserialized_operation = ChronicleOperation::from_json(serialized_operation).await?;
        assert!(
            ChronicleOperation::from_json(deserialized_operation.to_json()).await?
                == deserialized_operation
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_activity_uses() -> Result<(), ProcessorError> {
        let uuid =
            Uuid::parse_str("a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8").map_err(|e| eprintln!("{}", e));
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid.unwrap());

        let id = EntityId::from_name("test_entity");
        let activity = ActivityId::from_name("test_activity");
        let operation: ChronicleOperation = ChronicleOperation::ActivityUses(ActivityUses {
            namespace,
            id,
            activity,
        });

        let serialized_operation = operation.to_json();
        let deserialized_operation = ChronicleOperation::from_json(serialized_operation).await?;
        assert!(
            ChronicleOperation::from_json(deserialized_operation.to_json()).await?
                == deserialized_operation
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_end_activity() -> Result<(), ProcessorError> {
        let uuid =
            Uuid::parse_str("a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8").map_err(|e| eprintln!("{}", e));
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid.unwrap());

        let id = ActivityId::from_name("test_activity");
        let agent = crate::prov::AgentId::from_name("test_agent");
        let time = chrono::DateTime::<chrono::Utc>::from_utc(
            chrono::NaiveDateTime::from_timestamp(61, 0),
            chrono::Utc,
        );
        let operation: ChronicleOperation =
            super::ChronicleOperation::EndActivity(crate::prov::operations::EndActivity {
                namespace,
                id,
                agent,
                time,
            });

        let serialized_operation = operation.to_json();
        let deserialized_operation = ChronicleOperation::from_json(serialized_operation).await?;
        assert!(
            ChronicleOperation::from_json(deserialized_operation.to_json()).await?
                == deserialized_operation
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_start_activity() -> Result<(), ProcessorError> {
        let uuid =
            Uuid::parse_str("a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8").map_err(|e| eprintln!("{}", e));
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid.unwrap());

        let id = ActivityId::from_name("test_activity");
        let agent = crate::prov::AgentId::from_name("test_agent");
        let time = chrono::DateTime::<chrono::Utc>::from_utc(
            chrono::NaiveDateTime::from_timestamp(61, 0),
            chrono::Utc,
        );
        let operation: ChronicleOperation = ChronicleOperation::StartActivity(StartActivity {
            namespace,
            id,
            agent,
            time,
        });

        let serialized_operation = operation.to_json();
        let deserialized_operation = ChronicleOperation::from_json(serialized_operation).await?;
        assert!(
            ChronicleOperation::from_json(deserialized_operation.to_json()).await?
                == deserialized_operation
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_create_activity() -> Result<(), ProcessorError> {
        let uuid =
            Uuid::parse_str("a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8").map_err(|e| eprintln!("{}", e));
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid.unwrap());
        let name = NamePart::name_part(&ActivityId::from_name("test_activity")).to_owned();

        let operation: ChronicleOperation =
            ChronicleOperation::CreateActivity(CreateActivity { namespace, name });

        let serialized_operation = operation.to_json();
        let deserialized_operation = ChronicleOperation::from_json(serialized_operation).await?;
        assert!(
            ChronicleOperation::from_json(deserialized_operation.to_json()).await?
                == deserialized_operation
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_register_key() -> Result<(), ProcessorError> {
        let uuid =
            Uuid::parse_str("a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8").map_err(|e| eprintln!("{}", e));
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid.unwrap());
        let id = AgentId::from_name("test_agent");
        let publickey =
            "02197db854d8c6a488d4a0ef3ef1fcb0c06d66478fae9e87a237172cf6f6f7de23".to_string();
        let operation: ChronicleOperation = ChronicleOperation::RegisterKey(RegisterKey {
            namespace,
            id,
            publickey,
        });

        let serialized_operation = operation.to_json();
        let deserialized_operation = ChronicleOperation::from_json(serialized_operation).await?;
        assert!(
            ChronicleOperation::from_json(deserialized_operation.to_json()).await?
                == deserialized_operation
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_create_agent_acts_on_behalf_of_no_activity() -> Result<(), ProcessorError> {
        let uuid =
            Uuid::parse_str("a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8").map_err(|e| eprintln!("{}", e));
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid.unwrap());
        let id = AgentId::from_name("test_agent");
        let delegate_id = AgentId::from_name("test_delegate");
        let activity_id = None;

        let operation: ChronicleOperation =
            ChronicleOperation::AgentActsOnBehalfOf(ActsOnBehalfOf {
                namespace,
                id,
                delegate_id,
                activity_id,
            });

        let serialized_operation = operation.to_json();
        let deserialized_operation = ChronicleOperation::from_json(serialized_operation).await?;
        assert!(
            ChronicleOperation::from_json(deserialized_operation.to_json()).await?
                == deserialized_operation
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_create_agent_acts_on_behalf_of() -> Result<(), ProcessorError> {
        let uuid =
            Uuid::parse_str("a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8").map_err(|e| eprintln!("{}", e));
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid.unwrap());
        let id = AgentId::from_name("test_agent");
        let delegate_id = AgentId::from_name("test_delegate");
        let activity_id = Some(ActivityId::from_name("test_activity"));

        let operation: ChronicleOperation =
            ChronicleOperation::AgentActsOnBehalfOf(ActsOnBehalfOf {
                namespace,
                id,
                delegate_id,
                activity_id,
            });

        let serialized_operation = operation.to_json();
        let deserialized_operation = ChronicleOperation::from_json(serialized_operation).await?;
        assert!(
            ChronicleOperation::from_json(deserialized_operation.to_json()).await?
                == deserialized_operation
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_create_agent_from_json() -> Result<(), ProcessorError> {
        let uuid =
            Uuid::parse_str("a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8").map_err(|e| eprintln!("{}", e));
        let namespace: NamespaceId = NamespaceId::from_name("testns", uuid.unwrap());
        let name: Name = NamePart::name_part(&AgentId::from_name("test_agent")).clone();
        let operation: ChronicleOperation =
            ChronicleOperation::CreateAgent(CreateAgent { namespace, name });
        let serialized_operation = operation.to_json();
        let deserialized_operation = ChronicleOperation::from_json(serialized_operation).await?;
        assert!(
            ChronicleOperation::from_json(deserialized_operation.to_json()).await?
                == deserialized_operation
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_create_namespace_from_json() -> Result<(), ProcessorError> {
        let name = "testns";
        let uuid =
            Uuid::parse_str("a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8").map_err(|e| eprintln!("{}", e));
        let id = NamespaceId::from_name(name, uuid.unwrap());

        let operation =
            ChronicleOperation::CreateNamespace(CreateNamespace::new(id, name, uuid.unwrap()));
        let serialized_operation = operation.to_json();
        let deserialized_operation = ChronicleOperation::from_json(serialized_operation).await?;
        assert!(
            ChronicleOperation::from_json(deserialized_operation.to_json()).await?
                == deserialized_operation
        );
        Ok(())
    }
}
