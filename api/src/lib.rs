mod graphql;
mod persistence;

use chrono::{DateTime, Utc};
use custom_error::*;
use derivative::*;
use diesel::{r2d2::ConnectionManager, SqliteConnection};
use diesel_migrations::MigrationHarness;
use futures::{AsyncReadExt, Future};
use iref::IriBuf;
use k256::ecdsa::{signature::Signer, Signature};
use persistence::{ConnectionOptions, Store, StoreError, MIGRATIONS};
use r2d2::Pool;
use std::{
    convert::Infallible,
    net::{AddrParseError, SocketAddr},
    path::Path,
    sync::Arc,
    time::Duration,
};
use tokio::{
    sync::mpsc::{self, error::SendError, Sender},
    task::JoinError,
};

use common::{
    commands::*,
    ledger::{LedgerReader, LedgerWriter, SubmissionError},
    prov::{vocab::Chronicle as ChronicleVocab, Domaintype, NamespaceId, ProvModel},
    prov::{
        ActivityUses, ChronicleTransaction, CreateActivity, CreateAgent, CreateNamespace,
        EndActivity, EntityAttach, GenerateEntity, RegisterKey, StartActivity,
    },
    signing::{DirectoryStoredKeys, SignerError},
};

use tracing::{debug, error, instrument, trace};

use user_error::UFE;
use uuid::Uuid;

pub use graphql::serve_graphql;

custom_error! {pub ApiError
    Store{source: persistence::StoreError}                      = "Storage",
    Transaction{source: diesel::result::Error}                  = "Transaction failed",
    Iri{source: iref::Error}                                    = "Invalid IRI",
    JsonLD{message: String}                                     = "Json LD processing",
    Ledger{source: SubmissionError}                             = "Ledger error",
    Signing{source: SignerError}                                = "Signing",
    NoCurrentAgent{}                                            = "No agent is currently in use, please call agent use or supply an agent in your call",
    CannotFindAttachment{}                                      = "Cannot locate attachment file",
    ApiShutdownRx                                               = "Api shut down before reply",
    ApiShutdownTx{source: SendError<ApiSendWithReply>}          = "Api shut down before send",
    LedgerShutdownTx{source: SendError<LedgerSendWithReply>}    = "Ledger shut down before send",
    AddressParse{source: AddrParseError}                        = "Invalid socket address",
    ConnectionPool{source: r2d2::Error}                         = "Connection pool",
    FileUpload{source: std::io::Error}                          = "File upload",
    Join{source: JoinError}                                     = "Blocking thread pool"
}

/// Ugly but we need this until ! is stable https://github.com/rust-lang/rust/issues/64715
impl From<Infallible> for ApiError {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}

impl UFE for ApiError {}

type LedgerSendWithReply = (
    Vec<ChronicleTransaction>,
    Sender<Result<(), SubmissionError>>,
);

/// Blocking ledger writer, as we need to execute this within diesel transaction scope
#[derive(Derivative)]
#[derivative(Debug, Clone)]
pub struct BlockingLedgerWriter {
    tx: Sender<LedgerSendWithReply>,
}

impl BlockingLedgerWriter {
    #[instrument(skip(ledger_writer))]
    pub fn new<W: LedgerWriter + 'static + Send>(mut ledger_writer: W) -> Self {
        let (tx, mut rx) = mpsc::channel::<LedgerSendWithReply>(10);

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        std::thread::spawn(move || {
            let local = tokio::task::LocalSet::new();
            local.spawn_local(async move {
                loop {
                    if let Some((submission, reply)) = rx.recv().await {
                        trace!(?rx, "Recv submittion from channel");

                        let result = ledger_writer.submit(submission.iter().collect()).await;

                        trace!(?result, "Reply with");
                        reply
                            .send(result)
                            .await
                            .map_err(|e| {
                                error!(?e);
                            })
                            .ok();
                    } else {
                        return;
                    }
                }
            });

            rt.block_on(local)
        });

        Self { tx }
    }

    fn submit_blocking(&mut self, tx: &Vec<ChronicleTransaction>) -> Result<(), ApiError> {
        let (reply_tx, mut reply_rx) = mpsc::channel(1);
        trace!(?tx, "Dispatch submission to ledger");
        self.tx.clone().blocking_send((tx.clone(), reply_tx))?;

        let reply = reply_rx.blocking_recv();

        if let Some(Err(ref error)) = reply {
            error!(?error, "Ledger dispatch");
        }

        Ok(reply.ok_or(ApiError::ApiShutdownRx {})??)
    }
}

type ApiSendWithReply = (ApiCommand, Sender<Result<ApiResponse, ApiError>>);

