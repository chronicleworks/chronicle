#![cfg_attr(feature = "strict", deny(warnings))]
pub mod chronicle_graphql;
mod persistence;

use chrono::{DateTime, Utc};
use custom_error::*;
use derivative::*;
use diesel::{r2d2::ConnectionManager, PgConnection};
use diesel_migrations::MigrationHarness;
use futures::{select, AsyncReadExt, FutureExt, StreamExt};

use common::{
    attributes::Attributes,
    commands::*,
    identity::{AuthId, IdentityError},
    k256::ecdsa::{signature::Signer, Signature},
    ledger::{
        LedgerReader, LedgerWriter, Offset, SubmissionError, SubmissionStage, SubscriptionError,
    },
    prov::{
        operations::{
            ActivityExists, ActivityUses, ActsOnBehalfOf, AgentExists, ChronicleOperation,
            CreateNamespace, DerivationType, EndActivity, EntityDerive, EntityExists,
            EntityHasEvidence, RegisterKey, SetAttributes, StartActivity, WasAssociatedWith,
            WasGeneratedBy, WasInformedBy,
        },
        to_json_ld::ToJson,
        ActivityId, AgentId, ChronicleTransaction, ChronicleTransactionId, Contradiction, EntityId,
        ExternalId, ExternalIdPart, IdentityId, NamespaceId, ProcessorError, ProvModel, Role,
    },
    signing::{DirectoryStoredKeys, SignerError},
};

pub use persistence::StoreError;
use persistence::{Store, MIGRATIONS};
use r2d2::Pool;
use std::{
    collections::HashMap, convert::Infallible, marker::PhantomData, net::AddrParseError,
    path::Path, sync::Arc,
};
use tokio::{
    sync::mpsc::{self, error::SendError, Sender},
    task::JoinError,
};

use tracing::{debug, error, info_span, instrument, trace, warn, Instrument};

pub use persistence::ConnectionOptions;
use user_error::UFE;
use uuid::Uuid;

custom_error! {
  pub ApiError
    Store{source: persistence::StoreError}                      = "Storage {source:?}",
    Transaction{source: diesel::result::Error}                  = "Transaction failed {source}",
    Iri{source: iref::Error}                                    = "Invalid IRI {source}",
    JsonLD{message: String}                                     = "Json LD processing {message}",
    Ledger{source: SubmissionError}                             = "Ledger error {source}",
    Signing{source: SignerError}                                = "Signing {source}",
    NoCurrentAgent{}                                            = "No agent is currently in use, please call agent use or supply an agent in your call",
    CannotFindAttachment{}                                      = "Cannot locate attachment file",
    ApiShutdownRx                                               = "Api shut down before reply",
    ApiShutdownTx{source: SendError<ApiSendWithReply>}          = "Api shut down before send",
    LedgerShutdownTx{source: SendError<LedgerSendWithReply>}    = "Ledger shut down before send",
    AddressParse{source: AddrParseError}                        = "Invalid socket address {source}",
    ConnectionPool{source: r2d2::Error}                         = "Connection pool {source}",
    FileUpload{source: std::io::Error}                          = "File upload",
    Join{source: JoinError}                                     = "Blocking thread pool",
    Subscription{source: SubscriptionError}                     = "State update subscription {source}",
    NotCurrentActivity{}                                        = "No appropriate activity to end",
    EvidenceSigning{source: common::k256::ecdsa::Error}         = "Could not sign message",
    Contradiction{source: Contradiction}                        = "Contradiction {source}",
    ProcessorError{source: ProcessorError}                      = "Processor {source}",
    IdentityError{source: IdentityError}                        = "Identity {source}",
}

/// Ugly but we need this until ! is stable, see <https://github.com/rust-lang/rust/issues/64715>
impl From<Infallible> for ApiError {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}

impl UFE for ApiError {}

type LedgerSendWithReply = (
    ChronicleTransaction,
    Sender<Result<ChronicleTransactionId, SubmissionError>>,
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
                        let result = ledger_writer.submit(&submission).await;

                        reply
                            .send(result.map_err(SubmissionError::from))
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
        tx: &ChronicleTransaction,
    ) -> Result<ChronicleTransactionId, ApiError> {
        let (reply_tx, mut reply_rx) = mpsc::channel(1);
        trace!(?tx.tx, "Dispatch submission to ledger");
        self.tx.clone().blocking_send((tx.clone(), reply_tx))?;

        let reply = reply_rx.blocking_recv();

        if let Some(Err(ref error)) = reply {
            error!(?error, "Ledger dispatch");
        }

        Ok(reply.ok_or(ApiError::ApiShutdownRx {})??)
    }
}

type ApiSendWithReply = ((ApiCommand, AuthId), Sender<Result<ApiResponse, ApiError>>);

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
    submit_tx: tokio::sync::broadcast::Sender<SubmissionStage>,
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
    pub notify_commit: tokio::sync::broadcast::Sender<SubmissionStage>,
}

