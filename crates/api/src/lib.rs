#![cfg_attr(feature = "strict", deny(warnings))]
pub mod chronicle_graphql;
pub mod inmem;
mod persistence;

use async_stl_client::{
    error::SawtoothCommunicationError,
    ledger::{BlockId, BlockingLedgerWriter, FromBlock},
};
use chronicle_protocol::{
    async_stl_client::ledger::{LedgerReader, LedgerWriter},
    messages::ChronicleSubmitTransaction,
    protocol::ChronicleOperationEvent,
};
use chrono::{DateTime, Utc};

use diesel::{r2d2::ConnectionManager, PgConnection};
use diesel_migrations::MigrationHarness;
use futures::{select, FutureExt, StreamExt};

use common::{
    attributes::Attributes,
    commands::*,
    identity::{AuthId, IdentityError},
    k256::ecdsa::SigningKey,
    ledger::{Commit, SubmissionError, SubmissionStage, SubscriptionError},
    prov::{
        operations::{
            ActivityExists, ActivityUses, ActsOnBehalfOf, AgentExists, ChronicleOperation,
            CreateNamespace, DerivationType, EndActivity, EntityDerive, EntityExists,
            SetAttributes, StartActivity, WasAssociatedWith, WasAttributedTo, WasGeneratedBy,
            WasInformedBy,
        },
        to_json_ld::ToJson,
        ActivityId, AgentId, ChronicleIri, ChronicleTransaction, ChronicleTransactionId,
        Contradiction, EntityId, ExternalId, ExternalIdPart, NamespaceId, ProcessorError,
        ProvModel, Role, SYSTEM_ID, SYSTEM_UUID,
    },
    signing::{DirectoryStoredKeys, SignerError},
};

use metrics::histogram;
use metrics_exporter_prometheus::PrometheusBuilder;
pub use persistence::StoreError;
use persistence::{Store, MIGRATIONS};
use r2d2::Pool;
use std::{
    collections::HashMap,
    convert::Infallible,
    marker::PhantomData,
    net::AddrParseError,
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};
use thiserror::Error;
use tokio::{
    sync::mpsc::{self, error::SendError, Sender},
    task::JoinError,
};

use tracing::{debug, error, info, info_span, instrument, trace, warn, Instrument};

