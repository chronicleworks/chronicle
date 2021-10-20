use crate::schema::*;
use diesel::{Insertable, Queryable};
use iref::{AsIri, Iri};

#[derive(Queryable)]
pub struct NameSpace {
    pub name: String,
    pub uuid: String,
}

impl AsIri for NameSpace {
    fn as_iri(&self) -> iref::Iri {
        Iri::new(&format!("chronicle:ns:{}:{}", self.name, self.uuid)).unwrap()
    }
}

#[derive(Insertable)]
#[table_name = "namespace"]
pub struct NewNamespace<'a> {
    pub name: &'a str,
    pub uuid: &'a str,
}

#[derive(Queryable)]
pub struct Agent {
    pub name: String,
    pub namespace: String,
    pub publickey: Option<String>,
    pub privatekeypath: Option<String>,
    pub current: i32,
}

impl AsIri for Agent {
    fn as_iri(&self) -> iref::Iri {
        Iri::new(&format!("chronicle:agent:{}", self.name)).unwrap()
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

impl<'a> AsIri for NewAgent<'a> {
    fn as_iri(&self) -> iref::Iri {
        Iri::new(&format!("chronicle:agent:{}", self.name)).unwrap()
    }
}
