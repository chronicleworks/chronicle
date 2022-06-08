#![allow(dead_code)]
pub mod model;
use std::{io::Write, path::Path};

use genco::prelude::*;

pub use model::{AttributesTypeName, Builder, CliName, PrimitiveType, Property, TypeName};

pub use self::model::{ActivityDef, AgentDef, AttributeDef, ChronicleDomainDef, EntityDef};

fn agent_union_type_name() -> String {
    "Agent".to_owned()
}

fn entity_union_type_name() -> String {
    "Entity".to_owned()
}

fn activity_union_type_name() -> String {
    "Activity".to_owned()
}

fn gen_attribute_scalars(attributes: &[AttributeDef]) -> rust::Tokens {
    let graphql_new_type = &rust::import("chronicle::async_graphql", "NewType");
    quote! {
        #(for attribute in attributes.iter() =>
        #[derive(Clone, #graphql_new_type)]
        #[graphql(name,visible=true)]
        pub struct #(attribute.as_scalar_type())(#(match attribute.primitive_type {
                        PrimitiveType::String => String,
                        PrimitiveType::Bool => bool,
                        PrimitiveType::Int => i32,
            }
        ));
       )
    }
}

fn gen_type_enums(domain: &ChronicleDomainDef) -> rust::Tokens {
    let graphql_enum = &rust::import("chronicle::async_graphql", "Enum");
    let domain_type_id = &rust::import("chronicle::common::prov", "DomaintypeId");
    quote! {

        #[derive(#graphql_enum, Copy, Clone, Eq, PartialEq)]
        pub enum AgentType {
            ProvAgent,
            #(for agent in domain.agents.iter() =>
                #(agent.as_type_name()),
            )
        }

        impl Into<Option<#domain_type_id>> for AgentType {
            fn into(self) -> Option<#domain_type_id> {
                match self {
                #(for agent in domain.agents.iter() =>
                AgentType::#(agent.as_type_name()) => Some(#domain_type_id::from_name(#_(#(agent.as_type_name())))),
                )
                AgentType::ProvAgent => None
                }
            }
        }

        #[derive(#graphql_enum, Copy, Clone, Eq, PartialEq)]
        pub enum EntityType {
            ProvEntity,
            #(for entity in domain.entities.iter() =>
                #(entity.as_type_name()),
            )
        }

        impl Into<Option<#domain_type_id>> for EntityType {
            fn into(self) -> Option<#domain_type_id> {
                match self {
                #(for entity in domain.entities.iter() =>
                EntityType::#(entity.as_type_name()) => Some(#domain_type_id::from_name(#_(#(entity.as_type_name())))),
                )
                EntityType::ProvEntity => None
                }
            }
        }

        #[derive(#graphql_enum, Copy, Clone, Eq, PartialEq)]
        pub enum ActivityType {
            ProvActivity,
            #(for activity in domain.activities.iter() =>
                #(activity.as_type_name()),
            )
        }

        impl Into<Option<#domain_type_id>> for ActivityType {
            fn into(self) -> Option<#domain_type_id> {
                match self {
                #(for activity in domain.activities.iter() =>
                ActivityType::#(activity.as_type_name()) => Some(#domain_type_id::from_name(#_(#(activity.as_type_name())))),
                )
                ActivityType::ProvActivity => None
                }
            }
        }
    }
}

fn gen_agent_union(agents: &[AgentDef]) -> rust::Tokens {
    let union_macro = rust::import("chronicle::async_graphql", "Union").qualified();
    quote! {
        #[allow(clippy::enum_variant_names)]
        #[derive(#union_macro)]
        pub enum Agent {
            ProvAgent(ProvAgent),
            #(for agent in agents =>
                #(&agent.as_type_name())(#(&agent.as_type_name())),
            )
        }
    }
}

fn gen_entity_union(entities: &[EntityDef]) -> rust::Tokens {
    let union_macro = rust::import("chronicle::async_graphql", "Union").qualified();
    quote! {
        #[allow(clippy::enum_variant_names)]
        #[derive(#union_macro)]
        pub enum Entity {
            ProvEntity(ProvEntity),
            #(for entity in entities =>
                #(&entity.as_type_name())(#(&entity.as_type_name())),
            )
        }
    }
}

