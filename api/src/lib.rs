mod persistence;

use chrono::{DateTime, Utc};
use custom_error::*;
use derivative::*;

use persistence::Store;
use std::{
    convert::Infallible,
    path::{Path, PathBuf},
};

use common::{
    ledger::{LedgerWriter, SubmissionError},
    models::{
        ActivityUses, ChronicleTransaction, CreateActivity, CreateAgent, CreateNamespace,
        EndActivity, GenerateEntity, ProvModel, RegisterKey, StartActivity,
    },
    signing::SignerError,
    vocab::Chronicle as ChronicleVocab,
};

use tracing::{debug, instrument};

use user_error::UFE;
use uuid::Uuid;

custom_error! {pub ApiError
    Store{source: persistence::StoreError}                      = "Storage",
    Iri{source: iref::Error}                                    = "Invalid IRI",
    JsonLD{source: json_ld::Error}                              = "Json LD processing",
    Ledger{source: SubmissionError}                             = "Ledger error",
    Signing{source: SignerError}                                = "Signing",
    NoCurrentAgent{}                                            = "No agent is currently in use, please call agent use",
}

///Kind of annoying but we need this until ! is stable https://github.com/rust-lang/rust/issues/64715
impl From<Infallible> for ApiError {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
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
pub enum ActivityCommand {
    Create {
        name: String,
        namespace: String,
    },
    Start {
        name: String,
        namespace: String,
        time: Option<DateTime<Utc>>,
    },
    End {
        name: Option<String>,
        namespace: Option<String>,
        time: Option<DateTime<Utc>>,
    },
    Use {
        name: String,
        namespace: String,
        activity: Option<String>,
    },
    Generate {
        name: String,
        namespace: String,
        activity: Option<String>,
    },
}

#[derive(Debug)]
pub enum EntityCommand {
    Attach {
        name: Option<String>,
        namespace: Option<String>,
        file: PathBuf,
        agent: Option<String>,
    },
}

#[derive(Debug)]
pub enum ApiCommand {
    NameSpace(NamespaceCommand),
    Agent(AgentCommand),
    Activity(ActivityCommand),
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

    /// Our resources all assume a namespace, or the default namspace, so automatically create it by name if it doesn't exist
    #[instrument]
    fn ensure_namespace(&self, namespace: &str) -> Result<(), ApiError> {
        let ns = self.store.namespace_by_name(namespace);

        if ns.is_err() {
            debug!(namespace, "Namespace does not exist, creating");
            self.create_namespace(namespace)?;
        }

        Ok(())
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
        self.ensure_namespace(namespace)?;
        let name = self.store.disambiguate_agent_name(name)?;

        let tx = ChronicleTransaction::CreateAgent(CreateAgent {
            name: name.to_owned(),
            id: ChronicleVocab::agent(&name).into(),
            namespace: self.store.namespace_by_name(namespace)?,
        });

        self.ledger.submit(vec![&tx])?;

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
            ApiCommand::Activity(ActivityCommand::Create { name, namespace }) => {
                self.create_activity(name, namespace)
            }
            ApiCommand::Activity(ActivityCommand::Start {
                name,
                namespace,
                time,
            }) => self.start_activity(name, namespace, time),
            ApiCommand::Activity(ActivityCommand::End {
                name,
                namespace,
                time,
            }) => self.end_activity(name, namespace, time),
            ApiCommand::Activity(ActivityCommand::Use {
                name,
                namespace,
                activity,
            }) => self.activity_use(name, namespace, activity),
            ApiCommand::Activity(ActivityCommand::Generate {
                name,
                namespace,
                activity,
            }) => self.activity_generate(name, namespace, activity),
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
        self.ensure_namespace(&namespace)?;
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

    fn use_agent(&self, name: String, namespace: String) -> Result<ApiResponse, ApiError> {
        self.store.use_agent(name, namespace)?;

        Ok(ApiResponse::Unit)
    }

    #[instrument]
    fn create_activity(&self, name: String, namespace: String) -> Result<ApiResponse, ApiError> {
        self.ensure_namespace(&namespace)?;
        let name = self.store.disambiguate_activity_name(&name)?;
        let namespace = self.store.namespace_by_name(&namespace)?;
        let id = ChronicleVocab::activity(&name);
        let tx = ChronicleTransaction::CreateActivity(CreateActivity {
            namespace,
            id: id.into(),
            name,
        });

        self.ledger.submit(vec![&tx])?;

        Ok(ApiResponse::Prov(self.store.apply(&tx)?))
    }

    #[instrument]
    pub(crate) fn start_activity(
        &self,
        name: String,
        namespace: String,
        time: Option<DateTime<Utc>>,
    ) -> Result<ApiResponse, ApiError> {
        let agent = self
            .store
            .get_current_agent()
            .map_err(|_| ApiError::NoCurrentAgent {})?;

        let name = self.store.disambiguate_activity_name(&name)?;
        let namespace = self.store.namespace_by_name(&namespace)?;
        let id = ChronicleVocab::activity(&name);
        let tx = ChronicleTransaction::StartActivity(StartActivity {
            namespace,
            id: id.into(),
            agent: ChronicleVocab::agent(&agent.name).into(),
            time: time.unwrap_or(Utc::now()),
        });

        self.ledger.submit(vec![&tx])?;

        Ok(ApiResponse::Prov(self.store.apply(&tx)?))
    }

    #[instrument]
    pub(crate) fn end_activity(
        &self,
        name: Option<String>,
        namespace: Option<String>,
        time: Option<DateTime<Utc>>,
    ) -> Result<ApiResponse, ApiError> {
        let activity = self
            .store
            .get_activity_by_name_or_last_started(name, namespace)?;

        let agent = self
            .store
            .get_current_agent()
            .map_err(|_| ApiError::NoCurrentAgent {})?;

        let namespace = self.store.namespace_by_name(&activity.namespace)?;

        let id = ChronicleVocab::activity(&activity.name);
        let tx = ChronicleTransaction::EndActivity(EndActivity {
            namespace,
            id: id.into(),
            agent: ChronicleVocab::agent(&agent.name).into(),
            time: time.unwrap_or(Utc::now()),
        });

        self.ledger.submit(vec![&tx])?;

        Ok(ApiResponse::Prov(self.store.apply(&tx)?))
    }

    #[instrument]
    pub(crate) fn activity_use(
        &self,
        name: String,
        namespace: String,
        activity: Option<String>,
    ) -> Result<ApiResponse, ApiError> {
        let activity = self
            .store
            .get_activity_by_name_or_last_started(activity, Some(namespace.clone()))?;

        self.ensure_namespace(&namespace)?;
        let namespace = self.store.namespace_by_name(&namespace)?;

        let name = self.store.disambiguate_entity_name(&name)?;
        let tx = ChronicleTransaction::ActivityUses(ActivityUses {
            namespace,
            id: ChronicleVocab::entity(&name).into(),
            activity: ChronicleVocab::activity(&activity.name).into(),
        });

        self.ledger.submit(vec![&tx])?;

        Ok(ApiResponse::Prov(self.store.apply(&tx)?))
    }

    #[instrument]
    pub(crate) fn activity_generate(
        &self,
        name: String,
        namespace: String,
        activity: Option<String>,
    ) -> Result<ApiResponse, ApiError> {
        self.ensure_namespace(&namespace)?;
        let activity = self
            .store
            .get_activity_by_name_or_last_started(activity, Some(namespace.clone()))?;

        let namespace = self.store.namespace_by_name(&namespace)?;
        let name = self.store.disambiguate_entity_name(&name)?;

        let tx = ChronicleTransaction::GenerateEntity(GenerateEntity {
            namespace,
            id: ChronicleVocab::entity(&name).into(),
            activity: ChronicleVocab::activity(&activity.name).into(),
        });

        self.ledger.submit(vec![&tx])?;

        Ok(ApiResponse::Prov(self.store.apply(&tx)?))
    }
}

#[cfg(test)]
mod test {
    use chrono::{TimeZone, Utc};
    use common::ledger::InMemLedger;
    use tempfile::TempDir;
    use tracing::Level;
    use uuid::Uuid;

