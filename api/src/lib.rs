mod persistence;

use custom_error::*;
use derivative::*;

use persistence::Store;
use std::path::Path;

use common::{
    ledger::{LedgerWriter, SubmissionError},
    models::{ChronicleTransaction, CreateAgent, CreateNamespace, ProvModel, RegisterKey},
    signing::SignerError,
    vocab::Chronicle as ChronicleVocab,
};

use tracing::instrument;

use user_error::UFE;
use uuid::Uuid;

custom_error! {pub ApiError
    Store{source: persistence::StoreError}                      = "Storage",
    Iri{source: iref::Error}                                    = "Invalid IRI",
    JsonLD{source: json_ld::Error}                              = "Json LD processing",
    Ledger{source: SubmissionError}                             = "Ledger error",
    Signing{source: SignerError}                                = "Signing",
    NamespaceNotFound{name: String}                             = "Namespace {} not found, please create it",
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
    Use {
        name: String,
        namespace: String,
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
    Prov(ProvModel),
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct Api {
    #[derivative(Debug = "ignore")]
    ledger: Box<dyn LedgerWriter>,
    #[derivative(Debug = "ignore")]
    store: persistence::Store,
    #[derivative(Debug = "ignore")]
    uuidsource: Box<dyn Fn() -> Uuid>,
}

impl Api {
    #[instrument(skip(ledger, uuidgen))]
    pub fn new<F>(
        database_url: &str,
        ledger: Box<dyn LedgerWriter>,
        _secret_path: &Path,
        uuidgen: F,
    ) -> Result<Self, ApiError>
    where
        F: Fn() -> Uuid,
        F: 'static,
    {
        Ok(Api {
            ledger: ledger,
            store: Store::new(database_url)?,
            uuidsource: Box::new(uuidgen),
        })
    }

    #[instrument]
    fn create_namespace(&self, name: &str) -> Result<ApiResponse, ApiError> {
        let uuid = (self.uuidsource)();
        let iri = ChronicleVocab::namespace(name, &uuid);

        let tx = ChronicleTransaction::CreateNamespace(CreateNamespace {
            id: iri.into(),
            name: name.to_owned(),
            uuid,
        });

        self.ledger.submit(vec![&tx])?;

        Ok(ApiResponse::Prov(self.store.apply(&tx)?))
    }

    #[instrument]
    fn create_agent(&self, name: &str, namespace: &str) -> Result<ApiResponse, ApiError> {
        let tx = ChronicleTransaction::CreateAgent(CreateAgent {
            name: name.to_owned(),
            id: ChronicleVocab::agent(name).into(),
            namespace: self.store.namespace_by_name(namespace).map_err(|_| {
                ApiError::NamespaceNotFound {
                    name: namespace.to_owned(),
                }
            })?,
        });

        self.ledger.submit(vec![&tx])?;
        self.store.apply(&tx)?;

        Ok(ApiResponse::Prov(self.store.apply(&tx)?))
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
            }) => self.register_key(name, namespace, public, private),
            ApiCommand::Agent(AgentCommand::Use { name, namespace }) => {
                self.use_agent(name, namespace)
            }
        }
    }

    #[instrument]
    fn register_key(
        &self,
        name: String,
        namespace: String,
        publickey: String,
        privatekeypath: Option<String>,
    ) -> Result<ApiResponse, ApiError> {
        let namespaceid = self.store.namespace_by_name(&namespace)?;
        let tx = ChronicleTransaction::RegisterKey(RegisterKey {
            id: ChronicleVocab::agent(&name).into(),
            name: name.clone(),
            namespace: namespaceid,
            publickey,
        });

        self.ledger.submit(vec![&tx])?;
        self.store.apply(&tx)?;
        privatekeypath.map(|pk| self.store.store_pk_path(name, namespace, pk));

        Ok(ApiResponse::Prov(self.store.apply(&tx)?))
    }

    fn use_agent(&self, _name: String, _namespace: String) -> Result<ApiResponse, ApiError> {
        todo!()
    }
}

#[cfg(test)]
mod test {
    use common::ledger::InMemLedger;
    use tempfile::TempDir;
    use tracing::Level;
    use uuid::Uuid;

    use crate::{AgentCommand, Api, ApiCommand, ApiResponse, NamespaceCommand};

    fn test_api() -> Api {
        tracing_subscriber::fmt()
            .pretty()
            .with_max_level(Level::TRACE)
            .try_init()
            .ok();

        let secretpath = TempDir::new().unwrap();
        Api::new(
            "file::memory:",
            Box::new(InMemLedger::default()),
            &secretpath.into_path(),
            || Uuid::parse_str("5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea").unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn create_namespace() {
        let prov = test_api()
            .dispatch(ApiCommand::NameSpace(NamespaceCommand::Create {
                name: "testns".to_owned(),
            }))
            .unwrap();

        match prov {
            ApiResponse::Prov(prov) => {
                insta::assert_snapshot!(prov.to_json().0.pretty(3))
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn create_agent() {
        let api = test_api();

        api.dispatch(ApiCommand::NameSpace(NamespaceCommand::Create {
            name: "testns".to_owned(),
        }))
        .unwrap();

        let prov = api
            .dispatch(ApiCommand::Agent(AgentCommand::Create {
                name: "testagent".to_owned(),
                namespace: "testns".to_owned(),
            }))
            .unwrap();

        match prov {
            ApiResponse::Prov(prov) => {
                insta::assert_snapshot!(prov.to_json().0.pretty(3))
            }
            _ => unreachable!(),
        }
    }
}
