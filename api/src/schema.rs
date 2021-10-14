table! {
    agent (name) {
        name -> Text,
        namespace -> Text,
        uuid -> Text,
        current -> Integer,
    }
}

table! {
    namespace (name) {
        name -> Text,
        uuid -> Text,
    }
}

joinable!(agent -> namespace (namespace));

allow_tables_to_appear_in_same_query!(
    agent,
    namespace,
);