impl ApiDispatch {
    #[instrument]
    pub async fn dispatch(
        &self,
        command: ApiCommand,
        identity: AuthId,
    ) -> Result<ApiResponse, ApiError> {
        let (reply_tx, mut reply_rx) = mpsc::channel(1);
        trace!(?command, "Dispatch command to api");
        self.tx
            .clone()
            .send(((command, identity), reply_tx))
            .await?;

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
    #[instrument(skip(ledger_writer, ledger_reader,))]
    pub async fn new<R, W>(
        pool: Pool<ConnectionManager<PgConnection>>,
        ledger_writer: W,
        ledger_reader: R,
        secret_path: &Path,
        uuidgen: U,
        namespace_bindings: HashMap<String, Uuid>,
    ) -> Result<ApiDispatch, ApiError>
    where
        R: LedgerReader + Send + Clone + Sync + 'static,
        W: LedgerWriter + Send + 'static,
    {
        let (commit_tx, mut commit_rx) = mpsc::channel::<ApiSendWithReply>(10);

        let (commit_notify_tx, _) = tokio::sync::broadcast::channel(20);
        let dispatch = ApiDispatch {
            tx: commit_tx.clone(),
            notify_commit: commit_notify_tx.clone(),
        };

        let secret_path = secret_path.to_owned();

        let store = Store::new(pool.clone())?;

        pool.get()?
            .build_transaction()
            .run(|connection| connection.run_pending_migrations(MIGRATIONS).map(|_| ()))
            .map_err(|migration| StoreError::DbMigration { migration })?;

        for (ns, uuid) in namespace_bindings {
            store.namespace_binding(&ns, uuid)?
        }

        let reuse_reader = ledger_reader.clone();

        tokio::task::spawn(async move {
            let keystore = DirectoryStoredKeys::new(secret_path).unwrap();

            let mut api = Api::<U> {
                tx: commit_tx.clone(),
                submit_tx: commit_notify_tx.clone(),
                keystore,
                ledger_writer: BlockingLedgerWriter::new(ledger_writer),
                store: store.clone(),
                uuidsource: PhantomData::default(),
            };

            debug!(?api, "Api running on localset");

            loop {
                let mut state_updates = reuse_reader
                    .clone()
                    .state_updates(
                        store
                            .get_last_offset()
                            .map(|x| x.map(|x| x.0).unwrap_or(Offset::Genesis))
                            .unwrap_or(Offset::Genesis),
                    )
                    .await
                    .unwrap();

                loop {
                    select! {
                            state = state_updates.next().fuse() =>{

                                match state {
                                  None => {
                                    warn!("Ledger reader disconnected");
                                    break;
                                  }
                                  // Ledger contradicted or error, so nothing to
                                  // apply, but forward notification
                                  Some(commit @ Err(_)) => {
                                    commit_notify_tx.send(SubmissionStage::committed(commit)).ok();
                                  },
                                  Some(ref stage @ Ok(ref commit)) => {
                                        let offset = commit.offset.clone();
                                        let tx_id = commit.tx_id.clone();
                                        let delta = commit.delta.clone();

                                        debug!(committed = ?tx_id);
                                        debug!(delta = %delta.to_json().compact().await.unwrap().pretty());

                                        api.sync( delta, offset.clone(),tx_id.clone())
                                            .instrument(info_span!("Incoming confirmation", offset = ?offset, tx_id = %tx_id))
                                            .await
                                            .map_err(|e| {
                                                error!(?e, "Api sync to confirmed commit");
                                            }).map(|_| commit_notify_tx.send(SubmissionStage::committed(stage.clone())).ok())
                                            .ok();
                                  },
                                }
                            },
                            cmd = commit_rx.recv().fuse() => {
                                if let Some((command, reply)) = cmd {

                                let result = api
                                    .dispatch(command)
                                    .await;

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
            }
        });

        Ok(dispatch)
    }

    /// Notify after a successful submission, for now this makes little
    /// difference, but with the future introduction of a submission queue,
    /// submission notifications will be decoupled from api invocation.
    /// This is a measure to keep the api interface stable once this is introduced
    fn submit_blocking(
        &mut self,
        tx: &ChronicleTransaction,
    ) -> Result<ChronicleTransactionId, ApiError> {
        let res = self.ledger_writer.submit_blocking(tx);

        match res {
            Ok(tx_id) => {
                self.submit_tx.send(SubmissionStage::submitted(&tx_id)).ok();
                Ok(tx_id)
            }
            Err(ApiError::Ledger { ref source }) => {
                self.submit_tx
                    .send(SubmissionStage::submitted_error(source))
                    .ok();
                Err(source.clone().into())
            }
            e => e,
        }
    }

    /// Ensures that the named namespace exists, returns an existing namespace, and a vector containing a `ChronicleTransaction` to create one if not present
    ///
    /// A namespace uri is of the form chronicle:ns:{external_id}:{uuid}
    /// Namespaces must be globally unique, so are disambiguated by uuid but are locally referred to by external_id only
    /// For coordination between chronicle nodes we also need a namespace binding operation to tie the UUID from another instance to a external_id
    /// # Arguments
    /// * `external_id` - an arbitrary namespace identifier
    #[instrument(skip(connection))]
    fn ensure_namespace(
        &mut self,
        connection: &mut PgConnection,
        external_id: &ExternalId,
    ) -> Result<(NamespaceId, Vec<ChronicleOperation>), ApiError> {
        let ns = self.store.namespace_by_external_id(connection, external_id);

        if ns.is_err() {
            debug!(?ns, "Namespace does not exist, creating");

            let uuid = U::uuid();
            let id: NamespaceId = NamespaceId::from_external_id(external_id, uuid);
            Ok((
                id.clone(),
                vec![ChronicleOperation::CreateNamespace(CreateNamespace::new(
                    id,
                    external_id,
                    uuid,
                ))],
            ))
        } else {
            Ok((ns?.0, vec![]))
        }
    }

    /// Creates and submits a (ChronicleTransaction::GenerateEntity), and possibly (ChronicleTransaction::Domaintype) if specified
    ///
    /// We use our local store for a best guess at the activity, either by external_id or the last one started as a convenience for command line
    #[instrument]
    async fn activity_generate(
        &self,
        id: EntityId,
        namespace: ExternalId,
        activity_id: ActivityId,
        identity: AuthId,
    ) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;

            connection.build_transaction().run(|connection| {
                let (namespace, mut to_apply) = api.ensure_namespace(connection, &namespace)?;

                let create = ChronicleOperation::WasGeneratedBy(WasGeneratedBy {
                    namespace,
                    id: id.clone(),
                    activity: activity_id,
                });

                to_apply.push(create);

                let identity = identity.signed_identity(&api.keystore)?;

                let model = ProvModel::from_tx(&to_apply)?;
                let tx_id = api.submit_blocking(&ChronicleTransaction::new(to_apply, identity))?;

                Ok(ApiResponse::submission(id, model, tx_id))
            })
        })
        .await?
    }

    /// Creates and submits a (ChronicleTransaction::ActivityUses), and possibly (ChronicleTransaction::Domaintype) if specified
    /// We use our local store for a best guess at the activity, either by name or the last one started as a convenience for command line
    #[instrument]
    async fn activity_use(
        &self,
        id: EntityId,
        namespace: ExternalId,
        activity_id: ActivityId,
        identity: AuthId,
    ) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;

            connection.build_transaction().run(|connection| {
                let (namespace, mut to_apply) = api.ensure_namespace(connection, &namespace)?;
                let (id, to_apply) = {
                    let create = ChronicleOperation::ActivityUses(ActivityUses {
                        namespace,
                        id: id.clone(),
                        activity: activity_id,
                    });

                    to_apply.push(create);

                    (id, to_apply)
                };

                let identity = identity.signed_identity(&api.keystore)?;

                let model = ProvModel::from_tx(&to_apply)?;
                let tx_id = api.submit_blocking(&ChronicleTransaction::new(to_apply, identity))?;

                Ok(ApiResponse::submission(id, model, tx_id))
            })
        })
        .await?
    }

    /// Creates and submits a (ChronicleTransaction::ActivityWasInformedBy)
    ///
    /// We use our local store for a best guess at the activity, either by external_id or the last one started as a convenience for command line
    #[instrument]
    async fn activity_was_informed_by(
        &self,
        id: ActivityId,
        namespace: ExternalId,
        informing_activity_id: ActivityId,
        identity: AuthId,
    ) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;

            connection.build_transaction().run(|connection| {
                let (namespace, mut to_apply) = api.ensure_namespace(connection, &namespace)?;
                let (id, to_apply) = {
                    let create = ChronicleOperation::WasInformedBy(WasInformedBy {
                        namespace,
                        activity: id.clone(),
                        informing_activity: informing_activity_id,
                    });

                    to_apply.push(create);

                    (id, to_apply)
                };

                let identity = identity.signed_identity(&api.keystore)?;

                let model = ProvModel::from_tx(&to_apply)?;
                let tx_id = api.submit_blocking(&ChronicleTransaction::new(to_apply, identity))?;

                Ok(ApiResponse::submission(id, model, tx_id))
            })
        })
        .await?
    }

    /// Submits operations [`CreateEntity`], and [`SetAttributes::Entity`]
    ///
    /// We use our local store to see if the agent already exists, disambiguating the URI if so
    #[instrument]
    async fn create_entity(
        &self,
        external_id: ExternalId,
        namespace: ExternalId,
        attributes: Attributes,
        identity: AuthId,
    ) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;

            connection.build_transaction().run(|connection| {
                let (namespace, mut to_apply) = api.ensure_namespace(connection, &namespace)?;

                let id = EntityId::from_external_id(&external_id);

                let create = ChronicleOperation::EntityExists(EntityExists {
                    namespace: namespace.clone(),
                    external_id: external_id.clone(),
                });

                to_apply.push(create);

                let set_type = ChronicleOperation::SetAttributes(SetAttributes::Entity {
                    id: EntityId::from_external_id(&external_id),
                    namespace,
                    attributes,
                });

                to_apply.push(set_type);

                let identity = identity.signed_identity(&api.keystore)?;

                let model = ProvModel::from_tx(&to_apply)?;
                let tx_id = api.submit_blocking(&ChronicleTransaction::new(to_apply, identity))?;

                Ok(ApiResponse::submission(id, model, tx_id))
            })
        })
        .await?
    }

    /// Submits operations [`CreateActivity`], and [`SetAttributes::Activity`]
    ///
    /// We use our local store to see if the activity already exists, disambiguating the URI if so
    #[instrument]
    async fn create_activity(
        &self,
        external_id: ExternalId,
        namespace: ExternalId,
        attributes: Attributes,
        identity: AuthId,
    ) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;

            connection.build_transaction().run(|connection| {
                let (namespace, mut to_apply) = api.ensure_namespace(connection, &namespace)?;

                let create = ChronicleOperation::ActivityExists(ActivityExists {
                    namespace: namespace.clone(),
                    external_id: external_id.clone(),
                });

                to_apply.push(create);

                let id = ActivityId::from_external_id(&external_id);
                let set_type = ChronicleOperation::SetAttributes(SetAttributes::Activity {
                    id: id.clone(),
                    namespace,
                    attributes,
                });

                to_apply.push(set_type);

                let identity = identity.signed_identity(&api.keystore)?;

                let model = ProvModel::from_tx(&to_apply)?;
                let tx_id = api.submit_blocking(&ChronicleTransaction::new(to_apply, identity))?;

                Ok(ApiResponse::submission(id, model, tx_id))
            })
        })
        .await?
    }

    /// Submits operations [`CreateAgent`], and [`SetAttributes::Agent`]
    ///
    /// We use our local store to see if the agent already exists, disambiguating the URI if so
    #[instrument]
    async fn create_agent(
        &self,
        external_id: ExternalId,
        namespace: ExternalId,
        attributes: Attributes,
        identity: AuthId,
    ) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;

            connection.build_transaction().run(|connection| {
                let (namespace, mut to_apply) = api.ensure_namespace(connection, &namespace)?;

                let create = ChronicleOperation::AgentExists(AgentExists {
                    external_id: external_id.to_owned(),
                    namespace: namespace.clone(),
                });

                to_apply.push(create);

                let id = AgentId::from_external_id(&external_id);
                let set_type = ChronicleOperation::SetAttributes(SetAttributes::Agent {
                    id: id.clone(),
                    namespace,
                    attributes,
                });

                to_apply.push(set_type);

                let identity = identity.signed_identity(&api.keystore)?;

                let model = ProvModel::from_tx(&to_apply)?;
                let tx_id = api.submit_blocking(&ChronicleTransaction::new(to_apply, identity))?;

                Ok(ApiResponse::submission(id, model, tx_id))
            })
        })
        .await?
    }

    /// Creates and submits a (ChronicleTransaction::CreateNamespace) if the external_id part does not already exist in local storage
    async fn create_namespace(
        &self,
        external_id: &ExternalId,
        identity: AuthId,
    ) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();
        let external_id = external_id.to_owned();
        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;
            connection.build_transaction().run(|connection| {
                let (namespace, to_apply) = api.ensure_namespace(connection, &external_id)?;

                let identity = identity.signed_identity(&api.keystore)?;

                let model = ProvModel::from_tx(&to_apply)?;
                let tx_id = api.submit_blocking(&ChronicleTransaction::new(to_apply, identity))?;

                Ok(ApiResponse::submission(namespace, model, tx_id))
            })
        })
        .await?
    }

    #[instrument]
    async fn dispatch(&mut self, command: (ApiCommand, AuthId)) -> Result<ApiResponse, ApiError> {
        match command {
            (ApiCommand::NameSpace(NamespaceCommand::Create { external_id }), identity) => {
                self.create_namespace(&external_id, identity).await
            }
            (
                ApiCommand::Agent(AgentCommand::Create {
                    external_id,
                    namespace,
                    attributes,
                }),
                identity,
            ) => {
                self.create_agent(external_id, namespace, attributes, identity)
                    .await
            }
            (
                ApiCommand::Agent(AgentCommand::RegisterKey {
                    id,
                    namespace,
                    registration,
                }),
                identity,
            ) => {
                self.register_key(id, namespace, registration, identity)
                    .await
            }
            (ApiCommand::Agent(AgentCommand::UseInContext { id, namespace }), _identity) => {
                self.use_agent_in_cli_context(id, namespace).await
            }
            (
                ApiCommand::Agent(AgentCommand::Delegate {
                    id,
                    delegate,
                    activity,
                    namespace,
                    role,
                }),
                identity,
            ) => {
                self.delegate(namespace, id, delegate, activity, role, identity)
                    .await
            }
            (
                ApiCommand::Activity(ActivityCommand::Create {
                    external_id,
                    namespace,
                    attributes,
                }),
                identity,
            ) => {
                self.create_activity(external_id, namespace, attributes, identity)
                    .await
            }
            (
                ApiCommand::Activity(ActivityCommand::Instant {
                    id,
                    namespace,
                    time,
                    agent,
                }),
                identity,
            ) => self.instant(id, namespace, time, agent, identity).await,
            (
                ApiCommand::Activity(ActivityCommand::Start {
                    id,
                    namespace,
                    time,
                    agent,
                }),
                identity,
            ) => {
                self.start_activity(id, namespace, time, agent, identity)
                    .await
            }
            (
                ApiCommand::Activity(ActivityCommand::End {
                    id,
                    namespace,
                    time,
                    agent,
                }),
                identity,
            ) => {
                self.end_activity(id, namespace, time, agent, identity)
                    .await
            }
            (
                ApiCommand::Activity(ActivityCommand::Use {
                    id,
                    namespace,
                    activity,
                }),
                identity,
            ) => self.activity_use(id, namespace, activity, identity).await,
            (
                ApiCommand::Activity(ActivityCommand::WasInformedBy {
                    id,
                    namespace,
                    informing_activity,
                }),
                identity,
            ) => {
                self.activity_was_informed_by(id, namespace, informing_activity, identity)
                    .await
            }
            (
                ApiCommand::Activity(ActivityCommand::Associate {
                    id,
                    namespace,
                    responsible,
                    role,
                }),
                identity,
            ) => {
                self.associate(namespace, responsible, id, role, identity)
                    .await
            }
            (
                ApiCommand::Entity(EntityCommand::Create {
                    external_id,
                    namespace,
                    attributes,
                }),
                identity,
            ) => {
                self.create_entity(external_id, namespace, attributes, identity)
                    .await
            }
            (
                ApiCommand::Activity(ActivityCommand::Generate {
                    id,
                    namespace,
                    activity,
                }),
                identity,
            ) => {
                self.activity_generate(id, namespace, activity, identity)
                    .await
            }
            (
                ApiCommand::Entity(EntityCommand::Attach {
                    id,
                    namespace,
                    file,
                    locator,
                    agent,
                }),
                identity,
            ) => {
                self.entity_attach(id, namespace, file.clone(), locator, agent, identity)
                    .await
            }
            (
                ApiCommand::Entity(EntityCommand::Derive {
                    id,
                    namespace,
                    activity,
                    used_entity,
                    derivation,
                }),
                identity,
            ) => {
                self.entity_derive(id, namespace, activity, used_entity, derivation, identity)
                    .await
            }
            (ApiCommand::Query(query), _identity) => self.query(query).await,
        }
    }

    #[instrument(skip(self))]
    async fn delegate(
        &self,
        namespace: ExternalId,
        responsible_id: AgentId,
        delegate_id: AgentId,
        activity_id: Option<ActivityId>,
        role: Option<Role>,
        identity: AuthId,
    ) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();

        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;

            connection.build_transaction().run(|connection| {
                let (namespace, mut to_apply) = api.ensure_namespace(connection, &namespace)?;

                let tx = ChronicleOperation::AgentActsOnBehalfOf(ActsOnBehalfOf::new(
                    &namespace,
                    &responsible_id,
                    &delegate_id,
                    activity_id.as_ref(),
                    role,
                ));

                to_apply.push(tx);

                let identity = identity.signed_identity(&api.keystore)?;

                let model = ProvModel::from_tx(&to_apply)?;
                let tx_id = api.submit_blocking(&ChronicleTransaction::new(to_apply, identity))?;
                Ok(ApiResponse::submission(responsible_id, model, tx_id))
            })
        })
        .await?
    }

    #[instrument(skip(self))]
    async fn associate(
        &self,
        namespace: ExternalId,
        responsible_id: AgentId,
        activity_id: ActivityId,
        role: Option<Role>,
        identity: AuthId,
    ) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();

        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;

            connection.build_transaction().run(|connection| {
                let (namespace, mut to_apply) = api.ensure_namespace(connection, &namespace)?;

                let tx = ChronicleOperation::WasAssociatedWith(WasAssociatedWith::new(
                    &namespace,
                    &activity_id,
                    &responsible_id,
                    role,
                ));

                to_apply.push(tx);

                let identity = identity.signed_identity(&api.keystore)?;

                let model = ProvModel::from_tx(&to_apply)?;
                let tx_id = api.submit_blocking(&ChronicleTransaction::new(to_apply, identity))?;
                Ok(ApiResponse::submission(responsible_id, model, tx_id))
            })
        })
        .await?
    }

    #[instrument]
    async fn entity_derive(
        &self,
        id: EntityId,
        namespace: ExternalId,
        activity_id: Option<ActivityId>,
        used_id: EntityId,
        typ: Option<DerivationType>,
        identity: AuthId,
    ) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();

        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;

            connection.build_transaction().run(|connection| {
                let (namespace, mut to_apply) = api.ensure_namespace(connection, &namespace)?;

                let tx = ChronicleOperation::EntityDerive(EntityDerive {
                    namespace,
                    id: id.clone(),
                    used_id: used_id.clone(),
                    activity_id: activity_id.clone(),
                    typ,
                });

                to_apply.push(tx);

                let identity = identity.signed_identity(&api.keystore)?;

                let model = ProvModel::from_tx(&to_apply)?;
                let tx_id = api.submit_blocking(&ChronicleTransaction::new(to_apply, identity))?;
                Ok(ApiResponse::submission(id, model, tx_id))
            })
        })
        .await?
    }

    /// Creates and submits a (ChronicleTransaction::EntityAttach), reading input files and using the agent's private keys as required
    ///
    /// # Notes
    /// Slightly messy combination of sync / async, very large input files will cause issues without the use of the async_signer crate
    #[instrument]
    async fn entity_attach(
        &self,
        id: EntityId,
        namespace: ExternalId,
        file: PathOrFile,
        locator: Option<String>,
        agent: Option<AgentId>,
        identity: AuthId,
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

            connection.build_transaction().run(|connection| {
                let (namespace, mut to_apply) = api.ensure_namespace(connection, &namespace)?;

                let mut connection = api.store.connection()?;
                let agent = agent
                    .map(|agent| {
                        api.store.agent_by_agent_external_id_and_namespace(
                            &mut connection,
                            agent.external_id_part(),
                            &namespace,
                        )
                    })
                    .unwrap_or_else(|| api.store.get_current_agent(&mut connection))?;

                let agent_id = AgentId::from_external_id(agent.external_id);

                let signer = api.keystore.agent_signing(&agent_id)?;

                let signature: Signature = signer.try_sign(&buf)?;

                let tx = ChronicleOperation::EntityHasEvidence(EntityHasEvidence {
                    namespace,
                    id: id.clone(),
                    agent: agent_id.clone(),
                    identityid: Some(IdentityId::from_external_id(
                        agent_id.external_id_part(),
                        &*hex::encode(signer.to_bytes()),
                    )),
                    signature: Some(hex::encode(signature)),
                    locator,
                    signature_time: Some(Utc::now()),
                });

                to_apply.push(tx);

                let identity = identity.signed_identity(&api.keystore)?;

                let model = ProvModel::from_tx(&to_apply)?;
                let tx_id = api.submit_blocking(&ChronicleTransaction::new(to_apply, identity))?;

                Ok(ApiResponse::submission(id, model, tx_id))
            })
        })
        .await?
    }

    async fn query(&self, query: QueryCommand) -> Result<ApiResponse, ApiError> {
        let api = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;

            let (_id, _) = api
                .store
                .namespace_by_external_id(&mut connection, &ExternalId::from(&query.namespace))?;
            Ok(ApiResponse::query_reply(
                api.store.prov_model_for_namespace(&mut connection, query)?,
            ))
        })
        .await?
    }

    #[instrument(level = "debug", skip(self), ret(Debug))]
    async fn sync(
        &self,
        prov: Box<ProvModel>,
        offset: Offset,
        tx_id: ChronicleTransactionId,
    ) -> Result<ApiResponse, ApiError> {
        let api = self.clone();

        tokio::task::spawn_blocking(move || {
            api.store.apply_prov(&prov)?;
            api.store.set_last_offset(offset, tx_id)?;

            Ok(ApiResponse::Unit)
        })
        .await?
    }

    /// Creates and submits a (ChronicleTransaction::RegisterKey) implicitly verifying the input keys and saving to the local key store as required
    #[instrument]
    async fn register_key(
        &self,
        id: AgentId,
        namespace: ExternalId,
        registration: KeyRegistration,
        identity: AuthId,
    ) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;
            connection.build_transaction().run(|connection| {
                let (namespace, mut to_apply) = api.ensure_namespace(connection, &namespace)?;

                match registration {
                    KeyRegistration::Generate => {
                        api.keystore.generate_agent(&id)?;
                    }
                    KeyRegistration::ImportSigning(KeyImport::FromPath { path }) => {
                        api.keystore.import_agent(&id, Some(&path), None)?
                    }
                    KeyRegistration::ImportSigning(KeyImport::FromPEMBuffer { buffer }) => {
                        api.keystore.store_agent(&id, Some(&buffer), None)?
                    }
                    KeyRegistration::ImportVerifying(KeyImport::FromPath { path }) => {
                        api.keystore.import_agent(&id, None, Some(&path))?
                    }
                    KeyRegistration::ImportVerifying(KeyImport::FromPEMBuffer { buffer }) => {
                        api.keystore.store_agent(&id, None, Some(&buffer))?
                    }
                }

                to_apply.push(ChronicleOperation::RegisterKey(RegisterKey {
                    id: id.clone(),
                    namespace,
                    publickey: hex::encode(api.keystore.agent_verifying(&id)?.to_bytes()),
                }));

                let identity = identity.signed_identity(&api.keystore)?;

                let model = ProvModel::from_tx(&to_apply)?;
                let tx_id = api.submit_blocking(&ChronicleTransaction::new(to_apply, identity))?;

                Ok(ApiResponse::submission(id, model, tx_id))
            })
        })
        .await?
    }

    /// Creates and submits a (ChronicleTransaction::StartActivity) determining the appropriate agent by external_id, or via [use_agent] context
    #[instrument]
    async fn instant(
        &self,
        id: ActivityId,
        namespace: ExternalId,
        time: Option<DateTime<Utc>>,
        agent: Option<AgentId>,
        identity: AuthId,
    ) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;
            connection.build_transaction().run(|connection| {
                let (namespace, mut to_apply) = api.ensure_namespace(connection, &namespace)?;
                let agent_id = {
                    if let Some(agent) = agent {
                        Some(agent)
                    } else {
                        api.store
                            .get_current_agent(connection)
                            .ok()
                            .map(|x| AgentId::from_external_id(x.external_id))
                    }
                };

                to_apply.push(ChronicleOperation::StartActivity(StartActivity {
                    namespace: namespace.clone(),
                    id: id.clone(),
                    time: time.unwrap_or_else(Utc::now),
                }));

                to_apply.push(ChronicleOperation::EndActivity(EndActivity {
                    namespace: namespace.clone(),
                    id: id.clone(),
                    time: time.unwrap_or_else(Utc::now),
                }));

                if let Some(agent_id) = agent_id {
                    to_apply.push(ChronicleOperation::WasAssociatedWith(
                        WasAssociatedWith::new(&namespace, &id, &agent_id, None),
                    ));
                }

                let identity = identity.signed_identity(&api.keystore)?;

                let model = ProvModel::from_tx(&to_apply)?;
                let tx_id = api.submit_blocking(&ChronicleTransaction::new(to_apply, identity))?;

                Ok(ApiResponse::submission(id, model, tx_id))
            })
        })
        .await?
    }

    /// Creates and submits a (ChronicleTransaction::StartActivity), determining the appropriate agent by name, or via [use_agent] context
    #[instrument]
    async fn start_activity(
        &self,
        id: ActivityId,
        namespace: ExternalId,
        time: Option<DateTime<Utc>>,
        agent: Option<AgentId>,
        identity: AuthId,
    ) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;
            connection.build_transaction().run(|connection| {
                let (namespace, mut to_apply) = api.ensure_namespace(connection, &namespace)?;

                let agent_id = {
                    if let Some(agent) = agent {
                        Some(agent)
                    } else {
                        api.store
                            .get_current_agent(connection)
                            .ok()
                            .map(|x| AgentId::from_external_id(x.external_id))
                    }
                };

                to_apply.push(ChronicleOperation::StartActivity(StartActivity {
                    namespace: namespace.clone(),
                    id: id.clone(),
                    time: time.unwrap_or_else(Utc::now),
                }));

                if let Some(agent_id) = agent_id {
                    to_apply.push(ChronicleOperation::WasAssociatedWith(
                        WasAssociatedWith::new(&namespace, &id, &agent_id, None),
                    ));
                }

                let identity = identity.signed_identity(&api.keystore)?;

                let model = ProvModel::from_tx(&to_apply)?;
                let tx_id = api.submit_blocking(&ChronicleTransaction::new(to_apply, identity))?;

                Ok(ApiResponse::submission(id, model, tx_id))
            })
        })
        .await?
    }

    /// Creates and submits a (ChronicleTransaction::EndActivity), determining the appropriate agent by name or via [use_agent] context
    #[instrument]
    async fn end_activity(
        &self,
        id: ActivityId,
        namespace: ExternalId,
        time: Option<DateTime<Utc>>,
        agent: Option<AgentId>,
        identity: AuthId,
    ) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;
            connection.build_transaction().run(|connection| {
                let (namespace, mut to_apply) = api.ensure_namespace(connection, &namespace)?;

                let agent_id = {
                    if let Some(agent) = agent {
                        Some(agent)
                    } else {
                        api.store
                            .get_current_agent(connection)
                            .ok()
                            .map(|x| AgentId::from_external_id(x.external_id))
                    }
                };

                to_apply.push(ChronicleOperation::EndActivity(EndActivity {
                    namespace: namespace.clone(),
                    id: id.clone(),
                    time: time.unwrap_or_else(Utc::now),
                }));

                if let Some(agent_id) = agent_id {
                    to_apply.push(ChronicleOperation::WasAssociatedWith(
                        WasAssociatedWith::new(&namespace, &id, &agent_id, None),
                    ));
                }

                let identity = identity.signed_identity(&api.keystore)?;

                let model = ProvModel::from_tx(&to_apply)?;
                let tx_id = api.submit_blocking(&ChronicleTransaction::new(to_apply, identity))?;

                Ok(ApiResponse::submission(id, model, tx_id))
            })
        })
        .await?
    }

    #[instrument]
    async fn use_agent_in_cli_context(
        &self,
        id: AgentId,
        namespace: ExternalId,
    ) -> Result<ApiResponse, ApiError> {
        let api = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;

            connection.build_transaction().run(|connection| {
                api.store
                    .use_agent(connection, id.external_id_part(), &namespace)
            })?;

            Ok(ApiResponse::Unit)
        })
        .await?
    }
}