#[derive(Derivative)]
#[derivative(Debug, Clone)]
pub struct Api<U: Fn() -> Uuid> {
    tx: Sender<ApiSendWithReply>,
    #[derivative(Debug = "ignore")]
    keystore: DirectoryStoredKeys,
    ledger_writer: BlockingLedgerWriter,
    #[derivative(Debug = "ignore")]
    store: persistence::Store,
    #[derivative(Debug = "ignore")]
    uuidsource: U,
}

#[derive(Debug, Clone)]
/// A clonable api handle
pub struct ApiDispatch {
    tx: Sender<ApiSendWithReply>,
}

impl ApiDispatch {
    #[instrument]
    pub async fn dispatch(&self, command: ApiCommand) -> Result<ApiResponse, ApiError> {
        let (reply_tx, mut reply_rx) = mpsc::channel(1);
        trace!(?command, "Dispatch command to api");
        self.tx.clone().send((command, reply_tx)).await?;

        let reply = reply_rx.recv().await;

        if let Some(Err(ref error)) = reply {
            error!(?error, "Api dispatch");
        }

        reply.ok_or(ApiError::ApiShutdownRx {})?
    }
}

impl<U: Fn() -> Uuid + Clone + Send + 'static> Api<U> {
    #[instrument(skip(ledger_writer, ledger_reader, uuidgen))]
    pub fn new<R, W>(
        addr: SocketAddr,
        dbpath: &str,
        ledger_writer: W,
        ledger_reader: R,
        secret_path: &Path,
        uuidgen: U,
    ) -> Result<(ApiDispatch, impl Future<Output = ()>), ApiError>
    where
        R: LedgerReader + 'static + Send,
        W: LedgerWriter + 'static + Send,
    {
        let (tx, mut rx) = mpsc::channel::<ApiSendWithReply>(10);

        let pool = Pool::builder()
            .connection_customizer(Box::new(ConnectionOptions {
                enable_wal: true,
                enable_foreign_keys: true,
                busy_timeout: Some(Duration::from_secs(2)),
            }))
            .build(ConnectionManager::<SqliteConnection>::new(dbpath))?;

        let dispatch = ApiDispatch { tx: tx.clone() };
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        let secret_path = secret_path.to_owned();

        let store = Store::new(pool.clone())?;

        pool.get()?
            .immediate_transaction(|connection| {
                connection.run_pending_migrations(MIGRATIONS).map(|_| ())
            })
            .map_err(|migration| StoreError::DbMigration { migration })?;

        let ql_pool = pool;

        // Get last committed offset from the store before we attach it to ledger state updates and the api
        std::thread::spawn(move || {
            let local = tokio::task::LocalSet::new();
            local.spawn_local(async move {
                let keystore = DirectoryStoredKeys::new(secret_path).unwrap();

                let mut api = Api {
                    tx: tx.clone(),
                    keystore,
                    ledger_writer: BlockingLedgerWriter::new(ledger_writer),
                    store,
                    uuidsource: Box::new(uuidgen),
                };

                debug!(?api, "Api running on localset");

                loop {
                    if let Some((command, reply)) = rx.recv().await {
                        trace!(?rx, "Recv api command from channel");

                        let result = api.dispatch(command).await;

                        reply
                            .send(result)
                            .await
                            .map_err(|e| {
                                error!(?e);
                            })
                            .ok();
                    } else {
                        return;
                    }
                }
            });

            rt.block_on(local)
        });

        Ok((
            dispatch.clone(),
            graphql::serve_graphql(ql_pool, dispatch, addr, true),
        ))
    }

    /// Ensures that the named namespace exists, returns an existing namespace, and a vector containing a `ChronicleTransaction` to create one if not present
    ///
    /// A namespace uri is of the form chronicle:ns:{name}:{uuid}
    /// Namespaces must be globally unique, so are disambiguated by uuid but are locally referred to by name only
    /// For coordination between chronicle nodes we also need a namespace binding operation to tie the UUID from another instance to a name
    /// # Arguments
    /// * `name` - an arbitrary namespace identifier
    #[instrument(skip(connection))]
    fn ensure_namespace(
        &mut self,
        connection: &mut SqliteConnection,
        name: &str,
    ) -> Result<(NamespaceId, Vec<ChronicleTransaction>), ApiError> {
        let ns = self.store.namespace_by_name(connection, name);

        if ns.is_err() {
            debug!(?ns, "Namespace does not exist, creating");

            let uuid = (self.uuidsource)();
            let iri = ChronicleVocab::namespace(name, &uuid);
            let id: NamespaceId = iri.into();
            Ok((
                id.clone(),
                vec![ChronicleTransaction::CreateNamespace(CreateNamespace {
                    id,
                    name: name.to_owned(),
                    uuid,
                })],
            ))
        } else {
            Ok((ns?, vec![]))
        }
    }

    /// Creates and submits a (ChronicleTransaction::GenerateEntity), and possibly (ChronicleTransaction::Domaintype) if specified
    ///
    /// We use our local store for a best guess at the activity, either by name or the last one started as a convenience for command line
    #[instrument]
    async fn activity_generate(
        &self,
        name: String,
        namespace: String,
        activity: Option<String>,
        domaintype: Option<String>,
    ) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;

            connection.immediate_transaction(|mut connection| {
                let (namespace, mut to_apply) =
                    api.ensure_namespace(&mut connection, &namespace)?;
                let activity = api.store.get_activity_by_name_or_last_started(
                    &mut connection,
                    activity,
                    &namespace,
                )?;

                let entity = api.store.entity_by_entity_name_and_namespace(
                    &mut connection,
                    &name,
                    &namespace,
                );

                let name = {
                    if let Ok(existing) = entity {
                        debug!(?existing, "Use existing entity");
                        existing.name
                    } else {
                        debug!(?name, "Need new entity");
                        api.store
                            .disambiguate_entity_name(&mut connection, &name, &namespace)?
                    }
                };

                let id = ChronicleVocab::entity(&name);
                let create = ChronicleTransaction::GenerateEntity(GenerateEntity {
                    namespace: namespace.clone(),
                    id: id.clone().into(),
                    activity: ChronicleVocab::activity(&activity.name).into(),
                });

                to_apply.push(create);

                if let Some(domaintype) = domaintype {
                    let set_type = ChronicleTransaction::Domaintype(Domaintype::Entity {
                        id: id.clone().into(),
                        namespace,
                        domaintype: Some(ChronicleVocab::domaintype(&domaintype).into()),
                    });

                    to_apply.push(set_type)
                }

                api.ledger_writer.submit_blocking(&to_apply)?;

                Ok(ApiResponse::Prov(
                    id,
                    vec![api.store.apply_tx(&mut connection, &to_apply)?],
                ))
            })
        })
        .await?
    }

    /// Creates and submits a (ChronicleTransaction::ActivityUses), and possibly (ChronicleTransaction::Domaintype) if specified
    ///
    /// We use our local store for a best guess at the activity, either by name or the last one started as a convenience for command line
    #[instrument]
    async fn activity_use(
        &self,
        name: String,
        namespace: String,
        activity: Option<String>,
        domaintype: Option<String>,
    ) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;

            connection.immediate_transaction(|mut connection| {
                let (namespace, mut to_apply) =
                    api.ensure_namespace(&mut connection, &namespace)?;
                let (id, to_apply) = {
                    let activity = api.store.get_activity_by_name_or_last_started(
                        &mut connection,
                        activity,
                        &namespace,
                    )?;

                    let entity = api.store.entity_by_entity_name_and_namespace(
                        &mut connection,
                        &name,
                        &namespace,
                    );

                    let name = {
                        if let Ok(existing) = entity {
                            debug!(?existing, "Use existing entity");
                            existing.name
                        } else {
                            debug!(?name, "Need new entity");
                            api.store.disambiguate_entity_name(
                                &mut connection,
                                &name,
                                &namespace,
                            )?
                        }
                    };

                    let id = ChronicleVocab::entity(&name);

                    let create = ChronicleTransaction::ActivityUses(ActivityUses {
                        namespace: namespace.clone(),
                        id: id.clone().into(),
                        activity: ChronicleVocab::activity(&activity.name).into(),
                    });

                    to_apply.push(create);

                    if let Some(domaintype) = domaintype {
                        let set_type = ChronicleTransaction::Domaintype(Domaintype::Entity {
                            id: id.clone().into(),
                            namespace,
                            domaintype: Some(ChronicleVocab::domaintype(&domaintype).into()),
                        });

                        to_apply.push(set_type)
                    }
                    (id, to_apply)
                };

                api.ledger_writer.submit_blocking(&to_apply)?;

                Ok(ApiResponse::Prov(
                    id,
                    vec![api.store.apply_tx(&mut connection, &to_apply)?],
                ))
            })
        })
        .await?
    }

    /// Creates and submits a (ChronicleTransaction::CreateActivity), and possibly (ChronicleTransaction::Domaintype) if specified
    ///
    /// We use our local store to see if the activity already exists, disambiguating the URI if so
    #[instrument]
    async fn create_activity(
        &self,
        name: String,
        namespace: String,
        domaintype: Option<String>,
    ) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;

            connection.immediate_transaction(|mut connection| {
                let (namespace, mut to_apply) =
                    api.ensure_namespace(&mut connection, &namespace)?;

                let name =
                    api.store
                        .disambiguate_activity_name(&mut connection, &name, &namespace)?;
                let id = ChronicleVocab::activity(&name);
                let create = ChronicleTransaction::CreateActivity(CreateActivity {
                    namespace: namespace.clone(),
                    id: id.clone().into(),
                    name,
                });

                to_apply.push(create);

                if let Some(domaintype) = domaintype {
                    let set_type = ChronicleTransaction::Domaintype(Domaintype::Activity {
                        id: id.clone().into(),
                        namespace,
                        domaintype: Some(ChronicleVocab::domaintype(&domaintype).into()),
                    });

                    to_apply.push(set_type)
                }

                api.ledger_writer.submit_blocking(&to_apply)?;

                Ok(ApiResponse::Prov(
                    id,
                    vec![api.store.apply_tx(&mut connection, &to_apply)?],
                ))
            })
        })
        .await?
    }

    /// Creates and submits a (ChronicleTransaction::CreateAgent), and possibly (ChronicleTransaction::Domaintype) if specified
    ///
    /// We use our local store to see if the agent already exists, disambiguating the URI if so
    #[instrument]
    async fn create_agent(
        &self,
        name: String,
        namespace: String,
        domaintype: Option<String>,
    ) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;

            connection.immediate_transaction(|mut connection| {
                let (namespace, mut to_apply) =
                    api.ensure_namespace(&mut connection, &namespace)?;

                let name = api
                    .store
                    .disambiguate_agent_name(&mut connection, &name, &namespace)?;

                let iri = ChronicleVocab::agent(&name);

                let create = ChronicleTransaction::CreateAgent(CreateAgent {
                    name: name.to_owned(),
                    id: iri.clone().into(),
                    namespace: namespace.clone(),
                });

                to_apply.push(create);

                if let Some(domaintype) = domaintype {
                    let set_type = ChronicleTransaction::Domaintype(Domaintype::Agent {
                        id: ChronicleVocab::agent(&name).into(),
                        namespace,
                        domaintype: Some(ChronicleVocab::domaintype(&domaintype).into()),
                    });

                    to_apply.push(set_type)
                }

                api.ledger_writer.submit_blocking(&to_apply)?;

                Ok(ApiResponse::Prov(
                    iri,
                    vec![api.store.apply_tx(&mut connection, &to_apply)?],
                ))
            })
        })
        .await?
    }

    /// Creates and submits a (ChronicleTransaction::CreateNamespace) if the name part does not already exist in local storage
    async fn create_namespace(&self, name: &str) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();
        let name = name.to_owned();
        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;
            connection.immediate_transaction(|mut connection| {
                let (namespace, to_apply) = api.ensure_namespace(&mut connection, &name)?;

                api.ledger_writer.submit_blocking(&to_apply)?;

                Ok(ApiResponse::Prov(
                    IriBuf::new(&*namespace).unwrap(),
                    vec![api.store.apply_tx(&mut connection, &to_apply)?],
                ))
            })
        })
        .await?
    }

    #[instrument]
    async fn dispatch(&mut self, command: ApiCommand) -> Result<ApiResponse, ApiError> {
        match command {
            ApiCommand::NameSpace(NamespaceCommand::Create { name }) => {
                self.create_namespace(&name).await
            }
            ApiCommand::Agent(AgentCommand::Create {
                name,
                namespace,
                domaintype,
            }) => self.create_agent(name, namespace, domaintype).await,
            ApiCommand::Agent(AgentCommand::RegisterKey {
                name,
                namespace,
                registration,
            }) => self.register_key(name, namespace, registration).await,
            ApiCommand::Agent(AgentCommand::Use { name, namespace }) => {
                self.use_agent(name, namespace).await
            }
            ApiCommand::Activity(ActivityCommand::Create {
                name,
                namespace,
                domaintype,
            }) => self.create_activity(name, namespace, domaintype).await,
            ApiCommand::Activity(ActivityCommand::Start {
                name,
                namespace,
                time,
                agent,
            }) => self.start_activity(name, namespace, time, agent).await,
            ApiCommand::Activity(ActivityCommand::End {
                name,
                namespace,
                time,
                agent,
            }) => self.end_activity(name, namespace, time, agent).await,
            ApiCommand::Activity(ActivityCommand::Use {
                name,
                namespace,
                activity,
                domaintype,
            }) => {
                self.activity_use(name, namespace, activity, domaintype)
                    .await
            }
            ApiCommand::Activity(ActivityCommand::Generate {
                name,
                namespace,
                activity,
                domaintype,
            }) => {
                self.activity_generate(name, namespace, activity, domaintype)
                    .await
            }
            ApiCommand::Entity(EntityCommand::Attach {
                name,
                namespace,
                file,
                locator,
                agent,
            }) => {
                self.entity_attach(name, namespace, file, locator, agent)
                    .await
            }
            ApiCommand::Query(query) => self.query(query).await,
            ApiCommand::Sync(SyncCommand { offset: _, prov }) => self.sync(prov).await,
        }
    }

    /// Creates and submits a (ChronicleTransaction::EntityAttach), reading input files and using the agent's private keys as required
    ///
    /// # Notes
    /// Slighty messy combination of sync / async, very large input files will cause issues without the use of the async_signer crate
    #[instrument]
    async fn entity_attach(
        &self,
        name: String,
        namespace: String,
        file: PathOrFile,
        locator: Option<String>,
        agent: Option<String>,
    ) -> Result<ApiResponse, ApiError> {
        // Do our file io in async context at least
        let buf = match file {
            PathOrFile::Path(ref path) => {
                std::fs::read(path).map_err(|_| ApiError::CannotFindAttachment {})
            }
            PathOrFile::File(mut file) => {
                let mut buf = vec![];
                Arc::get_mut(&mut file)
                    .unwrap()
                    .read_to_end(&mut buf)
                    .await
                    .map_err(|e| ApiError::FileUpload { source: e })?;

                Ok(buf)
            }
        }?;

        let mut api = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;

            connection.immediate_transaction(|mut connection| {
                let (namespace, mut to_apply) =
                    api.ensure_namespace(&mut connection, &namespace)?;

                let mut connection = api.store.connection()?;
                let agent = agent
                    .map(|agent| {
                        api.store.agent_by_agent_name_and_namespace(
                            &mut connection,
                            &agent,
                            &namespace,
                        )
                    })
                    .unwrap_or_else(|| api.store.get_current_agent(&mut connection))?;

                let id = ChronicleVocab::entity(&name);
                let agentid = ChronicleVocab::agent(&agent.name).into();

                let signature: Signature = api.keystore.agent_signing(&agentid)?.sign(&*buf);

                let tx = ChronicleTransaction::EntityAttach(EntityAttach {
                    namespace,
                    id: id.clone().into(),
                    agent: agentid,
                    signature: hex::encode_upper(signature),
                    locator,
                    signature_time: Utc::now(),
                });

                to_apply.push(tx);

                api.ledger_writer.submit_blocking(&to_apply)?;

                Ok(ApiResponse::Prov(
                    id,
                    vec![api.store.apply_tx(&mut connection, &to_apply)?],
                ))
            })
        })
        .await?
    }

    async fn query(&self, query: QueryCommand) -> Result<ApiResponse, ApiError> {
        let api = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;

            let id = api
                .store
                .namespace_by_name(&mut connection, &query.namespace)?;
            Ok(ApiResponse::Prov(
                IriBuf::new(id.as_str()).unwrap(),
                vec![api.store.prov_model_for_namespace(&mut connection, query)?],
            ))
        })
        .await?
    }

    async fn sync(&self, prov: ProvModel) -> Result<ApiResponse, ApiError> {
        let api = self.clone();
        tokio::task::spawn_blocking(move || {
            api.store.apply_prov(&prov)?;

            Ok(ApiResponse::Unit)
        })
        .await?
    }

    /// Creates and submits a (ChronicleTransaction::RegisterKey), implicitly verifying the input keys and saving to the local key store as required
    #[instrument]
    async fn register_key(
        &self,
        name: String,
        namespace: String,
        registration: KeyRegistration,
    ) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;
            connection.immediate_transaction(|mut connection| {
                let (namespace, mut to_apply) =
                    api.ensure_namespace(&mut connection, &namespace)?;

                let id = ChronicleVocab::agent(&name);
                match registration {
                    KeyRegistration::Generate => {
                        api.keystore.generate_agent(&id.clone().into())?;
                    }
                    KeyRegistration::ImportSigning(KeyImport::FromPath { path }) => api
                        .keystore
                        .import_agent(&id.clone().into(), Some(&path), None)?,
                    KeyRegistration::ImportSigning(KeyImport::FromPEMBuffer { buffer }) => api
                        .keystore
                        .store_agent(&id.clone().into(), Some(&buffer), None)?,
                    KeyRegistration::ImportVerifying(KeyImport::FromPath { path }) => api
                        .keystore
                        .import_agent(&id.clone().into(), None, Some(&path))?,
                    KeyRegistration::ImportVerifying(KeyImport::FromPEMBuffer { buffer }) => api
                        .keystore
                        .store_agent(&id.clone().into(), None, Some(&buffer))?,
                }

                to_apply.push(ChronicleTransaction::RegisterKey(RegisterKey {
                    id: id.clone().into(),
                    name,
                    namespace,
                    publickey: hex::encode(
                        api.keystore.agent_verifying(&id.clone().into())?.to_bytes(),
                    ),
                }));

                api.ledger_writer.submit_blocking(&to_apply)?;

                Ok(ApiResponse::Prov(
                    id,
                    vec![api.store.apply_tx(&mut connection, &to_apply)?],
                ))
            })
        })
        .await?
    }

    /// Creates and submits a (ChronicleTransaction::StartActivity), determining the appropriate agent by name, or via [use_agent] context
    #[instrument]
    async fn start_activity(
        &self,
        name: String,
        namespace: String,
        time: Option<DateTime<Utc>>,
        agent: Option<String>,
    ) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;
            connection.immediate_transaction(|mut connection| {
                let (namespace, mut to_apply) =
                    api.ensure_namespace(&mut connection, &namespace)?;
                let agent = {
                    if let Some(agent) = agent {
                        api.store.agent_by_agent_name_and_namespace(
                            &mut connection,
                            &agent,
                            &namespace,
                        )?
                    } else {
                        api.store
                            .get_current_agent(&mut connection)
                            .map_err(|_| ApiError::NoCurrentAgent {})?
                    }
                };

                let activity = api.store.activity_by_activity_name_and_namespace(
                    &mut connection,
                    &name,
                    &namespace,
                );

                let name = {
                    if let Ok(existing) = activity {
                        debug!(?existing, "Use existing activity");
                        existing.name
                    } else {
                        debug!(?name, "Need new activity");
                        api.store
                            .disambiguate_activity_name(&mut connection, &name, &namespace)?
                    }
                };

                let id = ChronicleVocab::activity(&name);
                to_apply.push(ChronicleTransaction::StartActivity(StartActivity {
                    namespace,
                    id: id.clone().into(),
                    agent: ChronicleVocab::agent(&agent.name).into(),
                    time: time.unwrap_or_else(Utc::now),
                }));

                api.ledger_writer.submit_blocking(&to_apply)?;

                Ok(ApiResponse::Prov(
                    id,
                    vec![api.store.apply_tx(&mut connection, &to_apply)?],
                ))
            })
        })
        .await?
    }

    /// Creates and submits a (ChronicleTransaction::EndActivity), determining the appropriate agent by name, or via [use_agent] context
    #[instrument]
    async fn end_activity(
        &self,
        name: Option<String>,
        namespace: String,
        time: Option<DateTime<Utc>>,
        agent: Option<String>,
    ) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;
            connection.immediate_transaction(|mut connection| {
                let (namespace, mut to_apply) =
                    api.ensure_namespace(&mut connection, &namespace)?;
                let activity = api.store.get_activity_by_name_or_last_started(
                    &mut connection,
                    name,
                    &namespace,
                )?;

                let agent = {
                    if let Some(agent) = agent {
                        api.store.agent_by_agent_name_and_namespace(
                            &mut connection,
                            &agent,
                            &namespace,
                        )?
                    } else {
                        api.store
                            .get_current_agent(&mut connection)
                            .map_err(|_| ApiError::NoCurrentAgent {})?
                    }
                };

                let id = ChronicleVocab::activity(&activity.name);
                to_apply.push(ChronicleTransaction::EndActivity(EndActivity {
                    namespace,
                    id: id.clone().into(),
                    agent: ChronicleVocab::agent(&agent.name).into(),
                    time: time.unwrap_or_else(Utc::now),
                }));

                api.ledger_writer.submit_blocking(&to_apply)?;

                Ok(ApiResponse::Prov(
                    id,
                    vec![api.store.apply_tx(&mut connection, &to_apply)?],
                ))
            })
        })
        .await?
    }

    #[instrument]
    async fn use_agent(&self, name: String, namespace: String) -> Result<ApiResponse, ApiError> {
        let api = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;

            connection.immediate_transaction(|mut connection| {
                api.store.use_agent(&mut connection, name, namespace)
            })?;

            Ok(ApiResponse::Unit)
        })
        .await?
    }
}

