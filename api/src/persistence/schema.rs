// @generated automatically by Diesel CLI.

diesel::table! {
    activity (id) {
        id -> Nullable<Integer>,
        name -> Text,
        namespace -> Text,
        started -> Nullable<Text>,
        ended -> Nullable<Text>,
    }
}

diesel::table! {
    agent (id) {
        id -> Nullable<Integer>,
        name -> Text,
        namespace -> Text,
        publickey -> Nullable<Text>,
        privatekeypath -> Nullable<Text>,
        current -> Integer,
    }
}

diesel::table! {
    entity (id) {
        id -> Nullable<Integer>,
        name -> Text,
        namespace -> Text,
        started -> Nullable<Text>,
        ended -> Nullable<Text>,
    }
}

diesel::table! {
    namespace (name) {
        name -> Text,
        uuid -> Text,
    }
}

diesel::table! {
    uses (id) {
        id -> Nullable<Integer>,
        agent -> Nullable<Integer>,
        entity -> Nullable<Integer>,
    }
}

diesel::table! {
    wasasociatedwith (id) {
        id -> Nullable<Integer>,
        agent -> Nullable<Integer>,
        activity -> Nullable<Integer>,
    }
}

diesel::table! {
    wasattributedto (id) {
        id -> Nullable<Integer>,
        agent -> Nullable<Integer>,
        activity -> Nullable<Integer>,
    }
}

diesel::table! {
    wasgeneratedby (id) {
        id -> Nullable<Integer>,
        agent -> Nullable<Integer>,
        entity -> Nullable<Integer>,
    }
}

diesel::joinable!(activity -> namespace (namespace));
diesel::joinable!(agent -> namespace (namespace));
diesel::joinable!(entity -> namespace (namespace));
diesel::joinable!(uses -> agent (agent));
diesel::joinable!(uses -> entity (entity));
diesel::joinable!(wasasociatedwith -> activity (activity));
diesel::joinable!(wasasociatedwith -> agent (agent));
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
    wasasociatedwith,
    wasattributedto,
    wasgeneratedby,
);
