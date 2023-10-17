use chrono::Utc;
use proptest::{option, prelude::*};

use uuid::Uuid;

use crate::{
    attributes::{Attribute, Attributes},
    prov::{
        operations::*, to_json_ld::ToJson, ActivityId, AgentId, Association, AssociationId,
        Attribution, Contradiction, Delegation, DelegationId, Derivation, DomaintypeId, EntityId,
        ExternalId, ExternalIdPart, Generation, NamespaceId, ProvModel, Role, Usage, UuidPart,
    },
};

use super::{ActivityUses, ActsOnBehalfOf, EntityDerive, StartActivity};

prop_compose! {
    fn an_external_id()(external_id in "[A-Za-z]") -> ExternalId {
        ExternalId::from(external_id)
    }
}

prop_compose! {
    fn a_symbol()(external_id in "[A-Za-z]") -> String {
        external_id
    }
}

// Choose from a limited selection of types so that we get multiple references
prop_compose! {
    fn typ()(names in prop::collection::vec(a_symbol(), 5), index in (0..5usize)) -> String {
        names.get(index).unwrap().to_owned()
    }
}

// Choose from a limited selection of names so that we get multiple references
prop_compose! {
    fn external_id()(external_ids in prop::collection::vec(an_external_id(), 5), index in (0..5usize)) -> ExternalId {
        external_ids.get(index).unwrap().to_owned()
    }
}

// Choose from a limited selection of domain types
prop_compose! {
    fn domain_type_id()(names in prop::collection::vec(a_symbol(), 5), index in (0..5usize)) -> DomaintypeId {
        DomaintypeId::from_external_id(&ExternalId::from(names.get(index).unwrap()))
    }
}

prop_compose! {
    fn a_namespace()
        (uuid in prop::collection::vec(0..255u8, 16),
         external_id in external_id()) -> NamespaceId {

        NamespaceId::from_external_id(&external_id,Uuid::from_bytes(uuid.as_slice().try_into().unwrap()))
    }
}

// Choose from a limited selection of namespaces so that we get multiple references
prop_compose! {
    fn namespace()(namespaces in prop::collection::vec(a_namespace(), 1), index in (0..1usize)) -> NamespaceId {
        namespaces.get(index).unwrap().to_owned()
    }
}

prop_compose! {
    fn create_namespace()(id in namespace()) -> CreateNamespace {
        let (external_id,uuid) = (id.external_id_part(), id.uuid_part());
        CreateNamespace {
            id: id.clone(),
            uuid: *uuid,
            external_id: external_id.to_owned(),
        }
    }
}

prop_compose! {
    fn create_agent() (external_id in external_id(),namespace in namespace()) -> AgentExists {
        let _id = AgentId::from_external_id(&external_id);
        AgentExists {
            namespace,
            external_id,
        }
    }
}

prop_compose! {
    fn create_activity() (external_id in external_id(),namespace in namespace()) -> ActivityExists {
        ActivityExists {
            namespace,
            external_id,
        }
    }
}

// Create times for start between 2-1 years in the past, to ensure start <= end
prop_compose! {
    fn start_activity() (external_id in external_id(),namespace in namespace(), offset in (0..10)) -> StartActivity {
        let id = ActivityId::from_external_id(&external_id);

        let today = Utc::now().date_naive().and_hms_micro_opt(0, 0, 0, 0).unwrap().and_local_timezone(Utc).unwrap();

        StartActivity {
            namespace,
            id,
            time: today - chrono::Duration::days(offset as _)
        }
    }
}

// Create times for start between 2-1 years in the past, to ensure start <= end
prop_compose! {
    fn end_activity() (external_id in external_id(),namespace in namespace(), offset in (0..10)) -> EndActivity {
        let id = ActivityId::from_external_id(&external_id);

        let today = Utc::now().date_naive().and_hms_micro_opt(0, 0, 0, 0).unwrap().and_local_timezone(Utc).unwrap();

        EndActivity {
            namespace,
            id,
            time: today - chrono::Duration::days(offset as _)
        }
    }
}

prop_compose! {
    fn used() (activity_name in external_id(), entity_name in external_id(),namespace in namespace()) -> ActivityUses {
        let activity = ActivityId::from_external_id(&activity_name);
        let id = EntityId::from_external_id(&entity_name);

        ActivityUses {
            namespace,
            id,
            activity
        }
    }
}

