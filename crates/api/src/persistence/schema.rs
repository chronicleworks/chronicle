// @generated automatically by Diesel CLI.

diesel::table! {
    activity (id) {
        id -> Int4,
        external_id -> Text,
        namespace_id -> Int4,
        domaintype -> Nullable<Text>,
        started -> Nullable<Timestamp>,
        ended -> Nullable<Timestamp>,
    }
}

diesel::table! {
    activity_attribute (activity_id, typename) {
        activity_id -> Int4,
        typename -> Text,
        value -> Text,
    }
}

diesel::table! {
    agent (id) {
        id -> Int4,
        external_id -> Text,
        namespace_id -> Int4,
        domaintype -> Nullable<Text>,
        current -> Int4,
        identity_id -> Nullable<Int4>,
    }
}

diesel::table! {
    agent_attribute (agent_id, typename) {
        agent_id -> Int4,
        typename -> Text,
        value -> Text,
    }
}

diesel::table! {
    association (agent_id, activity_id, role) {
        agent_id -> Int4,
        activity_id -> Int4,
        role -> Text,
    }
}

diesel::table! {
    attachment (id) {
        id -> Int4,
        namespace_id -> Int4,
        signature_time -> Timestamp,
        signature -> Text,
        signer_id -> Int4,
        locator -> Nullable<Text>,
    }
}

diesel::table! {
    delegation (responsible_id, delegate_id, activity_id, role) {
        delegate_id -> Int4,
        responsible_id -> Int4,
        activity_id -> Int4,
        role -> Text,
    }
}

diesel::table! {
    derivation (activity_id, used_entity_id, generated_entity_id, typ) {
        activity_id -> Int4,
        generated_entity_id -> Int4,
        used_entity_id -> Int4,
        typ -> Int4,
    }
}

diesel::table! {
    entity (id) {
        id -> Int4,
        external_id -> Text,
        namespace_id -> Int4,
        domaintype -> Nullable<Text>,
        attachment_id -> Nullable<Int4>,
    }
}

diesel::table! {
    entity_attribute (entity_id, typename) {
        entity_id -> Int4,
        typename -> Text,
        value -> Text,
    }
}

diesel::table! {
    generation (activity_id, generated_entity_id) {
        activity_id -> Int4,
        generated_entity_id -> Int4,
        typ -> Nullable<Text>,
    }
}

diesel::table! {
    hadattachment (entity_id, attachment_id) {
        entity_id -> Int4,
        attachment_id -> Int4,
    }
}

diesel::table! {
    hadidentity (agent_id, identity_id) {
        agent_id -> Int4,
        identity_id -> Int4,
    }
}

diesel::table! {
    identity (id) {
        id -> Int4,
        namespace_id -> Int4,
        public_key -> Text,
    }
}

diesel::table! {
    ledgersync (tx_id) {
        tx_id -> Text,
        bc_offset -> Nullable<Text>,
        sync_time -> Nullable<Timestamp>,
    }
}

diesel::table! {
    namespace (id) {
        id -> Int4,
        external_id -> Text,
        uuid -> Text,
    }
}

diesel::table! {
    usage (activity_id, entity_id) {
        activity_id -> Int4,
        entity_id -> Int4,
    }
}

diesel::table! {
    wasinformedby (activity_id, informing_activity_id) {
        activity_id -> Int4,
        informing_activity_id -> Int4,
    }
}

diesel::joinable!(activity -> namespace (namespace_id));
diesel::joinable!(activity_attribute -> activity (activity_id));
diesel::joinable!(agent -> identity (identity_id));
diesel::joinable!(agent -> namespace (namespace_id));
diesel::joinable!(agent_attribute -> agent (agent_id));
diesel::joinable!(association -> activity (activity_id));
diesel::joinable!(association -> agent (agent_id));
diesel::joinable!(attachment -> identity (signer_id));
diesel::joinable!(attachment -> namespace (namespace_id));
diesel::joinable!(delegation -> activity (activity_id));
diesel::joinable!(derivation -> activity (activity_id));
diesel::joinable!(entity -> attachment (attachment_id));
diesel::joinable!(entity -> namespace (namespace_id));
diesel::joinable!(entity_attribute -> entity (entity_id));
diesel::joinable!(generation -> activity (activity_id));
diesel::joinable!(generation -> entity (generated_entity_id));
diesel::joinable!(hadattachment -> attachment (attachment_id));
diesel::joinable!(hadattachment -> entity (entity_id));
diesel::joinable!(hadidentity -> agent (agent_id));
diesel::joinable!(hadidentity -> identity (identity_id));
diesel::joinable!(identity -> namespace (namespace_id));
diesel::joinable!(usage -> activity (activity_id));
diesel::joinable!(usage -> entity (entity_id));

diesel::allow_tables_to_appear_in_same_query!(
    activity,
    activity_attribute,
    agent,
    agent_attribute,
    association,
    attachment,
    delegation,
    derivation,
    entity,
    entity_attribute,
    generation,
    hadattachment,
    hadidentity,
    identity,
    ledgersync,
    namespace,
    usage,
    wasinformedby,
);
