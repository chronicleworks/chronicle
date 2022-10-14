#![cfg_attr(feature = "strict", deny(warnings))]
pub mod chronicle_graphql;
mod persistence;

use chrono::{DateTime, Utc};
use custom_error::*;
use derivative::*;
use diesel::{r2d2::ConnectionManager, SqliteConnection};
use diesel_migrations::MigrationHarness;
use futures::{select, AsyncReadExt, FutureExt, StreamExt};

use common::{
    k256::ecdsa::{signature::Signer, Signature},
    prov::{
        operations::{WasAssociatedWith, WasInformedBy},
        Role,
    },
};
use persistence::{Store, StoreError, MIGRATIONS};
use r2d2::Pool;
use std::{
    collections::HashMap, convert::Infallible, marker::PhantomData, net::AddrParseError,
    path::Path, sync::Arc,
};
use tokio::{
    sync::mpsc::{self, error::SendError, Sender},
    task::JoinError,
};

use common::{
    attributes::Attributes,
    commands::*,
    ledger::{LedgerReader, LedgerWriter, Offset, SubmissionError, SubscriptionError},
    prov::{
        operations::{
            ActivityExists, ActivityUses, ActsOnBehalfOf, AgentExists, ChronicleOperation,
            CreateNamespace, DerivationType, EndActivity, EntityDerive, EntityExists,
            EntityHasEvidence, Generated, RegisterKey, SetAttributes, StartActivity,
            WasGeneratedBy,
        },
        ActivityId, AgentId, ChronicleTransactionId, EntityId, ExternalId, ExternalIdPart,
        IdentityId, NamespaceId, ProvModel,
    },
    signing::{DirectoryStoredKeys, SignerError},
};

use tracing::{debug, error, info_span, instrument, trace, warn, Instrument};

pub use persistence::ConnectionOptions;
use user_error::UFE;
use uuid::Uuid;

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
    EvidenceSigning{source: common::k256::ecdsa::Error}         = "Could not sign message",
}

/// Ugly but we need this until ! is stable https://github.com/rust-lang/rust/issues/64715
impl From<Infallible> for ApiError {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}

impl UFE for ApiError {}