prop_compose! {
    fn create_entity() (external_id in external_id(),namespace in namespace()) -> EntityExists {
        EntityExists {
            namespace,
            external_id,
        }
    }
}

prop_compose! {
    fn entity_derive() (
        external_id in external_id(),
        used in external_id(),
        namespace in namespace(),
    ) -> EntityDerive {
        let id = EntityId::from_external_id(&external_id);
        let used_id = EntityId::from_external_id(&used);

        EntityDerive {
            namespace,
            id,
            used_id,
            activity_id: None,
            typ: DerivationType::None
        }
    }
}

prop_compose! {
    fn attribute() (
        typ in typ(),
    ) -> Attribute{

        Attribute {
            typ,
            value: serde_json::Value::String("data".to_owned()),
        }
    }
}

prop_compose! {
    fn attributes() (
        attributes in prop::collection::vec(attribute(), 5),
        typ in domain_type_id(),
    ) -> Attributes {

        Attributes {
            typ: Some(typ),
            attributes: attributes.into_iter().map(|a| (a.typ.clone(), a)).collect(),
        }
    }
}

prop_compose! {
    fn acted_on_behalf_of() (
        external_id in external_id(),
        activity in option::of(external_id()),
        role in option::of(external_id()),
        delegate in external_id(),
        namespace in namespace(),
    ) -> ActsOnBehalfOf {

        let responsible_id = AgentId::from_external_id(&external_id);
        let delegate_id = AgentId::from_external_id(&delegate);
        let activity_id = activity.map(|a| ActivityId::from_external_id(&a));
        let id = DelegationId::from_component_ids(&delegate_id, &responsible_id, activity_id.as_ref(), role.as_ref().map(|x| x.as_str()));

        ActsOnBehalfOf {
            id,
            responsible_id,
            delegate_id,
            role: role.as_ref().map(|x| Role::from(x.as_str())),
            activity_id,
            namespace,
        }

    }
}

prop_compose! {
    fn was_associated_with() (
        activity in external_id(),
        role in option::of(external_id()),
        agent in external_id(),
        namespace in namespace(),
    ) -> WasAssociatedWith {

        let agent_id = AgentId::from_external_id(&agent);
        let activity_id = ActivityId::from_external_id(&activity);
        let id = AssociationId::from_component_ids(&agent_id, &activity_id,  role.as_ref().map(|x| x.as_str()));

        WasAssociatedWith{id,agent_id,activity_id,role:role.as_ref().map(Role::from), namespace }

    }
}

prop_compose! {
    fn was_informed_by() (
        // we probably should disallow reflexivity for `wasInformedBy`
        activity1 in external_id(),
        activity2 in external_id(),
        namespace in namespace(),
    ) -> WasInformedBy {

        WasInformedBy{
            namespace,
            activity: ActivityId::from_external_id(&activity1),
            informing_activity: ActivityId::from_external_id(&activity2),
        }
    }
}

prop_compose! {
    fn entity_attributes() (
        external_id in external_id(),
        namespace in namespace(),
        attributes in attributes(),
    ) -> SetAttributes {

        SetAttributes::Entity{
                id: EntityId::from_external_id(&external_id),
                namespace,
                attributes,
        }
    }
}

prop_compose! {
    fn agent_attributes() (
        external_id in external_id(),
        namespace in namespace(),
        attributes in attributes(),
    ) -> SetAttributes {
        SetAttributes::Agent {
                id: AgentId::from_external_id(&external_id),
                namespace,
                attributes,
        }
    }
}
prop_compose! {
    fn activity_attributes() (
        external_id in external_id(),
        namespace in namespace(),
        attributes in attributes(),
    ) -> SetAttributes {
        SetAttributes::Activity{
                id: ActivityId::from_external_id(&external_id),
                namespace,
                attributes,
        }
    }
}

