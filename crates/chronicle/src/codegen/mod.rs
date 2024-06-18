#![allow(dead_code)]

use std::{io::Write, path::Path};

use genco::prelude::*;

pub use common::domain::{
	ActivityDef, AgentDef, AttributeDef, AttributesTypeName, Builder, ChronicleDomainDef, CliName,
	EntityDef, PrimitiveType, Property, TypeName,
};

pub mod linter;

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
	let chronicle_json = &rust::import("chronicle::common::prov", "ChronicleJSON");
	quote! {
		#(for attribute in attributes.iter() =>
		#[derive(Clone, #graphql_new_type)]
		#[graphql(name = #_(#(attribute.as_scalar_type())), visible=true)]
		#(if attribute.doc.is_some() {
			#[doc = #_(#(attribute.doc.as_ref().map(|s| s.to_owned()).unwrap_or_default()))]
		})
		pub struct #(attribute.as_scalar_type())(#(match attribute.primitive_type {
				PrimitiveType::String => String,
				PrimitiveType::Bool => bool,
				PrimitiveType::Int => i32,
				PrimitiveType::JSON => #chronicle_json,
			}
		));
	   )
	}
}

fn gen_association_and_attribution_unions() -> rust::Tokens {
	let simple_object = &rust::import("chronicle::async_graphql", "SimpleObject").qualified();

	let agent_ref_doc = include_str!("../../../../domain_docs/agent_ref.md");
	let association_doc = include_str!("../../../../domain_docs/association.md");
	let attribution_doc = include_str!("../../../../domain_docs/attribution.md");
	let entity_ref_doc = include_str!("../../../../domain_docs/entity_ref.md");

	quote! {

	#[doc = #_(#agent_ref_doc)]
	#[derive(#simple_object)]
	pub struct AgentRef {
		pub role: RoleType,
		pub agent: Agent,
	}

	#[doc = #_(#entity_ref_doc)]
	#[derive(#simple_object)]
	pub struct EntityRef {
		pub role: RoleType,
		pub entity: Entity,
	}

	#[doc = #_(#association_doc)]
	#[derive(#simple_object)]
	pub struct Association {
		pub responsible : AgentRef,
		pub delegate: Option<AgentRef>,
	}

	#[doc = #_(#attribution_doc)]
	#[derive(#simple_object)]
	pub struct Attribution {
		pub responsible : AgentRef,
	}

	#[doc = #_(#attribution_doc)]
	#[derive(#simple_object)]
	pub struct Attributed {
		pub attributed : EntityRef,
	}
	}
}

fn gen_type_enums(domain: &ChronicleDomainDef) -> rust::Tokens {
	let graphql_enum = &rust::import("chronicle::async_graphql", "Enum");
	let domain_type_id = &rust::import("chronicle::common::prov", "DomaintypeId");
	let prov_role = &rust::import("chronicle::common::prov", "Role").qualified();

	let activity_type_doc = include_str!("../../../../domain_docs/activity_type.md");
	let agent_type_doc = include_str!("../../../../domain_docs/agent_type.md");
	let entity_type_doc = include_str!("../../../../domain_docs/entity_type.md");
	let prov_activity_doc = include_str!("../../../../domain_docs/prov_activity.md");
	let prov_agent_doc = include_str!("../../../../domain_docs/prov_agent.md");
	let prov_entity_doc = include_str!("../../../../domain_docs/prov_entity.md");
	let role_doc = include_str!("../../../../domain_docs/role.md");
	let unspecified_doc = include_str!("../../../../domain_docs/unspecified.md");

	quote! {
		#[derive(#graphql_enum, Copy, Clone, Eq, PartialEq)]
		#[allow(clippy::upper_case_acronyms)]
		#[doc = #_(#role_doc)]
		#(if domain.roles_doc.is_some() {
			#[doc = ""]
			#[doc = #_(#(domain.roles_doc.as_ref().map(|s| s.to_owned()).unwrap_or_default()))]
		})
		pub enum RoleType {
			#[doc = #_(#unspecified_doc)]
			Unspecified,
			#(for role in domain.roles.iter() =>
			#[graphql(name = #_(#(role.preserve_inflection())), visible=true)]
				#(role.as_type_name()),
			)
		}

		#[allow(clippy::from_over_into)]
		impl Into<RoleType> for Option<#prov_role> {
			fn into(self) -> RoleType {
				match self.as_ref().map(|x| x.as_str()) {
				None => {RoleType::Unspecified}
				#(for role in domain.roles.iter() =>
				   Some(#_(#(role.preserve_inflection()))) => { RoleType::#(role.as_type_name())}
				)
				Some(&_) => {RoleType::Unspecified}
				}
			}
		}

		#[allow(clippy::from_over_into)]
		impl Into<Option<#prov_role>> for RoleType {
			fn into(self) -> Option<#prov_role> {
				match self {
				Self::Unspecified => None,
				#(for role in domain.roles.iter() =>
					RoleType::#(role.as_type_name()) => {
						Some(#prov_role::from(#_(#(role.preserve_inflection()))))
					}
				)
			}
			}
		}

		#[derive(#graphql_enum, Copy, Clone, Eq, PartialEq)]
		#[doc = #_(#agent_type_doc)]
		#[allow(clippy::upper_case_acronyms)]
		#[allow(clippy::enum_variant_names)]
		pub enum AgentType {
			#[doc = #_(#prov_agent_doc)]
			#[graphql(name = "ProvAgent", visible=true)]
			ProvAgent,
			#(for agent in domain.agents.iter() =>
			#(if agent.doc.is_some() {
				#[doc = #_(#(agent.doc.as_ref().map(|s| s.to_owned()).unwrap_or_default()))]
			})
			#[graphql(name = #_(#(agent.as_type_name())), visible=true)]
				#(agent.as_type_name()),
			)
		}

		#[allow(clippy::from_over_into)]
		impl Into<Option<#domain_type_id>> for AgentType {
			fn into(self) -> Option<#domain_type_id> {
				match self {
				#(for agent in domain.agents.iter() =>
				AgentType::#(agent.as_type_name()) => Some(#domain_type_id::from_external_id(#_(#(agent.as_type_name())))),
				)
				AgentType::ProvAgent => None
				}
			}
		}

		#[derive(#graphql_enum, Copy, Clone, Eq, PartialEq)]
		#[doc = #_(#entity_type_doc)]
		#[allow(clippy::upper_case_acronyms)]
		#[allow(clippy::enum_variant_names)]
		pub enum EntityType {
			#[doc = #_(#prov_entity_doc)]
			#[graphql(name = "ProvEntity", visible=true)]
			ProvEntity,
			#(for entity in domain.entities.iter() =>
			#(if entity.doc.is_some() {
				#[doc = #_(#(entity.doc.as_ref().map(|s| s.to_owned()).unwrap_or_default()))]
			})
			#[graphql(name = #_(#(entity.as_type_name())), visible=true)]
				#(entity.as_type_name()),
			)
		}

		#[allow(clippy::from_over_into)]
		impl Into<Option<#domain_type_id>> for EntityType {
			fn into(self) -> Option<#domain_type_id> {
				match self {
				#(for entity in domain.entities.iter() =>
				EntityType::#(entity.as_type_name()) => Some(#domain_type_id::from_external_id(#_(#(entity.as_type_name())))),
				)
				EntityType::ProvEntity => None
				}
			}
		}

		#[derive(#graphql_enum, Copy, Clone, Eq, PartialEq)]
		#[doc = #_(#activity_type_doc)]
		#[allow(clippy::upper_case_acronyms)]
		#[allow(clippy::enum_variant_names)]
		pub enum ActivityType {
			#[doc = #_(#prov_activity_doc)]
			#[graphql(name = "ProvActivity", visible=true)]
			ProvActivity,
			#(for activity in domain.activities.iter() =>
			#(if activity.doc.is_some() {
				#[doc = #_(#(activity.doc.as_ref().map(|s| s.to_owned()).unwrap_or_default()))]
			})
			#[graphql(name = #_(#(activity.as_type_name())), visible=true)]
				#(activity.as_type_name()),
			)
		}

		#[allow(clippy::from_over_into)]
		impl Into<Option<#domain_type_id>> for ActivityType {
			fn into(self) -> Option<#domain_type_id> {
				match self {
				#(for activity in domain.activities.iter() =>
				ActivityType::#(activity.as_type_name()) => Some(#domain_type_id::from_external_id(#_(#(activity.as_type_name())))),
				)
				ActivityType::ProvActivity => None
				}
			}
		}
	}
}