    use crate::{ActivityCommand, AgentCommand, Api, ApiCommand, ApiResponse, NamespaceCommand};

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

    #[test]
    fn agent_publiv_key() {
        let api = test_api();

        api.dispatch(ApiCommand::NameSpace(NamespaceCommand::Create {
            name: "testns".to_owned(),
        }))
        .unwrap();

        let prov = api
            .dispatch(ApiCommand::Agent(AgentCommand::RegisterKey {
                name: "testagent".to_owned(),
                namespace: "testns".to_owned(),
                public: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_owned(),
                private: Some("/test/path".to_owned()),
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
    fn create_activity() {
        let api = test_api();

        let prov = api
            .dispatch(ApiCommand::Activity(ActivityCommand::Create {
                name: "testactivity".to_owned(),
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

    #[test]
    fn start_activity() {
        let api = test_api();

        api.dispatch(ApiCommand::Agent(AgentCommand::Create {
            name: "testagent".to_owned(),
            namespace: "testns".to_owned(),
        }))
        .unwrap();

        api.dispatch(ApiCommand::Agent(AgentCommand::Use {
            name: "testagent_0".to_owned(),
            namespace: "testns".to_owned(),
        }))
        .unwrap();

        let prov = api
            .dispatch(ApiCommand::Activity(ActivityCommand::Start {
                name: "testactivity".to_owned(),
                namespace: "testns".to_owned(),
                time: Some(Utc.ymd(2014, 7, 8).and_hms(9, 10, 11)),
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
    fn end_activity() {
        let api = test_api();

        api.dispatch(ApiCommand::Agent(AgentCommand::Create {
            name: "testagent".to_owned(),
            namespace: "testns".to_owned(),
        }))
        .unwrap();

        api.dispatch(ApiCommand::Agent(AgentCommand::Use {
            name: "testagent_0".to_owned(),
            namespace: "testns".to_owned(),
        }))
        .unwrap();

        api.dispatch(ApiCommand::Activity(ActivityCommand::Start {
            name: "testactivity".to_owned(),
            namespace: "testns".to_owned(),
            time: Some(Utc.ymd(2014, 7, 8).and_hms(9, 10, 11)),
        }))
        .unwrap();

        let prov = api
            .dispatch(ApiCommand::Activity(ActivityCommand::End {
                name: None,
                namespace: None,
                time: Some(Utc.ymd(2014, 7, 8).and_hms(9, 10, 11)),
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
    fn activity_use() {
        let api = test_api();

        api.dispatch(ApiCommand::Activity(ActivityCommand::Create {
            name: "testactivity".to_owned(),
            namespace: "testns".to_owned(),
        }))
        .unwrap();

        let prov = api
            .dispatch(ApiCommand::Activity(ActivityCommand::Use {
                name: "testactivity".to_owned(),
                namespace: "testns".to_owned(),
                activity: None,
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
    fn activity_generate() {
        let api = test_api();

        api.dispatch(ApiCommand::Activity(ActivityCommand::Create {
            name: "testactivity".to_owned(),
            namespace: "testns".to_owned(),
        }))
        .unwrap();

        let prov = api
            .dispatch(ApiCommand::Activity(ActivityCommand::Generate {
                name: "testactivity".to_owned(),
                namespace: "testns".to_owned(),
                activity: None,
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
