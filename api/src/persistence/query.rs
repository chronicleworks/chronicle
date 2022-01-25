use super::schema::*;
use chrono::NaiveDateTime;
use diesel::prelude::*;

#[derive(Queryable)]
pub struct Namespace {
    pub name: String,
    pub uuid: String,
}

#[derive(Queryable)]
pub struct LedgerSync {
    pub offset: String,
    pub sync_time: Option<NaiveDateTime>,
}

#[derive(Insertable)]
#[table_name = "namespace"]
pub struct NewNamespace<'a> {
    pub name: &'a str,
    pub uuid: &'a str,
}

#[derive(Insertable)]
#[table_name = "ledgersync"]
pub struct NewOffset<'a> {
    pub offset: &'a str,
    pub sync_time: Option<NaiveDateTime>,
}

#[derive(Insertable)]
#[table_name = "activity"]
pub struct NewActivity<'a> {
    pub name: &'a str,
    pub namespace: &'a str,
    pub started: Option<NaiveDateTime>,
    pub ended: Option<NaiveDateTime>,
    pub domaintype: Option<&'a str>,
}

#[derive(Debug, Queryable)]
pub struct Agent {
    pub id: i32,
    pub name: String,
    pub namespace: String,
    pub domaintype: Option<String>,
    pub publickey: Option<String>,
    pub current: i32,
}

#[derive(Debug, Queryable)]
pub struct Activity {
    pub id: i32,
    pub name: String,
    pub namespace: String,
    pub domaintype: Option<String>,
    pub started: Option<NaiveDateTime>,
    pub ended: Option<NaiveDateTime>,
}

#[derive(Debug, Queryable)]
pub struct Entity {
    pub id: i32,
    pub name: String,
    pub namespace: String,
    pub domaintype: Option<String>,
    pub signature_time: Option<NaiveDateTime>,
    pub signature: Option<String>,
    pub locator: Option<String>,
}

#[derive(Insertable, AsChangeset, Default)]
#[table_name = "agent"]
pub struct NewAgent<'a> {
    pub name: &'a str,
    pub namespace: &'a str,
    pub current: i32,
    pub publickey: Option<&'a str>,
    pub domaintype: Option<&'a str>,
}