#[cfg(test)]
mod test {
    use std::{
        collections::{BTreeMap, HashMap},
        net::SocketAddr,
        str::FromStr,
    };

    use chrono::{TimeZone, Utc};
    use common::{
        commands::{ApiResponse, KeyImport},
        ledger::InMemLedger,
        prov::ProvModel,
    };
    use iref::IriBuf;
    use tempfile::TempDir;
    use tracing::Level;
    use uuid::Uuid;

    use crate::{Api, ApiDispatch, ApiError};

    use common::commands::{
        ActivityCommand, AgentCommand, ApiCommand, KeyRegistration, NamespaceCommand,
    };

    #[derive(Clone)]
    struct TestDispatch(ApiDispatch, ProvModel);

    impl TestDispatch {
        pub async fn dispatch(&mut self, command: ApiCommand) -> Result<(), ApiError> {
            // We can sort of get final on chain state here by using a map of subject to model
            if let ApiResponse::Prov(subject, prov) = self.0.dispatch(command).await? {
                for prov in prov {
                    self.1.merge(prov);
                }
            }

            Ok(())
        }
    }

    fn test_api() -> TestDispatch {
        tracing_log::LogTracer::init_with_filter(tracing::log::LevelFilter::Debug).ok();
        tracing_subscriber::fmt()
            .pretty()
            .with_max_level(Level::TRACE)
            .try_init()
            .ok();

        let secretpath = TempDir::new().unwrap();

        let mut ledger = InMemLedger::new();
        let reader = ledger.reader();

        let (dispatch, _ui) = Api::new(
            SocketAddr::from_str("0.0.0.0:8080").unwrap(),
            "file::memory:?cache=shared",
            ledger,
            reader,
            &secretpath.into_path(),
            || Uuid::parse_str("5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea").unwrap(),
        )
        .unwrap();

        TestDispatch(dispatch, ProvModel::default())
    }

