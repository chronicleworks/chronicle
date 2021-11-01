use super::schema::*;
use chrono::NaiveDateTime;
use diesel::prelude::*;

#[derive(Queryable)]
pub struct Namespace {
    pub name: String,
    pub uuid: String,
}

#[derive(Insertable)]
#[table_name = "namespace"]
pub struct NewNamespace<'a> {
    pub name: &'a str,
    pub uuid: &'a str,
}

#[derive(Insertable)]
#[table_name = "activity"]
pub struct NewActivity<'a> {
    pub name: &'a str,
    pub namespace: &'a str,
    pub started: Option<NaiveDateTime>,
    pub ended: Option<NaiveDateTime>,
}

#[derive(Queryable)]
pub struct Agent {
    pub id: i32,
    pub name: String,
    pub namespace: String,
    pub publickey: Option<String>,
    pub privatekeypath: Option<String>,
    pub current: i32,
}

#[derive(Queryable)]
pub struct Activity {
    pub id: i32,
    pub name: String,
    pub namespace: String,
    pub started: Option<NaiveDateTime>,
    pub ended: Option<NaiveDateTime>,
}

#[derive(Insertable, AsChangeset, Default)]
#[table_name = "agent"]
pub struct NewAgent<'a> {
    pub name: &'a str,
    pub namespace: &'a str,
    pub current: i32,
    pub publickey: Option<&'a str>,
    pub privatekeypath: Option<&'a str>,
}
