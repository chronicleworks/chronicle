use crate::schema::*;
use diesel::{Insertable, Queryable};
use iref::{IriBuf};

#[derive(Queryable)]
pub struct NameSpace {
    pub name: String,
    pub uuid: String,
}

impl From<&NameSpace> for IriBuf {
    fn from(ns: &NameSpace) -> Self {
        IriBuf::new(&format!("chronicle:ns:{}:{}", ns.name, ns.uuid)).unwrap()
    }
}

#[derive(Insertable)]
#[table_name = "namespace"]
pub struct NewNamespace<'a> {
    pub name: &'a str,
    pub uuid: &'a str,
}

impl<'a> From<&NewNamespace<'a>> for IriBuf {
    fn from(ns: &NewNamespace<'a>) -> Self {
        IriBuf::new(&format!("chronicle:ns:{}:{}", ns.name, ns.uuid)).unwrap()
    }
}

#[derive(Queryable)]
pub struct Agent {
    pub name: String,
    pub namespace: String,
    pub publickey: Option<String>,
    pub privatekeypath: Option<String>,
    pub current: i32,
}

impl From<&Agent> for IriBuf {
    fn from(agent: &Agent) -> Self {
        IriBuf::new(&format!("chronicle:agent:{}", agent.name)).unwrap()
    }
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

impl<'a> From<&NewAgent<'a>> for IriBuf {
    fn from(agent: &NewAgent<'a>) -> Self {
        IriBuf::new(&format!("chronicle:agent:{}", agent.name)).unwrap()
    }
}