    #[tokio::test]
    async fn create_namespace() {
        let mut api = test_api();

        api.dispatch(ApiCommand::NameSpace(NamespaceCommand::Create {
            name: "testns".to_owned(),
        }))
        .await
        .unwrap();

        insta::assert_yaml_snapshot!(api.1);
    }

    #[tokio::test]
    async fn create_agent() {
        let mut api = test_api();

        api.dispatch(ApiCommand::Agent(AgentCommand::Create {
            name: "testagent".to_owned(),
            namespace: "testns".to_owned(),
            domaintype: Some("testtype".to_owned()),
        }))
        .await
        .unwrap();

        insta::assert_yaml_snapshot!(api.1);
    }

    #[tokio::test]
    async fn agent_public_key() {
        let mut api = test_api();

        let pk = r#"
-----BEGIN PRIVATE KEY-----
MIGEAgEAMBAGByqGSM49AgEGBSuBBAAKBG0wawIBAQQgCyEwIMMP6BdfMi7qyj9n
CXfOgpTQqiEPHC7qOZl7wbGhRANCAAQZfbhU2MakiNSg7z7x/LDAbWZHj66eh6I3
Fyz29vfeI2LG5PAmY/rKJsn/cEHHx+mdz1NB3vwzV/DJqj0NM+4s
-----END PRIVATE KEY-----
"#;

        api.dispatch(ApiCommand::NameSpace(NamespaceCommand::Create {
            name: "testns".to_owned(),
        }))
        .await
        .unwrap();

        api.dispatch(ApiCommand::Agent(AgentCommand::RegisterKey {
            name: "testagent".to_owned(),
            namespace: "testns".to_owned(),
            registration: KeyRegistration::ImportSigning(KeyImport::FromPEMBuffer {
                buffer: pk.as_bytes().into(),
            }),
        }))
        .await
        .unwrap();

        insta::assert_yaml_snapshot!(api.1, {
            ".*.publickey" => "[public]"
        });
    }

