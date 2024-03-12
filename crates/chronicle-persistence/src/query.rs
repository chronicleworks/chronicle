use super::schema::*;
use chrono::NaiveDateTime;
use diesel::prelude::*;

#[derive(Queryable)]
pub struct Namespace {
	pub external_id: String,
	pub uuid: String,
}

#[derive(Queryable)]
pub struct LedgerSync {
	pub bc_offset: String,
	pub sync_time: Option<NaiveDateTime>,
}

#[derive(Insertable)]
#[diesel(table_name = namespace)]
pub struct NewNamespace<'a> {
	pub external_id: &'a str,
	pub uuid: &'a str,
}

#[derive(Insertable)]
#[diesel(table_name = ledgersync)]
pub struct NewOffset<'a> {
	pub bc_offset: &'a str,
	pub sync_time: Option<NaiveDateTime>,
}

#[derive(Insertable, Queryable, Selectable)]
#[diesel(table_name = entity_attribute)]
pub struct EntityAttribute {
	pub entity_id: i32,
	pub typename: String,
	pub value: String,
}

#[derive(Insertable, Queryable, Selectable)]
#[diesel(table_name = activity_attribute)]
pub struct ActivityAttribute {
	pub activity_id: i32,
	pub typename: String,
	pub value: String,
}

#[derive(Insertable, Queryable, Selectable)]
#[diesel(table_name = agent_attribute)]
pub struct AgentAttribute {
	pub agent_id: i32,
	pub typename: String,
	pub value: String,
}

#[derive(Insertable)]
#[diesel(table_name = activity)]
pub struct NewActivity<'a> {
	pub external_id: &'a str,
	pub namespace_id: i32,
	pub started: Option<NaiveDateTime>,
	pub ended: Option<NaiveDateTime>,
	pub domaintype: Option<&'a str>,
}

#[derive(Debug, Queryable, Selectable, Identifiable, Associations, PartialEq)]
#[diesel(belongs_to(Association, foreign_key = id))]
#[diesel(belongs_to(Delegation, foreign_key = id))]
#[diesel(belongs_to(Derivation, foreign_key = id))]
#[diesel(belongs_to(Generation, foreign_key = id))]
#[diesel(belongs_to(Usage, foreign_key = id))]
#[diesel(table_name = agent)]
pub struct Agent {
	pub id: i32,
	pub external_id: String,
	pub namespace_id: i32,
	pub domaintype: Option<String>,
	pub current: i32,
	pub identity_id: Option<i32>,
}

#[derive(Debug, Queryable, Selectable, Identifiable, PartialEq)]
#[diesel(belongs_to(Usage))]
#[diesel(belongs_to(Generation))]
#[diesel(table_name = activity)]
pub struct Activity {
	pub id: i32,
	pub external_id: String,
	pub namespace_id: i32,
	pub domaintype: Option<String>,
	pub started: Option<NaiveDateTime>,
	pub ended: Option<NaiveDateTime>,
}

#[derive(Debug, Queryable, Identifiable, Associations, Selectable)]
#[diesel(belongs_to(Generation, foreign_key=id))]
#[diesel(belongs_to(Usage, foreign_key=id))]
#[diesel(belongs_to(Attribution, foreign_key=id))]
#[diesel(belongs_to(Derivation, foreign_key=id))]
#[diesel(table_name = entity)]
pub struct Entity {
	pub id: i32,
	pub external_id: String,
	pub namespace_id: i32,
	pub domaintype: Option<String>,
}

#[derive(Debug, Queryable, Selectable, Associations, PartialEq)]
#[diesel(table_name = wasinformedby)]
#[diesel(belongs_to(Activity, foreign_key = activity_id , foreign_key = informing_activity_id))]
pub struct WasInformedBy {
	activity_id: i32,
	informing_activity_id: i32,
}

#[derive(Debug, Queryable, Selectable, Identifiable, Associations, PartialEq)]
#[diesel(primary_key(activity_id, generated_entity_id))]
#[diesel(table_name = generation)]
#[diesel(belongs_to(Activity))]
#[diesel(belongs_to(Entity, foreign_key = generated_entity_id))]
pub struct Generation {
	activity_id: i32,
	generated_entity_id: i32,
}

#[derive(Debug, Queryable, Selectable, Associations, PartialEq)]
#[diesel(table_name = usage)]
#[diesel(belongs_to(Activity))]
#[diesel(belongs_to(Entity))]
pub struct Usage {
	activity_id: i32,
	entity_id: i32,
}

#[derive(Debug, Queryable, Selectable, Associations, PartialEq)]
#[diesel(table_name = association)]
#[diesel(belongs_to(Agent))]
#[diesel(belongs_to(Activity))]
pub struct Association {
	agent_id: i32,
	activity_id: i32,
	role: String,
}

#[derive(Debug, Queryable, Selectable, Associations, Identifiable, PartialEq)]
#[diesel(table_name = attribution)]
#[diesel(primary_key(agent_id, entity_id, role))]
#[diesel(belongs_to(Agent))]
#[diesel(belongs_to(Entity))]
pub struct Attribution {
	agent_id: i32,
	entity_id: i32,
	role: String,
}

#[derive(Debug, Queryable, Selectable, Associations, PartialEq)]
#[diesel(table_name = delegation)]
#[diesel(belongs_to(Agent, foreign_key = delegate_id, foreign_key = responsible_id))]
#[diesel(belongs_to(Activity))]
pub struct Delegation {
	delegate_id: i32,
	responsible_id: i32,
	activity_id: i32,
	role: String,
}

#[derive(Debug, Queryable, Selectable, Associations, PartialEq)]
#[diesel(table_name = derivation)]
#[diesel(belongs_to(Activity))]
#[diesel(belongs_to(Entity, foreign_key = generated_entity_id, foreign_key = used_entity_id))]
pub struct Derivation {
	activity_id: i32,
	used_entity_id: i32,
	generated_entity_id: i32,
	typ: i32,
}

#[derive(Insertable, Queryable, Selectable)]
#[diesel(table_name = agent)]
pub struct NewAgent<'a> {
	pub external_id: &'a str,
	pub namespace_id: i32,
	pub current: i32,
	pub domaintype: Option<&'a str>,
}
