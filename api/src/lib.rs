#![cfg_attr(feature = "strict", deny(warnings))]
mod graphql;
mod persistence;

use chrono::{DateTime, Utc};
use custom_error::*;
use derivative::*;
use diesel::{r2d2::ConnectionManager, SqliteConnection};
use diesel_migrations::MigrationHarness;
use futures::{select, AsyncReadExt, Future, FutureExt, StreamExt};
use iref::IriBuf;
use k256::ecdsa::{signature::Signer, Signature};
use persistence::{ConnectionOptions, Store, StoreError, MIGRATIONS};
use r2d2::Pool;
use std::{
    convert::Infallible,
    marker::PhantomData,
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
    ledger::{LedgerReader, LedgerWriter, Offset, SubmissionError, SubscriptionError},
    prov::{
        vocab::Chronicle as ChronicleVocab, ActivityUses, ChronicleTransaction, CreateActivity,
        CreateAgent, CreateNamespace, Domaintype, EndActivity, EntityAttach, GenerateEntity,
        NamespaceId, ProvModel, RegisterKey, StartActivity,
    },
    signing::{DirectoryStoredKeys, SignerError},
};

use tracing::{debug, error, info_span, instrument, trace, warn, Instrument};

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
    Join{source: JoinError}                                     = "Blocking thread pool",
    Subscription{source: SubscriptionError}                     = "State update subscription",
    NotCurrentActivity{}                                        = "No appropriate activity to end",
}

/// Ugly but we need this until ! is stable https://github.com/rust-lang/rust/issues/64715
impl From<Infallible> for ApiError {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}

impl UFE for ApiError {}

