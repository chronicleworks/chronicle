// @generated automatically by Diesel CLI.

diesel::table! {
    agent (name) {
        name -> Text,
        namespace -> Text,
        publickey -> Nullable<Text>,
        privatekeypath -> Nullable<Text>,
        current -> Integer,
    }
}

diesel::table! {
    namespace (name) {
        name -> Text,
        uuid -> Text,
    }
}

diesel::joinable!(agent -> namespace (namespace));

diesel::allow_tables_to_appear_in_same_query!(
    agent,
    namespace,
);