fn transaction() -> impl Strategy<Value = ChronicleOperation> {
    prop_oneof![
        1 => create_agent().prop_map(ChronicleOperation::AgentExists),
        1 => create_activity().prop_map(ChronicleOperation::ActivityExists),
        1 => start_activity().prop_map(ChronicleOperation::StartActivity),
        1 => end_activity().prop_map(ChronicleOperation::EndActivity),
        1 => used().prop_map(ChronicleOperation::ActivityUses),
        1 => create_entity().prop_map(ChronicleOperation::EntityExists),
        1 => entity_derive().prop_map(ChronicleOperation::EntityDerive),
        1 => acted_on_behalf_of().prop_map(ChronicleOperation::AgentActsOnBehalfOf),
        1 => was_associated_with().prop_map(ChronicleOperation::WasAssociatedWith),
        1 => was_informed_by().prop_map(ChronicleOperation::WasInformedBy),
        1 => entity_attributes().prop_map(ChronicleOperation::SetAttributes),
        1 => activity_attributes().prop_map(ChronicleOperation::SetAttributes),
        1 => agent_attributes().prop_map(ChronicleOperation::SetAttributes),
    ]
}

fn operation_seq() -> impl Strategy<Value = Vec<ChronicleOperation>> {
    proptest::collection::vec(transaction(), 1..50)
}

fn compact_json(prov: &ProvModel) -> serde_json::Value {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async move { prov.to_json().compact().await })
        .unwrap()
}

fn prov_from_json_ld(json: serde_json::Value) -> ProvModel {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async move {
        let mut prov = ProvModel::default();
        prov.apply_json_ld(json).await.unwrap();
        prov
    })
}

