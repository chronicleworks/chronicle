#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;

mod models;
mod schema;

use custom_error::*;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;

use iref::IriBuf;
use user_error::UFE;
use uuid::Uuid;

embed_migrations!();

custom_error! {pub ApiError
    Db{source: diesel::result::Error}                           = "Database operation failed",
    DbConnection{source: diesel::ConnectionError}               = "Database connection failed",
    DbMigration{source: diesel_migrations::RunMigrationsError}  = "Database migration failed",
    Iri{source: iref::Error}                                    = "Invalid IRI",
}

impl UFE for ApiError {}

pub enum NamespaceCommand {
    Create { name: String },
}

pub enum AgentCommand {
    Create { name: String, namespace: String },
}

pub enum ApiCommand {
    NameSpace(NamespaceCommand),
    Agent(AgentCommand),
}

pub enum ApiResponse {
    Unit,
    Iri(IriBuf),
}

pub struct Api {
    connection: SqliteConnection,
}

impl Api {
    pub fn new(database_url: &str) -> Result<Self, ApiError> {
        let connection = SqliteConnection::establish(database_url)?;
        embedded_migrations::run(&connection)?;

        Ok(Api { connection })
    }

    fn create_namespace(&self, name: &str) -> Result<ApiResponse, ApiError> {
        diesel::insert_or_ignore_into(schema::namespace::table)
            .values(models::NewNamespace { name })
            .execute(&self.connection)
            .map(|_| ApiResponse::Unit)
            .map_err(ApiError::from)
    }

    fn create_agent(&self, name: &str, namespace: &str) -> Result<ApiResponse, ApiError> {
        self.create_namespace(namespace)?;
        let uuid = Uuid::new_v4();
        diesel::insert_or_ignore_into(schema::agent::table)
            .values(models::NewAgent {
                name,
                namespace,
                uuid: &uuid.to_string(),
                current: 0,
            })
            .execute(&self.connection)
            .map_err(ApiError::from)
            .and_then(|_| {
                Ok(ApiResponse::Iri(IriBuf::new(&format!(
                    "{}:{}/{}",
                    namespace, name, uuid
                ))?))
            })
    }

    pub fn dispatch(&self, command: ApiCommand) -> Result<ApiResponse, ApiError> {
        match command {
            ApiCommand::NameSpace(NamespaceCommand::Create { name }) => {
                self.create_namespace(&name)
            }
            ApiCommand::Agent(AgentCommand::Create { name, namespace }) => {
                self.create_agent(&name, &namespace)
            }
        }
    }
}
