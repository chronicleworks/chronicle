#![cfg_attr(feature = "strict", deny(warnings))]
pub mod chronicle_graphql;
pub mod commands;

use chronicle_signing::{ChronicleKnownKeyNamesSigner, ChronicleSigning, SecretError};
use chrono::{DateTime, Utc};

use common::{
	attributes::Attributes,
	identity::{AuthId, IdentityError, SignedIdentity},
	ledger::{Commit, SubmissionError, SubmissionStage},
	opa::PolicyAddress,
	prov::{
		json_ld::ToJson,
		operations::{
			ActivityExists, ActivityUses, ActsOnBehalfOf, AgentExists, ChronicleOperation,
			CreateNamespace, DerivationType, EndActivity, EntityDerive, EntityExists,
			SetAttributes, StartActivity, WasAssociatedWith, WasAttributedTo, WasGeneratedBy,
			WasInformedBy,
		},
		ActivityId, AgentId, ChronicleIri, ChronicleTransactionId, Contradiction, EntityId,
		ExternalId, ExternalIdPart, NamespaceId, ProcessorError, ProvModel, Role, UuidPart,
		SYSTEM_ID, SYSTEM_UUID,
	},
};
use diesel::{r2d2::ConnectionManager, PgConnection};
use diesel_migrations::MigrationHarness;
use futures::{select, FutureExt, StreamExt};

pub use chronicle_persistence::StoreError;
use chronicle_persistence::{Store, MIGRATIONS};
use diesel::r2d2::Pool;
use metrics::histogram;
use metrics_exporter_prometheus::PrometheusBuilder;
use protocol_substrate::SubxtClientError;
use protocol_substrate_chronicle::{
	protocol::{BlockId, FromBlock, LedgerReader, LedgerWriter},
	ChronicleEvent, ChronicleTransaction,
};
use std::{
	convert::Infallible,
	marker::PhantomData,
	net::AddrParseError,
	time::{Duration, Instant},
};
use thiserror::Error;
use tokio::{
	sync::mpsc::{self, error::SendError, Sender},
	task::JoinError,
};

use commands::*;
pub mod import;
use tracing::{debug, error, info, info_span, instrument, trace, warn, Instrument};

use user_error::UFE;
use uuid::Uuid;
#[derive(Error, Debug)]
pub enum ApiError {
	#[error("Storage: {0:?}")]
	Store(
		#[from]
		#[source]
		chronicle_persistence::StoreError,
	),