    #[tokio::test]
    async fn create_activity() {
        let mut api = test_api();

        api.dispatch(ApiCommand::Activity(ActivityCommand::Create {
            name: "testactivity".to_owned(),
            namespace: "testns".to_owned(),
            domaintype: Some("testtype".to_owned()),
        }))
        .await
        .unwrap();

        insta::assert_yaml_snapshot!(api.1);
    }

    #[tokio::test]
    async fn start_activity() {
        let mut api = test_api();

        api.dispatch(ApiCommand::Agent(AgentCommand::Create {
            name: "testagent".to_owned(),
            namespace: "testns".to_owned(),
            domaintype: Some("testtype".to_owned()),
        }))
        .await
        .unwrap();

        api.dispatch(ApiCommand::Agent(AgentCommand::Use {
            name: "testagent".to_owned(),
            namespace: "testns".to_owned(),
        }))
        .await
        .unwrap();

        api.dispatch(ApiCommand::Activity(ActivityCommand::Start {
            name: "testactivity".to_owned(),
            namespace: "testns".to_owned(),
            time: Some(Utc.ymd(2014, 7, 8).and_hms(9, 10, 11)),
            agent: None,
        }))
        .await
        .unwrap();

        let v: serde_json::Value =
            serde_json::from_str(&*api.1.to_json().compact().await.unwrap().to_string()).unwrap();

        insta::assert_snapshot!(serde_json::to_string_pretty(&v).unwrap());
    }

