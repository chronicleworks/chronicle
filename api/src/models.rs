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
