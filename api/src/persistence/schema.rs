// @generated automatically by Diesel CLI.

diesel::table! {
    activity (id) {
        id -> Integer,
        name -> Text,
        namespace -> Text,
        domaintype -> Nullable<Text>,
        started -> Nullable<Timestamp>,
        ended -> Nullable<Timestamp>,
    }
}

diesel::table! {
    agent (id) {
        id -> Integer,
        name -> Text,
        namespace -> Text,
        domaintype -> Nullable<Text>,
        publickey -> Nullable<Text>,
        current -> Integer,
    }
}

diesel::table! {
    entity (id) {
        id -> Integer,
        name -> Text,
        namespace -> Text,
        domaintype -> Nullable<Text>,
        signature_time -> Nullable<Timestamp>,
        signature -> Nullable<Text>,
        locator -> Nullable<Text>,
    }
}

diesel::table! {
    ledgersync (offset) {
        offset -> Text,
        sync_time -> Nullable<Timestamp>,
    }
}

diesel::table! {
    namespace (name) {
        name -> Text,
        uuid -> Text,
    }
}

diesel::table! {
    used (activity, entity) {
        activity -> Integer,
        entity -> Integer,
    }
}

diesel::table! {
    wasassociatedwith (agent, activity) {
        agent -> Integer,
        activity -> Integer,
    }
}

diesel::table! {
    wasgeneratedby (activity, entity) {
        activity -> Integer,
        entity -> Integer,
    }
}

diesel::joinable!(activity -> namespace (namespace));
diesel::joinable!(agent -> namespace (namespace));
diesel::joinable!(entity -> namespace (namespace));
diesel::joinable!(used -> activity (activity));
diesel::joinable!(used -> entity (entity));
diesel::joinable!(wasassociatedwith -> activity (activity));
diesel::joinable!(wasassociatedwith -> agent (agent));
diesel::joinable!(wasgeneratedby -> activity (activity));
diesel::joinable!(wasgeneratedby -> entity (entity));

diesel::allow_tables_to_appear_in_same_query!(
    activity,
    agent,
    entity,
    ledgersync,
    namespace,
    used,
    wasassociatedwith,
    wasgeneratedby,
);