	#[error("Storage: {0:?}")]
	ArrowService(#[source] anyhow::Error),

	#[error("Transaction failed: {0}")]
	Transaction(
		#[from]
		#[source]
		diesel::result::Error,
	),

	#[error("Invalid IRI: {0}")]
	Iri(
		#[from]
		#[source]
		iref::Error,
	),

	#[error("JSON-LD processing: {0}")]
	JsonLD(String),

	#[error("Signing: {0}")]
	Signing(
		#[from]
		#[source]
		SecretError,
	),

	#[error("No agent is currently in use, please call agent use or supply an agent in your call")]
	NoCurrentAgent,

	#[error("Api shut down before reply")]
	ApiShutdownRx,

	#[error("Api shut down before send: {0}")]
	ApiShutdownTx(
		#[from]
		#[source]
		SendError<ApiSendWithReply>,
	),

	#[error("Ledger shut down before send: {0}")]
	LedgerShutdownTx(
		#[from]
		#[source]
		SendError<LedgerSendWithReply>,
	),

	#[error("Invalid socket address: {0}")]
	AddressParse(
		#[from]
		#[source]
		AddrParseError,
	),

	#[error("Connection pool: {0}")]
	ConnectionPool(
		#[from]
		#[source]
		r2d2::Error,
	),

	#[error("IO error: {0}")]
	InputOutput(
		#[from]
		#[source]
		std::io::Error,
	),

	#[error("Blocking thread pool: {0}")]
	Join(
		#[from]
		#[source]
		JoinError,
	),

	#[error("No appropriate activity to end")]
	NotCurrentActivity,

	#[error("Processor: {0}")]
	ProcessorError(
		#[from]
		#[source]
		ProcessorError,
	),

	#[error("Identity: {0}")]
	IdentityError(
		#[from]
		#[source]
		IdentityError,
	),

	#[error("Authentication endpoint error: {0}")]
	AuthenticationEndpoint(
		#[from]
		#[source]
		chronicle_graphql::AuthorizationError,
	),

	#[error("Substrate : {0}")]
	ClientError(
		#[from]
		#[source]
		SubxtClientError,
	),

	#[error("Submission : {0}")]
	Submission(
		#[from]
		#[source]
		SubmissionError,
	),

	#[error("Contradiction: {0}")]
	Contradiction(Contradiction),

	#[error("Embedded substrate: {0}")]
	EmbeddedSubstrate(anyhow::Error),
}

/// Ugly but we need this until ! is stable, see <https://github.com/rust-lang/rust/issues/64715>
impl From<Infallible> for ApiError {
	fn from(_: Infallible) -> Self {
		unreachable!()
	}
}

impl From<Contradiction> for ApiError {
	fn from(x: Contradiction) -> Self {
		Self::Contradiction(x)
	}
}

impl UFE for ApiError {}

type LedgerSendWithReply =
	(ChronicleTransaction, Sender<Result<ChronicleTransactionId, SubmissionError>>);

type ApiSendWithReply = ((ApiCommand, AuthId), Sender<Result<ApiResponse, ApiError>>);

pub trait UuidGen {
	fn uuid() -> Uuid {
		Uuid::new_v4()
	}
}

pub trait ChronicleSigned {
	/// Get the user identity's [`SignedIdentity`]
	fn signed_identity<S: ChronicleKnownKeyNamesSigner>(
		&self,
		store: &S,
	) -> Result<SignedIdentity, IdentityError>;
}

impl ChronicleSigned for AuthId {
	fn signed_identity<S: ChronicleKnownKeyNamesSigner>(
		&self,
		store: &S,
	) -> Result<SignedIdentity, IdentityError> {
		let signable = self.to_string();
		let signature = futures::executor::block_on(store.chronicle_sign(signable.as_bytes()))
			.map_err(|e| IdentityError::Signing(e.into()))?;
		let public_key = futures::executor::block_on(store.chronicle_verifying())
			.map_err(|e| IdentityError::Signing(e.into()))?;

		Ok(SignedIdentity {
			identity: signable,
			signature: signature.into(),
			verifying_key: Some(public_key.to_bytes().to_vec()),
		})
	}
}

#[derive(Clone)]
pub struct Api<
	U: UuidGen + Send + Sync + Clone,
	W: LedgerWriter<Transaction = ChronicleTransaction, Error = SubxtClientError>
		+ Clone
		+ Send
		+ Sync
		+ 'static,
> {
	submit_tx: tokio::sync::broadcast::Sender<SubmissionStage>,
	signing: ChronicleSigning,
	ledger_writer: W,
	store: chronicle_persistence::Store,
	uuid_source: PhantomData<U>,
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
		self.tx.clone().send(((command, identity), reply_tx)).await?;

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
		operations: Vec<ChronicleOperation>,
	) -> Result<ApiResponse, ApiError> {
		self.import_operations(identity, operations).await
	}

	#[instrument]
	async fn import_operations(
		&self,
		identity: AuthId,
		operations: Vec<ChronicleOperation>,
	) -> Result<ApiResponse, ApiError> {
		self.dispatch(ApiCommand::Import(ImportCommand { operations }), identity.clone())
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
		self.dispatch(ApiCommand::DepthCharge(DepthChargeCommand { namespace }), identity.clone())
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
		},
	};

	if let Err(e) = PrometheusBuilder::new().with_http_listener(metrics_listen_socket).install() {
		error!("Prometheus exporter installation for liveness check metrics failed: {e:?}");
	} else {
		debug!("Liveness check metrics Prometheus exporter installed with endpoint on {metrics_endpoint}/metrics");
	}
}