fn gen_activity_union(activities: &[ActivityDef]) -> rust::Tokens {
    let union_macro = rust::import("chronicle::async_graphql", "Union").qualified();
    quote! {
        #[allow(clippy::enum_variant_names)]
        #[derive(#union_macro)]
        pub enum Activity {
            ProvActivity(ProvActivity),
            #(for activity in activities =>
                #(&activity.as_type_name())(#(&activity.as_type_name())),
            )
        }
    }
}

fn gen_activity_definition(activity: &ActivityDef) -> rust::Tokens {
    let abstract_activity =
        &rust::import("chronicle::api::chronicle_graphql", "Activity").qualified();
    let activity_impl = &rust::import("chronicle::api::chronicle_graphql", "activity").qualified();
    let namespace = &rust::import("chronicle::api::chronicle_graphql", "Namespace").qualified();
    let activity_id = &rust::import("chronicle::common::prov", "EntityId").qualified();

    let object = rust::import("chronicle::async_graphql", "Object").qualified();
    let async_result = &rust::import("chronicle::async_graphql", "Result").qualified();
    let context = &rust::import("chronicle::async_graphql", "Context").qualified();
    let domain_type_id = &rust::import("chronicle::common::prov", "DomaintypeId");
    let date_time = &rust::import("chronicle::chrono", "DateTime");
    let utc = &rust::import("chronicle::chrono", "Utc");
    quote! {
    #(register(activity_impl))

    pub struct #(&activity.as_type_name())(#abstract_activity);

    #[#object]
    impl #(&activity.as_type_name()) {
        async fn id(&self) -> #activity_id {
            #activity_id::from_name(&*self.0.name)
        }

        async fn namespace<'a>(&self, ctx: &#context<'a>) -> #async_result<#namespace> {
            #activity_impl::namespace(self.0.namespace_id, ctx).await
        }

        async fn name(&self) -> &str {
            &self.0.name
        }

        async fn started(&self) -> Option<#date_time<#utc>> {
            self.0.started.map(|x| #date_time::from_utc(x, #utc))
        }

        async fn ended(&self) -> Option<#date_time<#utc>> {
            self.0.ended.map(|x| #date_time::from_utc(x, #utc))
        }

        #[graphql(name = "type")]
        async fn typ(&self) -> Option<#domain_type_id> {
            self.0.domaintype.as_deref().map(#domain_type_id::from_name)
        }

        async fn was_associated_with<'a>(
            &self,
            ctx: &#context<'a>,
        ) -> #async_result<Vec<#(agent_union_type_name())>> {
            Ok(
                #activity_impl::was_associated_with(self.0.id, ctx)
                    .await?
                    .into_iter()
                    .map(map_agent_to_domain_type)
                    .collect(),
            )
        }

        async fn used<'a>(&self, ctx: &#context<'a>) -> #async_result<Vec<#(entity_union_type_name())>> {
            Ok(#activity_impl::used(self.0.id, ctx)
                .await?
                .into_iter()
                .map(map_entity_to_domain_type)
                .collect())
        }

        #(for attribute in &activity.attributes =>
        async fn #(attribute.as_property())<'a>(&self, ctx: &#context<'a>) -> #async_result<Option<#(attribute.as_scalar_type())>> {
            Ok(#(match attribute.primitive_type {
              PrimitiveType::String =>
                #activity_impl::load_attribute(self.0.id, #_(#(attribute.as_type_name())), ctx)
                    .await?
                    .and_then(|attr| attr.as_str().map(|attr| attr.to_owned()))
                    .map(#(attribute.as_scalar_type())),
              PrimitiveType::Bool =>
                #activity_impl::load_attribute(self.0.id, #_(#(attribute.as_type_name())), ctx)
                    .await?
                    .and_then(|attr| attr.as_bool())
                    .map(#(attribute.as_scalar_type())),
              PrimitiveType::Int =>
                #activity_impl::load_attribute(self.0.id, #_(#(attribute.as_type_name())), ctx)
                    .await?
                    .and_then(|attr| attr.as_i64().map(|attr| attr as _))
                    .map(#(attribute.as_scalar_type())),
        }))
        })
    }
    }
}