pub use persistence::ConnectionOptions;
use user_error::UFE;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("Storage: {0:?}")]
    Store(#[from] persistence::StoreError),

    #[error("Transaction failed: {0}")]
    Transaction(#[from] diesel::result::Error),

    #[error("Invalid IRI: {0}")]
    Iri(#[from] iref::Error),

    #[error("JSON-LD processing: {0}")]
    JsonLD(String),

    #[error("Ledger error: {0}")]
    Ledger(#[from] SubmissionError),

    #[error("Signing: {0}")]
    Signing(#[from] SignerError),

    #[error("No agent is currently in use, please call agent use or supply an agent in your call")]
    NoCurrentAgent,

    #[error("Api shut down before reply")]
    ApiShutdownRx,

    #[error("Api shut down before send: {0}")]
    ApiShutdownTx(#[from] SendError<ApiSendWithReply>),

    #[error("Ledger shut down before send: {0}")]
    LedgerShutdownTx(#[from] SendError<LedgerSendWithReply>),

    #[error("Invalid socket address: {0}")]
    AddressParse(#[from] AddrParseError),

    #[error("Connection pool: {0}")]
    ConnectionPool(#[from] r2d2::Error),

    #[error("IO error: {0}")]
    InputOutput(#[from] std::io::Error),

    #[error("Blocking thread pool: {0}")]
    Join(#[from] JoinError),

    #[error("State update subscription: {0}")]
    Subscription(#[from] SubscriptionError),

    #[error("No appropriate activity to end")]
    NotCurrentActivity,

    #[error("Contradiction: {0}")]
    Contradiction(#[from] Contradiction),

    #[error("Processor: {0}")]
    ProcessorError(#[from] ProcessorError),

    #[error("Identity: {0}")]
    IdentityError(#[from] IdentityError),

    #[error("Sawtooth communication error: {0}")]
    SawtoothCommunicationError(#[from] SawtoothCommunicationError),

    #[error("ETL error: {0}")]
    ETLError(#[from] common::etl::ETLError),
}

/// Ugly but we need this until ! is stable, see <https://github.com/rust-lang/rust/issues/64715>
impl From<Infallible> for ApiError {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}

impl UFE for ApiError {}

type LedgerSendWithReply = (
    ChronicleSubmitTransaction,
    Sender<Result<ChronicleTransactionId, SubmissionError>>,
);

type ApiSendWithReply = ((ApiCommand, AuthId), Sender<Result<ApiResponse, ApiError>>);

pub trait UuidGen {
    fn uuid() -> Uuid {
        Uuid::new_v4()
    }
}

#[derive(Clone)]
pub struct Api<
    U: UuidGen + Send + Sync + Clone,
    W: LedgerWriter<Transaction = ChronicleSubmitTransaction, Error = SawtoothCommunicationError>
        + Clone
        + Send
        + Sync
        + 'static,
> {
    _reply_tx: Sender<ApiSendWithReply>,
    submit_tx: tokio::sync::broadcast::Sender<SubmissionStage>,
    keystore: DirectoryStoredKeys,
    ledger_writer: Arc<BlockingLedgerWriter<W>>,
    store: persistence::Store,
    signer: SigningKey,
    uuid_source: PhantomData<U>,
    policy_name: Option<String>,
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

    #[instrument]
    pub async fn handle_import_command(
        &self,
        identity: AuthId,
        namespace: NamespaceId,
        operations: Vec<ChronicleOperation>,
    ) -> Result<ApiResponse, ApiError> {
        self.import_operations(identity, namespace, operations)
            .await
    }

    #[instrument]
    async fn import_operations(
        &self,
        identity: AuthId,
        namespace: NamespaceId,
        operations: Vec<ChronicleOperation>,
    ) -> Result<ApiResponse, ApiError> {
        self.dispatch(
            ApiCommand::Import(ImportCommand {
                namespace,
                operations,
            }),
            identity.clone(),
        )
        .await
    }

    #[instrument]
    pub async fn handle_depth_charge(
        &self,
        namespace: &str,
        uuid: &Uuid,
    ) -> Result<ApiResponse, ApiError> {
        self.dispatch_depth_charge(
            AuthId::Chronicle,
            NamespaceId::from_external_id(namespace, *uuid),
        )
        .await
    }

    #[instrument]
    async fn dispatch_depth_charge(
        &self,
        identity: AuthId,
        namespace: NamespaceId,
    ) -> Result<ApiResponse, ApiError> {
        self.dispatch(
            ApiCommand::DepthCharge(DepthChargeCommand { namespace }),
            identity.clone(),
        )
        .await
    }
}

fn install_prometheus_metrics_exporter() {
    let metrics_endpoint = "127.0.0.1:9000";
    let metrics_listen_socket = match metrics_endpoint.parse::<std::net::SocketAddrV4>() {
        Ok(addr) => addr,
        Err(e) => {
            error!("Unable to parse metrics listen socket address: {e:?}");
            return;
        }
    };

    if let Err(e) = PrometheusBuilder::new()
        .with_http_listener(metrics_listen_socket)
        .install()
    {
        error!("Prometheus exporter installation for liveness check metrics failed: {e:?}");
    } else {
        debug!("Liveness check metrics Prometheus exporter installed with endpoint on {metrics_endpoint}/metrics");
    }
}

impl<U, LEDGER> Api<U, LEDGER>
where
    U: UuidGen + Send + Sync + Clone + std::fmt::Debug + 'static,
    LEDGER: LedgerWriter<Transaction = ChronicleSubmitTransaction, Error = SawtoothCommunicationError>
        + Clone
        + Send
        + Sync
        + 'static
        + LedgerReader<Event = ChronicleOperationEvent, Error = SawtoothCommunicationError>,
{
    #[instrument(skip(ledger))]
    pub async fn new(
        pool: Pool<ConnectionManager<PgConnection>>,
        ledger: LEDGER,
        secret_path: &Path,
        uuidgen: U,
        namespace_bindings: HashMap<String, Uuid>,
        policy_name: Option<String>,
        liveness_check_interval: Option<u64>,
    ) -> Result<ApiDispatch, ApiError> {
        let (commit_tx, mut commit_rx) = mpsc::channel::<ApiSendWithReply>(10);

        let (commit_notify_tx, _) = tokio::sync::broadcast::channel(20);
        let dispatch = ApiDispatch {
            tx: commit_tx.clone(),
            notify_commit: commit_notify_tx.clone(),
        };

        let secret_path = secret_path.to_owned();

        let store = Store::new(pool.clone())?;

        let keystore = DirectoryStoredKeys::new(secret_path)?;
        let retrieve_signer = common::signing::directory_signing_key;
        let signing = keystore.chronicle_signing(retrieve_signer)?;

        pool.get()?
            .build_transaction()
            .run(|connection| connection.run_pending_migrations(MIGRATIONS).map(|_| ()))
            .map_err(|migration| StoreError::DbMigration(migration))?;

        let system_namespace_uuid = (SYSTEM_ID, Uuid::try_from(SYSTEM_UUID).unwrap());

        // Append namespace bindings and system namespace
        store.namespace_binding(system_namespace_uuid.0, system_namespace_uuid.1)?;
        for (ns, uuid) in namespace_bindings {
            store.namespace_binding(&ns, uuid)?
        }

        let reuse_reader = ledger.clone();

        let last_seen_block = store.get_last_block_id();

        let start_from_block = if let Ok(Some(start_from_block)) = last_seen_block {
            FromBlock::BlockId(start_from_block)
        } else {
            FromBlock::First //Full catch up, as we have no last seen block
        };

        debug!(start_from_block = ?start_from_block, "Starting from block");

        tokio::task::spawn(async move {
            let mut api = Api::<U, LEDGER> {
                _reply_tx: commit_tx.clone(),
                submit_tx: commit_notify_tx.clone(),
                keystore,
                signer: signing.clone(),
                ledger_writer: Arc::new(BlockingLedgerWriter::new(ledger)),
                store: store.clone(),
                uuid_source: PhantomData,
                policy_name,
            };

            loop {
                let state_updates = reuse_reader.clone();

                let state_updates = state_updates
                    .state_updates("chronicle/prov-update", start_from_block, None)
                    .await;

                if let Err(e) = state_updates {
                    error!(subscribe_to_events = ?e);
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    continue;
                }

                let mut state_updates = state_updates.unwrap();

                loop {
                    select! {
                            state = state_updates.next().fuse() =>{

                                match state {
                                  None => {
                                    debug!("Ledger reader stream ended");
                                    break;
                                  }
                                  // Ledger contradicted or error, so nothing to
                                  // apply, but forward notification
                                  Some((ChronicleOperationEvent(Err(e), id),tx,_block_id,_position, _span)) => {
                                    commit_notify_tx.send(SubmissionStage::not_committed(
                                      ChronicleTransactionId::from(tx.as_str()),e.clone(), id
                                    )).ok();
                                  },
                                  // Successfully committed to ledger, so apply
                                  // to db and broadcast notification to
                                  // subscription subscribers
                                  Some((ChronicleOperationEvent(Ok(ref commit), id,),tx,block_id,_position,_span )) => {

                                        debug!(committed = ?tx);
                                        debug!(delta = %serde_json::to_string_pretty(&commit.to_json().compact().await.unwrap()).unwrap());

                                        api.sync( commit.clone().into(), &block_id,ChronicleTransactionId::from(tx.as_str()))
                                            .instrument(info_span!("Incoming confirmation", offset = ?block_id, tx_id = %tx))
                                            .await
                                            .map_err(|e| {
                                                error!(?e, "Api sync to confirmed commit");
                                            }).map(|_| commit_notify_tx.send(SubmissionStage::committed(Commit::new(
                                               ChronicleTransactionId::from(tx.as_str()),block_id, Box::new(commit.clone())
                                            ), id )).ok())
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

        if let Some(interval) = liveness_check_interval {
            debug!("Starting liveness depth charge task");

            let depth_charge_api = dispatch.clone();

            tokio::task::spawn(async move {
                // Configure and install Prometheus exporter
                install_prometheus_metrics_exporter();

                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
                    let api = depth_charge_api.clone();

                    let start_time = Instant::now();

                    let response = api
                        .handle_depth_charge(system_namespace_uuid.0, &system_namespace_uuid.1)
                        .await;

                    match response {
                        Ok(ApiResponse::DepthChargeSubmitted { tx_id }) => {
                            let mut tx_notifications = api.notify_commit.subscribe();

                            loop {
                                let stage = match tx_notifications.recv().await {
                                    Ok(stage) => stage,
                                    Err(e) => {
                                        error!("Error receiving depth charge transaction notifications: {}", e);
                                        continue;
                                    }
                                };

                                match stage {
                                    SubmissionStage::Submitted(Ok(id)) => {
                                        if id == tx_id {
                                            debug!("Depth charge operation submitted: {id}");
                                            continue;
                                        }
                                    }
                                    SubmissionStage::Submitted(Err(err)) => {
                                        if err.tx_id() == &tx_id {
                                            error!("Depth charge transaction rejected by Chronicle: {} {}",
                                                err,
                                                err.tx_id()
                                            );
                                            break;
                                        }
                                    }
                                    SubmissionStage::Committed(commit, _) => {
                                        if commit.tx_id == tx_id {
                                            let end_time = Instant::now();
                                            let elapsed_time = end_time - start_time;

                                            debug!(
                                                "Depth charge transaction committed: {}",
                                                commit.tx_id
                                            );
                                            debug!(
                                                "Depth charge round trip time: {:.2?}",
                                                elapsed_time
                                            );
                                            histogram!(
                                                "depth_charge_round_trip",
                                                elapsed_time.as_millis() as f64
                                            );
                                            break;
                                        }
                                    }
                                    SubmissionStage::NotCommitted((id, contradiction, _)) => {
                                        if id == tx_id {
                                            error!("Depth charge transaction rejected by ledger: {id} {contradiction}");
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        Ok(res) => error!("Unexpected ApiResponse from depth charge: {res:?}"),
                        Err(e) => error!("ApiError submitting depth charge: {e}"),
                    }
                }
            });
        }

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
        if cfg!(feature = "export") {
            let path = Path::new("export.json");
            let operations =
                common::etl::read_and_update_operations_from_file(&tx.tx, &path.into())?;
            common::etl::write_operations_to_file(&operations, &path.into())?;
        }

        let res = self.ledger_writer.do_submit(
            &ChronicleSubmitTransaction {
                tx: tx.clone(),
                signer: self.signer.clone(),
                policy_name: self.policy_name.clone(),
            },
            &self.signer,
        );

        match res {
            Ok(tx_id) => {
                let tx_id = ChronicleTransactionId::from(tx_id.as_str());
                self.submit_tx.send(SubmissionStage::submitted(&tx_id)).ok();
                Ok(tx_id)
            }
            Err((Some(tx_id), e)) => {
                // We need the cloneable SubmissionError wrapper here
                let submission_error = SubmissionError::communication(
                    &ChronicleTransactionId::from(tx_id.as_str()),
                    e,
                );
                self.submit_tx
                    .send(SubmissionStage::submitted_error(&submission_error))
                    .ok();
                Err(submission_error.into())
            }
            Err((None, e)) => Err(e.into()),
        }
    }

    /// Generate and submit the signed identity to send to the Transaction Processor along with the transactions to be applied
    fn submit(
        &mut self,
        id: impl Into<ChronicleIri>,
        identity: AuthId,
        to_apply: Vec<ChronicleOperation>,
    ) -> Result<ApiResponse, ApiError> {
        let kms = common::signing::KMS::Directory(&self.keystore);
        let identity = identity.signed_identity(kms)?;
        let model = ProvModel::from_tx(&to_apply)?;
        let tx_id = self.submit_blocking(&ChronicleTransaction::new(to_apply, identity))?;

        Ok(ApiResponse::submission(id, model, tx_id))
    }

    /// Ensure that Chronicle API calls that will not result in any changes in state should not be dispatched
    ///
    /// # Arguments
    /// * `state` - `ProvModel` for the operations' namespace
    /// * `to_apply` - Chronicle operations resulting from an API call
    #[instrument(skip(self))]
    fn ensure_effects(
        &mut self,
        state: &mut ProvModel,
        to_apply: &Vec<ChronicleOperation>,
    ) -> Result<Option<Vec<ChronicleOperation>>, ApiError> {
        let mut transactions = Vec::new();

        for tx in to_apply {
            let mut state_with_effects = state.clone();
            state_with_effects.apply(tx)?;
            if state_with_effects != *state {
                transactions.push(tx.clone());
                state.apply(tx)?;
            } else {
                info!(?tx, "Transaction has no effect, data already recorded");
            }
        }

        if transactions.is_empty() {
            Ok(None)
        } else {
            Ok(Some(transactions))
        }
    }

    fn apply_effects_and_submit(
        &mut self,
        connection: &mut PgConnection,
        id: impl Into<ChronicleIri>,
        identity: AuthId,
        to_apply: Vec<ChronicleOperation>,
        namespace: NamespaceId,
        applying_new_namespace: bool,
    ) -> Result<ApiResponse, ApiError> {
        if applying_new_namespace {
            self.submit(id, identity, to_apply)
        } else if let Some(to_apply) = {
            let mut state = self
                .store
                .prov_model_for_namespace(connection, &namespace)?;
            state.namespace_context(&namespace);
            self.ensure_effects(&mut state, &to_apply)?
        } {
            self.submit(id, identity, to_apply)
        } else {
            info!("API call will not result in any data changes");
            let model = ProvModel::from_tx(&to_apply)?;
            Ok(ApiResponse::already_recorded(id, model))
        }
    }

    /// Ensures that the named namespace exists, returns an existing namespace, and a vector containing a `ChronicleTransaction` to create one if not present
    ///
    /// A namespace uri is of the form chronicle:ns:{external_id}:{uuid}
    /// Namespaces must be globally unique, so are disambiguated by uuid but are locally referred to by external_id only
    /// For coordination between chronicle nodes we also need a namespace binding operation to tie the UUID from another instance to a external_id
    /// # Arguments
    /// * `external_id` - an arbitrary namespace identifier
    #[instrument(skip(self, connection))]
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
    #[instrument(skip(self))]
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

                let applying_new_namespace = !to_apply.is_empty();

                let create = ChronicleOperation::WasGeneratedBy(WasGeneratedBy {
                    namespace: namespace.clone(),
                    id: id.clone(),
                    activity: activity_id,
                });

                to_apply.push(create);

                api.apply_effects_and_submit(
                    connection,
                    id,
                    identity,
                    to_apply,
                    namespace,
                    applying_new_namespace,
                )
            })
        })
        .await?
    }

    /// Creates and submits a (ChronicleTransaction::ActivityUses), and possibly (ChronicleTransaction::Domaintype) if specified
    /// We use our local store for a best guess at the activity, either by name or the last one started as a convenience for command line
    #[instrument(skip(self))]
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

                let applying_new_namespace = !to_apply.is_empty();

                let (id, to_apply) = {
                    let create = ChronicleOperation::ActivityUses(ActivityUses {
                        namespace: namespace.clone(),
                        id: id.clone(),
                        activity: activity_id,
                    });

                    to_apply.push(create);

                    (id, to_apply)
                };

                api.apply_effects_and_submit(
                    connection,
                    id,
                    identity,
                    to_apply,
                    namespace,
                    applying_new_namespace,
                )
            })
        })
        .await?
    }

    /// Creates and submits a (ChronicleTransaction::ActivityWasInformedBy)
    ///
    /// We use our local store for a best guess at the activity, either by external_id or the last one started as a convenience for command line
    #[instrument(skip(self))]
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

                let applying_new_namespace = !to_apply.is_empty();

                let (id, to_apply) = {
                    let create = ChronicleOperation::WasInformedBy(WasInformedBy {
                        namespace: namespace.clone(),
                        activity: id.clone(),
                        informing_activity: informing_activity_id,
                    });

                    to_apply.push(create);

                    (id, to_apply)
                };

                api.apply_effects_and_submit(
                    connection,
                    id,
                    identity,
                    to_apply,
                    namespace,
                    applying_new_namespace,
                )
            })
        })
        .await?
    }

    /// Submits operations [`CreateEntity`], and [`SetAttributes::Entity`]
    ///
    /// We use our local store to see if the agent already exists, disambiguating the URI if so
    #[instrument(skip(self))]
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

                let applying_new_namespace = !to_apply.is_empty();

                let id = EntityId::from_external_id(&external_id);

                let create = ChronicleOperation::EntityExists(EntityExists {
                    namespace: namespace.clone(),
                    external_id: external_id.clone(),
                });

                to_apply.push(create);

                let set_type = ChronicleOperation::SetAttributes(SetAttributes::Entity {
                    id: EntityId::from_external_id(&external_id),
                    namespace: namespace.clone(),
                    attributes,
                });

                to_apply.push(set_type);

                api.apply_effects_and_submit(
                    connection,
                    id,
                    identity,
                    to_apply,
                    namespace,
                    applying_new_namespace,
                )
            })
        })
        .await?
    }

    /// Submits operations [`CreateActivity`], and [`SetAttributes::Activity`]
    ///
    /// We use our local store to see if the activity already exists, disambiguating the URI if so
    #[instrument(skip(self))]
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

                let applying_new_namespace = !to_apply.is_empty();

                let create = ChronicleOperation::ActivityExists(ActivityExists {
                    namespace: namespace.clone(),
                    external_id: external_id.clone(),
                });

                to_apply.push(create);

                let id = ActivityId::from_external_id(&external_id);
                let set_type = ChronicleOperation::SetAttributes(SetAttributes::Activity {
                    id: id.clone(),
                    namespace: namespace.clone(),
                    attributes,
                });

                to_apply.push(set_type);

                api.apply_effects_and_submit(
                    connection,
                    id,
                    identity,
                    to_apply,
                    namespace,
                    applying_new_namespace,
                )
            })
        })
        .await?
    }

    /// Submits operations [`CreateAgent`], and [`SetAttributes::Agent`]
    ///
    /// We use our local store to see if the agent already exists, disambiguating the URI if so
    #[instrument(skip(self))]
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

                let applying_new_namespace = !to_apply.is_empty();

                let create = ChronicleOperation::AgentExists(AgentExists {
                    external_id: external_id.to_owned(),
                    namespace: namespace.clone(),
                });

                to_apply.push(create);

                let id = AgentId::from_external_id(&external_id);
                let set_type = ChronicleOperation::SetAttributes(SetAttributes::Agent {
                    id: id.clone(),
                    namespace: namespace.clone(),
                    attributes,
                });

                to_apply.push(set_type);

                api.apply_effects_and_submit(
                    connection,
                    id,
                    identity,
                    to_apply,
                    namespace,
                    applying_new_namespace,
                )
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

                api.submit(namespace, identity, to_apply)
            })
        })
        .await?
    }

    #[instrument(skip(self))]
    async fn depth_charge(
        &self,
        namespace: NamespaceId,
        identity: AuthId,
    ) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();
        let id = ActivityId::from_external_id(Uuid::new_v4().to_string());
        tokio::task::spawn_blocking(move || {
            let to_apply = vec![
                ChronicleOperation::StartActivity(StartActivity {
                    namespace: namespace.clone(),
                    id: id.clone(),
                    time: Utc::now(),
                }),
                ChronicleOperation::EndActivity(EndActivity {
                    namespace,
                    id,
                    time: Utc::now(),
                }),
            ];
            api.submit_depth_charge(identity, to_apply)
        })
        .await?
    }

    fn submit_depth_charge(
        &mut self,
        identity: AuthId,
        to_apply: Vec<ChronicleOperation>,
    ) -> Result<ApiResponse, ApiError> {
        let identity = identity.signed_identity(common::signing::KMS::Directory(&self.keystore))?;
        let tx_id = self.submit_blocking(&ChronicleTransaction::new(to_apply, identity))?;
        Ok(ApiResponse::depth_charge_submission(tx_id))
    }

    #[instrument(skip(self))]
    async fn dispatch(&mut self, command: (ApiCommand, AuthId)) -> Result<ApiResponse, ApiError> {
        match command {
            (ApiCommand::DepthCharge(DepthChargeCommand { namespace }), identity) => {
                self.depth_charge(namespace, identity).await
            }
            (
                ApiCommand::Import(ImportCommand {
                    namespace,
                    operations,
                }),
                identity,
            ) => {
                self.submit_import_operations(identity, namespace, operations)
                    .await
            }
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
                ApiCommand::Entity(EntityCommand::Attribute {
                    id,
                    namespace,
                    responsible,
                    role,
                }),
                identity,
            ) => {
                self.attribute(namespace, responsible, id, role, identity)
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

                let applying_new_namespace = !to_apply.is_empty();

                let tx = ChronicleOperation::AgentActsOnBehalfOf(ActsOnBehalfOf::new(
                    &namespace,
                    &responsible_id,
                    &delegate_id,
                    activity_id.as_ref(),
                    role,
                ));

                to_apply.push(tx);

                api.apply_effects_and_submit(
                    connection,
                    responsible_id,
                    identity,
                    to_apply,
                    namespace,
                    applying_new_namespace,
                )
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

                let applying_new_namespace = !to_apply.is_empty();

                let tx = ChronicleOperation::WasAssociatedWith(WasAssociatedWith::new(
                    &namespace,
                    &activity_id,
                    &responsible_id,
                    role,
                ));

                to_apply.push(tx);

                api.apply_effects_and_submit(
                    connection,
                    responsible_id,
                    identity,
                    to_apply,
                    namespace,
                    applying_new_namespace,
                )
            })
        })
        .await?
    }

    #[instrument(skip(self))]
    async fn attribute(
        &self,
        namespace: ExternalId,
        responsible_id: AgentId,
        entity_id: EntityId,
        role: Option<Role>,
        identity: AuthId,
    ) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();

        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;

            connection.build_transaction().run(|connection| {
                let (namespace, mut to_apply) = api.ensure_namespace(connection, &namespace)?;

                let applying_new_namespace = !to_apply.is_empty();

                let tx = ChronicleOperation::WasAttributedTo(WasAttributedTo::new(
                    &namespace,
                    &entity_id,
                    &responsible_id,
                    role,
                ));

                to_apply.push(tx);

                api.apply_effects_and_submit(
                    connection,
                    responsible_id,
                    identity,
                    to_apply,
                    namespace,
                    applying_new_namespace,
                )
            })
        })
        .await?
    }

    #[instrument(skip(self))]
    async fn entity_derive(
        &self,
        id: EntityId,
        namespace: ExternalId,
        activity_id: Option<ActivityId>,
        used_id: EntityId,
        typ: DerivationType,
        identity: AuthId,
    ) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();

        tokio::task::spawn_blocking(move || {
            let mut connection = api.store.connection()?;

            connection.build_transaction().run(|connection| {
                let (namespace, mut to_apply) = api.ensure_namespace(connection, &namespace)?;

                let applying_new_namespace = !to_apply.is_empty();

                let tx = ChronicleOperation::EntityDerive(EntityDerive {
                    namespace: namespace.clone(),
                    id: id.clone(),
                    used_id: used_id.clone(),
                    activity_id: activity_id.clone(),
                    typ,
                });

                to_apply.push(tx);

                api.apply_effects_and_submit(
                    connection,
                    id,
                    identity,
                    to_apply,
                    namespace,
                    applying_new_namespace,
                )
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
                .namespace_by_external_id(&mut connection, &ExternalId::from(&query.namespace))?;
            Ok(ApiResponse::query_reply(
                api.store.prov_model_for_namespace(&mut connection, &id)?,
            ))
        })
        .await?
    }

    async fn submit_import_operations(
        &self,
        identity: AuthId,
        namespace: NamespaceId,
        operations: Vec<ChronicleOperation>,
    ) -> Result<ApiResponse, ApiError> {
        let mut api = self.clone();
        let identity = identity.signed_identity(common::signing::KMS::Directory(&self.keystore))?;
        let model = ProvModel::from_tx(&operations)?;
        tokio::task::spawn_blocking(move || {
            // Check here to ensure that import operations result in data changes
            let mut connection = api.store.connection()?;
            connection.build_transaction().run(|connection| {
                // If the namespace exists, get the model for the namespace and check each operation will have effects
                let mut state = match api.store.prov_model_for_namespace(connection, &namespace) {
                    Ok(mut state) => {
                        info!("Importing data to existing namespace: {namespace}");
                        // `prov_model_for_namespace` returns a model with the namespace context not applied
                        state.namespace_context(&namespace);
                        state
                    }
                    _ => {
                        // If namespace does not exist, create a new model
                        info!("Importing data to new namespace: {namespace}");
                        ProvModel::default()
                    }
                };
                if let Some(operations) = api.ensure_effects(&mut state, &operations)? {
                    info!("Submitting import operations to ledger");
                    let tx_id =
                        api.submit_blocking(&ChronicleTransaction::new(operations, identity))?;
                    Ok(ApiResponse::import_submitted(model, tx_id))
                } else {
                    info!("Import will not result in any data changes");
                    let model = ProvModel::from_tx(&operations)?;
                    Ok(ApiResponse::already_recorded(namespace, model))
                }
            })
        })
        .await?
    }

    #[instrument(level = "debug", skip(self), ret(Debug))]
    async fn sync(
        &self,
        prov: Box<ProvModel>,
        block_id: &BlockId,
        tx_id: ChronicleTransactionId,
    ) -> Result<ApiResponse, ApiError> {
        let api = self.clone();
        let block_id = *block_id;
        tokio::task::spawn_blocking(move || {
            api.store.apply_prov(&prov)?;
            api.store.set_last_block_id(&block_id, tx_id)?;

            Ok(ApiResponse::Unit)
        })
        .await?
    }

    /// Creates and submits a (ChronicleTransaction::StartActivity) determining the appropriate agent by external_id, or via [use_agent] context
    #[instrument(skip(self))]
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

                let applying_new_namespace = !to_apply.is_empty();

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

                api.apply_effects_and_submit(
                    connection,
                    id,
                    identity,
                    to_apply,
                    namespace,
                    applying_new_namespace,
                )
            })
        })
        .await?
    }

    /// Creates and submits a (ChronicleTransaction::StartActivity), determining the appropriate agent by name, or via [use_agent] context
    #[instrument(skip(self))]
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

                let applying_new_namespace = !to_apply.is_empty();

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

                api.apply_effects_and_submit(
                    connection,
                    id,
                    identity,
                    to_apply,
                    namespace,
                    applying_new_namespace,
                )
            })
        })
        .await?
    }

    /// Creates and submits a (ChronicleTransaction::EndActivity), determining the appropriate agent by name or via [use_agent] context
    #[instrument(skip(self))]
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

                let applying_new_namespace = !to_apply.is_empty();

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

                api.apply_effects_and_submit(
                    connection,
                    id,
                    identity,
                    to_apply,
                    namespace,
                    applying_new_namespace,
                )
            })
        })
        .await?
    }

    #[instrument(skip(self))]
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

    use crate::{inmem::EmbeddedChronicleTp, Api, ApiDispatch, ApiError, UuidGen};

    use chrono::{TimeZone, Utc};
    use common::{
        attributes::{Attribute, Attributes},
        commands::{
            ActivityCommand, AgentCommand, ApiCommand, ApiResponse, EntityCommand, ImportCommand,
            NamespaceCommand,
        },
        database::TemporaryDatabase,
        identity::AuthId,
        k256::sha2::{Digest, Sha256},
        prov::{
            operations::{ChronicleOperation, DerivationType},
            to_json_ld::ToJson,
            ActivityId, AgentId, ChronicleTransactionId, DomaintypeId, EntityId, ExpandedJson,
            NamespaceId, ProvModel,
        },
        signing::DirectoryStoredKeys,
    };
    use opa_tp_protocol::state::{policy_address, policy_meta_address, PolicyMeta};
    use protobuf::Message;
    use sawtooth_sdk::messages::setting::{Setting, Setting_Entry};

    use std::collections::HashMap;
    use tempfile::TempDir;
    use uuid::Uuid;

    struct TestDispatch<'a> {
        api: ApiDispatch,
        _db: TemporaryDatabase<'a>, // share lifetime
        _tp: EmbeddedChronicleTp,
    }

    impl<'a> TestDispatch<'a> {
        pub async fn dispatch(
            &mut self,
            command: ApiCommand,
            identity: AuthId,
        ) -> Result<Option<(Box<ProvModel>, ChronicleTransactionId)>, ApiError> {
            // We can sort of get final on chain state here by using a map of subject to model
            match self.api.dispatch(command, identity).await? {
                ApiResponse::Submission { .. } | ApiResponse::ImportSubmitted { .. } => {
                    // Recv until we get a commit notification
                    loop {
                        let commit = self.api.notify_commit.subscribe().recv().await.unwrap();
                        match commit {
                            common::ledger::SubmissionStage::Submitted(Ok(_)) => continue,
                            common::ledger::SubmissionStage::Committed(commit, _id) => {
                                return Ok(Some((commit.delta, commit.tx_id)))
                            }
                            common::ledger::SubmissionStage::Submitted(Err(e)) => panic!("{e:?}"),
                            common::ledger::SubmissionStage::NotCommitted((_, tx, _id)) => {
                                panic!("{tx:?}")
                            }
                        }
                    }
                }
                ApiResponse::AlreadyRecorded { subject: _, prov } => {
                    Ok(Some((prov, ChronicleTransactionId::from("null"))))
                }
                _ => Ok(None),
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

    fn embed_chronicle_tp() -> EmbeddedChronicleTp {
        chronicle_telemetry::telemetry(None, chronicle_telemetry::ConsoleLogging::Pretty);
        let mut buf = vec![];
        Setting {
            entries: vec![Setting_Entry {
                key: "chronicle.opa.policy_name".to_string(),
                value: "allow_transactions".to_string(),
                ..Default::default()
            }]
            .into(),
            ..Default::default()
        }
        .write_to_vec(&mut buf)
        .unwrap();
        let setting_id = (
            chronicle_protocol::settings::sawtooth_settings_address("chronicle.opa.policy_name"),
            buf,
        );
        let mut buf = vec![];
        Setting {
            entries: vec![Setting_Entry {
                key: "chronicle.opa.entrypoint".to_string(),
                value: "allow_transactions.allowed_users".to_string(),
                ..Default::default()
            }]
            .into(),
            ..Default::default()
        }
        .write_to_vec(&mut buf)
        .unwrap();

        let setting_entrypoint = (
            chronicle_protocol::settings::sawtooth_settings_address("chronicle.opa.entrypoint"),
            buf,
        );

        let d = env!("CARGO_MANIFEST_DIR").to_owned() + "/../../policies/bundle.tar.gz";
        let bin = std::fs::read(d).unwrap();

        let meta = PolicyMeta {
            id: "allow_transactions".to_string(),
            hash: hex::encode(Sha256::digest(&bin)),
            policy_address: policy_address("allow_transactions"),
        };

        EmbeddedChronicleTp::new_with_state(
            vec![
                setting_id,
                setting_entrypoint,
                (policy_address("allow_transactions"), bin),
                (
                    policy_meta_address("allow_transactions"),
                    serde_json::to_vec(&meta).unwrap(),
                ),
            ]
            .into_iter()
            .collect(),
        )
        .unwrap()
    }

    async fn test_api<'a>() -> TestDispatch<'a> {
        chronicle_telemetry::telemetry(None, chronicle_telemetry::ConsoleLogging::Pretty);

        let secretpath = TempDir::new().unwrap().into_path();

        let keystore_path = secretpath.clone();
        let keystore = DirectoryStoredKeys::new(keystore_path).unwrap();
        keystore.generate_chronicle().unwrap();

        let embed_tp = embed_chronicle_tp();
        let database = TemporaryDatabase::default();
        let pool = database.connection_pool().unwrap();

        let liveness_check_interval = None;

        let dispatch = Api::new(
            pool,
            embed_tp.ledger.clone(),
            &secretpath,
            SameUuid,
            HashMap::default(),
            Some("allow_transactions".into()),
            liveness_check_interval,
        )
        .await
        .unwrap();

        TestDispatch {
            api: dispatch,
            _db: database, // share the lifetime
            _tp: embed_tp,
        }
    }

    // Creates a mock file containing JSON-LD of the ChronicleOperations
    // that would be created by the given command, although not in any particular order.
    fn test_create_agent_operations_import() -> assert_fs::NamedTempFile {
        let file = assert_fs::NamedTempFile::new("import.json").unwrap();
        assert_fs::prelude::FileWriteStr::write_str(
            &file,
            r#"
        [
            {
                "@id": "_:n1",
                "@type": [
                "http://btp.works/chronicleoperations/ns#SetAttributes"
                ],
                "http://btp.works/chronicleoperations/ns#agentName": [
                {
                    "@value": "testagent"
                }
                ],
                "http://btp.works/chronicleoperations/ns#attributes": [
                {
                    "@type": "@json",
                    "@value": {}
                }
                ],
                "http://btp.works/chronicleoperations/ns#domaintypeId": [
                {
                    "@value": "type"
                }
                ],
                "http://btp.works/chronicleoperations/ns#namespaceName": [
                {
                    "@value": "testns"
                }
                ],
                "http://btp.works/chronicleoperations/ns#namespaceUuid": [
                {
                    "@value": "6803790d-5891-4dfa-b773-41827d2c630b"
                }
                ]
            },
            {
                "@id": "_:n1",
                "@type": [
                "http://btp.works/chronicleoperations/ns#CreateNamespace"
                ],
                "http://btp.works/chronicleoperations/ns#namespaceName": [
                {
                    "@value": "testns"
                }
                ],
                "http://btp.works/chronicleoperations/ns#namespaceUuid": [
                {
                    "@value": "6803790d-5891-4dfa-b773-41827d2c630b"
                }
                ]
            },
            {
                "@id": "_:n1",
                "@type": [
                "http://btp.works/chronicleoperations/ns#AgentExists"
                ],
                "http://btp.works/chronicleoperations/ns#agentName": [
                {
                    "@value": "testagent"
                }
                ],
                "http://btp.works/chronicleoperations/ns#namespaceName": [
                {
                    "@value": "testns"
                }
                ],
                "http://btp.works/chronicleoperations/ns#namespaceUuid": [
                {
                    "@value": "6803790d-5891-4dfa-b773-41827d2c630b"
                }
                ]
            }
        ]
         "#,
        )
        .unwrap();
        file
    }

    #[tokio::test]
    async fn test_import_operations() {
        let mut api = test_api().await;

        let file = test_create_agent_operations_import();

        let contents = std::fs::read_to_string(file.path()).unwrap();

        let json_array = serde_json::from_str::<Vec<serde_json::Value>>(&contents).unwrap();

        let mut operations = Vec::with_capacity(json_array.len());
        for value in json_array.into_iter() {
            let op = ChronicleOperation::from_json(ExpandedJson(value))
                .await
                .expect("Failed to parse imported JSON-LD to ChronicleOperation");
            operations.push(op);
        }

        let namespace = NamespaceId::from_external_id(
            "testns",
            Uuid::parse_str("6803790d-5891-4dfa-b773-41827d2c630b").unwrap(),
        );
        let identity = AuthId::chronicle();

        insta::assert_json_snapshot!(api
            .dispatch(ApiCommand::Import(ImportCommand { namespace: namespace.clone(), operations: operations.clone() } ), identity.clone())
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
                "chronicle:domaintype:type"
              ],
              "externalId": "testagent",
              "namespace": "chronicle:ns:testns:6803790d-5891-4dfa-b773-41827d2c630b",
              "value": {}
            },
            {
              "@id": "chronicle:ns:testns:6803790d-5891-4dfa-b773-41827d2c630b",
              "@type": "chronicle:Namespace",
              "externalId": "testns"
            }
          ]
        }
        "###);

        // Check that the operations that do not result in data changes are not submitted
        insta::assert_json_snapshot!(api
            .dispatch(ApiCommand::Import(ImportCommand { namespace, operations } ), identity)
            .await
            .unwrap()
            .unwrap()
            .1, @r###""null""###);
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

    #[tokio::test]
    async fn create_system_activity() {
        let mut api = test_api().await;

        let identity = AuthId::chronicle();

        insta::assert_json_snapshot!(api.dispatch(ApiCommand::Activity(ActivityCommand::Create {
            external_id: "testactivity".into(),
            namespace: common::prov::SYSTEM_ID.into(),
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
              "namespace": "chronicle:ns:chronicle%2Dsystem:00000000-0000-0000-0000-000000000001",
              "value": {
                "test": "test"
              }
            },
            {
              "@id": "chronicle:ns:chronicle%2Dsystem:00000000-0000-0000-0000-000000000001",
              "@type": "chronicle:Namespace",
              "externalId": "chronicle-system"
            }
          ]
        }
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

        insta::assert_snapshot!(res.err().unwrap().to_string(), @r###"Contradiction: Contradiction { attribute value change: test Attribute { typ: "test", value: String("test2") } Attribute { typ: "test", value: String("test") } }"###);
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

        insta::assert_snapshot!(res.err().unwrap().to_string(), @"Contradiction: Contradiction { start date alteration: 2014-07-08 09:10:11 UTC 2018-07-08 09:10:11 UTC }");
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

        insta::assert_snapshot!(res.err().unwrap().to_string(), @"Contradiction: Contradiction { end date alteration: 2018-07-08 09:10:11 UTC 2022-07-08 09:10:11 UTC }");
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
            derivation: DerivationType::None,
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
            derivation: DerivationType::PrimarySource,
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
            derivation: DerivationType::Revision,
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
            derivation: DerivationType::Quotation,
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