proptest! {
   #![proptest_config(ProptestConfig {
        max_shrink_iters: std::u32::MAX, verbose: 0, .. ProptestConfig::default()
    })]
    #[test]
    fn operations(operations in operation_seq()) {
        let mut prov = ProvModel::default();

        let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap();


        // Keep track of the operations that were applied successfully
        let mut applied_operations = vec![];
        // If we encounter a contradiction, store it along with the operation
        // that caused it. prov will be in the final state before the contradiction
        let mut contradiction: Option<(ChronicleOperation,Contradiction)> = None;

        // Apply each operation in order, stopping if any fail
        for op in operations.iter() {
            // Check that serialization of operation is symmetric
            let op_json = op.to_json().0;
            prop_assert_eq!(op,
                &rt.block_on(ChronicleOperation::from_json(&op.to_json().0)).unwrap(),
                "Serialized operation {}", serde_json::to_string_pretty(&op_json).unwrap());

            let res = prov.apply(op);
            if let Err(raised) = res {
                contradiction = Some((op.clone(), raised));
                break;
            } else {
                applied_operations.push(op.clone());
            }
        }

        // If we encountered a contradiction, check it is consistent with the
        // operation

        if let Some((_op,Contradiction {id: _,namespace: _,contradiction})) = contradiction {
          let _contradiction = contradiction.get(0).unwrap();
        }

        // Now assert that the final prov object matches what we would expect from the input operations
        for op in applied_operations.iter() {
            match op {
                ChronicleOperation::CreateNamespace(CreateNamespace{id,external_id,uuid}) => {
                    prop_assert!(prov.namespaces.contains_key(id));
                    let ns = prov.namespaces.get(id).unwrap();
                    prop_assert_eq!(&ns.id, id);
                    prop_assert_eq!(&ns.external_id, external_id);
                    prop_assert_eq!(&ns.uuid, uuid);
                },
                ChronicleOperation::AgentExists(
                    AgentExists { namespace, external_id}) => {
                    let agent = &prov.agents.get(&(namespace.to_owned(),AgentId::from_external_id(external_id)));
                    prop_assert!(agent.is_some());
                    let agent = agent.unwrap();
                    prop_assert_eq!(&agent.external_id, external_id);
                    prop_assert_eq!(&agent.namespaceid, namespace);
                },
                ChronicleOperation::AgentActsOnBehalfOf(
                    ActsOnBehalfOf {namespace,id: _,delegate_id,activity_id, role, responsible_id }
                ) => {
                    let agent = &prov.agents.get(&(namespace.to_owned(),responsible_id.to_owned()));
                    prop_assert!(agent.is_some());
                    let agent = agent.unwrap();

                    let delegate = &prov.agents.get(&(namespace.to_owned(),delegate_id.to_owned()));
                    prop_assert!(delegate.is_some());
                    let delegate = delegate.unwrap();

                    if let Some(activity_id) = activity_id {
                        let activity = &prov.activities.get(&(namespace.to_owned(),activity_id.to_owned()));
                        prop_assert!(activity.is_some());
                    }

                    let has_delegation = prov.delegation.get(&(namespace.to_owned(),responsible_id.to_owned()))
                        .unwrap()
                        .contains(&Delegation::new(
                            namespace,
                            &delegate.id,
                            &agent.id,
                            activity_id.as_ref(),
                            role.clone()
                        ));

                    prop_assert!(has_delegation);

                }
                ChronicleOperation::ActivityExists(
                    ActivityExists { namespace,  external_id }) => {
                    let activity = &prov.activities.get(&(namespace.clone(),ActivityId::from_external_id(external_id)));
                    prop_assert!(activity.is_some());
                    let activity = activity.unwrap();
                    prop_assert_eq!(&activity.external_id, external_id);
                    prop_assert_eq!(&activity.namespaceid, namespace);
                },
                ChronicleOperation::StartActivity(
                    StartActivity { namespace, id, time }) =>  {
                    let activity = &prov.activities.get(&(namespace.clone(),id.clone()));
                    prop_assert!(activity.is_some());
                    let activity = activity.unwrap();
                    prop_assert_eq!(&activity.external_id, id.external_id_part());
                    prop_assert_eq!(&activity.namespaceid, namespace);

                    prop_assert!(activity.started == Some(time.to_owned()));
                },
                ChronicleOperation::EndActivity(
                    EndActivity { namespace, id, time }) => {
                    let activity = &prov.activities.get(&(namespace.to_owned(),id.to_owned()));
                    prop_assert!(activity.is_some());
                    let activity = activity.unwrap();
                    prop_assert_eq!(&activity.external_id, id.external_id_part());
                    prop_assert_eq!(&activity.namespaceid, namespace);

                    prop_assert!(activity.ended == Some(time.to_owned()));
                }
                ChronicleOperation::WasAssociatedWith(WasAssociatedWith { id : _, role, namespace, activity_id, agent_id }) => {
                    let has_asoc = prov.association.get(&(namespace.to_owned(), activity_id.to_owned()))
                        .unwrap()
                        .contains(&Association::new(
                            namespace,
                            agent_id,
                            activity_id,
                            role.clone())
                        );

                    prop_assert!(has_asoc);
                }
                ChronicleOperation::WasAttributedTo(WasAttributedTo { id : _, role, namespace, entity_id, agent_id }) => {
                    let has_attribution = prov.attribution.get(&(namespace.to_owned(), entity_id.to_owned()))
                        .unwrap()
                        .contains(&Attribution::new(
                            namespace,
                            agent_id,
                            entity_id,
                            role.clone())
                        );

                    prop_assert!(has_attribution);
                }
                ChronicleOperation::ActivityUses(
                    ActivityUses { namespace, id, activity }) => {
                    let activity_id = activity;
                    let entity = &prov.entities.get(&(namespace.to_owned(),id.to_owned()));
                    prop_assert!(entity.is_some());
                    let entity = entity.unwrap();
                    prop_assert_eq!(&entity.external_id, id.external_id_part());
                    prop_assert_eq!(&entity.namespaceid, namespace);

                    let activity = &prov.activities.get(&(namespace.to_owned(),activity_id.to_owned()));
                    prop_assert!(activity.is_some());
                    let activity = activity.unwrap();
                    prop_assert_eq!(&activity.external_id, activity_id.external_id_part());
                    prop_assert_eq!(&activity.namespaceid, namespace);

                    let has_usage = prov.usage.get(&(namespace.to_owned(), activity_id.to_owned()))
                        .unwrap()
                        .contains(&Usage {
                            activity_id: activity_id.clone(),
                            entity_id: id.clone(),
                        });

                    prop_assert!(has_usage);
                },
                ChronicleOperation::EntityExists(
                    EntityExists { namespace, external_id}) => {
                    let entity = &prov.entities.get(&(namespace.to_owned(),EntityId::from_external_id(external_id)));
                    prop_assert!(entity.is_some());
                    let entity = entity.unwrap();
                    prop_assert_eq!(&entity.external_id, external_id);
                    prop_assert_eq!(&entity.namespaceid, namespace);
                },
                ChronicleOperation::WasGeneratedBy(WasGeneratedBy{namespace, id, activity}) => {
                    let activity_id = activity;
                    let entity = &prov.entities.get(&(namespace.to_owned(),id.to_owned()));
                    prop_assert!(entity.is_some());
                    let entity = entity.unwrap();
                    prop_assert_eq!(&entity.external_id, id.external_id_part());
                    prop_assert_eq!(&entity.namespaceid, namespace);

                    let activity = &prov.activities.get(&(namespace.to_owned(),activity.to_owned()));
                    prop_assert!(activity.is_some());
                    let activity = activity.unwrap();
                    prop_assert_eq!(&activity.external_id, activity_id.external_id_part());
                    prop_assert_eq!(&activity.namespaceid, namespace);

                    let has_generation = prov.generation.get(
                        &(namespace.clone(),id.clone()))
                        .unwrap()
                        .contains(& Generation {
                            activity_id: activity_id.clone(),
                            generated_id: id.clone(),
                        });

                    prop_assert!(has_generation);
                }
                ChronicleOperation::WasInformedBy(WasInformedBy{namespace, activity, informing_activity}) => {
                    let informed_activity = &prov.activities.get(&(namespace.to_owned(), activity.to_owned()));
                    prop_assert!(informed_activity.is_some());
                    let informed_activity = informed_activity.unwrap();
                    prop_assert_eq!(&informed_activity.external_id, activity.external_id_part());
                    prop_assert_eq!(&informed_activity.namespaceid, namespace);

                    let was_informed_by = prov.was_informed_by.get(
                        &(namespace.clone(), activity.clone()))
                        .unwrap()
                        .contains(&(namespace.to_owned(), informing_activity.to_owned()));

                    prop_assert!(was_informed_by);
                },
                ChronicleOperation::EntityDerive(EntityDerive {
                  namespace,
                  id,
                  used_id,
                  activity_id,
                  typ,
                }) => {
                    let generated_entity = &prov.entities.get(&(namespace.to_owned(),id.to_owned()));
                    prop_assert!(generated_entity.is_some());

                    let used_entity = &prov.entities.get(&(namespace.to_owned(),used_id.to_owned()));
                    prop_assert!(used_entity.is_some());

                    let has_derivation = prov.derivation.get(
                        &(namespace.clone(),id.clone()))
                        .unwrap()
                        .contains(& Derivation {

                            used_id: used_id.clone(),
                            activity_id: activity_id.clone(),
                            generated_id: id.clone(),
                            typ: *typ
                    });

                    prop_assert!(has_derivation);
                }
                ChronicleOperation::SetAttributes(
                    SetAttributes::Entity  { namespace, id, attributes}) => {
                    let entity = &prov.entities.get(&(namespace.to_owned(),id.to_owned()));
                    prop_assert!(entity.is_some());
                    let entity = entity.unwrap();

                    prop_assert_eq!(&entity.domaintypeid, &attributes.typ);
                },
                ChronicleOperation::SetAttributes(SetAttributes::Activity{ namespace, id, attributes}) => {
                    let activity = &prov.activities.get(&(namespace.to_owned(),id.to_owned()));
                    prop_assert!(activity.is_some());
                    let activity = activity.unwrap();

                    prop_assert_eq!(&activity.domaintypeid, &attributes.typ);
                },
                ChronicleOperation::SetAttributes(SetAttributes::Agent { namespace, id, attributes}) => {
                    let agent = &prov.agents.get(&(namespace.to_owned(),id.to_owned()));
                    prop_assert!(agent.is_some());
                    let agent = agent.unwrap();

                    prop_assert_eq!(&agent.domaintypeid, &attributes.typ);
                },
            }
        }

        // Test that serialisation to and from JSON-LD is symmetric
        let lhs_json_expanded = prov.to_json().0;

        let lhs_json = compact_json(&prov);

        let serialized_prov = prov_from_json_ld(lhs_json.clone());


        prop_assert_eq!(&prov, &serialized_prov, "Prov reserialisation compact: \n{} expanded \n {}",
            serde_json::to_string_pretty(&lhs_json).unwrap(), serde_json::to_string_pretty(&lhs_json_expanded).unwrap());

        // Test that serialisation to JSON-LD is deterministic
        for _ in 0..10 {
            let lhs_json_2 = compact_json(&prov).clone();
            prop_assert_eq!( lhs_json.clone().to_string(), lhs_json_2.to_string());
        }
    }
}