fn gen_entity_definition(entity: &EntityDef) -> rust::Tokens {
    let abstract_entity = &rust::import("chronicle::api::chronicle_graphql", "Entity").qualified();
    let entity_impl = &rust::import("chronicle::api::chronicle_graphql", "entity").qualified();
    let namespace = &rust::import("chronicle::api::chronicle_graphql", "Namespace").qualified();
    let evidence = &rust::import("chronicle::api::chronicle_graphql", "Evidence");
    let entity_id = &rust::import("chronicle::common::prov", "EntityId").qualified();

    let object = rust::import("chronicle::async_graphql", "Object").qualified();
    let async_result = &rust::import("chronicle::async_graphql", "Result").qualified();
    let context = &rust::import("chronicle::async_graphql", "Context").qualified();
    let domain_type_id = &rust::import("chronicle::common::prov", "DomaintypeId");

    quote! {

    #(register(entity_impl))
    pub struct #(&entity.as_type_name())(#abstract_entity);

    #[#object]
    impl #(&entity.as_type_name()){
        async fn id(&self) -> #entity_id {
            #entity_id::from_name(&*self.0.name)
        }

        async fn namespace<'a>(&self, ctx: &#context<'a>) -> #async_result<#namespace> {
            #entity_impl::namespace(self.0.namespace_id, ctx).await
        }

        async fn name(&self) -> &str {
            &self.0.name
        }

        #[graphql(name = "type")]
        async fn typ(&self) -> Option<#domain_type_id> {
            self.0.domaintype.as_deref().map(#domain_type_id::from_name)
        }

        async fn evidence<'a>(&self, ctx: &#context<'a>) -> #async_result<Option<#evidence>> {
            #entity_impl::evidence(self.0.attachment_id, ctx).await
        }

        async fn was_generated_by<'a>(
            &self,
            ctx: &#context<'a>,
        ) -> #async_result<Vec<#(activity_union_type_name())>> {
            Ok(#entity_impl::was_generated_by(self.0.id, ctx)
                .await?
                .into_iter()
                .map(map_activity_to_domain_type)
                .collect())
        }

        async fn was_derived_from<'a>(&self, ctx: &#context<'a>) -> #async_result<Vec<#(entity_union_type_name())>> {
            Ok(#entity_impl::was_derived_from(self.0.id, ctx)
                .await?
                .into_iter()
                .map(map_entity_to_domain_type)
                .collect())
        }

        async fn had_primary_source<'a>(
            &self,
            ctx: &#context<'a>,
        ) -> #async_result<Vec<#(entity_union_type_name())>> {
            Ok(
                #entity_impl::had_primary_source(self.0.id, ctx)
                    .await?
                    .into_iter()
                    .map(map_entity_to_domain_type)
                    .collect(),
            )
        }

        async fn was_revision_of<'a>(&self, ctx: &#context<'a>) -> #async_result<Vec<#(entity_union_type_name())>> {
            Ok(#entity_impl::was_revision_of(self.0.id, ctx)
                .await?
                .into_iter()
                .map(map_entity_to_domain_type)
                .collect())
        }
        async fn was_quoted_from<'a>(&self, ctx: &#context<'a>) -> #async_result<Vec<#(entity_union_type_name())>> {
            Ok(#entity_impl::was_quoted_from(self.0.id, ctx)
                .await?
                .into_iter()
                .map(map_entity_to_domain_type)
                .collect())
        }

        #(for attribute in &entity.attributes =>
        async fn #(attribute.as_property())<'a>(&self, ctx: &#context<'a>) -> #async_result<Option<#(attribute.as_scalar_type())>> {
            Ok(#(match attribute.primitive_type {
              PrimitiveType::String =>
                #entity_impl::load_attribute(self.0.id, #_(#(attribute.as_type_name())), ctx)
                    .await?
                    .and_then(|attr| attr.as_str().map(|attr| attr.to_owned()))
                    .map(#(attribute.as_scalar_type())),
              PrimitiveType::Bool =>
                #entity_impl::load_attribute(self.0.id, #_(#(attribute.as_type_name())), ctx)
                    .await?
                    .and_then(|attr| attr.as_bool())
                    .map(#(attribute.as_scalar_type())),
              PrimitiveType::Int =>
                #entity_impl::load_attribute(self.0.id, #_(#(attribute.as_type_name())), ctx)
                    .await?
                    .and_then(|attr| attr.as_i64().map(|attr| attr as _))
                    .map(#(attribute.as_scalar_type())),
        }))
        })
    }
    }
}