#[cfg(test)]
mod test {

    use crate::{Api, ApiDispatch, ApiError, UuidGen};
    use chrono::{TimeZone, Utc};
    use common::{
        attributes::{Attribute, Attributes},
        commands::{
            ActivityCommand, AgentCommand, ApiCommand, ApiResponse, EntityCommand, KeyImport,
            KeyRegistration, NamespaceCommand,
        },
        database::{get_embedded_db_connection, Database},
        identity::AuthId,
        k256::{
            pkcs8::{EncodePrivateKey, LineEnding},
            SecretKey,
        },
        ledger::InMemLedger,
        prov::{
            operations::DerivationType, to_json_ld::ToJson, ActivityId, AgentId,
            ChronicleTransactionId, DomaintypeId, EntityId, ProvModel,
        },
        signing::DirectoryStoredKeys,
    };
    use rand_core::SeedableRng;

    use std::collections::HashMap;
    use tempfile::TempDir;
    use uuid::Uuid;

    struct TestDispatch {
        api: ApiDispatch,
        _db: Database, // share lifetime
    }

    impl TestDispatch {
        pub async fn dispatch(
            &mut self,
            command: ApiCommand,
            identity: AuthId,
        ) -> Result<Option<(Box<ProvModel>, ChronicleTransactionId)>, ApiError> {
            // We can sort of get final on chain state here by using a map of subject to model
            if let ApiResponse::Submission { .. } = self.api.dispatch(command, identity).await? {
                // Recv until we get a commit notification
                loop {
                    let commit = self.api.notify_commit.subscribe().recv().await.unwrap();
                    match commit {
                        common::ledger::SubmissionStage::Submitted(Ok(_)) => continue,
                        common::ledger::SubmissionStage::Committed(Ok(commit)) => {
                            return Ok(Some((commit.delta, commit.tx_id)))
                        }
                        common::ledger::SubmissionStage::Submitted(Err(e)) => panic!("{e:?}"),
                        common::ledger::SubmissionStage::Committed(Err(e)) => panic!("{e:?}"),
                    }
                }
            } else {
                Ok(None)
            }
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
        chronicle_telemetry::telemetry(None, chronicle_telemetry::ConsoleLogging::Pretty);

        let secretpath = TempDir::new().unwrap().into_path();

        let keystore_path = secretpath.clone();
        let keystore = DirectoryStoredKeys::new(keystore_path).unwrap();
        keystore.generate_chronicle().unwrap();

        let mut ledger = InMemLedger::new();
        let reader = ledger.reader();

        let (database, pool) = get_embedded_db_connection().await.unwrap();

        let dispatch = Api::new(
            pool,
            ledger,
            reader,
            &secretpath,
            SameUuid,
            HashMap::default(),
        )
        .await
        .unwrap();

        TestDispatch {
            api: dispatch,
            _db: database, // share the lifetime
        }
    }