impl<U, LEDGER> Api<U, LEDGER>
where
	U: UuidGen + Send + Sync + Clone + core::fmt::Debug + 'static,
	LEDGER: LedgerWriter<Transaction = ChronicleTransaction, Error = SubxtClientError>
		+ LedgerReader<Event = ChronicleEvent, Error = SubxtClientError>
		+ Clone
		+ Send
		+ Sync
		+ 'static,
{
	#[instrument(skip(ledger))]
	pub async fn new(
		pool: Pool<ConnectionManager<PgConnection>>,
		ledger: LEDGER,
		uuidgen: U,
		signing: ChronicleSigning,
		namespace_bindings: Vec<NamespaceId>,
		policy_address: Option<PolicyAddress>,
		liveness_check_interval: Option<u64>,
	) -> Result<ApiDispatch, ApiError> {
		let (commit_tx, mut commit_rx) = mpsc::channel::<ApiSendWithReply>(10);

		let (commit_notify_tx, _) = tokio::sync::broadcast::channel(20);
		let dispatch =
			ApiDispatch { tx: commit_tx.clone(), notify_commit: commit_notify_tx.clone() };

		let store = Store::new(pool.clone())?;

		pool.get()?
			.build_transaction()
			.run(|connection| connection.run_pending_migrations(MIGRATIONS).map(|_| ()))
			.map_err(StoreError::DbMigration)?;

		let system_namespace_uuid = (SYSTEM_ID, Uuid::try_from(SYSTEM_UUID).unwrap());

		// Append namespace bindings and system namespace
		store.namespace_binding(system_namespace_uuid.0, system_namespace_uuid.1)?;
		for ns in namespace_bindings {
			info!(
				"Binding namespace with external ID: {}, UUID: {}",
				ns.external_id_part().as_str(),
				ns.uuid_part()
			);
			store.namespace_binding(ns.external_id_part().as_str(), ns.uuid_part().to_owned())?
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
				submit_tx: commit_notify_tx.clone(),
				signing,
				ledger_writer: ledger,
				store: store.clone(),
				uuid_source: PhantomData,
			};

			loop {
				let state_updates = reuse_reader.clone();

				let state_updates = state_updates.state_updates(start_from_block, None).await;

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
								  Some((ChronicleEvent::Contradicted{contradiction,identity,..},tx,_block_id,_position, _span)) => {
									commit_notify_tx.send(SubmissionStage::not_committed(
									  tx,contradiction, identity
									)).ok();
								  },
								  // Successfully committed to ledger, so apply
								  // to db and broadcast notification to
								  // subscription subscribers
								  Some((ChronicleEvent::Committed{ref diff, ref identity, ..},tx,block_id,_position,_span )) => {

										debug!(diff = ?diff.summarize());
										trace!(delta = %serde_json::to_string_pretty(&diff.to_json().compact().await.unwrap()).unwrap());

										api.sync( diff.clone().into(), &block_id,tx )
											.instrument(info_span!("Incoming confirmation", offset = ?block_id, tx = %tx))
											.await
											.map_err(|e| {
												error!(?e, "Api sync to confirmed commit");
											}).map(|_| commit_notify_tx.send(SubmissionStage::committed(Commit::new(
											   tx,block_id.to_string(), Box::new(diff.clone())
											), identity.clone() )).ok())
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
									},
								};

								match stage {
									SubmissionStage::Submitted(Ok(id)) => {
										if id == tx_id {
											debug!("Depth charge operation submitted: {id}");
											continue;
										}
									},
									SubmissionStage::Submitted(Err(err)) => {
										if err.tx_id() == &tx_id {
											error!("Depth charge transaction rejected by Chronicle: {} {}",
                                                err,
                                                err.tx_id()
                                            );
											break;
										}
									},
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
											let hist = histogram!("depth_charge_round_trip",);

											hist.record(elapsed_time.as_millis() as f64);

											break;
										}
									},
									SubmissionStage::NotCommitted((id, contradiction, _)) => {
										if id == tx_id {
											error!("Depth charge transaction rejected by ledger: {id} {contradiction}");
											break;
										}
									},
								}
							}
						},
						Ok(res) => error!("Unexpected ApiResponse from depth charge: {res:?}"),
						Err(e) => error!("ApiError submitting depth charge: {e}"),
					}
				}
			});
		}

		Ok(dispatch)
	}

	/// Notify after a successful submission, depending on the consistency requirement TODO: set in
	/// the transaction
	fn submit_blocking(
		&mut self,
		tx: ChronicleTransaction,
	) -> Result<ChronicleTransactionId, ApiError> {
		let (submission, _id) = futures::executor::block_on(self.ledger_writer.pre_submit(tx))?;

		let res =
			futures::executor::block_on(self.ledger_writer.do_submit(
				protocol_substrate_chronicle::protocol::WriteConsistency::Weak,
				submission,
			));
		match res {
			Ok(tx_id) => {
				self.submit_tx.send(SubmissionStage::submitted(&tx_id)).ok();
				Ok(tx_id)
			},
			Err((e, id)) => {
				// We need the cloneable SubmissionError wrapper here
				let submission_error = SubmissionError::communication(&id, e.into());
				self.submit_tx.send(SubmissionStage::submitted_error(&submission_error)).ok();
				Err(submission_error.into())
			},
		}
	}

	/// Generate and submit the signed identity to send to the Transaction Processor along with the
	/// transactions to be applied
	fn submit(
		&mut self,
		id: impl Into<ChronicleIri>,
		identity: AuthId,
		to_apply: Vec<ChronicleOperation>,
	) -> Result<ApiResponse, ApiError> {
		let identity = identity.signed_identity(&self.signing)?;
		let model = ProvModel::from_tx(&to_apply).map_err(ApiError::Contradiction)?;
		let tx_id = self.submit_blocking(futures::executor::block_on(
			ChronicleTransaction::new(&self.signing, identity, to_apply),
		)?)?;

		Ok(ApiResponse::submission(id, model, tx_id))
	}

	/// Checks if ChronicleOperations resulting from Chronicle API calls will result in any changes
	/// in state
	///
	/// # Arguments
	/// * `connection` - Connection to the Chronicle database
	/// * `to_apply` - Chronicle operations resulting from an API call
	#[instrument(skip(self, connection, to_apply))]
	fn check_for_effects(
		&mut self,
		connection: &mut PgConnection,
		to_apply: &Vec<ChronicleOperation>,
	) -> Result<Option<Vec<ChronicleOperation>>, ApiError> {
		let mut model = ProvModel::default();
		let mut transactions = Vec::<ChronicleOperation>::with_capacity(to_apply.len());
		for op in to_apply {
			let mut applied_model = match op {
				ChronicleOperation::CreateNamespace(CreateNamespace { id, .. }) => {
					let (namespace, _) =
						self.ensure_namespace(connection, id.external_id_part())?;
					model.namespace_context(&namespace);
					model
				},
				ChronicleOperation::AgentExists(AgentExists { ref namespace, ref id }) => {
					self.store.apply_prov_model_for_agent_id(
						connection,
						model,
						id,
						namespace.external_id_part(),
					)?
				},
				ChronicleOperation::ActivityExists(ActivityExists { ref namespace, ref id }) => {
					self.store.apply_prov_model_for_activity_id(
						connection,
						model,
						id,
						namespace.external_id_part(),
					)?
				},
				ChronicleOperation::EntityExists(EntityExists { ref namespace, ref id }) => {
					self.store.apply_prov_model_for_entity_id(
						connection,
						model,
						id,
						namespace.external_id_part(),
					)?
				},
				ChronicleOperation::ActivityUses(ActivityUses {
					ref namespace,
					ref id,
					ref activity,
				}) => self.store.prov_model_for_usage(
					connection,
					model,
					id,
					activity,
					namespace.external_id_part(),
				)?,
				ChronicleOperation::SetAttributes(ref o) => match o {
					SetAttributes::Activity { namespace, id, .. } => {
						self.store.apply_prov_model_for_activity_id(
							connection,
							model,
							id,
							namespace.external_id_part(),
						)?
					},
					SetAttributes::Agent { namespace, id, .. } => {
						self.store.apply_prov_model_for_agent_id(
							connection,
							model,
							id,
							namespace.external_id_part(),
						)?
					},
					SetAttributes::Entity { namespace, id, .. } => {
						self.store.apply_prov_model_for_entity_id(
							connection,
							model,
							id,
							namespace.external_id_part(),
						)?
					},
				},
				ChronicleOperation::StartActivity(StartActivity { namespace, id, .. }) => {
					self.store.apply_prov_model_for_activity_id(
						connection,
						model,
						id,
						namespace.external_id_part(),
					)?
				},
				ChronicleOperation::EndActivity(EndActivity { namespace, id, .. }) => {
					self.store.apply_prov_model_for_activity_id(
						connection,
						model,
						id,
						namespace.external_id_part(),
					)?
				},
				ChronicleOperation::WasInformedBy(WasInformedBy {
					namespace,
					activity,
					informing_activity,
				}) => {
					let model = self.store.apply_prov_model_for_activity_id(
						connection,
						model,
						activity,
						namespace.external_id_part(),
					)?;
					self.store.apply_prov_model_for_activity_id(
						connection,
						model,
						informing_activity,
						namespace.external_id_part(),
					)?
				},
				ChronicleOperation::AgentActsOnBehalfOf(ActsOnBehalfOf {
					activity_id,
					responsible_id,
					delegate_id,
					namespace,
					..
				}) => {
					let model = self.store.apply_prov_model_for_agent_id(
						connection,
						model,
						responsible_id,
						namespace.external_id_part(),
					)?;
					let model = self.store.apply_prov_model_for_agent_id(
						connection,
						model,
						delegate_id,
						namespace.external_id_part(),
					)?;
					if let Some(id) = activity_id {
						self.store.apply_prov_model_for_activity_id(
							connection,
							model,
							id,
							namespace.external_id_part(),
						)?
					} else {
						model
					}
				},
				ChronicleOperation::WasAssociatedWith(WasAssociatedWith {
					namespace,
					activity_id,
					agent_id,
					..
				}) => {
					let model = self.store.apply_prov_model_for_activity_id(
						connection,
						model,
						activity_id,
						namespace.external_id_part(),
					)?;

					self.store.apply_prov_model_for_agent_id(
						connection,
						model,
						agent_id,
						namespace.external_id_part(),
					)?
				},
				ChronicleOperation::WasGeneratedBy(WasGeneratedBy { namespace, id, activity }) => {
					let model = self.store.apply_prov_model_for_activity_id(
						connection,
						model,
						activity,
						namespace.external_id_part(),
					)?;

					self.store.apply_prov_model_for_entity_id(
						connection,
						model,
						id,
						namespace.external_id_part(),
					)?
				},
				ChronicleOperation::EntityDerive(EntityDerive {
					namespace,
					id,
					used_id,
					activity_id,
					..
				}) => {
					let model = self.store.apply_prov_model_for_entity_id(
						connection,
						model,
						id,
						namespace.external_id_part(),
					)?;

					let model = self.store.apply_prov_model_for_entity_id(
						connection,
						model,
						used_id,
						namespace.external_id_part(),
					)?;

					if let Some(id) = activity_id {
						self.store.apply_prov_model_for_activity_id(
							connection,
							model,
							id,
							namespace.external_id_part(),
						)?
					} else {
						model
					}
				},
				ChronicleOperation::WasAttributedTo(WasAttributedTo {
					namespace,
					entity_id,
					agent_id,
					..
				}) => {
					let model = self.store.apply_prov_model_for_entity_id(
						connection,
						model,
						entity_id,
						namespace.external_id_part(),
					)?;

					self.store.apply_prov_model_for_agent_id(
						connection,
						model,
						agent_id,
						namespace.external_id_part(),
					)?
				},
			};
			let state = applied_model.clone();
			applied_model.apply(op)?;
			if state != applied_model {
				transactions.push(op.clone());
			}

			model = applied_model;
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
		applying_new_namespace: bool,
	) -> Result<ApiResponse, ApiError> {
		if applying_new_namespace {
			self.submit(id, identity, to_apply)
		} else if let Some(to_apply) = self.check_for_effects(connection, &to_apply)? {
			self.submit(id, identity, to_apply)
		} else {
			info!("API call will not result in any data changes");
			let model = ProvModel::from_tx(&to_apply)?;
			Ok(ApiResponse::already_recorded(id, model))
		}
	}

	/// Ensures that the named namespace exists, returns an existing namespace, and a vector
	/// containing a `ChronicleTransaction` to create one if not present
	///
	/// A namespace uri is of the form chronicle:ns:{external_id}:{uuid}
	/// Namespaces must be globally unique, so are disambiguated by uuid but are locally referred to
	/// by external_id only For coordination between chronicle nodes we also need a namespace
	/// binding operation to tie the UUID from another instance to a external_id # Arguments
	/// * `external_id` - an arbitrary namespace identifier
	#[instrument(skip(self, connection))]
	fn ensure_namespace(
		&mut self,
		connection: &mut PgConnection,
		id: &ExternalId,
	) -> Result<(NamespaceId, Vec<ChronicleOperation>), ApiError> {
		match self.store.namespace_by_external_id(connection, id) {
			Ok((namespace_id, _)) => {
				trace!(%id, "Namespace already exists.");
				Ok((namespace_id, vec![]))
			},
			Err(e) => {
				debug!(error = %e, %id, "Namespace does not exist, creating.");
				let uuid = Uuid::new_v4();
				let namespace_id = NamespaceId::from_external_id(id, uuid);
				let create_namespace_op =
					ChronicleOperation::CreateNamespace(CreateNamespace::new(namespace_id.clone()));
				Ok((namespace_id, vec![create_namespace_op]))
			},
		}
	}

	/// Creates and submits a (ChronicleTransaction::GenerateEntity), and possibly
	/// (ChronicleTransaction::Domaintype) if specified
	///
	/// We use our local store for a best guess at the activity, either by external_id or the last
	/// one started as a convenience for command line
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
					namespace,
					id: id.clone(),
					activity: activity_id,
				});

				to_apply.push(create);

				api.apply_effects_and_submit(
					connection,
					id,
					identity,
					to_apply,
					applying_new_namespace,
				)
			})
		})
		.await?
	}

	/// Creates and submits a (ChronicleTransaction::ActivityUses), and possibly
	/// (ChronicleTransaction::Domaintype) if specified We use our local store for a best guess at
	/// the activity, either by name or the last one started as a convenience for command line
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
						namespace,
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
					applying_new_namespace,
				)
			})
		})
		.await?
	}

	/// Creates and submits a (ChronicleTransaction::ActivityWasInformedBy)
	///
	/// We use our local store for a best guess at the activity, either by external_id or the last
	/// one started as a convenience for command line
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
						namespace,
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
	#[instrument(skip(self))]
	async fn create_entity(
		&self,
		id: EntityId,
		namespace_id: ExternalId,
		attributes: Attributes,
		identity: AuthId,
	) -> Result<ApiResponse, ApiError> {
		let mut api = self.clone();
		tokio::task::spawn_blocking(move || {
			let mut connection = api.store.connection()?;

			connection.build_transaction().run(|connection| {
				let (namespace, mut to_apply) = api.ensure_namespace(connection, &namespace_id)?;

				let applying_new_namespace = !to_apply.is_empty();

				let create = ChronicleOperation::EntityExists(EntityExists {
					namespace: namespace.clone(),
					id: id.clone(),
				});

				to_apply.push(create);

				let set_type = ChronicleOperation::SetAttributes(SetAttributes::Entity {
					id: id.clone(),
					namespace,
					attributes,
				});

				to_apply.push(set_type);

				api.apply_effects_and_submit(
					connection,
					id,
					identity,
					to_apply,
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
		activity_id: ExternalId,
		namespace_id: ExternalId,
		attributes: Attributes,
		identity: AuthId,
	) -> Result<ApiResponse, ApiError> {
		let mut api = self.clone();
		tokio::task::spawn_blocking(move || {
			let mut connection = api.store.connection()?;

			connection.build_transaction().run(|connection| {
				let (namespace, mut to_apply) = api.ensure_namespace(connection, &namespace_id)?;

				let applying_new_namespace = !to_apply.is_empty();

				let create = ChronicleOperation::ActivityExists(ActivityExists {
					namespace: namespace.clone(),
					id: ActivityId::from_external_id(&activity_id),
				});

				to_apply.push(create);

				let set_type = ChronicleOperation::SetAttributes(SetAttributes::Activity {
					id: ActivityId::from_external_id(&activity_id),
					namespace,
					attributes,
				});

				to_apply.push(set_type);

				api.apply_effects_and_submit(
					connection,
					ActivityId::from_external_id(&activity_id),
					identity,
					to_apply,
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
		agent_id: ExternalId,
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
					id: AgentId::from_external_id(&agent_id),
					namespace: namespace.clone(),
				});

				to_apply.push(create);

				let id = AgentId::from_external_id(&agent_id);
				let set_type = ChronicleOperation::SetAttributes(SetAttributes::Agent {
					id: id.clone(),
					namespace,
					attributes,
				});

				to_apply.push(set_type);

				api.apply_effects_and_submit(
					connection,
					id,
					identity,
					to_apply,
					applying_new_namespace,
				)
			})
		})
		.await?
	}

	/// Creates and submits a (ChronicleTransaction::CreateNamespace) if the external_id part does
	/// not already exist in local storage
	async fn create_namespace(
		&self,
		name: &ExternalId,
		identity: AuthId,
	) -> Result<ApiResponse, ApiError> {
		let mut api = self.clone();
		let name = name.to_owned();
		tokio::task::spawn_blocking(move || {
			let mut connection = api.store.connection()?;
			connection.build_transaction().run(|connection| {
				let (namespace, to_apply) = api.ensure_namespace(connection, &name)?;

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
					time: Utc::now().into(),
				}),
				ChronicleOperation::EndActivity(EndActivity {
					namespace,
					id,
					time: Utc::now().into(),
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
		let identity = identity.signed_identity(&self.signing)?;
		let tx_id = self.submit_blocking(futures::executor::block_on(
			ChronicleTransaction::new(&self.signing, identity, to_apply),
		)?)?;
		Ok(ApiResponse::depth_charge_submission(tx_id))
	}

	#[instrument(skip(self))]
	async fn dispatch(&mut self, command: (ApiCommand, AuthId)) -> Result<ApiResponse, ApiError> {
		match command {
			(ApiCommand::DepthCharge(DepthChargeCommand { namespace }), identity) => {
				self.depth_charge(namespace, identity).await
			},
			(ApiCommand::Import(ImportCommand { operations }), identity) => {
				self.submit_import_operations(identity, operations).await
			},
			(ApiCommand::NameSpace(NamespaceCommand::Create { id }), identity) => {
				self.create_namespace(&id, identity).await
			},
			(ApiCommand::Agent(AgentCommand::Create { id, namespace, attributes }), identity) => {
				self.create_agent(id, namespace, attributes, identity).await
			},
			(ApiCommand::Agent(AgentCommand::UseInContext { id, namespace }), _identity) => {
				self.use_agent_in_cli_context(id, namespace).await
			},
			(
				ApiCommand::Agent(AgentCommand::Delegate {
					id,
					delegate,
					activity,
					namespace,
					role,
				}),
				identity,
			) => self.delegate(namespace, id, delegate, activity, role, identity).await,
			(
				ApiCommand::Activity(ActivityCommand::Create { id, namespace, attributes }),
				identity,
			) => self.create_activity(id, namespace, attributes, identity).await,
			(
				ApiCommand::Activity(ActivityCommand::Instant { id, namespace, time, agent }),
				identity,
			) => self.instant(id, namespace, time, agent, identity).await,
			(
				ApiCommand::Activity(ActivityCommand::Start { id, namespace, time, agent }),
				identity,
			) => self.start_activity(id, namespace, time, agent, identity).await,
			(
				ApiCommand::Activity(ActivityCommand::End { id, namespace, time, agent }),
				identity,
			) => self.end_activity(id, namespace, time, agent, identity).await,
			(ApiCommand::Activity(ActivityCommand::Use { id, namespace, activity }), identity) => {
				self.activity_use(id, namespace, activity, identity).await
			},
			(
				ApiCommand::Activity(ActivityCommand::WasInformedBy {
					id,
					namespace,
					informing_activity,
				}),
				identity,
			) => self.activity_was_informed_by(id, namespace, informing_activity, identity).await,
			(
				ApiCommand::Activity(ActivityCommand::Associate {
					id,
					namespace,
					responsible,
					role,
				}),
				identity,
			) => self.associate(namespace, responsible, id, role, identity).await,
			(
				ApiCommand::Entity(EntityCommand::Attribute { id, namespace, responsible, role }),
				identity,
			) => self.attribute(namespace, responsible, id, role, identity).await,
			(ApiCommand::Entity(EntityCommand::Create { id, namespace, attributes }), identity) => {
				self.create_entity(EntityId::from_external_id(&id), namespace, attributes, identity)
					.await
			},
			(
				ApiCommand::Activity(ActivityCommand::Generate { id, namespace, activity }),
				identity,
			) => self.activity_generate(id, namespace, activity, identity).await,
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
			},
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

				let tx = ChronicleOperation::agent_acts_on_behalf_of(
					namespace,
					responsible_id.clone(),
					delegate_id,
					activity_id,
					role,
				);

				to_apply.push(tx);

				api.apply_effects_and_submit(
					connection,
					responsible_id,
					identity,
					to_apply,
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

				let tx = ChronicleOperation::was_associated_with(
					namespace,
					activity_id,
					responsible_id.clone(),
					role,
				);

				to_apply.push(tx);

				api.apply_effects_and_submit(
					connection,
					responsible_id,
					identity,
					to_apply,
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

				let tx = ChronicleOperation::was_attributed_to(
					namespace,
					entity_id,
					responsible_id.clone(),
					role,
				);

				to_apply.push(tx);

				api.apply_effects_and_submit(
					connection,
					responsible_id,
					identity,
					to_apply,
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
					namespace,
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
			Ok(ApiResponse::query_reply(api.store.prov_model_for_namespace(&mut connection, &id)?))
		})
		.await?
	}

	async fn submit_import_operations(
		&self,
		identity: AuthId,
		operations: Vec<ChronicleOperation>,
	) -> Result<ApiResponse, ApiError> {
		let mut api = self.clone();
		let identity = identity.signed_identity(&self.signing)?;
		let model = ProvModel::from_tx(&operations)?;
		let signer = self.signing.clone();
		tokio::task::spawn_blocking(move || {
			// Check here to ensure that import operations result in data changes
			let mut connection = api.store.connection()?;
			connection.build_transaction().run(|connection| {
				if let Some(operations_to_apply) = api.check_for_effects(connection, &operations)? {
					tracing::trace!(
						operations_to_apply = operations_to_apply.len(),
						"Import operations submitted"
					);
					let tx_id = api.submit_blocking(futures::executor::block_on(
						ChronicleTransaction::new(&signer, identity, operations_to_apply),
					)?)?;
					Ok(ApiResponse::import_submitted(model, tx_id))
				} else {
					info!("Import will not result in any data changes");
					Ok(ApiResponse::AlreadyRecordedAll)
				}
			})
		})
		.await?
	}

	#[instrument(level = "trace", skip(self), ret(Debug))]
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

	/// Creates and submits a (ChronicleTransaction::StartActivity) determining the appropriate
	/// agent by external_id, or via [use_agent] context
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

				let now = Utc::now();

				to_apply.push(ChronicleOperation::StartActivity(StartActivity {
					namespace: namespace.clone(),
					id: id.clone(),
					time: time.unwrap_or(now).into(),
				}));

				to_apply.push(ChronicleOperation::EndActivity(EndActivity {
					namespace: namespace.clone(),
					id: id.clone(),
					time: time.unwrap_or(now).into(),
				}));

				if let Some(agent_id) = agent_id {
					to_apply.push(ChronicleOperation::was_associated_with(
						namespace,
						id.clone(),
						agent_id,
						None,
					));
				}

				api.apply_effects_and_submit(
					connection,
					id,
					identity,
					to_apply,
					applying_new_namespace,
				)
			})
		})
		.await?
	}

	/// Creates and submits a (ChronicleTransaction::StartActivity), determining the appropriate
	/// agent by name, or via [use_agent] context
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
					time: time.unwrap_or_else(Utc::now).into(),
				}));

				if let Some(agent_id) = agent_id {
					to_apply.push(ChronicleOperation::was_associated_with(
						namespace,
						id.clone(),
						agent_id,
						None,
					));
				}

				api.apply_effects_and_submit(
					connection,
					id,
					identity,
					to_apply,
					applying_new_namespace,
				)
			})
		})
		.await?
	}

	/// Creates and submits a (ChronicleTransaction::EndActivity), determining the appropriate agent
	/// by name or via [use_agent] context
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
					time: time.unwrap_or_else(Utc::now).into(),
				}));

				if let Some(agent_id) = agent_id {
					to_apply.push(ChronicleOperation::was_associated_with(
						namespace,
						id.clone(),
						agent_id,
						None,
					));
				}

				api.apply_effects_and_submit(
					connection,
					id,
					identity,
					to_apply,
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
				api.store.use_agent(connection, id.external_id_part(), &namespace)
			})?;

			Ok(ApiResponse::Unit)
		})
		.await?
	}
}
