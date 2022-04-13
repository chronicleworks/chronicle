//! This crate implements the macro for `meta_chronicle` and should not be used directly.

use std::collections::HashMap;

use proc_macro2::{TokenStream, TokenTree};
use quote::quote;

#[derive(Debug)]
struct EntityModel {
    type_name: String,
}

#[derive(Debug)]
struct AgentModel {
    type_name: String,
}

#[derive(Debug)]
struct ActivityModel {
    type_name: String,
}

#[derive(Debug, Default)]
struct ChronicleModel {
    name: String,
    agents: HashMap<String, AgentModel>,
    activities: HashMap<String, ActivityModel>,
    entities: HashMap<String, EntityModel>,
}

enum ParsedType {
    Agent(AgentModel),
    Activity(ActivityModel),
    Entity(EntityModel),
}

fn rec_type(stream: &TokenStream) -> Result<Option<ParsedType>, syn::Error> {
    unimplemented!()
}

#[doc(hidden)]
pub fn meta_chronicle(item: TokenStream) -> Result<TokenStream, syn::Error> {
    print!("{:#?}", item);
    let mut model = ChronicleModel::default();

    if let Some(typ) = rec_type(&item)? {
        match typ {
            ParsedType::Agent(agent) => {
                model.agents.insert(agent.type_name.clone(), agent);
            }
            ParsedType::Activity(activity) => {
                model
                    .activities
                    .insert(activity.type_name.clone(), activity);
            }
            ParsedType::Entity(entity) => {
                model.entities.insert(entity.type_name.clone(), entity);
            }
        }
    }
    Ok(quote! {
        "TODO - lots"
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example() {
        assert!(meta_chronicle(quote! {
                agent(artist) {
                    properties: {
                    name: String,
                }
        }})
        .is_ok());
    }
}
