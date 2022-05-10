use chrono::Utc;
use json::JsonValue;
use proptest::prelude::*;

use uuid::Uuid;

use crate::{
    attributes::{Attribute, Attributes},
    prov::{
        operations::*, vocab::Chronicle, AgentId, Association, Delegation, Derivation,
        DomaintypeId, Generation, ProvModel, Useage,
    },
};

use super::{
    ActivityUses, ActsOnBehalfOf, CompactedJson, EntityAttach, EntityDerive, NamespaceId,
    StartActivity,
};

prop_compose! {
    fn a_name()(name in "[-A-Za-z0-9+]+") -> String {
        name
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
    fn name()(names in prop::collection::vec(a_name(), 5), index in (0..5usize)) -> String {
        names.get(index).unwrap().to_owned()
    }
}

// Choose from a limited selection of domain types
prop_compose! {
    fn domain_type_id()(names in prop::collection::vec(a_symbol(), 5), index in (0..5usize)) -> DomaintypeId {
        Chronicle::domaintype(names.get(index).unwrap()).into()
    }
}

prop_compose! {
    fn a_namespace()
        (uuid in prop::collection::vec(0..255u8, 16),
         name in name()) -> NamespaceId {
        Chronicle::namespace(&name,&Uuid::from_bytes(uuid.as_slice().try_into().unwrap())).into()
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
        let (name,uuid) = id.decompose();
        CreateNamespace {
            id: id.clone(),
            uuid,
            name: name.to_owned(),
        }
    }
}

prop_compose! {
    fn create_agent() (name in name(),namespace in namespace()) -> CreateAgent {
        let id = Chronicle::agent(&name).into();
        CreateAgent {
            namespace,
            name,
            id,
        }
    }
}

prop_compose! {
    fn register_key() (name in name(),namespace in namespace(), publickey in "[0-9a-f]{64}") -> RegisterKey {
        let id = Chronicle::agent(&name).into();
        RegisterKey {
            namespace,
            name,
            id,
            publickey
        }
    }
}

prop_compose! {
    fn create_activity() (name in name(),namespace in namespace()) -> CreateActivity {
        let id = Chronicle::activity(&name).into();
        CreateActivity {
            namespace,
            name,
            id,
        }
    }
}

// Create times for start between 2-1 years in the past, to ensure start <= end
prop_compose! {
    fn start_activity() (name in name(),namespace in namespace(), offset in (0..10)) -> StartActivity {
        let id = Chronicle::activity(&name).into();

        let today = Utc::today().and_hms_micro(0, 0,0,0);

        StartActivity {
            namespace,
            agent: Chronicle::agent(&name).into(),
            id,
            time: today - chrono::Duration::days(offset as _)
        }
    }
}

// Create times for start between 2-1 years in the past, to ensure start <= end
prop_compose! {
    fn end_activity() (name in name(),namespace in namespace(), offset in (0..10)) -> EndActivity {
        let id = Chronicle::activity(&name).into();

        let today = Utc::today().and_hms_micro(0, 0,0,0);

        EndActivity {
            namespace,
            agent: Chronicle::agent(&name).into(),
            id,
            time: today - chrono::Duration::days(offset as _)
        }
    }
}

prop_compose! {
    fn activity_uses() (activity_name in name(), entity_name in name(),namespace in namespace()) -> ActivityUses {
        let activity = Chronicle::activity(&activity_name).into();
        let id = Chronicle::entity(&entity_name).into();

        ActivityUses {
            namespace,
            id,
            activity
        }
    }
}

prop_compose! {
    fn create_entity() (name in name(),namespace in namespace()) -> CreateEntity {
        let id = Chronicle::entity(&name).into();
        CreateEntity {
            namespace,
            name,
            id,
        }
    }
}

prop_compose! {
    fn generate_entity() (activity_name in name(), entity_name in name(),namespace in namespace()) -> GenerateEntity {
        let activity = Chronicle::activity(&activity_name).into();
        let id = Chronicle::entity(&entity_name).into();


        GenerateEntity {
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
    ) -> EntityAttach {
        let id = Chronicle::entity(&name).into();
        let agent: AgentId = Chronicle::agent(&agent_name).into();
        let identityid = Chronicle::identity(&agent , &*public_key).into();

        let signature_time = Utc::today().and_hms_micro(offset, 0,0,0);

        EntityAttach {
            namespace,
            id,
            locator,
            agent,
            signature,
            identityid,
            signature_time
        }
    }
}

prop_compose! {
    fn entity_derive() (
        name in name(),
        used in name(),
        namespace in namespace(),
    ) -> EntityDerive {
        let id = Chronicle::entity(&name).into();
        let used_id = Chronicle::entity(&used).into();

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
    fn agent_acts_on_behalf_of() (
        name in name(),
        delegate in name(),
        namespace in namespace(),
    ) -> ActsOnBehalfOf {

        ActsOnBehalfOf {
            id: Chronicle::agent(&name).into(),
            delegate_id: Chronicle::agent(&delegate).into(),
            activity_id: None,
            namespace,
        }

    }
}

prop_compose! {
    fn entity_attributes() (
        name in name(),
        namespace in namespace(),
        attributes in attributes(),
    ) -> SetAttributes {

        SetAttributes::Entity{
                id: Chronicle::entity(&name).into(),
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
                id: Chronicle::agent(&name).into(),
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
                id: Chronicle::activity(&name).into(),
                namespace,
                attributes,
        }
    }
}

fn transaction() -> impl Strategy<Value = ChronicleOperation> {
    prop_oneof![
        1 => create_namespace().prop_map(ChronicleOperation::CreateNamespace),
        2 => create_agent().prop_map(ChronicleOperation::CreateAgent),
        2 => register_key().prop_map(ChronicleOperation::RegisterKey),
        4 => create_activity().prop_map(ChronicleOperation::CreateActivity),
        4 => start_activity().prop_map(ChronicleOperation::StartActivity),
        4 => end_activity().prop_map(ChronicleOperation::EndActivity),
        4 => activity_uses().prop_map(ChronicleOperation::ActivityUses),
        2 => create_entity().prop_map(ChronicleOperation::CreateEntity),
        4 => generate_entity().prop_map(ChronicleOperation::GenerateEntity),
        2 => entity_attach().prop_map(ChronicleOperation::EntityAttach),
        2 => entity_derive().prop_map(ChronicleOperation::EntityDerive),
        2 => agent_acts_on_behalf_of().prop_map(ChronicleOperation::AgentActsOnBehalfOf),
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
    fn operations(tx in operation_seq()) {
        let mut prov = ProvModel::default();

        // Apply each operation in order
        for tx in tx.iter() {
            prov.apply(tx);
        }

        // Now assert that the final prov object matches what we would expect from the input operations
        for tx in tx.iter() {
            match tx {
                ChronicleOperation::CreateNamespace(CreateNamespace{id,name,uuid}) => {
                    prop_assert!(prov.namespaces.contains_key(id));
                    let ns = prov.namespaces.get(id).unwrap();
                    prop_assert_eq!(&ns.id, id);
                    prop_assert_eq!(&ns.name, name);
                    prop_assert_eq!(&ns.uuid, uuid);
                },
                ChronicleOperation::CreateAgent(
                    CreateAgent { namespace, name, id }) => {
                    let agent = &prov.agents.get(&(namespace.to_owned(),id.to_owned()));
                    prop_assert!(agent.is_some());
                    let agent = agent.unwrap();
                    prop_assert_eq!(&agent.name, name);
                    prop_assert_eq!(&agent.namespaceid, namespace);
                },
                ChronicleOperation::AgentActsOnBehalfOf(
                    ActsOnBehalfOf {namespace, id, delegate_id, activity_id }
                ) => {
                    let agent = &prov.agents.get(&(namespace.to_owned(),id.to_owned()));
                    prop_assert!(agent.is_some());
                    let agent = agent.unwrap();

                    let delegate = &prov.agents.get(&(namespace.to_owned(),delegate_id.to_owned()));
                    prop_assert!(delegate.is_some());
                    let delegate = delegate.unwrap();

                    if let Some(activity_id) = activity_id {
                        let activity = &prov.activities.get(&(namespace.to_owned(),activity_id.to_owned()));
                        prop_assert!(activity.is_some());
                    }

                    let has_delegation = prov.delegation.get(&(namespace.to_owned(),id.to_owned()))
                        .unwrap()
                        .contains(&Delegation {
                            responsible_id: agent.id.clone(),
                            delegate_id: delegate.id.clone(),
                            activity_id: activity_id.clone(),
                        });

                    prop_assert!(has_delegation);

                }
                ChronicleOperation::RegisterKey(
                    RegisterKey { namespace, name, id, publickey}) => {
                        let agent = &prov.agents.get(&(namespace.clone(),id.clone()));
                        prop_assert!(agent.is_some());
                        let agent = agent.unwrap();
                        let identity = &prov.has_identity.get(&(namespace.clone(), agent.id.clone()));
                        prop_assert!(identity.is_some());
                        let identity = identity.unwrap();
                        let identity = prov.identities.get(identity);
                        prop_assert!(identity.is_some());
                        let identity = identity.unwrap();

                        prop_assert_eq!(&agent.name, &name.clone());
                        prop_assert_eq!(&agent.namespaceid, &namespace.clone());
                        prop_assert_eq!(&identity.public_key, &publickey.clone());
                },
                ChronicleOperation::CreateActivity(
                    CreateActivity { namespace, id, name }) => {
                    let activity = &prov.activities.get(&(namespace.clone(),id.clone()));
                    prop_assert!(activity.is_some());
                    let activity = activity.unwrap();
                    prop_assert_eq!(&activity.name, name);
                    prop_assert_eq!(&activity.namespaceid, namespace);
                },
                ChronicleOperation::StartActivity(
                    StartActivity { namespace, id, agent, time }) =>  {
                    let activity = &prov.activities.get(&(namespace.clone(),id.clone()));
                    prop_assert!(activity.is_some());
                    let activity = activity.unwrap();
                    prop_assert_eq!(&activity.name, id.decompose());
                    prop_assert_eq!(&activity.namespaceid, namespace);

                    prop_assert!(activity.started == Some(time.to_owned()));
                    prop_assert!(activity.ended.is_none() || activity.ended.unwrap() >= activity.started.unwrap());

                    let has_assoc = prov.association.get(&(namespace.to_owned(),id.to_owned()))
                        .unwrap()
                        .contains(&Association {
                            agent_id: agent.clone(),
                            activity_id: id.clone()
                        });

                    prop_assert!(has_assoc);
                },
                ChronicleOperation::EndActivity(
                    EndActivity { namespace, id, agent, time }) => {
                    let activity = &prov.activities.get(&(namespace.to_owned(),id.to_owned()));
                    prop_assert!(activity.is_some());
                    let activity = activity.unwrap();
                    prop_assert_eq!(&activity.name, id.decompose());
                    prop_assert_eq!(&activity.namespaceid, namespace);

                    prop_assert!(activity.ended == Some(time.to_owned()));
                    prop_assert!(activity.started.unwrap() <= *time);

                    let has_assoc = prov.association.get(&(namespace.to_owned(),id.to_owned()))
                        .unwrap()
                        .contains(&Association {
                            agent_id: agent.clone(),
                            activity_id: id.clone()
                        });

                    prop_assert!(has_assoc);
                }
                ChronicleOperation::ActivityUses(
                    ActivityUses { namespace, id, activity }) => {
                    let activity_id = activity;
                    let entity = &prov.entities.get(&(namespace.to_owned(),id.to_owned()));
                    prop_assert!(entity.is_some());
                    let entity = entity.unwrap();
                    prop_assert_eq!(&entity.name, &id.decompose());
                    prop_assert_eq!(&entity.namespaceid, namespace);

                    let activity = &prov.activities.get(&(namespace.to_owned(),activity_id.to_owned()));
                    prop_assert!(activity.is_some());
                    let activity = activity.unwrap();
                    prop_assert_eq!(&activity.name, &activity_id.decompose());
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
                ChronicleOperation::CreateEntity(
                    CreateEntity { namespace, name, id }) => {
                    let entity = &prov.entities.get(&(namespace.to_owned(),id.to_owned()));
                    prop_assert!(entity.is_some());
                    let entity = entity.unwrap();
                    prop_assert_eq!(&entity.name, name);
                    prop_assert_eq!(&entity.namespaceid, namespace);
                },
                ChronicleOperation::GenerateEntity(GenerateEntity{namespace, id, activity}) => {
                    let activity_id = activity;
                    let entity = &prov.entities.get(&(namespace.to_owned(),id.to_owned()));
                    prop_assert!(entity.is_some());
                    let entity = entity.unwrap();
                    prop_assert_eq!(&entity.name, &id.decompose());
                    prop_assert_eq!(&entity.namespaceid, namespace);

                    let activity = &prov.activities.get(&(namespace.to_owned(),activity.to_owned()));
                    prop_assert!(activity.is_some());
                    let activity = activity.unwrap();
                    prop_assert_eq!(&activity.name, &activity_id.decompose());
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
                ChronicleOperation::EntityAttach(
                    EntityAttach{
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
                    prop_assert_eq!(&entity.name, &id.decompose());
                    prop_assert_eq!(&entity.namespaceid, namespace);

                    let agent = &prov.agents.get(&(namespace.to_owned(),agent.to_owned()));
                    prop_assert!(agent.is_some());
                    let agent = agent.unwrap();
                    prop_assert_eq!(&agent.name, agent_id.decompose());
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
        println!("{}", json::stringify_pretty(lhs_json_expanded.clone(),2));

        let lhs_json = compact_json(&prov).0;

        let serialized_prov = prov_from_json_ld(lhs_json.clone());

        prop_assert_eq!(&prov,&serialized_prov,"Prov reserialisation compact: \n{} expanded \n {}",json::stringify_pretty(lhs_json, 2), json::stringify_pretty(lhs_json_expanded, 2));
    }
}
