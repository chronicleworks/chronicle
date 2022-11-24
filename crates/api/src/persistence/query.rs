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

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = agent)]
pub struct Agent {
    pub id: i32,
    pub external_id: String,
    pub namespace_id: i32,
    pub domaintype: Option<String>,
    pub current: i32,
    pub identity_id: Option<i32>,
}

#[derive(Debug, Queryable)]
pub struct Identity {
    pub id: i32,
    pub namespace_id: i32,
    pub public_key: String,
}

#[derive(Debug, Queryable)]
pub struct Activity {
    pub id: i32,
    pub external_id: String,
    pub namespace_id: i32,
    pub domaintype: Option<String>,
    pub started: Option<NaiveDateTime>,
    pub ended: Option<NaiveDateTime>,
}

#[derive(Debug, Queryable)]
pub struct Attachment {
    pub id: i32,
    pub namespace_id: i32,
    pub signature_time: NaiveDateTime,
    pub signature: String,
    pub signer_id: i32,
    pub locator: Option<String>,
}

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = entity)]
pub struct Entity {
    pub id: i32,
    pub external_id: String,
    pub namespace_id: i32,
    pub domaintype: Option<String>,
    pub attachment_id: Option<i32>,
}

#[derive(Insertable, Queryable, Selectable)]
#[diesel(table_name = agent)]
pub struct NewAgent<'a> {
    pub external_id: &'a str,
    pub namespace_id: i32,
    pub current: i32,
    pub domaintype: Option<&'a str>,
}
