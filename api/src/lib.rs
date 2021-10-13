#[macro_use]
extern crate diesel;

mod models;
mod schema;

use custom_error::*;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use dotenv::dotenv;
use std::borrow::Cow;
use std::env;
use user_error::UFE;

custom_error! {pub ApiError
    Db{source: diesel::result::Error}                   = "Database operation failed",
    DbConnection{source: diesel::ConnectionError}         = "Database connection failed",
}

impl UFE for ApiError {}

pub struct NameSpace<'a>(Cow<'a, str>);

impl<'a> NameSpace<'a> {
    pub fn new<S>(inner: S) -> Self
    where
        S: Into<Cow<'a, str>>,
    {
        NameSpace(inner.into())
    }
}

pub enum NamespaceCommand<'a> {
    Create(NameSpace<'a>),
}

pub struct Api {
    connection: SqliteConnection,
}

impl Api {
    pub fn new(database_url: &str) -> Result<Self, ApiError> {
        Ok(Api {
            connection: SqliteConnection::establish(database_url)?,
        })
    }

    pub fn name_space<'a>(&self, &NameSpace(ref name): &NameSpace<'a>) -> Result<(), ApiError> {
        diesel::insert_into(schema::namespace::table)
            .values(models::NewNamespace {
                name: name.as_ref(),
            })
            .execute(&self.connection)
            .map(|_| ())
            .map_err(ApiError::from)
    }
}