fn gen_agent_definition(agent: &AgentDef) -> rust::Tokens {
    let abstract_agent = &rust::import("chronicle::api::chronicle_graphql", "Agent").qualified();
    let agent_impl = &rust::import("chronicle::api::chronicle_graphql", "agent").qualified();
    let namespace = &rust::import("chronicle::api::chronicle_graphql", "Namespace").qualified();
    let identity = &rust::import("chronicle::api::chronicle_graphql", "Identity").qualified();
    let agent_union_type = &agent_union_type_name();
    let object = rust::import("chronicle::async_graphql", "Object").qualified();
    let async_result = &rust::import("chronicle::async_graphql", "Result").qualified();
    let context = &rust::import("chronicle::async_graphql", "Context").qualified();
    let agent_id = &rust::import("chronicle::common::prov", "AgentId");
    let domain_type_id = &rust::import("chronicle::common::prov", "DomaintypeId");

    quote! {

    #(register(agent_impl))

    pub struct #(agent.as_type_name())(#abstract_agent);

    #[#object]
    impl #(agent.as_type_name()) {
        async fn id(&self) -> #agent_id {
            #agent_id::from_name(&*self.0.name)
        }

        async fn name(&self) -> &str {
            &self.0.name
        }

        async fn namespace<'a>(&self, ctx: &#context<'a>) -> #async_result<#namespace> {
            #agent_impl::namespace(self.0.namespace_id, ctx).await
        }

        async fn identity<'a>(&self, ctx: &#context<'a>) -> #async_result<Option<#identity>> {
            #agent_impl::identity(self.0.identity_id, ctx).await
        }

        async fn acted_on_behalf_of<'a>(&self, ctx: &#context<'a>) -> #async_result<Vec<#agent_union_type>> {
            Ok(#agent_impl::acted_on_behalf_of(self.0.id, ctx)
                .await?
                .into_iter()
                .map(Self)
                .map(#agent_union_type::from)
                .collect())
        }

        #(for attribute in &agent.attributes =>
        async fn #(attribute.as_property())<'a>(&self, ctx: &#context<'a>) -> #async_result<Option<#(attribute.as_scalar_type())>> {
            Ok(#(match attribute.primitive_type {
              PrimitiveType::String =>
                #agent_impl::load_attribute(self.0.id, #_(#(attribute.as_type_name())), ctx)
                    .await?
                    .and_then(|attr| attr.as_str().map(|attr| attr.to_owned()))
                    .map(#(attribute.as_scalar_type())),
              PrimitiveType::Bool =>
                #agent_impl::load_attribute(self.0.id, #_(#(attribute.as_type_name())), ctx)
                    .await?
                    .and_then(|attr| attr.as_bool())
                    .map(#(attribute.as_scalar_type())),
              PrimitiveType::Int =>
                #agent_impl::load_attribute(self.0.id, #_(#(attribute.as_type_name())), ctx)
                    .await?
                    .and_then(|attr| attr.as_i64().map(|attr| attr as _))
                    .map(#(attribute.as_scalar_type())),
        }))
        })

        #[graphql(name = "type")]
        async fn typ(&self) -> Option<#domain_type_id> {
            self.0.domaintype.as_deref().map(#domain_type_id::from_name)
        }
    }
    }
}

fn gen_abstract_prov_attributes() -> rust::Tokens {
    let input_object = &rust::import("chronicle::async_graphql", "InputObject").qualified();
    let abstract_attributes =
        &rust::import("chronicle::common::attributes", "Attributes").qualified();
    let domain_type_id = &rust::import("chronicle::common::prov", "DomaintypeId");
    quote! {
    #[derive(#input_object, Clone)]
    pub struct ProvAgentAttributes {
        #[graphql(name = "type")]
        pub typ: Option<String>,
    }

    #[allow(clippy::from_over_into)]
    impl From<ProvAgentAttributes> for #abstract_attributes {
        fn from(attributes: ProvAgentAttributes) -> Self {
            Self {
                typ: attributes.typ.map(#domain_type_id::from_name),
                ..Default::default()
            }
        }
    }

    #[derive(#input_object, Clone)]
    pub struct ProvEntityAttributes {
        #[graphql(name = "type")]
        pub typ: Option<String>,
    }

    #[allow(clippy::from_over_into)]
    impl From<ProvEntityAttributes> for #abstract_attributes {
        fn from(attributes: ProvEntityAttributes) -> Self {
            Self {
                typ: attributes.typ.map(#domain_type_id::from_name),
                ..Default::default()
            }
        }
    }
    #[derive(#input_object, Clone)]
    pub struct ProvActivityAttributes {
        #[graphql(name = "type")]
        pub typ: Option<String>,
    }

    #[allow(clippy::from_over_into)]
    impl From<ProvActivityAttributes> for #abstract_attributes {
        fn from(attributes: ProvActivityAttributes) -> Self {
            Self {
                typ: attributes.typ.map(#domain_type_id::from_name),
                ..Default::default()
            }
        }
    }
    }
}