type LedgerSendWithReply = (
    Vec<ChronicleOperation>,
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
                        let result = ledger_writer.submit(submission.as_slice()).await;

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
        tx: &[ChronicleOperation],
    ) -> Result<ChronicleTransactionId, ApiError> {
        let (reply_tx, mut reply_rx) = mpsc::channel(1);
        trace!(?tx, "Dispatch submission to ledger");
        self.tx.clone().blocking_send((tx.to_vec(), reply_tx))?;

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
    pub notify_commit: tokio::sync::broadcast::Sender<(ProvModel, ChronicleTransactionId)>,
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
    #[instrument(skip(ledger_writer, ledger_reader,))]
    pub async fn new<R, W>(
        pool: Pool<ConnectionManager<SqliteConnection>>,
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
        let (tx, mut rx) = mpsc::channel::<ApiSendWithReply>(10);

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

        for (ns, uuid) in namespace_bindings {
            store.namespace_binding(&ns, uuid)?
        }

        let reuse_reader = ledger_reader.clone();

        tokio::task::spawn(async move {
            let keystore = DirectoryStoredKeys::new(secret_path).unwrap();

            let mut api = Api::<U> {
                tx: tx.clone(),
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

                                if state.is_none() {
                                    warn!("Ledger reader disconnected");
                                    break;
                                }

                                if let Some((offset, prov, correlation_id)) = state {
                                        api.sync(&prov, offset.clone(),correlation_id.clone())
                                            .instrument(info_span!("Incoming confirmation", offset = ?offset, correlation_id = %correlation_id))
                                            .await
                                            .map_err(|e| {
                                                error!(?e, "Api sync to confirmed commit");
                                            }).map(|_| commit_notify_tx.send((*prov,correlation_id))).ok();
                                }

                            },
                            cmd = rx.recv().fuse() => {
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
        connection: &mut SqliteConnection,
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
        activity: Option<ActivityId>,
    ) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;

            connection.immediate_transaction(|connection| {
                let (namespace, mut to_apply) = api.ensure_namespace(connection, &namespace)?;
                let activity = api.store.get_activity_by_external_id_or_last_started(
                    connection,
                    activity.map(|x| x.external_id_part().clone()),
                    namespace.clone(),
                )?;

                let entity = api.store.entity_by_entity_external_id_and_namespace(
                    connection,
                    id.external_id_part(),
                    &namespace,
                );

                let external_id: ExternalId = {
                    if let Ok(existing) = entity {
                        debug!(?existing, "Use existing entity");
                        existing.external_id.into()
                    } else {
                        debug!(?id, "Need new entity");
                        api.store.disambiguate_entity_external_id(
                            connection,
                            id.external_id_part().clone(),
                            namespace.clone(),
                        )?
                    }
                };

                let id = EntityId::from_external_id(&external_id);

                let create = ChronicleOperation::WasGeneratedBy(WasGeneratedBy {
                    namespace,
                    id: id.clone(),
                    activity: ActivityId::from_external_id(&activity.external_id),
                });

                to_apply.push(create);

                let correlation_id = api.ledger_writer.submit_blocking(&to_apply)?;

                Ok(ApiResponse::submission(
                    id,
                    ProvModel::from_tx(&to_apply),
                    correlation_id,
                ))
            })
        })
        .await?
    }

    /// Creates and submits a (ChronicleTransaction::ActivityUses), and possibly (ChronicleTransaction::Domaintype) if specified
    ///
    /// We use our local store for a best guess at the activity, either by external_id or the last one started as a convenience for command line
    #[instrument]
    async fn activity_use(
        &self,
        id: EntityId,
        namespace: ExternalId,
        activity: Option<ActivityId>,
    ) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;

            connection.immediate_transaction(|connection| {
                let (namespace, mut to_apply) = api.ensure_namespace(connection, &namespace)?;
                let (id, to_apply) = {
                    let activity = api.store.get_activity_by_external_id_or_last_started(
                        connection,
                        activity.map(|x| x.external_id_part().clone()),
                        namespace.clone(),
                    )?;

                    let entity = api.store.entity_by_entity_external_id_and_namespace(
                        connection,
                        id.external_id_part(),
                        &namespace,
                    );

                    let external_id: ExternalId = {
                        if let Ok(existing) = entity {
                            debug!(?existing, "Use existing entity");
                            existing.external_id.into()
                        } else {
                            debug!(?id, "Need new entity");
                            api.store.disambiguate_entity_external_id(
                                connection,
                                id.external_id_part().clone(),
                                namespace.clone(),
                            )?
                        }
                    };

                    let id = EntityId::from_external_id(&external_id);

                    let create = ChronicleOperation::ActivityUses(ActivityUses {
                        namespace,
                        id: id.clone(),
                        activity: ActivityId::from_external_id(&activity.external_id),
                    });

                    to_apply.push(create);

                    (id, to_apply)
                };

                let correlation_id = api.ledger_writer.submit_blocking(&to_apply)?;

                Ok(ApiResponse::submission(
                    id,
                    ProvModel::from_tx(&to_apply),
                    correlation_id,
                ))
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
    ) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;

            connection.immediate_transaction(|connection| {
                let (namespace, mut to_apply) = api.ensure_namespace(connection, &namespace)?;
                let (id, to_apply) = {
                    let activity = api.store.get_activity_by_external_id_or_last_started(
                        connection,
                        Some(id.external_id_part().clone()),
                        namespace.clone(),
                    );

                    let informing_activity =
                        api.store.activity_by_activity_external_id_and_namespace(
                            connection,
                            informing_activity_id.external_id_part(),
                            &namespace,
                        );

                    let external_id: ExternalId = {
                        if let Ok(existing) = activity {
                            debug!(?existing, "Use existing activity");
                            existing.external_id.into()
                        } else {
                            debug!(?id, "Need new activity");
                            api.store.disambiguate_activity_external_id(
                                connection,
                                id.external_id_part(),
                                &namespace,
                            )?
                        }
                    };

                    let id = ActivityId::from_external_id(&external_id);

                    let external_id: ExternalId = {
                        if let Ok(existing) = informing_activity {
                            debug!(?existing, "Use existing activity as informing activity");
                            existing.external_id.into()
                        } else {
                            debug!(?id, "Need new activity as informing activity");
                            api.store.disambiguate_activity_external_id(
                                connection,
                                id.external_id_part(),
                                &namespace,
                            )?
                        }
                    };

                    let informing_activity = ActivityId::from_external_id(&external_id);

                    let create = ChronicleOperation::WasInformedBy(WasInformedBy {
                        namespace,
                        activity: id.clone(),
                        informing_activity,
                    });

                    to_apply.push(create);

                    (id, to_apply)
                };

                let correlation_id = api.ledger_writer.submit_blocking(&to_apply)?;

                Ok(ApiResponse::submission(
                    id,
                    ProvModel::from_tx(&to_apply),
                    correlation_id,
                ))
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
    ) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;

            connection.immediate_transaction(|connection| {
                let (namespace, mut to_apply) = api.ensure_namespace(connection, &namespace)?;

                let external_id: ExternalId = api.store.disambiguate_entity_external_id(
                    connection,
                    external_id,
                    namespace.clone(),
                )?;

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

                let correlation_id = api.ledger_writer.submit_blocking(&to_apply)?;

                Ok(ApiResponse::submission(
                    id,
                    ProvModel::from_tx(&to_apply),
                    correlation_id,
                ))
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
    ) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;

            connection.immediate_transaction(|connection| {
                let (namespace, mut to_apply) = api.ensure_namespace(connection, &namespace)?;

                let external_id: ExternalId = api.store.disambiguate_activity_external_id(
                    connection,
                    &external_id,
                    &namespace,
                )?;

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
                let correlation_id = api.ledger_writer.submit_blocking(&to_apply)?;

                Ok(ApiResponse::submission(
                    id,
                    ProvModel::from_tx(&to_apply),
                    correlation_id,
                ))
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
    ) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;

            connection.immediate_transaction(|connection| {
                let (namespace, mut to_apply) = api.ensure_namespace(connection, &namespace)?;

                let external_id: ExternalId = api.store.disambiguate_agent_external_id(
                    connection,
                    &external_id,
                    &namespace,
                )?;

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

                let correlation_id = api.ledger_writer.submit_blocking(&to_apply)?;

                Ok(ApiResponse::submission(
                    id,
                    ProvModel::from_tx(&to_apply),
                    correlation_id,
                ))
            })
        })
        .await?
    }

    /// Creates and submits a (ChronicleTransaction::CreateNamespace) if the external_id part does not already exist in local storage
    async fn create_namespace(&self, external_id: &ExternalId) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();
        let external_id = external_id.to_owned();
        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;
            connection.immediate_transaction(|connection| {
                let (namespace, to_apply) = api.ensure_namespace(connection, &external_id)?;

                let correlation_id = api.ledger_writer.submit_blocking(&to_apply)?;

                Ok(ApiResponse::submission(
                    namespace,
                    ProvModel::from_tx(&to_apply),
                    correlation_id,
                ))
            })
        })
        .await?
    }

    #[instrument]
    async fn dispatch(&mut self, command: ApiCommand) -> Result<ApiResponse, ApiError> {
        match command {
            ApiCommand::NameSpace(NamespaceCommand::Create { external_id }) => {
                self.create_namespace(&external_id).await
            }
            ApiCommand::Agent(AgentCommand::Create {
                external_id,
                namespace,
                attributes,
            }) => self.create_agent(external_id, namespace, attributes).await,
            ApiCommand::Agent(AgentCommand::RegisterKey {
                id,
                namespace,
                registration,
            }) => self.register_key(id, namespace, registration).await,
            ApiCommand::Agent(AgentCommand::UseInContext { id, namespace }) => {
                self.use_in_context(id, namespace).await
            }
            ApiCommand::Agent(AgentCommand::Delegate {
                id,
                delegate,
                activity,
                namespace,
                role,
            }) => self.delegate(namespace, id, delegate, activity, role).await,
            ApiCommand::Activity(ActivityCommand::Create {
                external_id,
                namespace,
                attributes,
            }) => {
                self.create_activity(external_id, namespace, attributes)
                    .await
            }
            ApiCommand::Activity(ActivityCommand::Start {
                id,
                namespace,
                time,
                agent,
            }) => self.start_activity(id, namespace, time, agent).await,
            ApiCommand::Activity(ActivityCommand::End {
                id,
                namespace,
                time,
                agent,
            }) => self.end_activity(id, namespace, time, agent).await,
            ApiCommand::Activity(ActivityCommand::Use {
                id,
                namespace,
                activity,
            }) => self.activity_use(id, namespace, activity).await,
            ApiCommand::Activity(ActivityCommand::WasInformedBy {
                id,
                namespace,
                informing_activity,
            }) => {
                self.activity_was_informed_by(id, namespace, informing_activity)
                    .await
            }
            ApiCommand::Activity(ActivityCommand::Associate {
                id,
                namespace,
                responsible,
                role,
            }) => self.associate(namespace, responsible, id, role).await,
            ApiCommand::Entity(EntityCommand::Create {
                external_id,
                namespace,
                attributes,
            }) => self.create_entity(external_id, namespace, attributes).await,
            ApiCommand::Activity(ActivityCommand::Generate {
                id,
                namespace,
                activity,
            }) => self.activity_generate(id, namespace, activity).await,
            ApiCommand::Entity(EntityCommand::Attach {
                id,
                namespace,
                file,
                locator,
                agent,
            }) => {
                self.entity_attach(id, namespace, file.clone(), locator, agent)
                    .await
            }
            ApiCommand::Entity(EntityCommand::Derive {
                id,
                namespace,
                activity,
                used_entity,
                derivation,
            }) => {
                self.entity_derive(id, namespace, activity, used_entity, derivation)
                    .await
            }
            ApiCommand::Entity(EntityCommand::Generated {
                id,
                namespace,
                entity,
            }) => self.entity_generated(id, namespace, entity).await,
            ApiCommand::Query(query) => self.query(query).await,
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
    ) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();

        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;

            connection.immediate_transaction(|connection| {
                let (namespace, mut to_apply) = api.ensure_namespace(connection, &namespace)?;

                let tx = ChronicleOperation::AgentActsOnBehalfOf(ActsOnBehalfOf::new(
                    &namespace,
                    &responsible_id,
                    &delegate_id,
                    activity_id.as_ref(),
                    role,
                ));

                to_apply.push(tx);

                let correlation_id = api.ledger_writer.submit_blocking(&to_apply)?;
                Ok(ApiResponse::submission(
                    responsible_id,
                    ProvModel::from_tx(&to_apply),
                    correlation_id,
                ))
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
    ) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();

        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;

            connection.immediate_transaction(|connection| {
                let (namespace, mut to_apply) = api.ensure_namespace(connection, &namespace)?;

                let tx = ChronicleOperation::WasAssociatedWith(WasAssociatedWith::new(
                    &namespace,
                    &activity_id,
                    &responsible_id,
                    role,
                ));

                to_apply.push(tx);

                let correlation_id = api.ledger_writer.submit_blocking(&to_apply)?;
                Ok(ApiResponse::submission(
                    responsible_id,
                    ProvModel::from_tx(&to_apply),
                    correlation_id,
                ))
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
    ) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();

        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;

            connection.immediate_transaction(|connection| {
                let (namespace, mut to_apply) = api.ensure_namespace(connection, &namespace)?;

                let tx = ChronicleOperation::EntityDerive(EntityDerive {
                    namespace,
                    id: id.clone(),
                    used_id: used_id.clone(),
                    activity_id: activity_id.clone(),
                    typ,
                });

                to_apply.push(tx);

                let correlation_id = api.ledger_writer.submit_blocking(&to_apply)?;
                Ok(ApiResponse::submission(
                    id,
                    ProvModel::from_tx(&to_apply),
                    correlation_id,
                ))
            })
        })
        .await?
    }

    #[instrument]
    async fn entity_generated(
        &self,
        id: ActivityId,
        namespace: ExternalId,
        entity: Option<EntityId>,
    ) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;

            connection.immediate_transaction(|connection| {
                let (namespace, mut to_apply) = api.ensure_namespace(connection, &namespace)?;
                let entity = api.store.get_entity_by_external_id(
                    connection,
                    entity.map(|x| x.external_id_part().clone()),
                    namespace.clone(),
                )?;

                let activity = api.store.activity_by_activity_external_id_and_namespace(
                    connection,
                    id.external_id_part(),
                    &namespace,
                );

                let external_id: ExternalId = {
                    if let Ok(existing) = activity {
                        debug!(?existing, "Use existing activity");
                        existing.external_id.into()
                    } else {
                        debug!(?id, "Need new activity");
                        api.store.disambiguate_entity_external_id(
                            connection,
                            id.external_id_part().to_owned(),
                            namespace.clone(),
                        )?
                    }
                };

                let id = ActivityId::from_external_id(&external_id);

                let create = ChronicleOperation::Generated(Generated {
                    namespace,
                    id: id.clone(),
                    entity: EntityId::from_external_id(&entity.external_id),
                });

                to_apply.push(create);

                let correlation_id = api.ledger_writer.submit_blocking(&to_apply)?;

                Ok(ApiResponse::submission(
                    id,
                    ProvModel::from_tx(&to_apply),
                    correlation_id,
                ))
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
                        api.store.agent_by_agent_external_id_and_namespace(
                            &mut connection,
                            agent.external_id_part(),
                            &namespace,
                        )
                    })
                    .unwrap_or_else(|| api.store.get_current_agent(&mut connection))?;

                let agent_id = AgentId::from_external_id(&agent.external_id);

                let signer = api.keystore.agent_signing(&agent_id)?;

                let signature: Signature = signer.try_sign(&*buf)?;

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

                let correlation_id = api.ledger_writer.submit_blocking(&to_apply)?;

                Ok(ApiResponse::submission(
                    id,
                    ProvModel::from_tx(&to_apply),
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

            let (_id, _) = api
                .store
                .namespace_by_external_id(&mut connection, &ExternalId::from(&query.namespace))?;
            Ok(ApiResponse::query_reply(
                api.store.prov_model_for_namespace(&mut connection, query)?,
            ))
        })
        .await?
    }

    #[instrument]
    async fn sync(
        &self,
        prov: &ProvModel,
        offset: Offset,
        correlation_id: ChronicleTransactionId,
    ) -> Result<ApiResponse, ApiError> {
        let api = self.clone();
        let prov = prov.clone();

        tokio::task::spawn_blocking(move || {
            //TODO: This should be a single tx
            api.store.apply_prov(&prov)?;
            api.store.set_last_offset(offset, correlation_id)?;

            Ok(ApiResponse::Unit)
        })
        .await?
    }

    /// Creates and submits a (ChronicleTransaction::RegisterKey), implicitly verifying the input keys and saving to the local key store as required
    #[instrument]
    async fn register_key(
        &self,
        id: AgentId,
        namespace: ExternalId,
        registration: KeyRegistration,
    ) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;
            connection.immediate_transaction(|connection| {
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
                    publickey: hex::encode(api.keystore.agent_verifying(&id.clone())?.to_bytes()),
                }));

                let correlation_id = api.ledger_writer.submit_blocking(&to_apply)?;

                Ok(ApiResponse::submission(
                    id,
                    ProvModel::from_tx(&to_apply),
                    correlation_id,
                ))
            })
        })
        .await?
    }

    /// Creates and submits a (ChronicleTransaction::StartActivity), determining the appropriate agent by external_id, or via [use_agent] context
    #[instrument]
    async fn start_activity(
        &self,
        id: ActivityId,
        namespace: ExternalId,
        time: Option<DateTime<Utc>>,
        agent: Option<AgentId>,
    ) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;
            connection.immediate_transaction(|connection| {
                let (namespace, mut to_apply) = api.ensure_namespace(connection, &namespace)?;
                let agent = {
                    if let Some(agent) = agent {
                        api.store
                            .agent_by_agent_external_id_and_namespace(
                                connection,
                                agent.external_id_part(),
                                &namespace,
                            )
                            .ok()
                    } else {
                        api.store.get_current_agent(connection).ok()
                    }
                };

                let activity = api.store.activity_by_activity_external_id_and_namespace(
                    connection,
                    id.external_id_part(),
                    &namespace,
                );

                let external_id: ExternalId = {
                    if let Ok(existing) = activity {
                        debug!(?existing, "Use existing activity");
                        existing.external_id.into()
                    } else {
                        debug!(?id, "Need new activity");
                        api.store.disambiguate_activity_external_id(
                            connection,
                            id.external_id_part(),
                            &namespace,
                        )?
                    }
                };

                let id = ActivityId::from_external_id(&external_id);
                to_apply.push(ChronicleOperation::StartActivity(StartActivity {
                    namespace: namespace.clone(),
                    id: id.clone(),
                    time: time.unwrap_or_else(Utc::now),
                }));

                if let Some(agent) = agent {
                    to_apply.push(ChronicleOperation::WasAssociatedWith(
                        WasAssociatedWith::new(
                            &namespace,
                            &id,
                            &AgentId::from_external_id(&agent.external_id),
                            None,
                        ),
                    ));
                }

                let correlation_id = api.ledger_writer.submit_blocking(&to_apply)?;

                Ok(ApiResponse::submission(
                    id,
                    ProvModel::from_tx(&to_apply),
                    correlation_id,
                ))
            })
        })
        .await?
    }

    /// Creates and submits a (ChronicleTransaction::EndActivity), determining the appropriate agent by external_id, or via [use_agent] context
    #[instrument]
    async fn end_activity(
        &self,
        id: Option<ActivityId>,
        namespace: ExternalId,
        time: Option<DateTime<Utc>>,
        agent: Option<AgentId>,
    ) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;
            connection.immediate_transaction(|connection| {
                let (namespace, mut to_apply) = api.ensure_namespace(connection, &namespace)?;
                let activity = api
                    .store
                    .get_activity_by_external_id_or_last_started(
                        connection,
                        id.map(|id| id.external_id_part().clone()),
                        namespace.clone(),
                    )
                    .map_err(|_| ApiError::NotCurrentActivity {})?;

                let agent = {
                    if let Some(agent) = agent {
                        api.store
                            .agent_by_agent_external_id_and_namespace(
                                connection,
                                agent.external_id_part(),
                                &namespace,
                            )
                            .ok()
                    } else {
                        api.store
                            .get_current_agent(connection)
                            .map_err(|_| ApiError::NoCurrentAgent {})
                            .ok()
                    }
                };

                let id = ActivityId::from_external_id(&activity.external_id);
                to_apply.push(ChronicleOperation::EndActivity(EndActivity {
                    namespace: namespace.clone(),
                    id: id.clone(),
                    time: time.unwrap_or_else(Utc::now),
                }));

                if let Some(agent) = agent {
                    to_apply.push(ChronicleOperation::WasAssociatedWith(
                        WasAssociatedWith::new(
                            &namespace,
                            &id,
                            &AgentId::from_external_id(&agent.external_id),
                            None,
                        ),
                    ));
                }

                let correlation_id = api.ledger_writer.submit_blocking(&to_apply)?;

                Ok(ApiResponse::submission(
                    id,
                    ProvModel::from_tx(&to_apply),
                    correlation_id,
                ))
            })
        })
        .await?
    }

    #[instrument]
    async fn use_in_context(
        &self,
        id: AgentId,
        namespace: ExternalId,
    ) -> Result<ApiResponse, ApiError> {
        let api = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;

            connection.immediate_transaction(|connection| {
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

    use std::collections::HashMap;

    use chrono::{TimeZone, Utc};
    use common::{
        attributes::{Attribute, Attributes},
        commands::{ApiResponse, EntityCommand, KeyImport},
        ledger::InMemLedger,
        prov::{
            operations::DerivationType, to_json_ld::ToJson, ActivityId, AgentId,
            ChronicleTransactionId, DomaintypeId, EntityId, ProvModel,
        },
    };

    use crate::{persistence::ConnectionOptions, Api, ApiDispatch, ApiError, UuidGen};
    use diesel::{r2d2::ConnectionManager, SqliteConnection};
    use r2d2::Pool;
    use tempfile::TempDir;
    use uuid::Uuid;

    use common::commands::{
        ActivityCommand, AgentCommand, ApiCommand, KeyRegistration, NamespaceCommand,
    };

    #[derive(Clone)]
    struct TestDispatch(ApiDispatch, ProvModel);

    impl TestDispatch {
        pub async fn dispatch(
            &mut self,
            command: ApiCommand,
        ) -> Result<Option<(ProvModel, ChronicleTransactionId)>, ApiError> {
            // We can sort of get final on chain state here by using a map of subject to model
            if let ApiResponse::Submission { prov, .. } = self.0.dispatch(command).await? {
                self.1.merge(*prov);

                Ok(Some(self.0.notify_commit.subscribe().recv().await.unwrap()))
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
        telemetry::telemetry(None, telemetry::ConsoleLogging::Pretty);

        let secretpath = TempDir::new().unwrap();
        // We need to use a real file for sqlite, as in mem either re-creates between
        // macos temp dir permissions don't work with sqlite
        std::fs::create_dir("./sqlite_test").ok();
        let dbid = Uuid::new_v4();
        let mut ledger = InMemLedger::new();
        let reader = ledger.reader();

        let pool = Pool::builder()
            .connection_customizer(Box::new(ConnectionOptions {
                enable_wal: true,
                enable_foreign_keys: true,
                busy_timeout: Some(std::time::Duration::from_secs(2)),
            }))
            .build(ConnectionManager::<SqliteConnection>::new(&*format!(
                "./sqlite_test/db{}.sqlite",
                dbid
            )))
            .unwrap();

        let dispatch = Api::new(
            pool,
            ledger,
            reader,
            &secretpath.into_path(),
            SameUuid,
            HashMap::default(),
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
            external_id: "testns".into(),
        }))
        .await
        .unwrap();

        assert_json_ld!(api);
    }

    #[tokio::test]
    async fn create_agent() {
        let mut api = test_api().await;

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
            external_id: "testns".into(),
        }))
        .await
        .unwrap();

        api.dispatch(ApiCommand::Agent(AgentCommand::RegisterKey {
            id: AgentId::from_external_id("testagent"),
            namespace: "testns".into(),
            registration: KeyRegistration::ImportSigning(KeyImport::FromPEMBuffer {
                buffer: pk.as_bytes().into(),
            }),
        }))
        .await
        .unwrap();

        insta::assert_yaml_snapshot!(api.1, {
            ".*.publickey" => "[public]"
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
              public_key: 02197db854d8c6a488d4a0ef3ef1fcb0c06d66478fae9e87a237172cf6f6f7de23
          : id:
              external_id: testagent
              public_key: 02197db854d8c6a488d4a0ef3ef1fcb0c06d66478fae9e87a237172cf6f6f7de23
            namespaceid:
              external_id: testns
              uuid: 5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea
            public_key: 02197db854d8c6a488d4a0ef3ef1fcb0c06d66478fae9e87a237172cf6f6f7de23
        attachments: {}
        has_identity:
          ? - external_id: testns
              uuid: 5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea
            - testagent
          : - external_id: testns
              uuid: 5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea
            - external_id: testagent
              public_key: 02197db854d8c6a488d4a0ef3ef1fcb0c06d66478fae9e87a237172cf6f6f7de23
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
        }))
        .await
        .unwrap();

        assert_json_ld!(api);
    }

    #[tokio::test]
    async fn start_activity() {
        let mut api = test_api().await;

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
        }))
        .await
        .unwrap();

        api.dispatch(ApiCommand::Agent(AgentCommand::UseInContext {
            id: AgentId::from_external_id("testagent"),
            namespace: "testns".into(),
        }))
        .await
        .unwrap();

        api.dispatch(ApiCommand::Activity(ActivityCommand::Start {
            id: ActivityId::from_external_id("testactivity"),
            namespace: "testns".into(),
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
        }))
        .await
        .unwrap();

        api.dispatch(ApiCommand::Agent(AgentCommand::UseInContext {
            id: AgentId::from_external_id("testagent"),
            namespace: "testns".into(),
        }))
        .await
        .unwrap();

        api.dispatch(ApiCommand::Activity(ActivityCommand::Start {
            id: ActivityId::from_external_id("testactivity"),
            namespace: "testns".into(),
            time: Some(Utc.ymd(2014, 7, 8).and_hms(9, 10, 11)),
            agent: None,
        }))
        .await
        .unwrap();

        // Should end the last opened activity
        api.dispatch(ApiCommand::Activity(ActivityCommand::End {
            id: None,
            namespace: "testns".into(),
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
        }))
        .await
        .unwrap();

        api.dispatch(ApiCommand::Agent(AgentCommand::UseInContext {
            id: AgentId::from_external_id("testagent"),
            namespace: "testns".into(),
        }))
        .await
        .unwrap();

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
        }))
        .await
        .unwrap();

        api.dispatch(ApiCommand::Activity(ActivityCommand::Use {
            id: EntityId::from_external_id("testentity"),
            namespace: "testns".into(),
            activity: Some(ActivityId::from_external_id("testactivity")),
        }))
        .await
        .unwrap();

        api.dispatch(ApiCommand::Activity(ActivityCommand::End {
            id: None,
            namespace: "testns".into(),
            time: Some(Utc.ymd(2014, 7, 8).and_hms(9, 10, 11)),
            agent: Some(AgentId::from_external_id("testagent")),
        }))
        .await
        .unwrap();

        assert_json_ld!(api);
    }

    #[tokio::test]
    async fn activity_generate() {
        let mut api = test_api().await;

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
        }))
        .await
        .unwrap();

        api.dispatch(ApiCommand::Activity(ActivityCommand::Generate {
            id: EntityId::from_external_id("testentity"),
            namespace: "testns".into(),
            activity: Some(ActivityId::from_external_id("testactivity")),
        }))
        .await
        .unwrap();

        assert_json_ld!(api);
    }

    #[tokio::test]
    async fn derive_entity_abstract() {
        let mut api = test_api().await;

        api.dispatch(ApiCommand::Entity(EntityCommand::Derive {
            id: EntityId::from_external_id("testgeneratedentity"),
            namespace: "testns".into(),
            activity: None,
            used_entity: EntityId::from_external_id("testusedentity"),
            derivation: None,
        }))
        .await
        .unwrap();

        assert_json_ld!(api);
    }

    #[tokio::test]
    async fn derive_entity_primary_source() {
        let mut api = test_api().await;

        api.dispatch(ApiCommand::Entity(EntityCommand::Derive {
            id: EntityId::from_external_id("testgeneratedentity"),
            namespace: "testns".into(),
            activity: None,
            derivation: Some(DerivationType::PrimarySource),
            used_entity: EntityId::from_external_id("testusedentity"),
        }))
        .await
        .unwrap();

        assert_json_ld!(api);
    }

    #[tokio::test]
    async fn derive_entity_revision() {
        let mut api = test_api().await;

        api.dispatch(ApiCommand::Entity(EntityCommand::Derive {
            id: EntityId::from_external_id("testgeneratedentity"),
            namespace: "testns".into(),
            activity: None,
            used_entity: EntityId::from_external_id("testusedentity"),
            derivation: Some(DerivationType::Revision),
        }))
        .await
        .unwrap();

        assert_json_ld!(api);
    }

    #[tokio::test]
    async fn derive_entity_quotation() {
        let mut api = test_api().await;

        api.dispatch(ApiCommand::Entity(EntityCommand::Derive {
            id: EntityId::from_external_id("testgeneratedentity"),
            namespace: "testns".into(),
            activity: None,
            used_entity: EntityId::from_external_id("testusedentity"),
            derivation: Some(DerivationType::Quotation),
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
                external_id: format!("testactivity{}", i).into(),
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
            }))
            .await
            .unwrap();
        }

        assert_json_ld!(api);
    }
}
