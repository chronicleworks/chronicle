# DomainDocs

Chronicle documentation sources for use in doc comments in Rust code,
with the aim of adding documentation capabilities to generated Chronicle
domains and the GraphQL schema they produce.

Documentation in `domain_docs/` duplicates and in some cases supplements
Chronicle documentation in `docs/`.

## Examples

```rust
//! crates/chronicle/src/codegen/mod.rs
//!
fn gen_type_enums(domain: &ChronicleDomainDef) -> rust::Tokens {
    // ...

    let role_doc = include_str!("../../../../domain_docs/role.md");

    //...

    quote! {
        #[doc = #_(#role_doc)]
        #[derive(#graphql_enum, Copy, Clone, Eq, PartialEq)]
        #[allow(clippy::upper_case_acronyms)]
        pub enum RoleType {
            Unspecified,
            #(for role in domain.roles.iter() =>
            #[graphql(name = #_(#(role.preserve_inflection())), visible=true)]
                #(role.as_type_name()),
            )
        }
    // ...
    }
}
```

The above example from `codegen` results in the following GraphQL schema:

```graphql
# # `prov:Role`
#
# A role is the function of an entity or agent with respect to an activity, in
# the context of a usage, generation, invalidation, association, start, and end.
enum RoleType {
  UNSPECIFIED
  BUYER
  SELLER
  CREATOR
}
```