fn gen_attribute_definition(typ: impl TypeName, attributes: &[AttributeDef]) -> rust::Tokens {
    let abstract_attribute =
        &rust::import("chronicle::common::attributes", "Attribute").qualified();
    let abstract_attributes =
        &rust::import("chronicle::common::attributes", "Attributes").qualified();
    let input_object = rust::import("chronicle::async_graphql", "InputObject").qualified();
    let domain_type_id = rust::import("chronicle::common::prov", "DomaintypeId");
    let serde_value = &rust::import("chronicle::serde_json", "Value");

    quote! {
        #[derive(#input_object)]
        pub struct #(typ.attributes_type_name()) {
            #(for attribute in attributes =>
                pub #(&attribute.as_property()): #(
                    match attribute.primitive_type {
                        PrimitiveType::String => String,
                        PrimitiveType::Bool => bool,
                        PrimitiveType::Int => i32,
                    }),
            )
        }


        #[allow(clippy::from_over_into)]
        impl From<#(typ.attributes_type_name())> for #abstract_attributes{
            fn from(attributes: #(typ.attributes_type_name())) -> Self {
                #abstract_attributes {
                    typ: Some(#domain_type_id::from_name(#_(#(typ.as_type_name())))),
                    attributes: vec![
                    #(for attribute in attributes =>
                        (#_(#(&attribute.as_type_name())).to_owned() ,
                            #abstract_attribute::new(#_(#(&attribute.as_type_name())),
                            #serde_value::from(attributes.#(&attribute.as_property())))),
                    )
                    ].into_iter().collect(),
                }
            }
        }
    }
}

fn gen_mappers(domain: &ChronicleDomainDef) -> rust::Tokens {
    let agent_impl = &rust::import("chronicle::api::chronicle_graphql", "Agent").qualified();
    let entity_impl = &rust::import("chronicle::api::chronicle_graphql", "Entity").qualified();
    let activity_impl = &rust::import("chronicle::api::chronicle_graphql", "Activity").qualified();

    quote! {
    fn map_agent_to_domain_type(agent: #agent_impl) -> #(agent_union_type_name()) {
        match agent.domaintype.as_deref() {
            #(for agent in domain.agents.iter() =>
            Some(#_(#(&agent.as_type_name()))) => #(agent_union_type_name())::#(&agent.as_type_name())(
                #(&agent.as_type_name())(agent)
            ),
            )
            _ => #(agent_union_type_name())::ProvAgent(ProvAgent(agent))
        }
    }

    fn map_activity_to_domain_type(activity: #activity_impl) -> #(activity_union_type_name()) {
        match activity.domaintype.as_deref() {
            #(for activity in domain.activities.iter() =>
            Some(#_(#(&activity.as_type_name()))) => #(activity_union_type_name())::#(&activity.as_type_name())(
                #(&activity.as_type_name())(activity)
            ),
            )
            _ => #(activity_union_type_name())::ProvActivity(ProvActivity(activity))
        }
    }

    fn map_entity_to_domain_type(entity: #entity_impl) -> #(entity_union_type_name()) {
        match entity.domaintype.as_deref() {
            #(for entity in domain.entities.iter() =>
           Some(#_(#(&entity.as_type_name()))) => #(entity_union_type_name())::#(&entity.as_type_name())(
                #(entity.as_type_name())(entity)
            ),
            )
            _ => #(entity_union_type_name())::ProvEntity(ProvEntity(entity))
        }
    }
    }
}
fn gen_query() -> rust::Tokens {
    let query_impl = &rust::import("chronicle::api::chronicle_graphql", "query").qualified();

    let graphql_object = &rust::import("chronicle::async_graphql", "Object");
    let graphql_result = &rust::import("chronicle::async_graphql", "Result");
    let graphql_id = &rust::import("chronicle::async_graphql", "ID");
    let graphql_context = &rust::import("chronicle::async_graphql", "Context");
    let graphql_connection = &rust::import("chronicle::async_graphql::connection", "Connection");

    let agent_id = &rust::import("chronicle::common::prov", "AgentId");
    let entity_id = &rust::import("chronicle::common::prov", "EntityId");
    let empty_fields =
        &rust::import("chronicle::async_graphql::connection", "EmptyFields").qualified();

    quote! {
    #[derive(Copy, Clone)]
    pub struct Query;

    #[#graphql_object]
    impl Query {

    #[allow(clippy::too_many_arguments)]
    pub async fn activity_timeline<'a>(
        &self,
        ctx: &#graphql_context<'a>,
        activity_types: Vec<ActivityType>,
        for_entity: Vec<EntityId>,
        from: Option<DateTime<Utc>>,
        to: Option<DateTime<Utc>>,
        namespace: Option<ID>,
        after: Option<String>,
        before: Option<String>,
        first: Option<i32>,
        last: Option<i32>,
    ) -> #graphql_result<#graphql_connection<i32, #(activity_union_type_name()), #empty_fields, #empty_fields>> {
            Ok(#query_impl::activity_timeline(
                ctx,
                activity_types.into_iter().filter_map(|x| x.into()).collect(),
                for_entity,
                from,
                to,
                namespace,
                after,
                before,
                first,
                last,
            )
            .await?
            .map_node(map_activity_to_domain_type))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn agents_by_type<'a>(
        &self,
        ctx: &#graphql_context<'a>,
        agent_type: AgentType,
        namespace: Option<#graphql_id>,
        after: Option<String>,
        before: Option<String>,
        first: Option<i32>,
        last: Option<i32>,
    ) -> #graphql_result<#graphql_connection<i32, #(agent_union_type_name()), #empty_fields, #empty_fields>> {
        Ok(#query_impl::agents_by_type(
            ctx, agent_type.into(), namespace, after, before, first, last,
        )
        .await?
        .map_node(map_agent_to_domain_type))
    }
    pub async fn agent_by_id<'a>(
        &self,
        ctx: &#graphql_context<'a>,
        id: #agent_id,
        namespace: Option<String>,
    ) -> #graphql_result<Option<#(agent_union_type_name())>> {
        Ok(#query_impl::agent_by_id(ctx, id, namespace)
            .await?
            .map(map_agent_to_domain_type))
    }

    pub async fn entity_by_id<'a>(
        &self,
        ctx: &#graphql_context<'a>,
        id: #entity_id,
        namespace: Option<String>,
    ) -> #graphql_result<Option<#(entity_union_type_name())>> {
        Ok(#query_impl::entity_by_id(ctx, id, namespace)
            .await?
            .map(map_entity_to_domain_type))
    }
    }
    }
}

