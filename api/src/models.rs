use crate::schema::*;
use diesel::{Insertable, Queryable};

#[derive(Queryable)]
pub struct NameSpace<'a> {
    pub name: &'a str,
}

#[derive(Insertable)]
#[table_name = "namespace"]
pub struct NewNamespace<'a> {
    pub name: &'a str,
}

#[derive(Queryable)]
pub struct Agent<'a> {
    pub name: &'a str,
    pub namespace: &'a str,
}

#[derive(Insertable)]
#[table_name = "agent"]
pub struct NewAgent<'a> {
    pub name: &'a str,
    pub namespace: &'a str,
    pub uuid: &'a str,
    pub current: i32,
}