fn gen_agent_union(agents: &[AgentDef]) -> rust::Tokens {
	let union_macro = rust::import("chronicle::async_graphql", "Union").qualified();

	let agent_doc = include_str!("../../../../domain_docs/agent.md");
	let prov_agent_doc = include_str!("../../../../domain_docs/prov_agent.md");

	quote! {
		#[doc = #_(#agent_doc)]
		#[allow(clippy::enum_variant_names)]
		#[allow(clippy::upper_case_acronyms)]
		#[derive(#union_macro)]
		pub enum Agent {
			#[doc = #_(#prov_agent_doc)]
			ProvAgent(ProvAgent),
			#(for agent in agents =>
				#(if agent.doc.is_some() {
					#[doc = #_(#(agent.doc.as_ref().map(|s| s.to_owned()).unwrap_or_default()))]
				})
				#(&agent.as_type_name())(#(&agent.as_type_name())),
			)
		}
	}
}

fn gen_entity_union(entities: &[EntityDef]) -> rust::Tokens {
	let union_macro = rust::import("chronicle::async_graphql", "Union").qualified();

	let entity_doc = include_str!("../../../../domain_docs/entity.md");
	let prov_entity_doc = include_str!("../../../../domain_docs/prov_entity.md");

	quote! {
		#[doc = #_(#entity_doc)]
		#[allow(clippy::enum_variant_names)]
		#[allow(clippy::upper_case_acronyms)]
		#[derive(#union_macro)]
		pub enum Entity {
			#[doc = #_(#prov_entity_doc)]
			ProvEntity(ProvEntity),
			#(for entity in entities =>
				#(if entity.doc.is_some() {
					#[doc = #_(#(entity.doc.as_ref().map(|s| s.to_owned()).unwrap_or_default()))]
				})
				#(&entity.as_type_name())(#(&entity.as_type_name())),
			)
		}
	}
}

fn gen_activity_union(activities: &[ActivityDef]) -> rust::Tokens {
	let union_macro = rust::import("chronicle::async_graphql", "Union").qualified();

	let activity_doc = include_str!("../../../../domain_docs/activity.md");
	let prov_activity_doc = include_str!("../../../../domain_docs/prov_activity.md");

	quote! {
		#[doc = #_(#activity_doc)]
		#[allow(clippy::enum_variant_names)]
		#[allow(clippy::upper_case_acronyms)]
		#[derive(#union_macro)]
		pub enum Activity {
			#[doc = #_(#prov_activity_doc)]
			ProvActivity(ProvActivity),
			#(for activity in activities =>
				#(if activity.doc.is_some() {
					#[doc = #_(#(activity.doc.as_ref().map(|s| s.to_owned()).unwrap_or_default()))]
				})
				#(&activity.as_type_name())(#(&activity.as_type_name())),
			)
		}
	}
}

