#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;
#[macro_use]
extern crate iref_enum;
#[macro_use]
extern crate json;

mod models;
mod schema;
mod vocab;

use async_std::task;
use custom_error::*;
use diesel::{prelude::*, sqlite::SqliteConnection};
use json::JsonValue;
use json_ld::{context::Local, Document, JsonContext, NoLoader};

use iref::IriBuf;
use models::Agent;
use user_error::UFE;
use uuid::Uuid;

embed_migrations!();

custom_error! {pub ApiError
    Db{source: diesel::result::Error}                           = "Database operation failed",
    DbConnection{source: diesel::ConnectionError}               = "Database connection failed",
    DbMigration{source: diesel_migrations::RunMigrationsError}  = "Database migration failed",
    Iri{source: iref::Error}                                    = "Invalid IRI",
    JsonLD{source: json_ld::Error}                              = "Json LD processing",
    Api{source: sawtooth_interface::SubmissionError}            = "Json LD processing",
}

impl UFE for ApiError {}

pub enum NamespaceCommand {
    Create { name: String },
}

pub enum AgentCommand {
    Create {
        name: String,
        namespace: String,
    },
    RegisterKey {
        name: String,
        namespace: String,
        public: String,
        private: Option<String>,
    },
}

pub enum ApiCommand {
    NameSpace(NamespaceCommand),
    Agent(AgentCommand),
}

pub enum ApiResponse {
    Unit,
    Iri(IriBuf),
    Document(JsonValue),
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
        let uuid = Uuid::new_v4();
        diesel::insert_or_ignore_into(schema::namespace::table)
            .values(models::NewNamespace {
                name,
                uuid: &uuid.to_string(),
            })
            .execute(&self.connection)
            .map(|_| ApiResponse::Unit)
            .map_err(ApiError::from)
    }

    fn create_agent(&self, name: &str, namespace: &str) -> Result<ApiResponse, ApiError> {
        use self::schema::namespace::dsl as ns;

        self.create_namespace(namespace)?;

        let namespace_data = ns::namespace
            .filter(ns::name.eq(namespace))
            .first::<models::NameSpace>(&self.connection)?;

        let namespace_iri = IriBuf::new(&format!(
            "chronicle:{}/{}#",
            namespace_data.name, namespace_data.uuid
        ))?;
        let id = IriBuf::new(&format!("{}agent:{}", namespace_iri, name))?;

        let input = object! {
            "http://www.w3.org/ns/prov#Agent" : {
                "@id": (id.as_str())
            }
        };
        let context = object! {};

        let processed_context =
            task::block_on(context.process::<JsonContext, _>(&mut NoLoader, None))?;

        let output = task::block_on(input.compact(&processed_context, &mut NoLoader))?;

        diesel::insert_or_ignore_into(schema::agent::table)
            .values(models::NewAgent {
                name,
                namespace,
                current: 0,
                publickey: None,
                privatekeypath: None,
            })
            .execute(&self.connection)
            .map_err(ApiError::from)
            .map(|_| ApiResponse::Document(output))
    }

    pub fn dispatch(&self, command: ApiCommand) -> Result<ApiResponse, ApiError> {
        match command {
            ApiCommand::NameSpace(NamespaceCommand::Create { name }) => {
                self.create_namespace(&name)
            }
            ApiCommand::Agent(AgentCommand::Create { name, namespace }) => {
                self.create_agent(&name, &namespace)
            }
            ApiCommand::Agent(AgentCommand::RegisterKey {
                name,
                namespace,
                public,
                private,
            }) => self.register_key(&name, &namespace, &public, private),
        }
    }

    fn get_agent(
        &self,
        name: &str,
        namespace: &str,
    ) -> Result<Option<Agent>, diesel::result::Error> {
        use self::schema::agent::dsl as ns;
        ns::agent
            .filter(ns::name.eq(namespace).and(ns::namespace.eq(namespace)))
            .first::<models::Agent>(&self.connection)
            .optional()
    }

    fn register_key(
        &self,
        name: &str,
        namespace: &str,
        publickey: &str,
        privatekeypath: Option<String>,
    ) -> Result<ApiResponse, ApiError> {
        let update = models::NewAgent {
            name,
            namespace,
            current: 0,
            publickey: Some(publickey),
            privatekeypath: privatekeypath.map(|x| x.as_ref()),
        };

        if let Some(agent) = self.get_agent(name, nam, namespace)? {
            diesel::update(schema::agent::table).set(update)
        }

        diesel::insert_into(schema::agent::table)
            .values(update)
            .execute(&self.connection)
            .map_err(ApiError::from)
            .map(|_| ApiResponse::Unit)
    }
}