    #[tokio::test]
    async fn create_namespace() {
        let mut api = test_api().await;

        let identity = AuthId::chronicle();

        insta::assert_json_snapshot!(api
            .dispatch(ApiCommand::NameSpace(NamespaceCommand::Create {
                external_id: "testns".into(),
            }), identity)
            .await
            .unwrap()
            .unwrap()
            .0
            .to_json()
            .compact_stable_order()
            .await
            .unwrap(), @r###"
        {
          "@context": "https://btp.works/chr/1.0/c.jsonld",
          "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
          "@type": "chronicle:Namespace",
          "externalId": "testns"
        }
        "###);
    }

    #[tokio::test]
    async fn create_agent() {
        let mut api = test_api().await;

        let identity = AuthId::chronicle();

        insta::assert_json_snapshot!(api.dispatch(ApiCommand::Agent(AgentCommand::Create {
            external_id: "testagent".into(),
            namespace: "testns".into(),
            attributes: Attributes {
                typ: Some(DomaintypeId::from_external_id("test")),
                attributes: [(
                    "test".to_owned(),
                    Attribute {
                        typ: "test".to_owned(),
                        value: serde_json::Value::String("test".to_owned()),
                    },
                )]
                .into_iter()
                .collect(),
            },
            }), identity)
            .await
            .unwrap()
            .unwrap()
            .0
            .to_json()
            .compact_stable_order()
            .await
            .unwrap(), @r###"
        {
          "@context": "https://btp.works/chr/1.0/c.jsonld",
          "@graph": [
            {
              "@id": "chronicle:agent:testagent",
              "@type": [
                "prov:Agent",
                "chronicle:domaintype:test"
              ],
              "externalId": "testagent",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "test": "test"
              }
            },
            {
              "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "@type": "chronicle:Namespace",
              "externalId": "testns"
            }
          ]
        }
        "###);
    }

    fn key_from_seed(seed: u8) -> String {
        let secret: SecretKey = SecretKey::random(rand::rngs::StdRng::from_seed([seed; 32]));

        secret.to_pkcs8_pem(LineEnding::CRLF).unwrap().to_string()
    }

    #[tokio::test]
    async fn agent_public_key() {
        let mut api = test_api().await;

        let identity = AuthId::chronicle();

        let pk = key_from_seed(0);
        api.dispatch(
            ApiCommand::NameSpace(NamespaceCommand::Create {
                external_id: "testns".into(),
            }),
            identity.clone(),
        )
        .await
        .unwrap();

        let delta = api
            .dispatch(
                ApiCommand::Agent(AgentCommand::RegisterKey {
                    id: AgentId::from_external_id("testagent"),
                    namespace: "testns".into(),
                    registration: KeyRegistration::ImportSigning(KeyImport::FromPEMBuffer {
                        buffer: pk.as_bytes().into(),
                    }),
                }),
                identity,
            )
            .await
            .unwrap()
            .unwrap();

        insta::assert_yaml_snapshot!(delta.0, {
            ".*.public_key" => "[public]"
        }, @r###"
        ---
        namespaces:
          ? external_id: testns
            uuid: 5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea
          : id:
              external_id: testns
              uuid: 5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea
            uuid: 5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea
            external_id: testns
        agents:
          ? - external_id: testns
              uuid: 5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea
            - testagent
          : id: testagent
            namespaceid:
              external_id: testns
              uuid: 5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea
            external_id: testagent
            domaintypeid: ~
            attributes: {}
        activities: {}
        entities: {}
        identities:
          ? - external_id: testns
              uuid: 5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea
            - external_id: testagent
              public_key: 029bef8d556d80e43ae7e0becb3a7e6838b95defe45896ed6075bb9035d06c9964
          : id:
              external_id: testagent
              public_key: 029bef8d556d80e43ae7e0becb3a7e6838b95defe45896ed6075bb9035d06c9964
            namespaceid:
              external_id: testns
              uuid: 5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea
            public_key: 029bef8d556d80e43ae7e0becb3a7e6838b95defe45896ed6075bb9035d06c9964
        attachments: {}
        has_identity:
          ? - external_id: testns
              uuid: 5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea
            - testagent
          : - external_id: testns
              uuid: 5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea
            - external_id: testagent
              public_key: 029bef8d556d80e43ae7e0becb3a7e6838b95defe45896ed6075bb9035d06c9964
        had_identity: {}
        has_evidence: {}
        had_attachment: {}
        association: {}
        derivation: {}
        delegation: {}
        generation: {}
        usage: {}
        was_informed_by: {}
        generated: {}
        "###);
    }

    #[tokio::test]
    async fn create_activity() {
        let mut api = test_api().await;

        let identity = AuthId::chronicle();

        insta::assert_json_snapshot!(api.dispatch(ApiCommand::Activity(ActivityCommand::Create {
            external_id: "testactivity".into(),
            namespace: "testns".into(),
            attributes: Attributes {
                typ: Some(DomaintypeId::from_external_id("test")),
                attributes: [(
                    "test".to_owned(),
                    Attribute {
                        typ: "test".to_owned(),
                        value: serde_json::Value::String("test".to_owned()),
                    },
                )]
                .into_iter()
                .collect(),
            },
        }), identity)
        .await
        .unwrap()
        .unwrap()
        .0
        .to_json()
        .compact_stable_order()
        .await
        .unwrap(), @r###"
        {
          "@context": "https://btp.works/chr/1.0/c.jsonld",
          "@graph": [
            {
              "@id": "chronicle:activity:testactivity",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:test"
              ],
              "externalId": "testactivity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "test": "test"
              }
            },
            {
              "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "@type": "chronicle:Namespace",
              "externalId": "testns"
            }
          ]
        }
        "###);
    }

    #[tokio::test]
    async fn start_activity() {
        let mut api = test_api().await;

        let identity = AuthId::chronicle();

        insta::assert_json_snapshot!(
        api.dispatch(ApiCommand::Agent(AgentCommand::Create {
            external_id: "testagent".into(),
            namespace: "testns".into(),
            attributes: Attributes {
                typ: Some(DomaintypeId::from_external_id("test")),
                attributes: [(
                    "test".to_owned(),
                    Attribute {
                        typ: "test".to_owned(),
                        value: serde_json::Value::String("test".to_owned()),
                    },
                )]
                .into_iter()
                .collect(),
            },
        }), identity.clone())
        .await
        .unwrap()
        .unwrap()
        .0
        .to_json()
        .compact_stable_order()
        .await
        .unwrap(), @r###"
        {
          "@context": "https://btp.works/chr/1.0/c.jsonld",
          "@graph": [
            {
              "@id": "chronicle:agent:testagent",
              "@type": [
                "prov:Agent",
                "chronicle:domaintype:test"
              ],
              "externalId": "testagent",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "test": "test"
              }
            },
            {
              "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "@type": "chronicle:Namespace",
              "externalId": "testns"
            }
          ]
        }
        "###);

        api.dispatch(
            ApiCommand::Agent(AgentCommand::UseInContext {
                id: AgentId::from_external_id("testagent"),
                namespace: "testns".into(),
            }),
            identity.clone(),
        )
        .await
        .unwrap();

        insta::assert_json_snapshot!(
        api.dispatch(ApiCommand::Activity(ActivityCommand::Start {
            id: ActivityId::from_external_id("testactivity"),
            namespace: "testns".into(),
            time: Some(Utc.with_ymd_and_hms(2014, 7, 8, 9, 10, 11).unwrap()),
            agent: None,
        }), identity)
        .await
        .unwrap()
        .unwrap()
        .0
        .to_json()
        .compact_stable_order()
        .await
        .unwrap(), @r###"
        {
          "@context": "https://btp.works/chr/1.0/c.jsonld",
          "@graph": [
            {
              "@id": "chronicle:activity:testactivity",
              "@type": "prov:Activity",
              "externalId": "testactivity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "prov:qualifiedAssociation": {
                "@id": "chronicle:association:testagent:testactivity:role="
              },
              "startTime": "2014-07-08T09:10:11+00:00",
              "value": {},
              "wasAssociatedWith": [
                "chronicle:agent:testagent"
              ]
            },
            {
              "@id": "chronicle:association:testagent:testactivity:role=",
              "@type": "prov:Association",
              "agent": "chronicle:agent:testagent",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "prov:hadActivity": {
                "@id": "chronicle:activity:testactivity"
              }
            },
            {
              "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "@type": "chronicle:Namespace",
              "externalId": "testns"
            }
          ]
        }
        "###);
    }

    #[tokio::test]
    async fn contradict_attributes() {
        let mut api = test_api().await;

        let identity = AuthId::chronicle();

        insta::assert_json_snapshot!(
        api.dispatch(ApiCommand::Agent(AgentCommand::Create {
            external_id: "testagent".into(),
            namespace: "testns".into(),
            attributes: Attributes {
                typ: Some(DomaintypeId::from_external_id("test")),
                attributes: [(
                    "test".to_owned(),
                    Attribute {
                        typ: "test".to_owned(),
                        value: serde_json::Value::String("test".to_owned()),
                    },
                )]
                .into_iter()
                .collect(),
            },
        }), identity.clone())
        .await
        .unwrap()
        .unwrap()
        .0
        .to_json()
        .compact_stable_order()
        .await
        .unwrap(), @r###"
        {
          "@context": "https://btp.works/chr/1.0/c.jsonld",
          "@graph": [
            {
              "@id": "chronicle:agent:testagent",
              "@type": [
                "prov:Agent",
                "chronicle:domaintype:test"
              ],
              "externalId": "testagent",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "test": "test"
              }
            },
            {
              "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "@type": "chronicle:Namespace",
              "externalId": "testns"
            }
          ]
        }
        "###);

        let res = api
            .dispatch(
                ApiCommand::Agent(AgentCommand::Create {
                    external_id: "testagent".into(),
                    namespace: "testns".into(),
                    attributes: Attributes {
                        typ: Some(DomaintypeId::from_external_id("test")),
                        attributes: [(
                            "test".to_owned(),
                            Attribute {
                                typ: "test".to_owned(),
                                value: serde_json::Value::String("test2".to_owned()),
                            },
                        )]
                        .into_iter()
                        .collect(),
                    },
                }),
                identity,
            )
            .await;

        insta::assert_snapshot!(res.err().unwrap().to_string(), @r###"Ledger error Contradiction: Contradiction { attribute value change: test Attribute { typ: "test", value: String("test2") } Attribute { typ: "test", value: String("test") } }"###);
    }

    #[tokio::test]
    async fn contradict_start_time() {
        let mut api = test_api().await;

        let identity = AuthId::chronicle();

        insta::assert_json_snapshot!(
        api.dispatch(ApiCommand::Agent(AgentCommand::Create {
            external_id: "testagent".into(),
            namespace: "testns".into(),
            attributes: Attributes {
                typ: Some(DomaintypeId::from_external_id("test")),
                attributes: [(
                    "test".to_owned(),
                    Attribute {
                        typ: "test".to_owned(),
                        value: serde_json::Value::String("test".to_owned()),
                    },
                )]
                .into_iter()
                .collect(),
            },
        }), identity.clone())
        .await
        .unwrap()
        .unwrap()
        .0
        .to_json()
        .compact_stable_order()
        .await
        .unwrap(), @r###"
        {
          "@context": "https://btp.works/chr/1.0/c.jsonld",
          "@graph": [
            {
              "@id": "chronicle:agent:testagent",
              "@type": [
                "prov:Agent",
                "chronicle:domaintype:test"
              ],
              "externalId": "testagent",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "test": "test"
              }
            },
            {
              "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "@type": "chronicle:Namespace",
              "externalId": "testns"
            }
          ]
        }
        "###);

        api.dispatch(
            ApiCommand::Agent(AgentCommand::UseInContext {
                id: AgentId::from_external_id("testagent"),
                namespace: "testns".into(),
            }),
            identity.clone(),
        )
        .await
        .unwrap();

        insta::assert_json_snapshot!(
        api.dispatch(ApiCommand::Activity(ActivityCommand::Start {
            id: ActivityId::from_external_id("testactivity"),
            namespace: "testns".into(),
            time: Some(Utc.with_ymd_and_hms(2014, 7, 8, 9, 10, 11).unwrap()),
            agent: None,
        }), identity.clone())
        .await
        .unwrap()
        .unwrap()
        .0
        .to_json()
        .compact_stable_order()
        .await
        .unwrap(), @r###"
        {
          "@context": "https://btp.works/chr/1.0/c.jsonld",
          "@graph": [
            {
              "@id": "chronicle:activity:testactivity",
              "@type": "prov:Activity",
              "externalId": "testactivity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "prov:qualifiedAssociation": {
                "@id": "chronicle:association:testagent:testactivity:role="
              },
              "startTime": "2014-07-08T09:10:11+00:00",
              "value": {},
              "wasAssociatedWith": [
                "chronicle:agent:testagent"
              ]
            },
            {
              "@id": "chronicle:association:testagent:testactivity:role=",
              "@type": "prov:Association",
              "agent": "chronicle:agent:testagent",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "prov:hadActivity": {
                "@id": "chronicle:activity:testactivity"
              }
            },
            {
              "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "@type": "chronicle:Namespace",
              "externalId": "testns"
            }
          ]
        }
        "###);

        // Should contradict
        let res = api
            .dispatch(
                ApiCommand::Activity(ActivityCommand::Start {
                    id: ActivityId::from_external_id("testactivity"),
                    namespace: "testns".into(),
                    time: Some(Utc.with_ymd_and_hms(2018, 7, 8, 9, 10, 11).unwrap()),
                    agent: None,
                }),
                identity,
            )
            .await;

        insta::assert_snapshot!(res.err().unwrap().to_string(), @"Ledger error Contradiction: Contradiction { start date alteration: 2014-07-08 09:10:11 UTC 2018-07-08 09:10:11 UTC }");
    }

    #[tokio::test]
    async fn contradict_end_time() {
        let mut api = test_api().await;

        let identity = AuthId::chronicle();

        insta::assert_json_snapshot!(
        api.dispatch(ApiCommand::Agent(AgentCommand::Create {
            external_id: "testagent".into(),
            namespace: "testns".into(),
            attributes: Attributes {
                typ: Some(DomaintypeId::from_external_id("test")),
                attributes: [(
                    "test".to_owned(),
                    Attribute {
                        typ: "test".to_owned(),
                        value: serde_json::Value::String("test".to_owned()),
                    },
                )]
                .into_iter()
                .collect(),
            },
        }), identity.clone())
        .await
        .unwrap()
        .unwrap()
        .0
        .to_json()
        .compact_stable_order()
        .await
        .unwrap(), @r###"
        {
          "@context": "https://btp.works/chr/1.0/c.jsonld",
          "@graph": [
            {
              "@id": "chronicle:agent:testagent",
              "@type": [
                "prov:Agent",
                "chronicle:domaintype:test"
              ],
              "externalId": "testagent",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "test": "test"
              }
            },
            {
              "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "@type": "chronicle:Namespace",
              "externalId": "testns"
            }
          ]
        }
        "###);

        api.dispatch(
            ApiCommand::Agent(AgentCommand::UseInContext {
                id: AgentId::from_external_id("testagent"),
                namespace: "testns".into(),
            }),
            identity.clone(),
        )
        .await
        .unwrap();

        insta::assert_json_snapshot!(
        api.dispatch(ApiCommand::Activity(ActivityCommand::End {
            id: ActivityId::from_external_id("testactivity"),
            namespace: "testns".into(),
            time: Some(Utc.with_ymd_and_hms(2018, 7, 8, 9, 10, 11).unwrap()),
            agent: None,
        }), identity.clone())
        .await
        .unwrap()
        .unwrap()
        .0
        .to_json()
        .compact_stable_order()
        .await
        .unwrap(), @r###"
        {
          "@context": "https://btp.works/chr/1.0/c.jsonld",
          "@graph": [
            {
              "@id": "chronicle:activity:testactivity",
              "@type": "prov:Activity",
              "endTime": "2018-07-08T09:10:11+00:00",
              "externalId": "testactivity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "prov:qualifiedAssociation": {
                "@id": "chronicle:association:testagent:testactivity:role="
              },
              "value": {},
              "wasAssociatedWith": [
                "chronicle:agent:testagent"
              ]
            },
            {
              "@id": "chronicle:association:testagent:testactivity:role=",
              "@type": "prov:Association",
              "agent": "chronicle:agent:testagent",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "prov:hadActivity": {
                "@id": "chronicle:activity:testactivity"
              }
            },
            {
              "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "@type": "chronicle:Namespace",
              "externalId": "testns"
            }
          ]
        }
        "###);

        // Should contradict
        let res = api
            .dispatch(
                ApiCommand::Activity(ActivityCommand::End {
                    id: ActivityId::from_external_id("testactivity"),
                    namespace: "testns".into(),
                    time: Some(Utc.with_ymd_and_hms(2022, 7, 8, 9, 10, 11).unwrap()),
                    agent: None,
                }),
                identity,
            )
            .await;

        insta::assert_snapshot!(res.err().unwrap().to_string(), @"Ledger error Contradiction: Contradiction { end date alteration: 2018-07-08 09:10:11 UTC 2022-07-08 09:10:11 UTC }");
    }

    #[tokio::test]
    async fn end_activity() {
        let mut api = test_api().await;

        let identity = AuthId::chronicle();

        insta::assert_json_snapshot!(
        api.dispatch(ApiCommand::Agent(AgentCommand::Create {
            external_id: "testagent".into(),
            namespace: "testns".into(),
            attributes: Attributes {
                typ: Some(DomaintypeId::from_external_id("test")),
                attributes: [(
                    "test".to_owned(),
                    Attribute {
                        typ: "test".to_owned(),
                        value: serde_json::Value::String("test".to_owned()),
                    },
                )]
                .into_iter()
                .collect(),
            },
        }), identity.clone())
        .await
        .unwrap()
        .unwrap()
        .0
        .to_json()
        .compact_stable_order()
        .await
        .unwrap(), @r###"
        {
          "@context": "https://btp.works/chr/1.0/c.jsonld",
          "@graph": [
            {
              "@id": "chronicle:agent:testagent",
              "@type": [
                "prov:Agent",
                "chronicle:domaintype:test"
              ],
              "externalId": "testagent",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "test": "test"
              }
            },
            {
              "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "@type": "chronicle:Namespace",
              "externalId": "testns"
            }
          ]
        }
        "###);

        api.dispatch(
            ApiCommand::Agent(AgentCommand::UseInContext {
                id: AgentId::from_external_id("testagent"),
                namespace: "testns".into(),
            }),
            identity.clone(),
        )
        .await
        .unwrap();

        insta::assert_json_snapshot!(
        api.dispatch(ApiCommand::Activity(ActivityCommand::Start {
            id: ActivityId::from_external_id("testactivity"),
            namespace: "testns".into(),
            time: Some(Utc.with_ymd_and_hms(2014, 7, 8, 9, 10, 11).unwrap()),
            agent: None,
        }), identity.clone())
        .await
        .unwrap()
        .unwrap()
        .0
        .to_json()
        .compact_stable_order()
        .await
        .unwrap(), @r###"
        {
          "@context": "https://btp.works/chr/1.0/c.jsonld",
          "@graph": [
            {
              "@id": "chronicle:activity:testactivity",
              "@type": "prov:Activity",
              "externalId": "testactivity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "prov:qualifiedAssociation": {
                "@id": "chronicle:association:testagent:testactivity:role="
              },
              "startTime": "2014-07-08T09:10:11+00:00",
              "value": {},
              "wasAssociatedWith": [
                "chronicle:agent:testagent"
              ]
            },
            {
              "@id": "chronicle:association:testagent:testactivity:role=",
              "@type": "prov:Association",
              "agent": "chronicle:agent:testagent",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "prov:hadActivity": {
                "@id": "chronicle:activity:testactivity"
              }
            },
            {
              "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "@type": "chronicle:Namespace",
              "externalId": "testns"
            }
          ]
        }
        "###);

        insta::assert_json_snapshot!(
        api.dispatch(ApiCommand::Activity(ActivityCommand::End {

            id: ActivityId::from_external_id("testactivity"),
            namespace: "testns".into(),
            time: Some(Utc.with_ymd_and_hms(2014, 7, 8, 9, 10, 11).unwrap()),
            agent: None,
        }), identity.clone())
        .await
        .unwrap()
        .unwrap()
        .0
        .to_json()
        .compact_stable_order()
        .await
        .unwrap(), @r###"
        {
          "@context": "https://btp.works/chr/1.0/c.jsonld",
          "@graph": [
            {
              "@id": "chronicle:activity:testactivity",
              "@type": "prov:Activity",
              "endTime": "2014-07-08T09:10:11+00:00",
              "externalId": "testactivity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "startTime": "2014-07-08T09:10:11+00:00",
              "value": {}
            },
            {
              "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "@type": "chronicle:Namespace",
              "externalId": "testns"
            }
          ]
        }
        "###);
    }

    #[tokio::test]
    async fn activity_use() {
        let mut api = test_api().await;

        let identity = AuthId::chronicle();

        insta::assert_json_snapshot!(
        api.dispatch(ApiCommand::Agent(AgentCommand::Create {
            external_id: "testagent".into(),
            namespace: "testns".into(),
            attributes: Attributes {
                typ: Some(DomaintypeId::from_external_id("test")),
                attributes: [(
                    "test".to_owned(),
                    Attribute {
                        typ: "test".to_owned(),
                        value: serde_json::Value::String("test".to_owned()),
                    },
                )]
                .into_iter()
                .collect(),
            },
        }), identity.clone())
        .await
        .unwrap()
        .unwrap()
        .0
        .to_json()
        .compact_stable_order()
        .await
        .unwrap(), @r###"
        {
          "@context": "https://btp.works/chr/1.0/c.jsonld",
          "@graph": [
            {
              "@id": "chronicle:agent:testagent",
              "@type": [
                "prov:Agent",
                "chronicle:domaintype:test"
              ],
              "externalId": "testagent",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "test": "test"
              }
            },
            {
              "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "@type": "chronicle:Namespace",
              "externalId": "testns"
            }
          ]
        }
        "###);

        api.dispatch(
            ApiCommand::Agent(AgentCommand::UseInContext {
                id: AgentId::from_external_id("testagent"),
                namespace: "testns".into(),
            }),
            identity.clone(),
        )
        .await
        .unwrap();

        insta::assert_json_snapshot!(
        api.dispatch(ApiCommand::Activity(ActivityCommand::Create {
            external_id: "testactivity".into(),
            namespace: "testns".into(),
            attributes: Attributes {
                typ: Some(DomaintypeId::from_external_id("test")),
                attributes: [(
                    "test".to_owned(),
                    Attribute {
                        typ: "test".to_owned(),
                        value: serde_json::Value::String("test".to_owned()),
                    },
                )]
                .into_iter()
                .collect(),
            },
        }), identity.clone())
        .await
        .unwrap()
        .unwrap()
        .0
        .to_json()
        .compact_stable_order()
        .await
        .unwrap(), @r###"
        {
          "@context": "https://btp.works/chr/1.0/c.jsonld",
          "@graph": [
            {
              "@id": "chronicle:activity:testactivity",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:test"
              ],
              "externalId": "testactivity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "test": "test"
              }
            },
            {
              "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "@type": "chronicle:Namespace",
              "externalId": "testns"
            }
          ]
        }
        "###);

        insta::assert_json_snapshot!(
        api.dispatch(ApiCommand::Activity(ActivityCommand::Use {
            id: EntityId::from_external_id("testentity"),
            namespace: "testns".into(),
            activity: ActivityId::from_external_id("testactivity"),
        }), identity.clone())
        .await
        .unwrap()
        .unwrap()
        .0
        .to_json()
        .compact_stable_order()
        .await
        .unwrap(), @r###"
        {
          "@context": "https://btp.works/chr/1.0/c.jsonld",
          "@graph": [
            {
              "@id": "chronicle:activity:testactivity",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:test"
              ],
              "externalId": "testactivity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "used": [
                "chronicle:entity:testentity"
              ],
              "value": {
                "test": "test"
              }
            },
            {
              "@id": "chronicle:entity:testentity",
              "@type": "prov:Entity",
              "externalId": "testentity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {}
            },
            {
              "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "@type": "chronicle:Namespace",
              "externalId": "testns"
            }
          ]
        }
        "###);

        insta::assert_json_snapshot!(
        api.dispatch(ApiCommand::Activity(ActivityCommand::End {
            id: ActivityId::from_external_id("testactivity"),
            namespace: "testns".into(),
            time: Some(Utc.with_ymd_and_hms(2014, 7, 8, 9, 10, 11).unwrap()),
            agent: Some(AgentId::from_external_id("testagent")),
        }), identity)
        .await
        .unwrap()
        .unwrap()
        .0
        .to_json()
        .compact_stable_order()
        .await
        .unwrap(), @r###"
        {
          "@context": "https://btp.works/chr/1.0/c.jsonld",
          "@graph": [
            {
              "@id": "chronicle:activity:testactivity",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:test"
              ],
              "endTime": "2014-07-08T09:10:11+00:00",
              "externalId": "testactivity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "prov:qualifiedAssociation": {
                "@id": "chronicle:association:testagent:testactivity:role="
              },
              "used": [
                "chronicle:entity:testentity"
              ],
              "value": {
                "test": "test"
              },
              "wasAssociatedWith": [
                "chronicle:agent:testagent"
              ]
            },
            {
              "@id": "chronicle:association:testagent:testactivity:role=",
              "@type": "prov:Association",
              "agent": "chronicle:agent:testagent",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "prov:hadActivity": {
                "@id": "chronicle:activity:testactivity"
              }
            },
            {
              "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "@type": "chronicle:Namespace",
              "externalId": "testns"
            }
          ]
        }
        "###);
    }

