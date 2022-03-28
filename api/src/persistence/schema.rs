// @generated automatically by Diesel CLI.

diesel::table! {
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
    entity (id) {
        id -> Integer,
        name -> Text,
        namespace_id -> Integer,
        domaintype -> Nullable<Text>,
        attachment_id -> Nullable<Integer>,
    }
}

diesel::table! {
    hadattachment (entity_id, attachment_id) {
        entity_id -> Integer,
        attachment_id -> Integer,
    }
}

diesel::table! {
    hadidentity (agent_id, identity_id) {
        agent_id -> Integer,
        identity_id -> Integer,
    }
}

diesel::table! {
    identity (id) {
        id -> Integer,
        namespace_id -> Integer,
        public_key -> Text,
    }
}

diesel::table! {
    ledgersync (correlation_id) {
        correlation_id -> Text,
        offset -> Nullable<Text>,
        sync_time -> Nullable<Timestamp>,
    }
}

diesel::table! {
    namespace (id) {
        id -> Integer,
        name -> Text,
        uuid -> Text,
    }
}

diesel::table! {
    used (activity_id, entity_id) {
        activity_id -> Integer,
        entity_id -> Integer,
    }
}

diesel::table! {
    wasassociatedwith (agent_id, activity_id) {
        agent_id -> Integer,
        activity_id -> Integer,
    }
}

diesel::table! {
    wasgeneratedby (activity_id, entity_id) {
        activity_id -> Integer,
        entity_id -> Integer,
    }
}

diesel::joinable!(activity -> namespace (namespace_id));
diesel::joinable!(agent -> identity (identity_id));
diesel::joinable!(agent -> namespace (namespace_id));
diesel::joinable!(attachment -> identity (signer_id));
diesel::joinable!(attachment -> namespace (namespace_id));
diesel::joinable!(entity -> attachment (attachment_id));
diesel::joinable!(entity -> namespace (namespace_id));
diesel::joinable!(hadattachment -> attachment (attachment_id));
diesel::joinable!(hadattachment -> entity (entity_id));
diesel::joinable!(hadidentity -> agent (agent_id));
diesel::joinable!(hadidentity -> identity (identity_id));
diesel::joinable!(identity -> namespace (namespace_id));
diesel::joinable!(used -> activity (activity_id));
diesel::joinable!(used -> entity (entity_id));
diesel::joinable!(wasassociatedwith -> activity (activity_id));
diesel::joinable!(wasassociatedwith -> agent (agent_id));
diesel::joinable!(wasgeneratedby -> activity (activity_id));
diesel::joinable!(wasgeneratedby -> entity (entity_id));

diesel::allow_tables_to_appear_in_same_query!(
    activity,
    agent,
    attachment,
    entity,
    hadattachment,
    hadidentity,
    identity,
    ledgersync,
    namespace,
    used,
    wasassociatedwith,
    wasgeneratedby,
);
