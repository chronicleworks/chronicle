// @generated automatically by Diesel CLI.

diesel::table! {
    activity (id) {
        id -> Integer,
        name -> Text,
        namespace -> Text,
        started -> Nullable<Timestamp>,
        ended -> Nullable<Timestamp>,
    }
}

diesel::table! {
    agent (id) {
        id -> Integer,
        name -> Text,
        namespace -> Text,
        publickey -> Nullable<Text>,
        current -> Integer,
    }
}

diesel::table! {
    entity (id) {
        id -> Integer,
        name -> Text,
        namespace -> Text,
        signature_time -> Nullable<Timestamp>,
        signature -> Nullable<Timestamp>,
        locator -> Nullable<Text>,
    }
}

diesel::table! {
    namespace (name) {
        name -> Text,
        uuid -> Text,
    }
}

diesel::table! {
    uses (agent, entity) {
        agent -> Integer,
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
    wasattributedto (agent, activity) {
        agent -> Integer,
        activity -> Integer,
    }
}

diesel::table! {
    wasgeneratedby (agent, entity) {
        agent -> Integer,
        entity -> Integer,
    }
}

diesel::joinable!(activity -> namespace (namespace));
diesel::joinable!(agent -> namespace (namespace));
diesel::joinable!(entity -> namespace (namespace));
diesel::joinable!(uses -> agent (agent));
diesel::joinable!(uses -> entity (entity));
diesel::joinable!(wasassociatedwith -> activity (activity));
diesel::joinable!(wasassociatedwith -> agent (agent));
diesel::joinable!(wasattributedto -> activity (activity));
diesel::joinable!(wasattributedto -> agent (agent));
diesel::joinable!(wasgeneratedby -> agent (agent));
diesel::joinable!(wasgeneratedby -> entity (entity));

diesel::allow_tables_to_appear_in_same_query!(
    activity,
    agent,
    entity,
    namespace,
    uses,
    wasassociatedwith,
    wasattributedto,
    wasgeneratedby,
);