    #[tokio::test]
    async fn end_activity() {
        let mut api = test_api();

        api.dispatch(ApiCommand::Agent(AgentCommand::Create {
            name: "testagent".to_owned(),
            namespace: "testns".to_owned(),
            domaintype: Some("testtype".to_owned()),
        }))
        .await
        .unwrap();

        api.dispatch(ApiCommand::Agent(AgentCommand::Use {
            name: "testagent".to_owned(),
            namespace: "testns".to_owned(),
        }))
        .await
        .unwrap();

        api.dispatch(ApiCommand::Activity(ActivityCommand::Start {
            name: "testactivity".to_owned(),
            namespace: "testns".to_owned(),
            time: Some(Utc.ymd(2014, 7, 8).and_hms(9, 10, 11)),
            agent: None,
        }))
        .await
        .unwrap();

        // Should end the last opened activity
        api.dispatch(ApiCommand::Activity(ActivityCommand::End {
            name: None,
            namespace: "testns".to_owned(),
            time: Some(Utc.ymd(2014, 7, 8).and_hms(9, 10, 11)),
            agent: None,
        }))
        .await
        .unwrap();

        insta::assert_yaml_snapshot!(api.1);
    }

    #[tokio::test]
    async fn activity_use() {
        let mut api = test_api();

        api.dispatch(ApiCommand::Agent(AgentCommand::Create {
            name: "testagent".to_owned(),
            namespace: "testns".to_owned(),
            domaintype: Some("testtype".to_owned()),
        }))
        .await
        .unwrap();

        api.dispatch(ApiCommand::Agent(AgentCommand::Use {
            name: "testagent".to_owned(),
            namespace: "testns".to_owned(),
        }))
        .await
        .unwrap();

        api.dispatch(ApiCommand::Activity(ActivityCommand::Create {
            name: "testactivity".to_owned(),
            namespace: "testns".to_owned(),
            domaintype: Some("testtype".to_owned()),
        }))
        .await
        .unwrap();

        api.dispatch(ApiCommand::Activity(ActivityCommand::Use {
            name: "testentity".to_owned(),
            namespace: "testns".to_owned(),
            domaintype: Some("testtype".to_owned()),
            activity: Some("testactivity".to_owned()),
        }))
        .await
        .unwrap();

        api.dispatch(ApiCommand::Activity(ActivityCommand::Use {
            name: "testentity".to_owned(),
            namespace: "testns".to_owned(),
            domaintype: Some("testtype".to_owned()),
            activity: Some("testactivity".to_owned()),
        }))
        .await
        .unwrap();

        api.dispatch(ApiCommand::Activity(ActivityCommand::End {
            name: None,
            namespace: "testns".to_owned(),
            time: Some(Utc.ymd(2014, 7, 8).and_hms(9, 10, 11)),
            agent: None,
        }))
        .await
        .unwrap();

        // Note that use should be idempotent as the name will be unique
        insta::assert_yaml_snapshot!(api.1);
    }