type LedgerSendWithReply = (
    Uuid,
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
                    if let Some((correlation_id, submission, reply)) = rx.recv().await {
                        let result = ledger_writer
                            .submit(correlation_id, submission.iter().collect())
                            .await;

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

    fn submit_blocking(
        &mut self,
        correlation_id: Uuid,
        tx: &[ChronicleTransaction],
    ) -> Result<(), ApiError> {
        let (reply_tx, mut reply_rx) = mpsc::channel(1);
        trace!(?tx, "Dispatch submission to ledger");
        self.tx
            .clone()
            .blocking_send((correlation_id, tx.to_vec(), reply_tx))?;

        let reply = reply_rx.blocking_recv();

        if let Some(Err(ref error)) = reply {
            error!(?error, "Ledger dispatch");
        }

        Ok(reply.ok_or(ApiError::ApiShutdownRx {})??)
    }
}

type ApiSendWithReply = (ApiCommand, Sender<Result<ApiResponse, ApiError>>);

pub trait UuidGen {
    fn uuid() -> Uuid {
        Uuid::new_v4()
    }
}

#[derive(Derivative)]
#[derivative(Debug, Clone)]
pub struct Api<U>
where
    U: UuidGen + Send + Sync + Clone,
{
    tx: Sender<ApiSendWithReply>,
    #[derivative(Debug = "ignore")]
    keystore: DirectoryStoredKeys,
    ledger_writer: BlockingLedgerWriter,
    #[derivative(Debug = "ignore")]
    store: persistence::Store,
    #[derivative(Debug = "ignore")]
    uuidsource: PhantomData<U>,
}

#[derive(Debug, Clone)]
/// A clonable api handle
pub struct ApiDispatch {
    tx: Sender<ApiSendWithReply>,
    notify_commit: tokio::sync::broadcast::Sender<(ProvModel, Uuid)>,
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

impl<U> Api<U>
where
    U: UuidGen + Send + Sync + Clone + std::fmt::Debug + 'static,
{
    #[instrument(skip(ledger_writer, ledger_reader))]
    pub async fn new<R, W>(
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

        let (commit_notify_tx, _) = tokio::sync::broadcast::channel(20);
        let dispatch = ApiDispatch {
            tx: tx.clone(),
            notify_commit: commit_notify_tx.clone(),
        };

        let secret_path = secret_path.to_owned();

        let store = Store::new(pool.clone())?;

        pool.get()?
            .immediate_transaction(|connection| {
                connection.run_pending_migrations(MIGRATIONS).map(|_| ())
            })
            .map_err(|migration| StoreError::DbMigration { migration })?;

        let ql_pool = pool;

        tokio::task::spawn(async move {
            let keystore = DirectoryStoredKeys::new(secret_path).unwrap();

            // Get last committed offset from the store before we attach it to ledger state updates and the api
            let mut state_updates = ledger_reader
                .state_updates(
                    store
                        .get_last_offset()
                        .unwrap_or(Some(Offset::Genesis))
                        .unwrap_or(Offset::Genesis),
                )
                .await
                .unwrap();

            let mut api = Api::<U> {
                tx: tx.clone(),
                keystore,
                ledger_writer: BlockingLedgerWriter::new(ledger_writer),
                store,
                uuidsource: PhantomData::default(),
            };

            debug!(?api, "Api running on localset");
            loop {
                select! {
                        state = state_updates.next().fuse() =>{
                            if let Some((offset, prov, correlation_id)) = state {
                                    api.sync(&prov, offset.clone())
                                        .instrument(info_span!("Incoming confirmation", offset = ?offset, correlation_id = ?correlation_id))
                                        .await
                                        .map_err(|e| {
                                            error!(?e, "Api sync to confirmed commit");
                                        }).map(|_| commit_notify_tx.send((*prov,correlation_id))).ok();
                            }
                        },
                        cmd = rx.recv().fuse() => {
                            if let Some((command, reply)) = cmd {
                            trace!(?rx, "Recv api command from channel");

                            let result = api.dispatch(command).await;

                            reply
                                .send(result)
                                .await
                                .map_err(|e| {
                                    warn!(?e, "Send reply to Api consumer failed");
                                })
                                .ok();
                            }
                    }
                    complete => break
                }
            }
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

            let uuid = U::uuid();
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
            Ok((ns?.0, vec![]))
        }
    }

    /// Creates and submits a (ChronicleTransaction::GenerateEntity), and possibly (ChronicleTransaction::Domaintype) if specified
    ///
    /// We use our local store for a best guess at the activity, either by name or the last one started as a convenience for command line
    #[instrument]
    async fn activity_generate(
        &self,
        correlation_id: Uuid,
        name: String,
        namespace: String,
        activity: Option<String>,
        domaintype: Option<String>,
    ) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;

            connection.immediate_transaction(|connection| {
                let (namespace, mut to_apply) = api.ensure_namespace(connection, &namespace)?;
                let activity = api
                    .store
                    .get_activity_by_name_or_last_started(connection, activity, &namespace)?;

                let entity = api
                    .store
                    .entity_by_entity_name_and_namespace(connection, &name, &namespace);

                let name = {
                    if let Ok(existing) = entity {
                        debug!(?existing, "Use existing entity");
                        existing.name
                    } else {
                        debug!(?name, "Need new entity");
                        api.store
                            .disambiguate_entity_name(connection, &name, &namespace)?
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

                api.ledger_writer
                    .submit_blocking(correlation_id, &to_apply)?;

                Ok(ApiResponse::Prov(
                    id,
                    vec![ProvModel::from_tx(&to_apply)],
                    correlation_id,
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
        correlation_id: Uuid,
        name: String,
        namespace: String,
        activity: Option<String>,
        domaintype: Option<String>,
    ) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;

            connection.immediate_transaction(|connection| {
                let (namespace, mut to_apply) = api.ensure_namespace(connection, &namespace)?;
                let (id, to_apply) = {
                    let activity = api
                        .store
                        .get_activity_by_name_or_last_started(connection, activity, &namespace)?;

                    let entity = api
                        .store
                        .entity_by_entity_name_and_namespace(connection, &name, &namespace);

                    let name = {
                        if let Ok(existing) = entity {
                            debug!(?existing, "Use existing entity");
                            existing.name
                        } else {
                            debug!(?name, "Need new entity");
                            api.store
                                .disambiguate_entity_name(connection, &name, &namespace)?
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

                api.ledger_writer
                    .submit_blocking(correlation_id, &to_apply)?;

                Ok(ApiResponse::Prov(
                    id,
                    vec![ProvModel::from_tx(&to_apply)],
                    correlation_id,
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
        correlation_id: Uuid,
        name: String,
        namespace: String,
        domaintype: Option<String>,
    ) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;

            connection.immediate_transaction(|connection| {
                let (namespace, mut to_apply) = api.ensure_namespace(connection, &namespace)?;

                let name = api
                    .store
                    .disambiguate_activity_name(connection, &name, &namespace)?;
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

                api.ledger_writer
                    .submit_blocking(correlation_id, &to_apply)?;

                Ok(ApiResponse::Prov(
                    id,
                    vec![ProvModel::from_tx(&to_apply)],
                    correlation_id,
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
        correlation_id: Uuid,
        name: String,
        namespace: String,
        domaintype: Option<String>,
    ) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;

            connection.immediate_transaction(|connection| {
                let (namespace, mut to_apply) = api.ensure_namespace(connection, &namespace)?;

                let name = api
                    .store
                    .disambiguate_agent_name(connection, &name, &namespace)?;

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

                api.ledger_writer
                    .submit_blocking(correlation_id, &to_apply)?;

                Ok(ApiResponse::Prov(
                    iri,
                    vec![ProvModel::from_tx(&to_apply)],
                    correlation_id,
                ))
            })
        })
        .await?
    }

    /// Creates and submits a (ChronicleTransaction::CreateNamespace) if the name part does not already exist in local storage
    async fn create_namespace(
        &self,
        correlation_id: Uuid,
        name: &str,
    ) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();
        let name = name.to_owned();
        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;
            connection.immediate_transaction(|connection| {
                let (namespace, to_apply) = api.ensure_namespace(connection, &name)?;

                api.ledger_writer
                    .submit_blocking(correlation_id, &to_apply)?;

                Ok(ApiResponse::Prov(
                    IriBuf::new(&*namespace).unwrap(),
                    vec![ProvModel::from_tx(&to_apply)],
                    correlation_id,
                ))
            })
        })
        .await?
    }

    #[instrument]
    async fn dispatch(&mut self, command: ApiCommand) -> Result<ApiResponse, ApiError> {
        let correlation_id = Uuid::new_v4();
        match &command {
            ApiCommand::NameSpace(NamespaceCommand::Create { name }) => {
                self.create_namespace(correlation_id, name)
                    .instrument(info_span!("Api", %correlation_id, ?command))
                    .await
            }
            ApiCommand::Agent(AgentCommand::Create {
                name,
                namespace,
                domaintype,
            }) => {
                self.create_agent(
                    correlation_id,
                    name.to_owned(),
                    namespace.to_owned(),
                    domaintype.clone(),
                )
                .instrument(info_span!("Api", %correlation_id, ?command))
                .await
            }
            ApiCommand::Agent(AgentCommand::RegisterKey {
                name,
                namespace,
                registration,
            }) => {
                self.register_key(
                    correlation_id,
                    name.to_owned(),
                    namespace.to_owned(),
                    registration.to_owned(),
                )
                .instrument(info_span!("Api", %correlation_id, ?command))
                .await
            }
            ApiCommand::Agent(AgentCommand::Use { name, namespace }) => {
                self.use_agent(name.to_owned(), namespace.to_owned())
                    .instrument(info_span!("Api", %correlation_id, ?command))
                    .await
            }
            ApiCommand::Activity(ActivityCommand::Create {
                name,
                namespace,
                domaintype,
            }) => {
                self.create_activity(
                    correlation_id,
                    name.to_owned(),
                    namespace.to_owned(),
                    domaintype.clone(),
                )
                .instrument(info_span!("Api", %correlation_id, ?command))
                .await
            }
            ApiCommand::Activity(ActivityCommand::Start {
                name,
                namespace,
                time,
                agent,
            }) => {
                self.start_activity(
                    correlation_id,
                    name.to_owned(),
                    namespace.to_owned(),
                    time.to_owned(),
                    agent.to_owned(),
                )
                .instrument(info_span!("Api", %correlation_id, ?command))
                .await
            }
            ApiCommand::Activity(ActivityCommand::End {
                name,
                namespace,
                time,
                agent,
            }) => {
                self.end_activity(
                    correlation_id,
                    name.to_owned(),
                    namespace.to_owned(),
                    time.to_owned(),
                    agent.to_owned(),
                )
                .instrument(info_span!("Api", %correlation_id, ?command))
                .await
            }
            ApiCommand::Activity(ActivityCommand::Use {
                name,
                namespace,
                activity,
                domaintype,
            }) => {
                self.activity_use(
                    correlation_id,
                    name.to_owned(),
                    namespace.to_owned(),
                    activity.to_owned(),
                    domaintype.to_owned(),
                )
                .instrument(info_span!("Api", %correlation_id, ?command))
                .await
            }
            ApiCommand::Activity(ActivityCommand::Generate {
                name,
                namespace,
                activity,
                domaintype,
            }) => {
                self.activity_generate(
                    correlation_id,
                    name.to_owned(),
                    namespace.to_owned(),
                    activity.to_owned(),
                    domaintype.to_owned(),
                )
                .instrument(info_span!("Api", %correlation_id, ?command))
                .await
            }
            ApiCommand::Entity(EntityCommand::Attach {
                name,
                namespace,
                file,
                locator,
                agent,
            }) => {
                self.entity_attach(
                    correlation_id,
                    name.to_owned(),
                    namespace.to_owned(),
                    file.clone(),
                    locator.to_owned(),
                    agent.to_owned(),
                )
                .instrument(info_span!("Api", %correlation_id, ?command))
                .await
            }
            ApiCommand::Query(query) => self.query(query.to_owned()).await,
        }
    }

    /// Creates and submits a (ChronicleTransaction::EntityAttach), reading input files and using the agent's private keys as required
    ///
    /// # Notes
    /// Slighty messy combination of sync / async, very large input files will cause issues without the use of the async_signer crate
    #[instrument]
    async fn entity_attach(
        &self,
        correlation_id: Uuid,
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

            connection.immediate_transaction(|connection| {
                let (namespace, mut to_apply) = api.ensure_namespace(connection, &namespace)?;

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

                let signer = api.keystore.agent_signing(&agentid)?;

                let signature: Signature = signer.sign(&*buf);

                let tx = ChronicleTransaction::EntityAttach(EntityAttach {
                    namespace,
                    id: id.clone().into(),
                    agent: agentid.clone(),
                    identityid: ChronicleVocab::identity(
                        &agentid,
                        &*hex::encode_upper(signer.to_bytes()),
                    )
                    .into(),
                    signature: hex::encode_upper(signature),
                    locator,
                    signature_time: Utc::now(),
                });

                to_apply.push(tx);

                api.ledger_writer
                    .submit_blocking(correlation_id, &to_apply)?;

                Ok(ApiResponse::Prov(
                    id,
                    vec![ProvModel::from_tx(&to_apply)],
                    correlation_id,
                ))
            })
        })
        .await?
    }

    async fn query(&self, query: QueryCommand) -> Result<ApiResponse, ApiError> {
        let api = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;

            let (id, _) = api
                .store
                .namespace_by_name(&mut connection, &query.namespace)?;
            Ok(ApiResponse::Prov(
                IriBuf::new(id.as_str()).unwrap(),
                vec![api.store.prov_model_for_namespace(&mut connection, query)?],
                Uuid::default(),
            ))
        })
        .await?
    }

    #[instrument]
    async fn sync(&self, prov: &ProvModel, offset: Offset) -> Result<ApiResponse, ApiError> {
        let api = self.clone();
        let prov = prov.clone();

        tokio::task::spawn_blocking(move || {
            api.store.apply_prov(&prov)?;
            api.store.set_last_offset(offset)?;

            Ok(ApiResponse::Unit)
        })
        .await?
    }

    /// Creates and submits a (ChronicleTransaction::RegisterKey), implicitly verifying the input keys and saving to the local key store as required
    #[instrument]
    async fn register_key(
        &self,
        correlation_id: Uuid,
        name: String,
        namespace: String,
        registration: KeyRegistration,
    ) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;
            connection.immediate_transaction(|connection| {
                let (namespace, mut to_apply) = api.ensure_namespace(connection, &namespace)?;

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

                api.ledger_writer
                    .submit_blocking(correlation_id, &to_apply)?;

                Ok(ApiResponse::Prov(
                    id,
                    vec![ProvModel::from_tx(&to_apply)],
                    correlation_id,
                ))
            })
        })
        .await?
    }

    /// Creates and submits a (ChronicleTransaction::StartActivity), determining the appropriate agent by name, or via [use_agent] context
    #[instrument]
    async fn start_activity(
        &self,
        correlation_id: Uuid,
        name: String,
        namespace: String,
        time: Option<DateTime<Utc>>,
        agent: Option<String>,
    ) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;
            connection.immediate_transaction(|connection| {
                let (namespace, mut to_apply) = api.ensure_namespace(connection, &namespace)?;
                let agent = {
                    if let Some(agent) = agent {
                        api.store
                            .agent_by_agent_name_and_namespace(connection, &agent, &namespace)?
                    } else {
                        api.store
                            .get_current_agent(connection)
                            .map_err(|_| ApiError::NoCurrentAgent {})?
                    }
                };

                let activity = api
                    .store
                    .activity_by_activity_name_and_namespace(connection, &name, &namespace);

                let name = {
                    if let Ok(existing) = activity {
                        debug!(?existing, "Use existing activity");
                        existing.name
                    } else {
                        debug!(?name, "Need new activity");
                        api.store
                            .disambiguate_activity_name(connection, &name, &namespace)?
                    }
                };

                let id = ChronicleVocab::activity(&name);
                to_apply.push(ChronicleTransaction::StartActivity(StartActivity {
                    namespace,
                    id: id.clone().into(),
                    agent: ChronicleVocab::agent(&agent.name).into(),
                    time: time.unwrap_or_else(Utc::now),
                }));

                api.ledger_writer
                    .submit_blocking(correlation_id, &to_apply)?;

                Ok(ApiResponse::Prov(
                    id,
                    vec![ProvModel::from_tx(&to_apply)],
                    correlation_id,
                ))
            })
        })
        .await?
    }

    /// Creates and submits a (ChronicleTransaction::EndActivity), determining the appropriate agent by name, or via [use_agent] context
    #[instrument]
    async fn end_activity(
        &self,
        correlation_id: Uuid,
        name: Option<String>,
        namespace: String,
        time: Option<DateTime<Utc>>,
        agent: Option<String>,
    ) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;
            connection.immediate_transaction(|connection| {
                let (namespace, mut to_apply) = api.ensure_namespace(connection, &namespace)?;
                let activity = api
                    .store
                    .get_activity_by_name_or_last_started(connection, name, &namespace)
                    .map_err(|_| ApiError::NotCurrentActivity {})?;

                let agent = {
                    if let Some(agent) = agent {
                        api.store
                            .agent_by_agent_name_and_namespace(connection, &agent, &namespace)?
                    } else {
                        api.store
                            .get_current_agent(connection)
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

                api.ledger_writer
                    .submit_blocking(correlation_id, &to_apply)?;

                Ok(ApiResponse::Prov(
                    id,
                    vec![ProvModel::from_tx(&to_apply)],
                    correlation_id,
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

            connection.immediate_transaction(|connection| {
                api.store.use_agent(connection, name, namespace)
            })?;

            Ok(ApiResponse::Unit)
        })
        .await?
    }
}

#[cfg(test)]
mod test {

    use std::{net::SocketAddr, str::FromStr, time::Duration};

    use chrono::{TimeZone, Utc};
    use common::{
        commands::{ApiResponse, KeyImport},
        ledger::InMemLedger,
        prov::ProvModel,
    };

    use tempfile::TempDir;
    use tracing::Level;
    use uuid::Uuid;

    use crate::{Api, ApiDispatch, ApiError, UuidGen};

    use common::commands::{
        ActivityCommand, AgentCommand, ApiCommand, KeyRegistration, NamespaceCommand,
    };

    #[derive(Clone)]
    struct TestDispatch(ApiDispatch, ProvModel);

    impl TestDispatch {
        pub async fn dispatch(&mut self, command: ApiCommand) -> Result<(), ApiError> {
            // We can sort of get final on chain state here by using a map of subject to model
            if let ApiResponse::Prov(_subject, prov, _correlation_id) =
                self.0.dispatch(command).await?
            {
                for prov in prov {
                    self.1.merge(prov);
                }
            }

            Ok(())
        }
    }

    #[derive(Debug, Clone)]
    struct SameUuid;

    impl UuidGen for SameUuid {
        fn uuid() -> Uuid {
            Uuid::parse_str("5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea").unwrap()
        }
    }

    async fn test_api() -> TestDispatch {
        tracing_log::LogTracer::init_with_filter(tracing::log::LevelFilter::Trace).ok();
        tracing_subscriber::fmt()
            .pretty()
            .with_max_level(Level::TRACE)
            .try_init()
            .ok();

        let secretpath = TempDir::new().unwrap();
        // We need to use a real file for sqlite, as in mem either re-creates between
        // macos temp dir permissions don't work with sqlite
        std::fs::create_dir("./sqlite_test").ok();
        let dbid = Uuid::new_v4();
        let mut ledger = InMemLedger::new();
        let reader = ledger.reader();

        let (dispatch, _ui) = Api::new(
            SocketAddr::from_str("0.0.0.0:8080").unwrap(),
            &*format!("./sqlite_test/db{}.sqlite", dbid),
            ledger,
            reader,
            &secretpath.into_path(),
            SameUuid,
        )
        .await
        .unwrap();

        TestDispatch(dispatch, ProvModel::default())
    }

    macro_rules! assert_json_ld {
        ($x:expr) => {
            let mut v: serde_json::Value =
                serde_json::from_str(&*$x.1.to_json().compact().await.unwrap().to_string())
                    .unwrap();

            // Sort @graph by //@id, as objects are unordered
            if let Some(v) = v.pointer_mut("/@graph") {
                v.as_array_mut().unwrap().sort_by(|l, r| {
                    l.as_object()
                        .unwrap()
                        .get("@id")
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .cmp(r.as_object().unwrap().get("@id").unwrap().as_str().unwrap())
                });
            }

            insta::assert_snapshot!(serde_json::to_string_pretty(&v).unwrap());
        };
    }

    #[tokio::test]
    async fn create_namespace() {
        let mut api = test_api().await;

        api.dispatch(ApiCommand::NameSpace(NamespaceCommand::Create {
            name: "testns".to_owned(),
        }))
        .await
        .unwrap();

        assert_json_ld!(api);
    }

    #[tokio::test]
    async fn create_agent() {
        let mut api = test_api().await;

        api.dispatch(ApiCommand::Agent(AgentCommand::Create {
            name: "testagent".to_owned(),
            namespace: "testns".to_owned(),
            domaintype: Some("testtype".to_owned()),
        }))
        .await
        .unwrap();

        assert_json_ld!(api);
    }

    #[tokio::test]
    async fn agent_public_key() {
        let mut api = test_api().await;

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
        let mut api = test_api().await;

        api.dispatch(ApiCommand::Activity(ActivityCommand::Create {
            name: "testactivity".to_owned(),
            namespace: "testns".to_owned(),
            domaintype: Some("testtype".to_owned()),
        }))
        .await
        .unwrap();

        assert_json_ld!(api);
    }

    #[tokio::test]
    async fn start_activity() {
        let mut api = test_api().await;

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

        tokio::time::sleep(Duration::from_secs(2)).await;

        api.dispatch(ApiCommand::Activity(ActivityCommand::Start {
            name: "testactivity".to_owned(),
            namespace: "testns".to_owned(),
            time: Some(Utc.ymd(2014, 7, 8).and_hms(9, 10, 11)),
            agent: None,
        }))
        .await
        .unwrap();

        assert_json_ld!(api);
    }

    #[tokio::test]
    async fn end_activity() {
        let mut api = test_api().await;

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

        tokio::time::sleep(Duration::from_secs(2)).await;

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

        assert_json_ld!(api);
    }

    #[tokio::test]
    async fn activity_use() {
        let mut api = test_api().await;

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

        tokio::time::sleep(Duration::from_secs(2)).await;

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

        tokio::time::sleep(Duration::from_secs(4)).await;

        api.dispatch(ApiCommand::Activity(ActivityCommand::End {
            name: None,
            namespace: "testns".to_owned(),
            time: Some(Utc.ymd(2014, 7, 8).and_hms(9, 10, 11)),
            agent: Some("testagent".to_string()),
        }))
        .await
        .unwrap();

        assert_json_ld!(api);
    }

    #[tokio::test]
    async fn activity_generate() {
        let mut api = test_api().await;

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

        assert_json_ld!(api);
    }

    #[tokio::test]
    async fn many_activities() {
        let mut api = test_api().await;

        for i in 0..100 {
            api.dispatch(ApiCommand::Activity(ActivityCommand::Create {
                name: format!("testactivity{}", i),
                namespace: "testns".to_owned(),
                domaintype: Some("testtype".to_owned()),
            }))
            .await
            .unwrap();
        }

        assert_json_ld!(api);
    }
}