fn gen_activity_definition(activity: &ActivityDef) -> rust::Tokens {
	let abstract_activity =
		&rust::import("chronicle::persistence::queryable", "Activity").qualified();
	let activity_impl = &rust::import("chronicle::api::chronicle_graphql", "activity").qualified();
	let namespace = &rust::import("chronicle::persistence::queryable", "Namespace").qualified();
	let activity_id = &rust::import("chronicle::common::prov", "ActivityId").qualified();
	let async_graphql_error_extensions =
		&rust::import("chronicle::async_graphql", "ErrorExtensions").qualified();

	let timezone = &rust::import("chronicle::chrono", "TimeZone").direct();
	let object = rust::import("chronicle::async_graphql", "Object").qualified();
	let async_result = &rust::import("chronicle::async_graphql", "Result").qualified();
	let context = &rust::import("chronicle::async_graphql", "Context").qualified();
	let domain_type_id = &rust::import("chronicle::common::prov", "DomaintypeId");
	let date_time = &rust::import("chronicle::chrono", "DateTime");
	let utc = &rust::import("chronicle::chrono", "Utc");
	let chronicle_json = &rust::import("chronicle::common::prov", "ChronicleJSON");

	let end_doc = include_str!("../../../../domain_docs/end.md");
	let external_id_doc = include_str!("../../../../domain_docs/external_id.md");
	let generated_doc = include_str!("../../../../domain_docs/generated.md");
	let id_doc = include_str!("../../../../domain_docs/id.md");
	let namespace_doc = include_str!("../../../../domain_docs/namespace.md");
	let start_doc = include_str!("../../../../domain_docs/start.md");
	let type_doc = include_str!("../../../../domain_docs/type.md");
	let used_doc = include_str!("../../../../domain_docs/used.md");
	let was_associated_with_doc = include_str!("../../../../domain_docs/was_associated_with.md");
	let was_informed_by_doc = include_str!("../../../../domain_docs/was_informed_by.md");

	quote! {
	#(register(activity_impl))

	#[allow(clippy::upper_case_acronyms)]
	pub struct #(&activity.as_type_name())(#abstract_activity);

	#[#object(name = #_(#(activity.as_type_name())))]
	#(if activity.doc.is_some() {
		#[doc = #_(#(activity.doc.as_ref().map(|s| s.to_owned()).unwrap_or_default()))]
	})
	impl #(&activity.as_type_name()) {
		#[doc = #_(#id_doc)]
		async fn id(&self) -> #activity_id {
			#activity_id::from_external_id(&*self.0.external_id)
		}

		#[doc = #_(#namespace_doc)]
		async fn namespace<'a>(&self, ctx: &#context<'a>) -> #async_result<#namespace> {
			#activity_impl::namespace(self.0.namespace_id, ctx).await.map_err(|e| #async_graphql_error_extensions::extend(&e))
		}

		#[doc = #_(#external_id_doc)]
		async fn external_id(&self) -> &str {
			&self.0.external_id
		}

		#[doc = #_(#start_doc)]
		async fn started(&self) -> Option<#date_time<#utc>> {
			self.0.started.as_ref().map(|x| #timezone::from_utc_datetime(&#utc,x))
		}

		#[doc = #_(#end_doc)]
		async fn ended(&self) -> Option<#date_time<#utc>> {
			self.0.ended.as_ref().map(|x| #timezone::from_utc_datetime(&#utc,x))
		}

		#[doc = #_(#type_doc)]
		#[graphql(name = "type")]
		async fn typ(&self) -> Option<#domain_type_id> {
			self.0.domaintype.as_deref().map(#domain_type_id::from_external_id)
		}

		#[doc = #_(#was_associated_with_doc)]
		async fn was_associated_with<'a>(
			&self,
			ctx: &#context<'a>,
		) -> #async_result<Vec<Association>> {
			Ok(
				#activity_impl::was_associated_with(self.0.id, ctx)
					.await
					.map_err(|e| #async_graphql_error_extensions::extend(&e))?
					.into_iter()
					.map(|(r_agent, r_role, d_agent, d_role)| map_association_to_role(r_agent, d_agent, r_role, d_role))
					.collect(),
			)
		}

		#[doc = #_(#used_doc)]
		async fn used<'a>(&self, ctx: &#context<'a>) -> #async_result<Vec<#(entity_union_type_name())>> {
			Ok(#activity_impl::used(self.0.id, ctx)
				.await
				.map_err(|e| #async_graphql_error_extensions::extend(&e))?
				.into_iter()
				.map(map_entity_to_domain_type)
				.collect())
		}

		#[doc = #_(#was_informed_by_doc)]
		async fn was_informed_by<'a>(&self, ctx: &#context<'a>) -> #async_result<Vec<#(activity_union_type_name())>> {
			Ok(#activity_impl::was_informed_by(self.0.id, ctx)
				.await
				.map_err(|e| #async_graphql_error_extensions::extend(&e))?
				.into_iter()
				.map(map_activity_to_domain_type)
				.collect())
		}

		#[doc = #_(#generated_doc)]
		async fn generated<'a>(
			&self,
			ctx: &#context<'a>,
		) -> #async_result<Vec<#(entity_union_type_name())>> {
			Ok(#activity_impl::generated(self.0.id, ctx)
				.await
				.map_err(|e| #async_graphql_error_extensions::extend(&e))?
				.into_iter()
				.map(map_entity_to_domain_type)
				.collect())
		}

		#(for attribute in &activity.attributes =>
		#[graphql(name = #_(#(attribute.preserve_inflection())))]
		#(if attribute.doc.is_some() {
			#[doc = #_(#(attribute.doc.as_ref().map(|s| s.to_owned()).unwrap_or_default()))]
		})
		async fn #(attribute.as_property())<'a>(&self, ctx: &#context<'a>) -> #async_result<Option<#(attribute.as_scalar_type())>> {
			Ok(#(match attribute.primitive_type {
			  PrimitiveType::String =>
				#activity_impl::load_attribute(self.0.id, #_(#(attribute.preserve_inflection())), ctx)
					.await
					.map_err(|e| #async_graphql_error_extensions::extend(&e))?
					.and_then(|attr| attr.as_str().map(|attr| attr.to_owned()))
					.map(#(attribute.as_scalar_type())),
			  PrimitiveType::Bool =>
				#activity_impl::load_attribute(self.0.id, #_(#(attribute.preserve_inflection())), ctx)
					.await
					.map_err(|e| #async_graphql_error_extensions::extend(&e))?
					.and_then(|attr| attr.as_bool())
					.map(#(attribute.as_scalar_type())),
			  PrimitiveType::Int =>
				#activity_impl::load_attribute(self.0.id, #_(#(attribute.preserve_inflection())), ctx)
					.await
					.map_err(|e| #async_graphql_error_extensions::extend(&e))?
					.and_then(|attr| attr.as_i64().map(|attr| attr as _))
					.map(#(attribute.as_scalar_type())),
			  PrimitiveType::JSON =>
				#activity_impl::load_attribute(self.0.id, #_(#(attribute.preserve_inflection())), ctx)
					.await
					.map_err(|e| #async_graphql_error_extensions::extend(&e))?
					.map(#chronicle_json)
					.map(#(attribute.as_scalar_type()))
		}))
		})
	}
	}
}

fn gen_entity_definition(entity: &EntityDef) -> rust::Tokens {
	let abstract_entity = &rust::import("chronicle::persistence::queryable", "Entity").qualified();
	let entity_impl = &rust::import("chronicle::api::chronicle_graphql", "entity").qualified();
	let namespace = &rust::import("chronicle::persistence::queryable", "Namespace").qualified();
	let entity_id = &rust::import("chronicle::common::prov", "EntityId").qualified();

	let object = rust::import("chronicle::async_graphql", "Object").qualified();
	let async_result = &rust::import("chronicle::async_graphql", "Result").qualified();
	let context = &rust::import("chronicle::async_graphql", "Context").qualified();
	let domain_type_id = &rust::import("chronicle::common::prov", "DomaintypeId");
	let chronicle_json = &rust::import("chronicle::common::prov", "ChronicleJSON");
	let async_graphql_error_extensions =
		&rust::import("chronicle::async_graphql", "ErrorExtensions").qualified();

	let external_id_doc = include_str!("../../../../domain_docs/external_id.md");
	let had_primary_source_doc = include_str!("../../../../domain_docs/had_primary_source.md");
	let id_doc = include_str!("../../../../domain_docs/id.md");
	let namespace_doc = include_str!("../../../../domain_docs/namespace.md");
	let type_doc = include_str!("../../../../domain_docs/type.md");
	let was_attributed_to_doc = include_str!("../../../../domain_docs/was_attributed_to.md");
	let was_derived_from_doc = include_str!("../../../../domain_docs/was_derived_from.md");
	let was_generated_by_doc = include_str!("../../../../domain_docs/was_generated_by.md");
	let was_quoted_from_doc = include_str!("../../../../domain_docs/was_quoted_from.md");
	let was_revision_of_doc = include_str!("../../../../domain_docs/was_revision_of.md");

	quote! {

	#(register(entity_impl))
	#[allow(clippy::upper_case_acronyms)]
	pub struct #(&entity.as_type_name())(#abstract_entity);

	#[#object(name = #_(#(entity.as_type_name())))]
	#(if entity.doc.is_some() {
		#[doc = #_(#(entity.doc.as_ref().map(|s| s.to_owned()).unwrap_or_default()))]
	})
	impl #(&entity.as_type_name()){
		#[doc = #_(#id_doc)]
		async fn id(&self) -> #entity_id {
			#entity_id::from_external_id(&*self.0.external_id)
		}

		#[doc = #_(#namespace_doc)]
		async fn namespace<'a>(&self, ctx: &#context<'a>) -> #async_result<#namespace> {
			#entity_impl::namespace(self.0.namespace_id, ctx).await.map_err(|e| #async_graphql_error_extensions::extend(&e))
		}

		#[doc = #_(#external_id_doc)]
		async fn external_id(&self) -> &str {
			&self.0.external_id
		}

		#[doc = #_(#type_doc)]
		#[graphql(name = "type")]
		async fn typ(&self) -> Option<#domain_type_id> {
			self.0.domaintype.as_deref().map(#domain_type_id::from_external_id)
		}

		#[doc = #_(#was_attributed_to_doc)]
		async fn was_attributed_to<'a>(
			&self,
			ctx: &#context<'a>,
		) -> #async_result<Vec<Attribution>> {
			Ok(
				#entity_impl::was_attributed_to(self.0.id, ctx)
					.await
					.map_err(|e| #async_graphql_error_extensions::extend(&e))?
					.into_iter()
					.map(|(agent, role)| map_attribution_to_role(agent, role))
					.collect(),
			)
		}

		#[doc = #_(#was_generated_by_doc)]
		async fn was_generated_by<'a>(
			&self,
			ctx: &#context<'a>,
		) -> #async_result<Vec<#(activity_union_type_name())>> {
			Ok(#entity_impl::was_generated_by(self.0.id, ctx)
				.await
				.map_err(|e| #async_graphql_error_extensions::extend(&e))?
				.into_iter()
				.map(map_activity_to_domain_type)
				.collect())
		}

		#[doc = #_(#was_derived_from_doc)]
		async fn was_derived_from<'a>(&self, ctx: &#context<'a>) -> #async_result<Vec<#(entity_union_type_name())>> {
			Ok(#entity_impl::was_derived_from(self.0.id, ctx)
				.await
				.map_err(|e| #async_graphql_error_extensions::extend(&e))?
				.into_iter()
				.map(map_entity_to_domain_type)
				.collect())
		}

		#[doc = #_(#had_primary_source_doc)]
		async fn had_primary_source<'a>(
			&self,
			ctx: &#context<'a>,
		) -> #async_result<Vec<#(entity_union_type_name())>> {
			Ok(
				#entity_impl::had_primary_source(self.0.id, ctx)
					.await
					.map_err(|e| #async_graphql_error_extensions::extend(&e))?
					.into_iter()
					.map(map_entity_to_domain_type)
					.collect(),
			)
		}

		#[doc = #_(#was_revision_of_doc)]
		async fn was_revision_of<'a>(&self, ctx: &#context<'a>) -> #async_result<Vec<#(entity_union_type_name())>> {
			Ok(#entity_impl::was_revision_of(self.0.id, ctx)
				.await
				.map_err(|e| #async_graphql_error_extensions::extend(&e))?
				.into_iter()
				.map(map_entity_to_domain_type)
				.collect())
		}

		#[doc = #_(#was_quoted_from_doc)]
		async fn was_quoted_from<'a>(&self, ctx: &#context<'a>) -> #async_result<Vec<#(entity_union_type_name())>> {
			Ok(#entity_impl::was_quoted_from(self.0.id, ctx)
				.await
				.map_err(|e| #async_graphql_error_extensions::extend(&e))?
				.into_iter()
				.map(map_entity_to_domain_type)
				.collect())
		}

		#(for attribute in &entity.attributes =>
		#(if attribute.doc.is_some() {
			#[doc = #_(#(attribute.doc.as_ref().map(|s| s.to_owned()).unwrap_or_default()))]
		})
		#[graphql(name = #_(#(attribute.preserve_inflection())))]
		async fn #(attribute.as_property())<'a>(&self, ctx: &#context<'a>) -> #async_result<Option<#(attribute.as_scalar_type())>> {
			Ok(#(match attribute.primitive_type {
			  PrimitiveType::String =>
				#entity_impl::load_attribute(self.0.id, #_(#(attribute.preserve_inflection())), ctx)
					.await
					.map_err(|e| #async_graphql_error_extensions::extend(&e))?
					.and_then(|attr| attr.as_str().map(|attr| attr.to_owned()))
					.map(#(attribute.as_scalar_type())),
			  PrimitiveType::Bool =>
				#entity_impl::load_attribute(self.0.id, #_(#(attribute.preserve_inflection())), ctx)
					.await
					.map_err(|e| #async_graphql_error_extensions::extend(&e))?
					.and_then(|attr| attr.as_bool())
					.map(#(attribute.as_scalar_type())),
			  PrimitiveType::Int =>
				#entity_impl::load_attribute(self.0.id, #_(#(attribute.preserve_inflection())), ctx)
					.await
					.map_err(|e| #async_graphql_error_extensions::extend(&e))?
					.and_then(|attr| attr.as_i64().map(|attr| attr as _))
					.map(#(attribute.as_scalar_type())),
			PrimitiveType::JSON =>
				#entity_impl::load_attribute(self.0.id, #_(#(attribute.preserve_inflection())), ctx)
					.await
					.map_err(|e| #async_graphql_error_extensions::extend(&e))?
					.map(#chronicle_json)
					.map(#(attribute.as_scalar_type()))
				}))
			})
		}
	}
}

fn gen_agent_definition(agent: &AgentDef) -> rust::Tokens {
	let abstract_agent = &rust::import("chronicle::persistence::queryable", "Agent").qualified();
	let agent_impl = &rust::import("chronicle::api::chronicle_graphql", "agent").qualified();
	let namespace = &rust::import("chronicle::persistence::queryable", "Namespace").qualified();
	let agent_union_type = &agent_union_type_name();
	let object = rust::import("chronicle::async_graphql", "Object").qualified();
	let async_result = &rust::import("chronicle::async_graphql", "Result").qualified();
	let context = &rust::import("chronicle::async_graphql", "Context").qualified();
	let agent_id = &rust::import("chronicle::common::prov", "AgentId");
	let domain_type_id = &rust::import("chronicle::common::prov", "DomaintypeId");
	let chronicle_json = &rust::import("chronicle::common::prov", "ChronicleJSON");
	let async_graphql_error_extensions =
		&rust::import("chronicle::async_graphql", "ErrorExtensions").qualified();

	let acted_on_behalf_of_doc = include_str!("../../../../domain_docs/acted_on_behalf_of.md");
	let attribution_doc = include_str!("../../../../domain_docs/attribution.md");
	let external_id_doc = include_str!("../../../../domain_docs/external_id.md");
	let id_doc = include_str!("../../../../domain_docs/id.md");
	let namespace_doc = include_str!("../../../../domain_docs/namespace.md");
	let type_doc = include_str!("../../../../domain_docs/type.md");

	quote! {

	#(register(agent_impl))

	#[allow(clippy::upper_case_acronyms)]
	pub struct #(agent.as_type_name())(#abstract_agent);

	#[#object(name = #_(#(agent.as_type_name())))]
	#(if agent.doc.is_some() {
		#[doc = #_(#(agent.doc.as_ref().map(|s| s.to_owned()).unwrap_or_default()))]
	})
	impl #(agent.as_type_name()) {
		#[doc = #_(#id_doc)]
		async fn id(&self) -> #agent_id {
			#agent_id::from_external_id(&*self.0.external_id)
		}

		#[doc = #_(#external_id_doc)]
		async fn external_id(&self) -> &str {
			&self.0.external_id
		}

		#[doc = #_(#namespace_doc)]
		async fn namespace<'a>(&self, ctx: &#context<'a>) -> #async_result<#namespace> {
			#agent_impl::namespace(self.0.namespace_id, ctx).await.map_err(|e| #async_graphql_error_extensions::extend(&e))
		}

		#[doc = #_(#acted_on_behalf_of_doc)]
		async fn acted_on_behalf_of<'a>(&self, ctx: &#context<'a>) -> #async_result<Vec<AgentRef>> {
			Ok(#agent_impl::acted_on_behalf_of(self.0.id, ctx)
				.await
				.map_err(|e| #async_graphql_error_extensions::extend(&e))?
				.into_iter()
				.map(|(agent,role)|(Self(agent),role))
				.map(|(agent,role)| AgentRef {agent : #agent_union_type::from(agent), role: role.into()})
				.collect())
		}

		#[doc = #_(#attribution_doc)]
		async fn attribution<'a>(&self, ctx: &#context<'a>) -> #async_result<Vec<Attributed>> {
			Ok(#agent_impl::attribution(self.0.id, ctx)
				.await
				.map_err(|e| #async_graphql_error_extensions::extend(&e))?
				.into_iter()
				.map(|(entity, role)| {
					map_attributed_to_role(entity, role)
				})
				.collect())
		}

		#(for attribute in &agent.attributes =>
		#(if attribute.doc.is_some() {
			#[doc = #_(#(attribute.doc.as_ref().map(|s| s.to_owned()).unwrap_or_default()))]
		})
		#[graphql(name = #_(#(attribute.preserve_inflection())))]
		async fn #(attribute.as_property())<'a>(&self, ctx: &#context<'a>) -> #async_result<Option<#(attribute.as_scalar_type())>> {
			Ok(#(match attribute.primitive_type {
			  PrimitiveType::String =>
				#agent_impl::load_attribute(self.0.id, #_(#(attribute.preserve_inflection())), ctx)
					.await
					.map_err(|e| #async_graphql_error_extensions::extend(&e))?
					.and_then(|attr| attr.as_str().map(|attr| attr.to_owned()))
					.map(#(attribute.as_scalar_type())),
			  PrimitiveType::Bool =>
				#agent_impl::load_attribute(self.0.id, #_(#(attribute.preserve_inflection())), ctx)
					.await
					.map_err(|e| #async_graphql_error_extensions::extend(&e))?
					.and_then(|attr| attr.as_bool())
					.map(#(attribute.as_scalar_type())),
			  PrimitiveType::Int =>
				#agent_impl::load_attribute(self.0.id, #_(#(attribute.preserve_inflection())), ctx)
					.await
					.map_err(|e| #async_graphql_error_extensions::extend(&e))?
					.and_then(|attr| attr.as_i64().map(|attr| attr as _))
					.map(#(attribute.as_scalar_type())),
			  PrimitiveType::JSON =>
				#agent_impl::load_attribute(self.0.id, #_(#(attribute.preserve_inflection())), ctx)
					.await
					.map_err(|e| #async_graphql_error_extensions::extend(&e))?
					.map(#chronicle_json)
					.map(#(attribute.as_scalar_type()))
			}))
		})

		#[doc = #_(#type_doc)]
		#[graphql(name = "type")]
		async fn typ(&self) -> Option<#domain_type_id> {
			self.0.domaintype.as_deref().map(#domain_type_id::from_external_id)
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
			Self::new(
				attributes.typ.map(#domain_type_id::from_external_id),
				vec![]
			)
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
			Self::new(
				attributes.typ.map(#domain_type_id::from_external_id),
				vec![]
			)
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
			Self::new(
				attributes.typ.map(#domain_type_id::from_external_id),
				vec![]
			)
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

	if attributes.is_empty() {
		return quote! {};
	}

	quote! {
		#[derive(#input_object)]
		#[graphql(name = #_(#(typ.attributes_type_name_preserve_inflection())))]
		pub struct #(typ.attributes_type_name_preserve_inflection()) {
			#(for attribute in attributes =>
				#[graphql(name = #_(#(attribute.preserve_inflection())))]
				pub #(&attribute.as_property()): #(
					match attribute.primitive_type {
						PrimitiveType::String => String,
						PrimitiveType::Bool => bool,
						PrimitiveType::Int => i32,
						PrimitiveType::JSON => Value,
					}),
			)
		}


		#[allow(clippy::from_over_into)]
		#[allow(clippy::useless_conversion)]
		impl From<#(typ.attributes_type_name_preserve_inflection())> for #abstract_attributes{
			fn from(attributes: #(typ.attributes_type_name_preserve_inflection())) -> Self {
				#abstract_attributes::new(
					Some(#domain_type_id::from_external_id(#_(#(typ.as_type_name())))),
					vec![
					 #(for attribute in attributes =>
						(
							#abstract_attribute::new(#_(#(&attribute.preserve_inflection())),
							#serde_value::from(attributes.#(&attribute.as_property())))),
						)
					]
				)
			}
		}
	}
}

fn gen_mappers(domain: &ChronicleDomainDef) -> rust::Tokens {
	let agent_impl = &rust::import("chronicle::persistence::queryable", "Agent").qualified();
	let role = &rust::import("chronicle::common::prov", "Role").qualified();
	let entity_impl = &rust::import("chronicle::persistence::queryable", "Entity").qualified();
	let activity_impl = &rust::import("chronicle::persistence::queryable", "Activity").qualified();

	quote! {
	#[allow(clippy::match_single_binding)]
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
	/// Maps to an association, missing roles, or ones that are no longer specified in the domain will be returned as RoleType::Unspecified
	fn map_association_to_role(responsible: #agent_impl, delegate: Option<#agent_impl>, responsible_role: Option<#role>, delegate_role: Option<#role>) -> Association {
		Association {
			responsible: match responsible_role.as_ref().map(|x| x.as_str()) {
				None => {
					AgentRef{ agent: map_agent_to_domain_type(responsible), role: RoleType::Unspecified }
				},
				#(for role in domain.roles.iter() =>
				Some(#_(#(&role.preserve_inflection()))) => {AgentRef { role: RoleType::#(role.as_type_name()),
					agent: map_agent_to_domain_type(responsible)
				}}
				)
				Some(&_) => {
					AgentRef{ agent: map_agent_to_domain_type(responsible), role: RoleType::Unspecified }
				}
			},
			delegate: match (delegate,delegate_role.as_ref().map(|x| x.as_str())) {
				(None,_) => None,
				(Some(delegate), None) => {
					Some(AgentRef{role: RoleType::Unspecified, agent: map_agent_to_domain_type(delegate)})
				},
				#(for role in domain.roles.iter() =>
				(Some(delegate),Some(#_(#(&role.preserve_inflection())))) => {
					Some(AgentRef{ role: RoleType::#(role.as_type_name()),
					 agent: map_agent_to_domain_type(delegate)
				})})
				(Some(delegate), Some(&_)) => {
					Some(AgentRef{ role: RoleType::Unspecified, agent: map_agent_to_domain_type(delegate)})
				},
			}
		}
	}
	/// Maps an `Agent` and, if applicable, `Role` to an attribution. Missing roles, or ones that are no longer specified in the domain, will be returned as `RoleType::Unspecified`
	fn map_attribution_to_role(responsible: #agent_impl, responsible_role: Option<#role>) -> Attribution {
		Attribution {
			responsible: match responsible_role.as_ref().map(|x| x.as_str()) {
				None => {
					AgentRef{ agent: map_agent_to_domain_type(responsible), role: RoleType::Unspecified }
				},
				#(for role in domain.roles.iter() =>
				Some(#_(#(&role.preserve_inflection()))) => {AgentRef { role: RoleType::#(role.as_type_name()),
					agent: map_agent_to_domain_type(responsible)
				}}
				)
				Some(&_) => {
					AgentRef{ agent: map_agent_to_domain_type(responsible), role: RoleType::Unspecified }
				}
			}
		}
	}
	/// Maps to an attribution. Missing roles, or ones that are no longer specified in the domain, will be returned as `RoleType::Unspecified`
	fn map_attributed_to_role(entity: #entity_impl, attributed_role: Option<#role>) -> Attributed {
		Attributed {
			attributed: match attributed_role.as_ref().map(|x| x.as_str()) {
				None => {
					EntityRef{ entity: map_entity_to_domain_type(entity), role: RoleType::Unspecified }
				},
				#(for role in domain.roles.iter() =>
				Some(#_(#(&role.preserve_inflection()))) => {EntityRef { role: RoleType::#(role.as_type_name()),
					entity: map_entity_to_domain_type(entity)
				}}
				)
				Some(&_) => {
					EntityRef{ entity: map_entity_to_domain_type(entity), role: RoleType::Unspecified }
				}
			}
		}
	}
	#[allow(clippy::match_single_binding)]
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
	#[allow(clippy::match_single_binding)]
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
	let async_graphql_error_extensions =
		&rust::import("chronicle::async_graphql", "ErrorExtensions").qualified();

	let agent_id = &rust::import("chronicle::common::prov", "AgentIdOrExternal");
	let entity_id = &rust::import("chronicle::common::prov", "EntityIdOrExternal");
	let activity_id = &rust::import("chronicle::common::prov", "ActivityIdOrExternal");
	let empty_fields =
		&rust::import("chronicle::async_graphql::connection", "EmptyFields").qualified();

	let timeline_order =
		&rust::import("chronicle::api::chronicle_graphql", "TimelineOrder").qualified();

	let activities_by_type_doc = include_str!("../../../../domain_docs/activities_by_type.md");
	let activity_by_id_doc = include_str!("../../../../domain_docs/activity_by_id.md");
	let activity_timeline_doc = include_str!("../../../../domain_docs/activity_timeline.md");
	let agent_by_id_doc = include_str!("../../../../domain_docs/agent_by_id.md");
	let agents_by_type_doc = include_str!("../../../../domain_docs/agents_by_type.md");
	let entities_by_type_doc = include_str!("../../../../domain_docs/entities_by_type.md");
	let entity_by_id_doc = include_str!("../../../../domain_docs/entity_by_id.md");

	quote! {
	#[derive(Copy, Clone)]
	pub struct Query;

	#[#graphql_object]
	impl Query {

	#[doc = #_(#activity_timeline_doc)]
	#[allow(clippy::too_many_arguments)]
	pub async fn activity_timeline<'a>(
		&self,
		ctx: &#graphql_context<'a>,
		activity_types: Option<Vec<ActivityType>>,
		for_entity: Option<Vec<#entity_id>>,
		for_agent: Option<Vec<#agent_id>>,
		from: Option<DateTime<Utc>>,
		to: Option<DateTime<Utc>>,
		order: Option<#timeline_order>,
		namespace: Option<ID>,
		after: Option<String>,
		before: Option<String>,
		first: Option<i32>,
		last: Option<i32>,
	) -> #graphql_result<#graphql_connection<i32, #(activity_union_type_name()), #empty_fields, #empty_fields>> {
			let connection = #query_impl::activity_timeline(
				ctx,
				activity_types.map(|xs| xs
					.into_iter()
					.filter_map(|x| x.into())
					.collect()),
				for_agent.map(|xs| xs
					.into_iter()
					.map(|x| x.into())
					.collect()),
				for_entity.map(|xs| xs
					.into_iter()
					.map(|x| x.into())
					.collect()),
				from,
				to,
				order,
				namespace,
				after,
				before,
				first,
				last,
			)
			.await
			.map_err(|e| #async_graphql_error_extensions::extend(&e))?;

			let mut new_edges = Vec::with_capacity(connection.edges.len());

			for (i, edge) in connection.edges.into_iter().enumerate() {
				let new_node = map_activity_to_domain_type(edge.node);
				new_edges.push(connection::Edge::with_additional_fields(i as i32, new_node, #empty_fields));
			}

			let mut new_connection = #graphql_connection::new(connection.has_previous_page, connection.has_next_page);

			new_connection.edges.extend(new_edges);

			Ok(new_connection)
	}

	#[doc = #_(#agents_by_type_doc)]
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
		let connection = #query_impl::agents_by_type(
			ctx,
			agent_type.into(),
			namespace,
			after,
			before,
			first,
			last,
		)
		.await
		.map_err(|e| #async_graphql_error_extensions::extend(&e))?;

		let mut new_edges = Vec::with_capacity(connection.edges.len());

		for (i, edge) in connection.edges.into_iter().enumerate() {
			let new_node = map_agent_to_domain_type(edge.node);
			new_edges.push(connection::Edge::with_additional_fields(i as i32, new_node, #empty_fields));
		}

		let mut new_connection = #graphql_connection::new(connection.has_previous_page, connection.has_next_page);

		new_connection.edges.extend(new_edges);

		Ok(new_connection)
	}

	#[doc = #_(#activities_by_type_doc)]
	#[allow(clippy::too_many_arguments)]
	pub async fn activities_by_type<'a>(
		&self,
		ctx: &#graphql_context<'a>,
		activity_type: ActivityType,
		namespace: Option<#graphql_id>,
		after: Option<String>,
		before: Option<String>,
		first: Option<i32>,
		last: Option<i32>,
	) -> #graphql_result<#graphql_connection<i32, #(activity_union_type_name()), #empty_fields, #empty_fields>> {
		let connection = #query_impl::activities_by_type(
			ctx,
			activity_type.into(),
			namespace,
			after,
			before,
			first,
			last,
		)
		.await
		.map_err(|e| #async_graphql_error_extensions::extend(&e))?;

		let mut new_edges = Vec::with_capacity(connection.edges.len());

		for (i, edge) in connection.edges.into_iter().enumerate() {
			let new_node = map_activity_to_domain_type(edge.node);
			new_edges.push(connection::Edge::with_additional_fields(i as i32, new_node, #empty_fields));
		}

		let mut new_connection = #graphql_connection::new(connection.has_previous_page, connection.has_next_page);

		new_connection.edges.extend(new_edges);

		Ok(new_connection)
	}

	#[doc = #_(#entities_by_type_doc)]
	#[allow(clippy::too_many_arguments)]
	pub async fn entities_by_type<'a>(
		&self,
		ctx: &#graphql_context<'a>,
		entity_type: EntityType,
		namespace: Option<#graphql_id>,
		after: Option<String>,
		before: Option<String>,
		first: Option<i32>,
		last: Option<i32>,
	) -> #graphql_result<#graphql_connection<i32, #(entity_union_type_name()), #empty_fields, #empty_fields>> {
		let connection = #query_impl::entities_by_type(
			ctx,
			entity_type.into(),
			namespace,
			after,
			before,
			first,
			last,
		)
		.await
		.map_err(|e| #async_graphql_error_extensions::extend(&e))?;

		let mut new_edges = Vec::with_capacity(connection.edges.len());

		for (i, edge) in connection.edges.into_iter().enumerate() {
			let new_node = map_entity_to_domain_type(edge.node);
			new_edges.push(connection::Edge::with_additional_fields(i as i32, new_node, #empty_fields));
		}

		let mut new_connection = #graphql_connection::new(connection.has_previous_page, connection.has_next_page);

		new_connection.edges.extend(new_edges);

		Ok(new_connection)
	}

	#[doc = #_(#agent_by_id_doc)]
	pub async fn agent_by_id<'a>(
		&self,
		ctx: &#graphql_context<'a>,
		id: #agent_id,
		namespace: Option<String>,
	) -> #graphql_result<Option<#(agent_union_type_name())>> {
		Ok(#query_impl::agent_by_id(ctx, id.into(), namespace)
			.await
			.map_err(|e| #async_graphql_error_extensions::extend(&e))?
			.map(map_agent_to_domain_type))
	}

	#[doc = #_(#activity_by_id_doc)]
	pub async fn activity_by_id<'a>(
		&self,
		ctx: &#graphql_context<'a>,
		id: #activity_id,
		namespace: Option<String>,
	) -> #graphql_result<Option<#(activity_union_type_name())>> {
		Ok(#query_impl::activity_by_id(ctx, id.into(), namespace)
			.await
			.map_err(|e| #async_graphql_error_extensions::extend(&e))?
			.map(map_activity_to_domain_type))
	}

	#[doc = #_(#entity_by_id_doc)]
	pub async fn entity_by_id<'a>(
		&self,
		ctx: &#graphql_context<'a>,
		id: #entity_id,
		namespace: Option<String>,
	) -> #graphql_result<Option<#(entity_union_type_name())>> {
		Ok(#query_impl::entity_by_id(ctx, id.into(), namespace)
			.await
			.map_err(|e| #async_graphql_error_extensions::extend(&e))?
			.map(map_entity_to_domain_type))
	}
	}
	}
}