    #[tokio::test]
    async fn activity_generate() {
        let mut api = test_api();

        api.dispatch(ApiCommand::Activity(ActivityCommand::Create {
            name: "testactivity".to_owned(),
            namespace: "testns".to_owned(),
            domaintype: Some("testtype".to_owned()),
        }))
        .await
        .unwrap();

        api.dispatch(ApiCommand::Activity(ActivityCommand::Generate {
            name: "testentity".to_owned(),
            namespace: "testns".to_owned(),
            domaintype: Some("testtype".to_owned()),
            activity: Some("testactivity".to_owned()),
        }))
        .await
        .unwrap();

        api.dispatch(ApiCommand::Activity(ActivityCommand::Generate {
            name: "testentity".to_owned(),
            namespace: "testns".to_owned(),
            domaintype: Some("testtype".to_owned()),
            activity: Some("testactivity".to_owned()),
        }))
        .await
        .unwrap();

        // Note that generate should be idempotent as the name will be unique
        insta::assert_yaml_snapshot!(api.1);
    }

    #[tokio::test]
    async fn many_activities() {
        let mut api = test_api();

        for _ in 0..100 {
            api.dispatch(ApiCommand::Activity(ActivityCommand::Create {
                name: "testactivity".to_owned(),
                namespace: "testns".to_owned(),
                domaintype: Some("testtype".to_owned()),
            }))
            .await
            .unwrap();
        }

        insta::assert_yaml_snapshot!(api.1);
    }

    #[tokio::test]
    async fn many_concurrent_activities() {
        let api = test_api();

        let mut join = vec![];

        for _ in 0..100 {
            let mut api = api.clone();
            join.push(tokio::task::spawn(async move {
                api.dispatch(ApiCommand::Activity(ActivityCommand::Create {
                    name: "testactivity".to_owned(),
                    namespace: "testns".to_owned(),
                    domaintype: Some("testtype".to_owned()),
                }))
                .await
                .unwrap();
            }));
        }

        futures::future::join_all(join).await;

        insta::assert_yaml_snapshot!(api.1);
    }
}