    #[tokio::test]
    async fn activity_generate() {
        let mut api = test_api().await;

        let identity = AuthId::chronicle();

        insta::assert_json_snapshot!(
        api.dispatch(ApiCommand::Activity(ActivityCommand::Create {
            external_id: "testactivity".into(),
            namespace: "testns".into(),
            attributes: Attributes {
                typ: Some(DomaintypeId::from_external_id("test")),
                attributes: [(
                    "test".to_owned(),
                    Attribute {
                        typ: "test".to_owned(),
                        value: serde_json::Value::String("test".to_owned()),
                    },
                )]
                .into_iter()
                .collect(),
            },
        }), identity.clone())
        .await
        .unwrap()
        .unwrap()
        .0
        .to_json()
        .compact_stable_order()
        .await
        .unwrap(), @r###"
        {
          "@context": "https://btp.works/chr/1.0/c.jsonld",
          "@graph": [
            {
              "@id": "chronicle:activity:testactivity",
              "@type": [
                "prov:Activity",
                "chronicle:domaintype:test"
              ],
              "externalId": "testactivity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {
                "test": "test"
              }
            },
            {
              "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "@type": "chronicle:Namespace",
              "externalId": "testns"
            }
          ]
        }
        "###);

        insta::assert_json_snapshot!(
        api.dispatch(ApiCommand::Activity(ActivityCommand::Generate {
            id: EntityId::from_external_id("testentity"),
            namespace: "testns".into(),
            activity: ActivityId::from_external_id("testactivity"),
        }), identity)
        .await
        .unwrap()
        .unwrap()
        .0
        .to_json()
        .compact_stable_order()
        .await
        .unwrap(), @r###"
        {
          "@context": "https://btp.works/chr/1.0/c.jsonld",
          "@graph": [
            {
              "@id": "chronicle:entity:testentity",
              "@type": "prov:Entity",
              "externalId": "testentity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {},
              "wasGeneratedBy": [
                "chronicle:activity:testactivity"
              ]
            },
            {
              "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "@type": "chronicle:Namespace",
              "externalId": "testns"
            }
          ]
        }
        "###);
    }

    #[tokio::test]
    async fn derive_entity_abstract() {
        let mut api = test_api().await;

        let identity = AuthId::chronicle();

        insta::assert_json_snapshot!(
        api.dispatch(ApiCommand::Entity(EntityCommand::Derive {
            id: EntityId::from_external_id("testgeneratedentity"),
            namespace: "testns".into(),
            activity: None,
            used_entity: EntityId::from_external_id("testusedentity"),
            derivation: None,
        }), identity)
        .await
        .unwrap()
        .unwrap()
        .0
        .to_json()
        .compact_stable_order()
        .await
        .unwrap(), @r###"
        {
          "@context": "https://btp.works/chr/1.0/c.jsonld",
          "@graph": [
            {
              "@id": "chronicle:entity:testgeneratedentity",
              "@type": "prov:Entity",
              "externalId": "testgeneratedentity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {},
              "wasDerivedFrom": [
                "chronicle:entity:testusedentity"
              ]
            },
            {
              "@id": "chronicle:entity:testusedentity",
              "@type": "prov:Entity",
              "externalId": "testusedentity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {}
            },
            {
              "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "@type": "chronicle:Namespace",
              "externalId": "testns"
            }
          ]
        }
        "###);
    }

    #[tokio::test]
    async fn derive_entity_primary_source() {
        let mut api = test_api().await;

        let identity = AuthId::chronicle();

        insta::assert_json_snapshot!(
        api.dispatch(ApiCommand::Entity(EntityCommand::Derive {
            id: EntityId::from_external_id("testgeneratedentity"),
            namespace: "testns".into(),
            activity: None,
            derivation: Some(DerivationType::PrimarySource),
            used_entity: EntityId::from_external_id("testusedentity"),
        }), identity)
        .await
        .unwrap()
        .unwrap()
        .0
        .to_json()
        .compact_stable_order()
        .await
        .unwrap(), @r###"
        {
          "@context": "https://btp.works/chr/1.0/c.jsonld",
          "@graph": [
            {
              "@id": "chronicle:entity:testgeneratedentity",
              "@type": "prov:Entity",
              "externalId": "testgeneratedentity",
              "hadPrimarySource": [
                "chronicle:entity:testusedentity"
              ],
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {}
            },
            {
              "@id": "chronicle:entity:testusedentity",
              "@type": "prov:Entity",
              "externalId": "testusedentity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {}
            },
            {
              "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "@type": "chronicle:Namespace",
              "externalId": "testns"
            }
          ]
        }
        "###);
    }

    #[tokio::test]
    async fn derive_entity_revision() {
        let mut api = test_api().await;

        let identity = AuthId::chronicle();

        insta::assert_json_snapshot!(
        api.dispatch(ApiCommand::Entity(EntityCommand::Derive {
            id: EntityId::from_external_id("testgeneratedentity"),
            namespace: "testns".into(),
            activity: None,
            used_entity: EntityId::from_external_id("testusedentity"),
            derivation: Some(DerivationType::Revision),
        }), identity)
        .await
        .unwrap()
        .unwrap()
        .0
        .to_json()
        .compact_stable_order()
        .await
        .unwrap(), @r###"
        {
          "@context": "https://btp.works/chr/1.0/c.jsonld",
          "@graph": [
            {
              "@id": "chronicle:entity:testgeneratedentity",
              "@type": "prov:Entity",
              "externalId": "testgeneratedentity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {},
              "wasRevisionOf": [
                "chronicle:entity:testusedentity"
              ]
            },
            {
              "@id": "chronicle:entity:testusedentity",
              "@type": "prov:Entity",
              "externalId": "testusedentity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {}
            },
            {
              "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "@type": "chronicle:Namespace",
              "externalId": "testns"
            }
          ]
        }
        "###);
    }

    #[tokio::test]
    async fn derive_entity_quotation() {
        let mut api = test_api().await;

        let identity = AuthId::chronicle();

        insta::assert_json_snapshot!(
        api.dispatch(ApiCommand::Entity(EntityCommand::Derive {
            id: EntityId::from_external_id("testgeneratedentity"),
            namespace: "testns".into(),
            activity: None,
            used_entity: EntityId::from_external_id("testusedentity"),
            derivation: Some(DerivationType::Quotation),
        }), identity)
        .await
        .unwrap()
        .unwrap()
        .0
        .to_json()
        .compact_stable_order()
        .await
        .unwrap(), @r###"
        {
          "@context": "https://btp.works/chr/1.0/c.jsonld",
          "@graph": [
            {
              "@id": "chronicle:entity:testgeneratedentity",
              "@type": "prov:Entity",
              "externalId": "testgeneratedentity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {},
              "wasQuotedFrom": [
                "chronicle:entity:testusedentity"
              ]
            },
            {
              "@id": "chronicle:entity:testusedentity",
              "@type": "prov:Entity",
              "externalId": "testusedentity",
              "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "value": {}
            },
            {
              "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
              "@type": "chronicle:Namespace",
              "externalId": "testns"
            }
          ]
        }
        "###);
    }
}