fn gen_mutation(domain: &ChronicleDomainDef) -> rust::Tokens {
	let graphql_object = &rust::import("chronicle::async_graphql", "Object");

	let graphql_result = &rust::import("chronicle::async_graphql", "Result");
	let graphql_context = &rust::import("chronicle::async_graphql", "Context");
	let async_graphql_error_extensions =
		&rust::import("chronicle::async_graphql", "ErrorExtensions").qualified();

	let submission = &rust::import("chronicle::api::chronicle_graphql", "Submission");
	let impls = &rust::import("chronicle::api::chronicle_graphql", "mutation");

	let entity_id = &rust::import("chronicle::common::prov", "EntityIdOrExternal");
	let agent_id = &rust::import("chronicle::common::prov", "AgentIdOrExternal");
	let activity_id = &rust::import("chronicle::common::prov", "ActivityIdOrExternal");
	let domain_type_id = &rust::import("chronicle::common::prov", "DomaintypeId");

	let abstract_attributes =
		&rust::import("chronicle::common::attributes", "Attributes").qualified();

	let acted_on_behalf_of_doc = include_str!("../../../../domain_docs/acted_on_behalf_of.md");
	let define_doc = include_str!("../../../../domain_docs/define.md");
	let end_doc = include_str!("../../../../domain_docs/end_activity.md");
	let had_primary_source_doc = include_str!("../../../../domain_docs/had_primary_source.md");
	let instant_activity_doc = include_str!("../../../../domain_docs/instant_activity.md");
	let prov_activity_doc = include_str!("../../../../domain_docs/prov_activity.md");
	let prov_agent_doc = include_str!("../../../../domain_docs/prov_agent.md");
	let prov_entity_doc = include_str!("../../../../domain_docs/prov_entity.md");
	let start_doc = include_str!("../../../../domain_docs/start_activity.md");
	let used_doc = include_str!("../../../../domain_docs/used.md");
	let was_associated_with_doc = include_str!("../../../../domain_docs/was_associated_with.md");
	let was_attributed_to_doc = include_str!("../../../../domain_docs/was_attributed_to.md");
	let was_derived_from_doc = include_str!("../../../../domain_docs/was_derived_from.md");
	let was_generated_by_doc = include_str!("../../../../domain_docs/was_generated_by.md");
	let was_informed_by_doc = include_str!("../../../../domain_docs/was_informed_by.md");
	let was_quoted_from_doc = include_str!("../../../../domain_docs/was_quoted_from.md");
	let was_revision_of_doc = include_str!("../../../../domain_docs/was_revision_of.md");

	quote! {
	#[derive(Copy, Clone)]
	pub struct Mutation;

	#[#graphql_object]
	impl Mutation {
		#[doc = #_(#define_doc)]
		#[doc = ""]
		#[doc = #_(#prov_agent_doc)]
		pub async fn define_agent<'a>(
			&self,
			ctx: &#graphql_context<'a>,
			external_id: String,
			namespace: Option<String>,
			attributes: ProvAgentAttributes,
		) -> async_graphql::#graphql_result<#submission> {
			#impls::agent(ctx, external_id, namespace, attributes.into()).await.map_err(|e| #async_graphql_error_extensions::extend(&e))
		}

		#(for agent in domain.agents.iter() =>
			#[doc = #_(#define_doc)]
			#(if agent.doc.is_some() {
				#[doc = ""]
				#[doc = #_(#(agent.doc.as_ref().map(|s| s.to_owned()).unwrap_or_default()))]
			})
			#(if agent.attributes.is_empty() {
			#[graphql(name = #_(#(agent.as_method_name())))]
			pub async fn #(&agent.as_property())<'a>(
				&self,
				ctx: &#graphql_context<'a>,
				external_id: String,
				namespace: Option<String>,
			) -> async_graphql::#graphql_result<#submission> {
				#impls::agent(ctx, external_id, namespace,
					#abstract_attributes::type_only(Some(
						#domain_type_id::from_external_id(#_(#(agent.as_type_name())))
					))
				).await.map_err(|e| #async_graphql_error_extensions::extend(&e))
			}
			} else {
			#[graphql(name = #_(#(agent.as_method_name())))]
			pub async fn #(&agent.as_property())<'a>(
				&self,
				ctx: &#graphql_context<'a>,
				external_id: String,
				namespace: Option<String>,
				attributes: #(agent.attributes_type_name_preserve_inflection()),
			) -> async_graphql::#graphql_result<#submission> {
				#impls::agent(ctx, external_id, namespace, attributes.into()).await.map_err(|e| #async_graphql_error_extensions::extend(&e))
			}
			}
			)
		)

		#[doc = #_(#define_doc)]
		#[doc = ""]
		#[doc = #_(#prov_activity_doc)]
		pub async fn define_activity<'a>(
			&self,
			ctx: &#graphql_context<'a>,
			external_id: String,
			namespace: Option<String>,
			attributes: ProvActivityAttributes,
		) -> async_graphql::#graphql_result<#submission> {
			#impls::activity(ctx, external_id, namespace, attributes.into()).await.map_err(|e| #async_graphql_error_extensions::extend(&e))
		}

		#(for activity in domain.activities.iter() =>
			#[doc = #_(#define_doc)]
			#(if activity.doc.is_some() {
				#[doc = ""]
				#[doc = #_(#(activity.doc.as_ref().map(|s| s.to_owned()).unwrap_or_default()))]
			})
			#(if activity.attributes.is_empty() {
			#[graphql(name = #_(#(activity.as_method_name())))]
			pub async fn #(&activity.as_property())<'a>(
				&self,
				ctx: &#graphql_context<'a>,
				external_id: String,
				namespace: Option<String>,
			) -> async_graphql::#graphql_result<#submission> {
				#impls::activity(ctx, external_id, namespace,
					#abstract_attributes::type_only(Some(
						#domain_type_id::from_external_id(#_(#(activity.as_type_name())))
					))
				).await.map_err(|e| #async_graphql_error_extensions::extend(&e))
			}
			} else {
			#[graphql(name = #_(#(activity.as_method_name())))]
			pub async fn #(&activity.as_property())<'a>(
				&self,
				ctx: &#graphql_context<'a>,
				external_id: String,
				namespace: Option<String>,
				attributes: #(activity.attributes_type_name_preserve_inflection()),
			) -> async_graphql::#graphql_result<#submission> {
				#impls::activity(ctx, external_id, namespace, attributes.into()).await.map_err(|e| #async_graphql_error_extensions::extend(&e))
			}
			}
			)
		)

		#[doc = #_(#define_doc)]
		#[doc = ""]
		#[doc = #_(#prov_entity_doc)]
		pub async fn define_entity<'a>(
			&self,
			ctx: &#graphql_context<'a>,
			external_id: String,
			namespace: Option<String>,
			attributes: ProvEntityAttributes,
		) -> async_graphql::#graphql_result<#submission> {
			#impls::entity(ctx, external_id, namespace, attributes.into()).await.map_err(|e| #async_graphql_error_extensions::extend(&e))
		}

		#(for entity in domain.entities.iter() =>
			#[doc = #_(#define_doc)]
			#(if entity.doc.is_some() {
				#[doc = ""]
				#[doc = #_(#(entity.doc.as_ref().map(|s| s.to_owned()).unwrap_or_default()))]
			})
			#(if entity.attributes.is_empty() {
			#[graphql(name = #_(#(entity.as_method_name())))]
			pub async fn #(&entity.as_property())<'a>(
				&self,
				ctx: &#graphql_context<'a>,
				external_id: String,
				namespace: Option<String>,
			) -> async_graphql::#graphql_result<#submission> {
				#impls::entity(ctx, external_id, namespace,
					#abstract_attributes::type_only(Some(
						#domain_type_id::from_external_id(#_(#(entity.as_type_name())))
					))
				).await.map_err(|e| #async_graphql_error_extensions::extend(&e))
			}
			} else {
			#[graphql(name = #_(#(entity.as_method_name())))]
			pub async fn #(&entity.as_property())<'a>(
				&self,
				ctx: &#graphql_context<'a>,
				external_id: String,
				namespace: Option<String>,
				attributes: #(entity.attributes_type_name_preserve_inflection()),
			) -> async_graphql::#graphql_result<#submission> {
				#impls::entity(ctx, external_id, namespace, attributes.into()).await.map_err(|e| #async_graphql_error_extensions::extend(&e))
			}
			}
			)
		)

		#[doc = #_(#acted_on_behalf_of_doc)]
		pub async fn acted_on_behalf_of<'a>(
			&self,
			ctx: &#graphql_context<'a>,
			namespace: Option<String>,
			responsible: #agent_id,
			delegate: #agent_id,
			activity: Option<#activity_id>,
			role: RoleType,
		) -> async_graphql::#graphql_result<#submission> {
			let activity = activity.map(|activity| activity.into());
			#impls::acted_on_behalf_of(ctx, namespace, responsible.into(), delegate.into(), activity, role.into()).await.map_err(|e| #async_graphql_error_extensions::extend(&e))
		}

		#[doc = #_(#was_derived_from_doc)]
		pub async fn was_derived_from<'a>(
			&self,
			ctx: &#graphql_context<'a>,
			namespace: Option<String>,
			generated_entity: #entity_id,
			used_entity: #entity_id,
		) -> async_graphql::#graphql_result<#submission> {
			#impls::was_derived_from(ctx, namespace, generated_entity.into(), used_entity.into())
				.await.map_err(|e| #async_graphql_error_extensions::extend(&e))
		}

		#[doc = #_(#was_revision_of_doc)]
		pub async fn was_revision_of<'a>(
			&self,
			ctx: &#graphql_context<'a>,
			namespace: Option<String>,
			generated_entity: #entity_id,
			used_entity: #entity_id,
		) -> async_graphql::#graphql_result<#submission> {
			#impls::was_revision_of(ctx, namespace, generated_entity.into(), used_entity.into())
				.await.map_err(|e| #async_graphql_error_extensions::extend(&e))
		}

		#[doc = #_(#had_primary_source_doc)]
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
				generated_entity.into(),
				used_entity.into(),
			)
			.await.map_err(|e| #async_graphql_error_extensions::extend(&e))
		}

		#[doc = #_(#was_quoted_from_doc)]
		pub async fn was_quoted_from<'a>(
			&self,
			ctx: &#graphql_context<'a>,
			namespace: Option<String>,
			generated_entity: #entity_id,
			used_entity: #entity_id,
		) -> async_graphql::#graphql_result<#submission> {
			#impls::was_quoted_from(ctx, namespace, generated_entity.into(), used_entity.into())
				.await.map_err(|e| #async_graphql_error_extensions::extend(&e))
		}

		#[doc = #_(#instant_activity_doc)]
		pub async fn instant_activity<'a>(
			&self,
			ctx: &#graphql_context<'a>,
			id: #activity_id,
			namespace: Option<String>,
			agent: Option<#agent_id>,
			time: Option<DateTime<Utc>>,
		) -> async_graphql::#graphql_result<#submission> {
			let agent = agent.map(|agent| agent.into());
			#impls::instant_activity(ctx, id.into(), namespace, agent, time).await.map_err(|e| #async_graphql_error_extensions::extend(&e))
		}

		#[doc = #_(#start_doc)]
		pub async fn start_activity<'a>(
			&self,
			ctx: &#graphql_context<'a>,
			id: #activity_id,
			namespace: Option<String>,
			agent: Option<#agent_id>,
			time: Option<DateTime<Utc>>,
		) -> async_graphql::#graphql_result<#submission> {
			let agent = agent.map(|agent| agent.into());
			#impls::start_activity(ctx, id.into(), namespace, agent, time).await.map_err(|e| #async_graphql_error_extensions::extend(&e))
		}

		#[doc = #_(#end_doc)]
		pub async fn end_activity<'a>(
			&self,
			ctx: &#graphql_context<'a>,
			id: #activity_id,
			namespace: Option<String>,
			agent: Option<#agent_id>,
			time: Option<DateTime<Utc>>,
		) -> async_graphql::#graphql_result<#submission> {
			let agent = agent.map(|agent| agent.into());
			#impls::end_activity(ctx, id.into(), namespace, agent, time).await.map_err(|e| #async_graphql_error_extensions::extend(&e))
		}

		#[doc = #_(#was_associated_with_doc)]
		pub async fn was_associated_with<'a>(
			&self,
			ctx: &#graphql_context<'a>,
			namespace: Option<String>,
			responsible: #agent_id,
			activity: #activity_id,
			role: RoleType
		) -> async_graphql::#graphql_result<#submission> {
			#impls::was_associated_with(ctx, namespace, responsible.into(), activity.into(), role.into()).await.map_err(|e| #async_graphql_error_extensions::extend(&e))
		}

		#[doc = #_(#was_attributed_to_doc)]
		pub async fn was_attributed_to<'a>(
			&self,
			ctx: &#graphql_context<'a>,
			namespace: Option<String>,
			responsible: #agent_id,
			entity: #entity_id,
			role: RoleType
		) -> async_graphql::#graphql_result<#submission> {
			#impls::was_attributed_to(ctx, namespace, responsible.into(), entity.into(), role.into()).await.map_err(|e| #async_graphql_error_extensions::extend(&e))
		}

		#[doc = #_(#used_doc)]
		pub async fn used<'a>(
			&self,
			ctx: &#graphql_context<'a>,
			activity: #activity_id,
			id: #entity_id,
			namespace: Option<String>,
		) -> async_graphql::#graphql_result<#submission> {
			#impls::used(ctx, activity.into(), id.into(), namespace).await.map_err(|e| #async_graphql_error_extensions::extend(&e))
		}

		#[doc = #_(#was_informed_by_doc)]
		pub async fn was_informed_by<'a>(
			&self,
			ctx: &#graphql_context<'a>,
			activity: #activity_id,
			informing_activity: #activity_id,
			namespace: Option<String>,
		) -> async_graphql::#graphql_result<#submission> {
			#impls::was_informed_by(ctx, activity.into(), informing_activity.into(), namespace).await.map_err(|e| #async_graphql_error_extensions::extend(&e))
		}

		#[doc = #_(#was_generated_by_doc)]
		pub async fn was_generated_by<'a>(
			&self,
			ctx: &#graphql_context<'a>,
			activity: #activity_id,
			id: #entity_id,
			namespace: Option<String>,
		) -> async_graphql::#graphql_result<#submission> {
			#impls::was_generated_by(ctx, activity.into(), id.into(), namespace).await.map_err(|e| #async_graphql_error_extensions::extend(&e))
		}
	}
	}
}

