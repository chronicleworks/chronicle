#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;
#[macro_use]
extern crate iref_enum;
#[macro_use]
extern crate json;

mod persistence;

use async_std::task;
use custom_error::*;
use derivative::*;
use diesel::{prelude::*, sqlite::SqliteConnection};
use json::JsonValue;
use json_ld::{context::Local, Document, JsonContext, NoLoader};
use persistence::Store;
use std::path::Path;

use common::{
    ledger::{LedgerWriter, SubmissionError},
    models::{ChronicleTransaction, CreateAgent, CreateNamespace},
    signing::{DirectoryStoredKeys, SignerError},
    vocab::Chronicle as ChronicleVocab,
};
use iref::IriBuf;
use proto::messaging::SawtoothValidator;
use tracing::instrument;
use url::Url;
use user_error::UFE;
use uuid::Uuid;

custom_error! {pub ApiError
    Store{source: persistence::StoreError}                      = "Storage",
    Iri{source: iref::Error}                                    = "Invalid IRI",
    JsonLD{source: json_ld::Error}                              = "Json LD processing",
    Ledger{source: SubmissionError}                             = "Ledger error",
    Signing{source: SignerError}                                = "Signing",
}

impl UFE for ApiError {}

#[derive(Debug)]
pub enum NamespaceCommand {
    Create { name: String },
}

#[derive(Debug)]
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

#[derive(Debug)]
pub enum ApiCommand {
    NameSpace(NamespaceCommand),
    Agent(AgentCommand),
}

#[derive(Debug)]
pub enum ApiResponse {
    Unit,
    Iri(IriBuf),
    Document(JsonValue),
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct Api {
    #[derivative(Debug = "ignore")]
    ledger: Box<dyn LedgerWriter>,
    #[derivative(Debug = "ignore")]
    store: persistence::Store,
}

impl Api {
    pub fn new(
        database_url: &str,
        sawtooth_url: &Url,
        secret_path: &Path,
    ) -> Result<Self, ApiError> {
        let ledger = SawtoothValidator::new(
            sawtooth_url,
            DirectoryStoredKeys::new(secret_path)?.default(),
        );
        Ok(Api {
            ledger: Box::new(ledger),
            store: Store::new(database_url)?,
        })
    }

    #[instrument]
    fn create_namespace(&self, name: &str) -> Result<ApiResponse, ApiError> {
        let uuid = Uuid::new_v4();
        let iri = ChronicleVocab::namespace(name, &uuid);

        let tx = ChronicleTransaction::CreateNamespace(CreateNamespace {
            id: iri.into(),
            name: name.to_owned(),
            uuid: Uuid::new_v4(),
        });

        self.ledger.submit(vec![&tx])?;

        self.store.apply(&tx)?;

        Ok(ApiResponse::Unit)
    }

    #[instrument]
    fn create_agent(&self, name: &str, namespace: &str) -> Result<ApiResponse, ApiError> {
        let tx = ChronicleTransaction::CreateAgent(CreateAgent {
            name: name.to_owned(),
            id: ChronicleVocab::agent(name).into(),
            namespace: self.store.namespace_by_name(namespace)?,
        });

        self.ledger.submit(vec![&tx])?;
        self.store.apply(&tx);

        Ok(ApiResponse::Unit)
    }

    #[instrument]
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

    #[instrument]
    fn get_agent(
        &self,
        name: &str,
        namespace: &str,
    ) -> Result<Option<Agent>, diesel::result::Error> {
    }

    #[instrument]
    fn register_key(
        &self,
        name: &str,
        namespace: &str,
        publickey: &str,
        privatekeypath: Option<String>,
    ) -> Result<ApiResponse, ApiError> {
    }
}