fn gen_mutation(domain: &ChronicleDomainDef) -> rust::Tokens {
    let graphql_object = &rust::import("chronicle::async_graphql", "Object");

    let graphql_result = &rust::import("chronicle::async_graphql", "Result");
    let graphql_upload = &rust::import("chronicle::async_graphql", "Upload");
    let graphql_context = &rust::import("chronicle::async_graphql", "Context");

    let submission = &rust::import("chronicle::api::chronicle_graphql", "Submission");
    let impls = &rust::import("chronicle::api::chronicle_graphql", "mutation");

    let entity_id = &rust::import("chronicle::common::prov", "EntityId");
    let agent_id = &rust::import("chronicle::common::prov", "AgentId");
    let activity_id = &rust::import("chronicle::common::prov", "ActivityId");

    quote! {
    #[derive(Copy, Clone)]
    pub struct Mutation;

    #[#graphql_object]
    impl Mutation {

        pub async fn agent<'a>(
            &self,
            ctx: &#graphql_context<'a>,
            name: String,
            namespace: Option<String>,
            attributes: ProvAgentAttributes,
        ) -> async_graphql::#graphql_result<#submission> {
            #impls::agent(ctx, name, namespace, attributes.into()).await
        }

        #(for agent in domain.agents.iter() =>
            pub async fn #(&agent.as_property())<'a>(
                &self,
                ctx: &#graphql_context<'a>,
                name: String,
                namespace: Option<String>,
                attributes: #(agent.attributes_type_name()),
            ) -> async_graphql::#graphql_result<#submission> {
                #impls::agent(ctx, name, namespace, attributes.into()).await
            }
        )

        pub async fn activity<'a>(
            &self,
            ctx: &#graphql_context<'a>,
            name: String,
            namespace: Option<String>,
            attributes: ProvActivityAttributes,
        ) -> async_graphql::#graphql_result<#submission> {
            #impls::activity(ctx, name, namespace, attributes.into()).await
        }

        #(for activity in domain.activities.iter() =>
            pub async fn #(&activity.as_property())<'a>(
                &self,
                ctx: &#graphql_context<'a>,
                name: String,
                namespace: Option<String>,
                attributes: #(&activity.attributes_type_name()),
            ) -> async_graphql::#graphql_result<#submission> {
                #impls::activity(ctx, name, namespace, attributes.into()).await
            }
        )

        pub async fn entity<'a>(
            &self,
            ctx: &#graphql_context<'a>,
            name: String,
            namespace: Option<String>,
            attributes: ProvEntityAttributes,
        ) -> async_graphql::#graphql_result<#submission> {
            #impls::entity(ctx, name, namespace, attributes.into()).await
        }

        #(for entity in domain.entities.iter() =>
            pub async fn #(&entity.as_property())<'a>(
                &self,
                ctx: &#graphql_context<'a>,
                name: String,
                namespace: Option<String>,
                attributes: #(entity.attributes_type_name()),
            ) -> async_graphql::#graphql_result<#submission> {
                #impls::entity(ctx, name, namespace, attributes.into()).await
            }
        )

        pub async fn acted_on_behalf_of<'a>(
            &self,
            ctx: &#graphql_context<'a>,
            namespace: Option<String>,
            responsible: #agent_id,
            delegate: #agent_id,
        ) -> async_graphql::#graphql_result<#submission> {
            #impls::acted_on_behalf_of(ctx, namespace, responsible, delegate).await
        }

        pub async fn was_derived_from<'a>(
            &self,
            ctx: &#graphql_context<'a>,
            namespace: Option<String>,
            generated_entity: #entity_id,
            used_entity: #entity_id,
        ) -> async_graphql::#graphql_result<#submission> {
            #impls::was_derived_from(ctx, namespace, generated_entity, used_entity)
                .await
        }

        pub async fn was_revision_of<'a>(
            &self,
            ctx: &#graphql_context<'a>,
            namespace: Option<String>,
            generated_entity: #entity_id,
            used_entity: #entity_id,
        ) -> async_graphql::#graphql_result<#submission> {
            #impls::was_revision_of(ctx, namespace, generated_entity, used_entity)
                .await
        }
        pub async fn had_primary_source<'a>(
            &self,
            ctx: &#graphql_context<'a>,
            namespace: Option<String>,
            generated_entity: #entity_id,
            used_entity: #entity_id,
        ) -> async_graphql::#graphql_result<#submission> {
            #impls::had_primary_source(
                ctx,
                namespace,
                generated_entity,
                used_entity,
            )
            .await
        }
        pub async fn was_quoted_from<'a>(
            &self,
            ctx: &#graphql_context<'a>,
            namespace: Option<String>,
            generated_entity: #entity_id,
            used_entity: #entity_id,
        ) -> async_graphql::#graphql_result<#submission> {
            #impls::was_quoted_from(ctx, namespace, generated_entity, used_entity)
                .await
        }

        pub async fn generate_key<'a>(
            &self,
            ctx: &#graphql_context<'a>,
            id: #agent_id,
            namespace: Option<String>,
        ) -> async_graphql::#graphql_result<#submission> {
            #impls::generate_key(ctx, id, namespace).await
        }

        pub async fn start_activity<'a>(
            &self,
            ctx: &#graphql_context<'a>,
            id: #activity_id,
            namespace: Option<String>,
            agent: #agent_id,
            time: Option<DateTime<Utc>>,
        ) -> async_graphql::#graphql_result<#submission> {
            #impls::start_activity(ctx, id, namespace, agent, time).await
        }

        pub async fn end_activity<'a>(
            &self,
            ctx: &#graphql_context<'a>,
            id: #activity_id,
            namespace: Option<String>,
            agent: #agent_id,
            time: Option<DateTime<Utc>>,
        ) -> async_graphql::#graphql_result<#submission> {
            #impls::end_activity(ctx, id, namespace, agent, time).await
        }

        pub async fn used<'a>(
            &self,
            ctx: &#graphql_context<'a>,
            activity: #activity_id,
            id: #entity_id,
            namespace: Option<String>,
        ) -> async_graphql::#graphql_result<#submission> {
            #impls::used(ctx, activity, id, namespace).await
        }

        pub async fn was_generated_by<'a>(
            &self,
            ctx: &#graphql_context<'a>,
            activity: #activity_id,
            id: #entity_id,
            namespace: Option<String>,
        ) -> async_graphql::#graphql_result<#submission> {
            #impls::was_generated_by(ctx, activity, id, namespace).await
        }

        pub async fn has_attachment<'a>(
            &self,
            ctx: &#graphql_context<'a>,
            id: #entity_id,
            namespace: Option<String>,
            attachment: #graphql_upload,
            agent: #agent_id,
            locator: String,
        ) -> async_graphql::#graphql_result<#submission> {
            #impls::has_attachment(ctx, id, namespace, attachment, agent, locator)
                .await
        }
    }
    }
}

