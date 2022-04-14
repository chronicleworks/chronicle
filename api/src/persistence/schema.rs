// @generated automatically by Diesel CLI.

diesel::table! {
    use diesel::sql_types::*;
    use common::prov::*;

    activity (id) {
        id -> Integer,
        name -> Text,
        namespace_id -> Integer,
        domaintype -> Nullable<Text>,
        started -> Nullable<Timestamp>,
        ended -> Nullable<Timestamp>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use common::prov::*;

    agent (id) {
        id -> Integer,
        name -> Text,
        namespace_id -> Integer,
        domaintype -> Nullable<Text>,
        current -> Integer,
        identity_id -> Nullable<Integer>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use common::prov::*;

    association (offset, agent_id) {
        offset -> Integer,
        agent_id -> Integer,
        activity_id -> Integer,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use common::prov::*;

    attachment (id) {
        id -> Integer,
        namespace_id -> Integer,
        signature_time -> Timestamp,
        signature -> Text,
        signer_id -> Integer,
        locator -> Nullable<Text>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use common::prov::*;

    delegation (offset, delegate_id, responsible_id) {
        offset -> Integer,
        delegate_id -> Integer,
        responsible_id -> Integer,
        activity_id -> Nullable<Integer>,
        typ -> Nullable<Text>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use common::prov::*;

    derivation (offset, generated_entity_id) {
        offset -> Integer,
        activity_id -> Nullable<Integer>,
        generated_entity_id -> Integer,
        used_entity_id -> Integer,
        typ -> Nullable<Integer>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use common::prov::*;

    entity (id) {
        id -> Integer,
        name -> Text,
        namespace_id -> Integer,
        domaintype -> Nullable<Text>,
        attachment_id -> Nullable<Integer>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use common::prov::*;

    generation (offset, generated_entity_id) {
        offset -> Integer,
        activity_id -> Integer,
        generated_entity_id -> Integer,
        typ -> Nullable<Text>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use common::prov::*;

    hadattachment (entity_id, attachment_id) {
        entity_id -> Integer,
        attachment_id -> Integer,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use common::prov::*;

    hadidentity (agent_id, identity_id) {
        agent_id -> Integer,
        identity_id -> Integer,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use common::prov::*;

    identity (id) {
        id -> Integer,
        namespace_id -> Integer,
        public_key -> Text,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use common::prov::*;

    ledgersync (correlation_id) {
        correlation_id -> Text,
        offset -> Nullable<Text>,
        sync_time -> Nullable<Timestamp>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use common::prov::*;

    namespace (id) {
        id -> Integer,
        name -> Text,
        uuid -> Text,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use common::prov::*;

    useage (offset, entity_id) {
        offset -> Integer,
        activity_id -> Integer,
        entity_id -> Integer,
    }
}

diesel::joinable!(activity -> namespace (namespace_id));
diesel::joinable!(agent -> identity (identity_id));
diesel::joinable!(agent -> namespace (namespace_id));
diesel::joinable!(association -> activity (activity_id));
diesel::joinable!(association -> agent (agent_id));
diesel::joinable!(attachment -> identity (signer_id));
diesel::joinable!(attachment -> namespace (namespace_id));
diesel::joinable!(delegation -> activity (activity_id));
diesel::joinable!(derivation -> activity (activity_id));
diesel::joinable!(entity -> attachment (attachment_id));
diesel::joinable!(entity -> namespace (namespace_id));
diesel::joinable!(generation -> activity (activity_id));
diesel::joinable!(generation -> entity (generated_entity_id));
diesel::joinable!(hadattachment -> attachment (attachment_id));
diesel::joinable!(hadattachment -> entity (entity_id));
diesel::joinable!(hadidentity -> agent (agent_id));
diesel::joinable!(hadidentity -> identity (identity_id));
diesel::joinable!(identity -> namespace (namespace_id));
diesel::joinable!(useage -> activity (activity_id));
diesel::joinable!(useage -> entity (entity_id));

diesel::allow_tables_to_appear_in_same_query!(
    activity,
    agent,
    association,
    attachment,
    delegation,
    derivation,
    entity,
    generation,
    hadattachment,
    hadidentity,
    identity,
    ledgersync,
    namespace,
    useage,
);