fn gen_graphql_type(domain: &ChronicleDomainDef) -> rust::Tokens {
	let prov_agent = AgentDef {
		external_id: "ProvAgent".to_owned(),
		doc: Some(include_str!("../../../../domain_docs/prov_agent.md").to_string()),
		attributes: vec![],
	};
	let prov_activity = ActivityDef {
		external_id: "ProvActivity".to_owned(),
		doc: Some(include_str!("../../../../domain_docs/prov_activity.md").to_string()),
		attributes: vec![],
	};
	let prov_entity = EntityDef {
		external_id: "ProvEntity".to_owned(),
		doc: Some(include_str!("../../../../domain_docs/prov_entity.md").to_string()),
		attributes: vec![],
	};

	let chronicledomaindef = &rust::import("chronicle::codegen", "ChronicleDomainDef");
	let tokio = &rust::import("chronicle", "tokio");

	let bootstrap = rust::import("chronicle::bootstrap", "bootstrap");
	let chronicle_graphql = rust::import("chronicle::api::chronicle_graphql", "ChronicleGraphQl");

	quote! {
	#(gen_attribute_scalars(&domain.attributes))
	#(gen_type_enums(domain))
	#(gen_association_and_attribution_unions())
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


		let model = #chronicledomaindef::from_input_string(#_(#(&domain.to_json_string().unwrap()))).unwrap();

		#bootstrap(model, #chronicle_graphql::new(Query, Mutation)).await
	}

	}
}

pub fn generate_chronicle_domain_schema(domain: ChronicleDomainDef, path: impl AsRef<Path>) {
	let tokens = gen_graphql_type(&domain);

	path.as_ref().parent().map(std::fs::create_dir_all);
	let mut f = std::fs::File::create(path).unwrap();
	f.write_all(tokens.to_file_string().unwrap().as_bytes()).unwrap();

	f.flush().unwrap();
}
