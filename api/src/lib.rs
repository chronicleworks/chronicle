#[macro_use]
extern crate diesel;

mod models;
mod schema;

use custom_error::*;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use dotenv::dotenv;
use std::borrow::Cow;
use std::env;

custom_error! {pub ApiError
    Db{source: diesel::result::Error}         = "Database operation failed",
}

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
    connection: PgConnection,
}

impl Api {
    pub fn new() -> Self {
        dotenv().ok();

        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        Api {
            connection: PgConnection::establish(&database_url)
                .expect(&format!("Error connecting to {}", database_url)),
        }
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