fn gen_graphql_type(domain: &ChronicleDomainDef) -> rust::Tokens {
    let prov_agent = AgentDef {
        name: "ProvAgent".to_owned(),
        attributes: vec![],
    };
    let prov_activity = ActivityDef {
        name: "ProvActivity".to_owned(),
        attributes: vec![],
    };
    let prov_entity = EntityDef {
        name: "ProvEntity".to_owned(),
        attributes: vec![],
    };

    let builder = &rust::import("chronicle::codegen", "Builder");
    let primitive_type = &rust::import("chronicle::codegen", "PrimitiveType");
    let tokio = &rust::import("chronicle", "tokio");

    let bootstrap = rust::import("chronicle::bootstrap", "bootstrap");
    let chronicle_graphql = rust::import("chronicle::api::chronicle_graphql", "ChronicleGraphQl");

    quote! {
    #(gen_attribute_scalars(&domain.attributes))
    #(gen_type_enums(domain))
    #(gen_abstract_prov_attributes())
    #(for agent in domain.agents.iter() => #(gen_attribute_definition(agent, &agent.attributes)))
    #(for activity in domain.activities.iter() => #(gen_attribute_definition(activity, &activity.attributes)))
    #(for entity in domain.entities.iter() => #(gen_attribute_definition(entity, &entity.attributes)))
    #(gen_agent_union(&domain.agents))
    #(gen_entity_union(&domain.entities))
    #(gen_activity_union(&domain.activities))
    #(gen_mappers(domain))
    #(gen_agent_definition(&prov_agent))
    #(gen_activity_definition(&prov_activity))
    #(gen_entity_definition(&prov_entity))
    #(for agent in domain.agents.iter() => #(gen_agent_definition(agent)))
    #(for activity in domain.activities.iter() => #(gen_activity_definition(activity)))
    #(for entity in domain.entities.iter() => #(gen_entity_definition(entity)))
    #(gen_query())
    #(gen_mutation(domain))

    #[#tokio::main]
    pub async fn main() {
        let model = #builder::new("chronicle")
        .with_attribute_type("string", #primitive_type::String)
        .unwrap()
        .with_attribute_type("int", #primitive_type::Int)
        .unwrap()
        .with_attribute_type("bool", #primitive_type::Bool)
        .unwrap()
        .with_entity("octopi", |b| {
            b.with_attribute("string")
                .unwrap()
                .with_attribute("int")
                .unwrap()
                .with_attribute("bool")
        })
        .unwrap()
        .with_entity("the sea", |b| {
            b.with_attribute("string")
                .unwrap()
                .with_attribute("int")
                .unwrap()
                .with_attribute("bool")
        })
        .unwrap()
        .with_activity("gardening", |b| {
            b.with_attribute("string")
                .unwrap()
                .with_attribute("int")
                .unwrap()
                .with_attribute("bool")
        })
        .unwrap()
        .with_activity("swim about", |b| {
            b.with_attribute("string")
                .unwrap()
                .with_attribute("int")
                .unwrap()
                .with_attribute("bool")
        })
        .unwrap()
        .with_agent("friends", |b| {
            b.with_attribute("string")
                .unwrap()
                .with_attribute("int")
                .unwrap()
                .with_attribute("bool")
        })
        .unwrap()
        .build();

        #bootstrap(model, #chronicle_graphql::new(Query, Mutation)).await
    }

    }
}

pub fn generate_chronicle_domain_schema(domain: ChronicleDomainDef, path: impl AsRef<Path>) {
    let tokens = gen_graphql_type(&domain);

    path.as_ref().parent().map(std::fs::create_dir_all);
    let mut f = std::fs::File::create(path).unwrap();
    f.write_all(tokens.to_file_string().unwrap().as_bytes())
        .unwrap();

    f.flush().unwrap();
}
