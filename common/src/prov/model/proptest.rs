use chrono::Utc;
use json::JsonValue;
use proptest::{option, prelude::*};

use uuid::Uuid;

use crate::{
    attributes::{Attribute, Attributes},
    prov::{
        operations::*, to_json_ld::ToJson, ActivityId, AgentId, Association, AssociationId,
        Delegation, DelegationId, Derivation, DomaintypeId, EntityId, Generation, IdentityId, Name,
        NamePart, NamespaceId, ProvModel, Role, Useage, UuidPart,
    },
};

use super::{
    ActivityUses, ActsOnBehalfOf, CompactedJson, EntityDerive, EntityHasEvidence, StartActivity,
};

prop_compose! {
    fn a_name()(name in ".*") -> Name {
        Name::from(name)
    }
}

prop_compose! {
    fn a_symbol()(name in "[A-Za-z]") -> String {
        name
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
    fn name()(names in prop::collection::vec(a_name(), 5), index in (0..5usize)) -> Name {
        names.get(index).unwrap().to_owned()
    }
}

// Choose from a limited selection of domain types
prop_compose! {
    fn domain_type_id()(names in prop::collection::vec(a_symbol(), 5), index in (0..5usize)) -> DomaintypeId {
        DomaintypeId::from_name(&Name::from(names.get(index).unwrap()))
    }
}

prop_compose! {
    fn a_namespace()
        (uuid in prop::collection::vec(0..255u8, 16),
         name in name()) -> NamespaceId {

        NamespaceId::from_name(&name,Uuid::from_bytes(uuid.as_slice().try_into().unwrap()))
    }
}

// Choose from a limited selection of namespaces so that we get multiple references
prop_compose! {
    fn namespace()(namespaces in prop::collection::vec(a_namespace(), 2), index in (0..2usize)) -> NamespaceId {
        namespaces.get(index).unwrap().to_owned()
    }
}

prop_compose! {
    fn create_namespace()(id in namespace()) -> CreateNamespace {
        let (name,uuid) = (id.name_part(), id.uuid_part());
        CreateNamespace {
            id: id.clone(),
            uuid: *uuid,
            name: name.to_owned(),
        }
    }
}

prop_compose! {
    fn create_agent() (name in name(),namespace in namespace()) -> AgentExists {
        let _id = AgentId::from_name(&name);
        AgentExists {
            namespace,
            name,
        }
    }
}

prop_compose! {
    fn register_key() (name in name(),namespace in namespace(), publickey in "[0-9a-f]{64}") -> RegisterKey {
        let id = AgentId::from_name(&name);
        RegisterKey {
            namespace,
            id,
            publickey
        }
    }
}

prop_compose! {
    fn create_activity() (name in name(),namespace in namespace()) -> ActivityExists {
        ActivityExists {
            namespace,
            name,
        }
    }
}

// Create times for start between 2-1 years in the past, to ensure start <= end
prop_compose! {
    fn start_activity() (name in name(),namespace in namespace(), offset in (0..10)) -> StartActivity {
        let id = ActivityId::from_name(&name);

        let today = Utc::today().and_hms_micro(0, 0,0,0);

        StartActivity {
            namespace,
            id,
            time: today - chrono::Duration::days(offset as _)
        }
    }
}

// Create times for start between 2-1 years in the past, to ensure start <= end
prop_compose! {
    fn end_activity() (name in name(),namespace in namespace(), offset in (0..10)) -> EndActivity {
        let id = ActivityId::from_name(&name);

        let today = Utc::today().and_hms_micro(0, 0,0,0);

        EndActivity {
            namespace,
            id,
            time: today - chrono::Duration::days(offset as _)
        }
    }
}

prop_compose! {
    fn used() (activity_name in name(), entity_name in name(),namespace in namespace()) -> ActivityUses {
        let activity = ActivityId::from_name(&activity_name);
        let id = EntityId::from_name(&entity_name);

        ActivityUses {
            namespace,
            id,
            activity
        }
    }
}

prop_compose! {
    fn create_entity() (name in name(),namespace in namespace()) -> EntityExists {
        EntityExists {
            namespace,
            name,
        }
    }
}

prop_compose! {
    fn generate_entity() (activity_name in name(), entity_name in name(),namespace in namespace()) -> WasGeneratedBy {
        let activity = ActivityId::from_name(&activity_name);
        let id = EntityId::from_name(&entity_name);


        WasGeneratedBy {
            namespace,
            id,
            activity
        }
    }
}

prop_compose! {
    fn entity_attach() (
        offset in (0..10u32),
        signature in "[0-9a-f]{64}",
        locator in proptest::option::of(any::<String>()),
        agent_name in name(),
        name in name(),
        namespace in namespace(),
        public_key in "[0-9a-f]{64}",
    ) -> EntityHasEvidence {
        let id = EntityId::from_name(&name);
        let agent: AgentId = AgentId::from_name(&agent_name);
        let identityid = IdentityId::from_name(&agent_name , &*public_key);

        let signature_time = Utc::today().and_hms_micro(offset, 0,0,0);

        EntityHasEvidence {
            namespace,
            id,
            locator,
            agent,
            signature: Some(signature),
            identityid: Some(identityid),
            signature_time: Some(signature_time),
        }
    }
}

prop_compose! {
    fn entity_derive() (
        name in name(),
        used in name(),
        namespace in namespace(),
    ) -> EntityDerive {
        let id = EntityId::from_name(&name);
        let used_id = EntityId::from_name(&used);

        EntityDerive {
            namespace,
            id,
            used_id,
            activity_id: None,
            typ: None
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
        name in name(),
        activity in option::of(name()),
        role in option::of(name()),
        delegate in name(),
        namespace in namespace(),
    ) -> ActsOnBehalfOf {

        let responsible_id = AgentId::from_name(&name);
        let delegate_id = AgentId::from_name(&delegate);
        let activity_id = activity.map(|a| ActivityId::from_name(&a));
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
        activity in name(),
        role in option::of(name()),
        agent in name(),
        namespace in namespace(),
    ) -> WasAssociatedWith {

        let agent_id = AgentId::from_name(&agent);
        let activity_id = ActivityId::from_name(&activity);
        let id = AssociationId::from_component_ids(&agent_id, &activity_id,  role.as_ref().map(|x| x.as_str()));

        WasAssociatedWith{id,agent_id,activity_id,role:role.as_ref().map(|x|Role::from(x)), namespace }

    }
}

prop_compose! {
    fn entity_attributes() (
        name in name(),
        namespace in namespace(),
        attributes in attributes(),
    ) -> SetAttributes {

        SetAttributes::Entity{
                id: EntityId::from_name(&name),
                namespace,
                attributes,
        }
    }
}

prop_compose! {
    fn agent_attributes() (
        name in name(),
        namespace in namespace(),
        attributes in attributes(),
    ) -> SetAttributes {
        SetAttributes::Agent {
                id: AgentId::from_name(&name),
                namespace,
                attributes,
        }
    }
}
prop_compose! {
    fn activity_attributes() (
        name in name(),
        namespace in namespace(),
        attributes in attributes(),
    ) -> SetAttributes {
        SetAttributes::Activity{
                id: ActivityId::from_name(&name),
                namespace,
                attributes,
        }
    }
}

fn transaction() -> impl Strategy<Value = ChronicleOperation> {
    prop_oneof![
        1 => create_namespace().prop_map(ChronicleOperation::CreateNamespace),
        2 => create_agent().prop_map(ChronicleOperation::AgentExists),
        2 => register_key().prop_map(ChronicleOperation::RegisterKey),
        4 => create_activity().prop_map(ChronicleOperation::ActivityExists),
        4 => start_activity().prop_map(ChronicleOperation::StartActivity),
        4 => end_activity().prop_map(ChronicleOperation::EndActivity),
        4 => used().prop_map(ChronicleOperation::ActivityUses),
        2 => create_entity().prop_map(ChronicleOperation::EntityExists),
        4 => generate_entity().prop_map(ChronicleOperation::WasGeneratedBy),
        2 => entity_attach().prop_map(ChronicleOperation::EntityHasEvidence),
        2 => entity_derive().prop_map(ChronicleOperation::EntityDerive),
        2 => acted_on_behalf_of().prop_map(ChronicleOperation::AgentActsOnBehalfOf),
        2 => was_associated_with().prop_map(ChronicleOperation::WasAssociatedWith),
        2 => entity_attributes().prop_map(ChronicleOperation::SetAttributes),
        2 => activity_attributes().prop_map(ChronicleOperation::SetAttributes),
        2 => agent_attributes().prop_map(ChronicleOperation::SetAttributes),
    ]
}

fn operation_seq() -> impl Strategy<Value = Vec<ChronicleOperation>> {
    proptest::collection::vec(transaction(), 1..50)
}

fn compact_json(prov: &ProvModel) -> CompactedJson {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async move { prov.to_json().compact().await })
        .unwrap()
}

fn prov_from_json_ld(json: JsonValue) -> ProvModel {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async move {
        let prov = ProvModel::default();
        prov.apply_json_ld(json).await.unwrap()
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

        // Apply each operation in order
        for op in operations.iter() {
            // Check that serialisation of operation is symmetric
            let op_json = op.to_json().0;
            prop_assert_eq!(op,
                &rt.block_on(ChronicleOperation::from_json(op.to_json())).unwrap(),
                "Serialised operation {}",json::stringify_pretty(op_json,2));

            prov.apply(op);
        }

        // Now assert that the final prov object matches what we would expect from the input operations
        for op in operations.iter() {
            match op {
                ChronicleOperation::CreateNamespace(CreateNamespace{id,name,uuid}) => {
                    prop_assert!(prov.namespaces.contains_key(id));
                    let ns = prov.namespaces.get(id).unwrap();
                    prop_assert_eq!(&ns.id, id);
                    prop_assert_eq!(&ns.name, name);
                    prop_assert_eq!(&ns.uuid, uuid);
                },
                ChronicleOperation::AgentExists(
                    AgentExists { namespace, name}) => {
                    let agent = &prov.agents.get(&(namespace.to_owned(),AgentId::from_name(name)));
                    prop_assert!(agent.is_some());
                    let agent = agent.unwrap();
                    prop_assert_eq!(&agent.name, name);
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
                ChronicleOperation::RegisterKey(
                    RegisterKey { namespace, id, publickey}) => {
                        let agent = &prov.agents.get(&(namespace.clone(),id.clone()));
                        prop_assert!(agent.is_some());
                        let agent = agent.unwrap();
                        let identity = &prov.has_identity.get(&(namespace.clone(), agent.id.clone()));
                        prop_assert!(identity.is_some());
                        let identity = identity.unwrap();
                        let identity = prov.identities.get(identity);
                        prop_assert!(identity.is_some());
                        let identity = identity.unwrap();

                        prop_assert_eq!(&agent.name, id.name_part());
                        prop_assert_eq!(&agent.namespaceid, &namespace.clone());
                        prop_assert_eq!(&identity.public_key, &publickey.clone());
                },
                ChronicleOperation::ActivityExists(
                    ActivityExists { namespace,  name }) => {
                    let activity = &prov.activities.get(&(namespace.clone(),ActivityId::from_name(name)));
                    prop_assert!(activity.is_some());
                    let activity = activity.unwrap();
                    prop_assert_eq!(&activity.name, name);
                    prop_assert_eq!(&activity.namespaceid, namespace);
                },
                ChronicleOperation::StartActivity(
                    StartActivity { namespace, id, time }) =>  {
                    let activity = &prov.activities.get(&(namespace.clone(),id.clone()));
                    prop_assert!(activity.is_some());
                    let activity = activity.unwrap();
                    prop_assert_eq!(&activity.name, id.name_part());
                    prop_assert_eq!(&activity.namespaceid, namespace);

                    prop_assert!(activity.started == Some(time.to_owned()));
                    prop_assert!(activity.ended.is_none() || activity.ended.unwrap() >= activity.started.unwrap());

                },
                ChronicleOperation::EndActivity(
                    EndActivity { namespace, id, time }) => {
                    let activity = &prov.activities.get(&(namespace.to_owned(),id.to_owned()));
                    prop_assert!(activity.is_some());
                    let activity = activity.unwrap();
                    prop_assert_eq!(&activity.name, id.name_part());
                    prop_assert_eq!(&activity.namespaceid, namespace);

                    prop_assert!(activity.ended == Some(time.to_owned()));
                    prop_assert!(activity.started.unwrap() <= *time);

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
                ChronicleOperation::ActivityUses(
                    ActivityUses { namespace, id, activity }) => {
                    let activity_id = activity;
                    let entity = &prov.entities.get(&(namespace.to_owned(),id.to_owned()));
                    prop_assert!(entity.is_some());
                    let entity = entity.unwrap();
                    prop_assert_eq!(&entity.name, id.name_part());
                    prop_assert_eq!(&entity.namespaceid, namespace);

                    let activity = &prov.activities.get(&(namespace.to_owned(),activity_id.to_owned()));
                    prop_assert!(activity.is_some());
                    let activity = activity.unwrap();
                    prop_assert_eq!(&activity.name, activity_id.name_part());
                    prop_assert_eq!(&activity.namespaceid, namespace);

                    let has_useage = prov.useage.get(&(namespace.to_owned(), activity_id.to_owned()))
                        .unwrap()
                        .contains(&Useage {
                            activity_id: activity_id.clone(),
                            entity_id: id.clone(),
                            time: None
                        });

                    prop_assert!(has_useage);
                },
                ChronicleOperation::EntityExists(
                    EntityExists { namespace, name}) => {
                    let entity = &prov.entities.get(&(namespace.to_owned(),EntityId::from_name(name)));
                    prop_assert!(entity.is_some());
                    let entity = entity.unwrap();
                    prop_assert_eq!(&entity.name, name);
                    prop_assert_eq!(&entity.namespaceid, namespace);
                },
                ChronicleOperation::WasGeneratedBy(WasGeneratedBy{namespace, id, activity}) => {
                    let activity_id = activity;
                    let entity = &prov.entities.get(&(namespace.to_owned(),id.to_owned()));
                    prop_assert!(entity.is_some());
                    let entity = entity.unwrap();
                    prop_assert_eq!(&entity.name, id.name_part());
                    prop_assert_eq!(&entity.namespaceid, namespace);

                    let activity = &prov.activities.get(&(namespace.to_owned(),activity.to_owned()));
                    prop_assert!(activity.is_some());
                    let activity = activity.unwrap();
                    prop_assert_eq!(&activity.name, activity_id.name_part());
                    prop_assert_eq!(&activity.namespaceid, namespace);

                    let has_generation = prov.generation.get(
                        &(namespace.clone(),id.clone()))
                        .unwrap()
                        .contains(& Generation {
                            activity_id: activity_id.clone(),
                            generated_id: id.clone(),
                            time: None });

                    prop_assert!(has_generation);
                }
                ChronicleOperation::EntityHasEvidence(
                    EntityHasEvidence{
                    namespace,
                    identityid: _,
                    id,
                    locator: _,
                    agent,
                    signature: _,
                    signature_time: _
                }) =>  {
                    let agent_id = agent;
                    let entity = &prov.entities.get(&(namespace.to_owned(),id.to_owned()));
                    prop_assert!(entity.is_some());
                    let entity = entity.unwrap();
                    prop_assert_eq!(&entity.name, id.name_part());
                    prop_assert_eq!(&entity.namespaceid, namespace);

                    let agent = &prov.agents.get(&(namespace.to_owned(),agent.to_owned()));
                    prop_assert!(agent.is_some());
                    let agent = agent.unwrap();
                    prop_assert_eq!(&agent.name, agent_id.name_part());
                    prop_assert_eq!(&agent.namespaceid, namespace);
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

        let lhs_json = compact_json(&prov).0;

        let serialized_prov = prov_from_json_ld(lhs_json.clone());

        prop_assert_eq!(&prov,&serialized_prov,"Prov reserialisation compact: \n{} expanded \n {}",json::stringify_pretty(lhs_json, 2), json::stringify_pretty(lhs_json_expanded, 2));
    }
}
