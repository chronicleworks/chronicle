use std::{net::SocketAddr, sync::Arc, vec::Vec};

use arrow_flight::{
	decode::FlightRecordBatchStream, flight_service_server::FlightService, Action, ActionType,
	Criteria, Empty, FlightData, FlightDescriptor, FlightEndpoint, FlightInfo, HandshakeRequest,
	HandshakeResponse, IpcMessage, PutResult, SchemaAsIpc, SchemaResult, Ticket,
};
use arrow_schema::ArrowError;
use diesel::{r2d2::ConnectionManager, PgConnection};
use futures::{
	future::join_all,
	stream::{self, BoxStream},
	FutureExt, StreamExt,
};
use lazy_static::lazy_static;
use r2d2::Pool;
use serde::Serialize;
use thiserror::Error;
use tokio::{sync::broadcast, task::spawn_blocking};
use tonic::{transport::Server, Request, Response, Status, Streaming};
use tracing::{info, instrument};

use api::{chronicle_graphql::EndpointSecurityConfiguration, ApiDispatch, ApiError};
use common::{
	domain::TypeName,
	prov::{DomaintypeId, ExternalIdPart, ParseIriError},
};
use meta::{DomainTypeMeta, Term};
use query::{
	activity_count_by_type, agent_count_by_type, entity_count_by_type, EntityAndReferences,
};

use crate::{
	meta::get_domain_type_meta_from_cache,
	operations::{batch_to_flight_data, process_record_batch},
	peekablestream::PeekableFlightDataStream,
	query::{
		load_activities_by_type, load_agents_by_type, load_entities_by_type, ActivityAndReferences,
		AgentAndReferences,
	},
};

mod meta;
mod operations;
mod peekablestream;
mod query;

#[derive(Error, Debug)]
pub enum ChronicleArrowError {
	#[error("Arrow error: {0}")]
	ArrowSchemaError(
		#[from]
		#[source]
		ArrowError,
	),
	#[error("Missing schema for the requested entity or activity")]
	MissingSchemaError,

	#[error("Schema field not found: {0}")]
	SchemaFieldNotFound(String),

	#[error("Missing column: {0}")]
	MissingColumn(String),

	#[error("Column type mismatch for: {0}")]
	ColumnTypeMismatch(String),

	#[error("Invalid value: {0}")]
	InvalidValue(String),

	#[error("Invalid descriptor path")]
	InvalidDescriptorPath,

	#[error("Metadata not found")]
	MetadataNotFound,

	#[error("API error: {0}")]
	ApiError(
		#[from]
		#[source]
		ApiError,
	),
	#[error("Parse IRI : {0}")]
	IriError(
		#[from]
		#[source]
		ParseIriError,
	),

	#[error("Database connection pool error: {0}")]
	PoolError(
		#[from]
		#[source]
		r2d2::Error,
	),

	#[error("Diesel error: {0}")]
	DieselError(
		#[from]
		#[source]
		diesel::result::Error,
	),

	#[error("Serde JSON error: {0}")]
	SerdeJsonError(
		#[from]
		#[source]
		serde_json::Error,
	),

	#[error("Join error: {0}")]
	JoinError(
		#[from]
		#[source]
		tokio::task::JoinError,
	),

	#[error("UUID parse error: {0}")]
	UuidParseError(
		#[from]
		#[source]
		uuid::Error,
	),
}

#[instrument(skip(pool, term, domaintype))]
pub async fn calculate_count_by_metadata_term(
	pool: &Pool<ConnectionManager<PgConnection>>,
	term: &Term,
	domaintype: Option<String>,
) -> Result<i64, Status> {
	let pool = pool.clone();
	match term {
		Term::Entity =>
			spawn_blocking(move || {
				entity_count_by_type(
					&pool,
					domaintype.map(|x| x.to_string()).iter().map(|s| s.as_str()).collect(),
				)
			})
			.await,
		Term::Agent =>
			spawn_blocking(move || {
				agent_count_by_type(
					&pool,
					domaintype.map(|x| x.to_string()).iter().map(|s| s.as_str()).collect(),
				)
			})
			.await,
		Term::Activity =>
			spawn_blocking(move || {
				activity_count_by_type(
					&pool,
					domaintype.map(|x| x.to_string()).iter().map(|s| s.as_str()).collect(),
				)
			})
			.await,
		_ => Ok(Ok(0)),
	}
	.map_err(|e| Status::from_error(e.into()))
	.and_then(|res| res.map_err(|e| Status::from_error(e.into())))
}

async fn create_flight_info_for_type(
	pool: Arc<Pool<ConnectionManager<PgConnection>>>,
	domain_items: Vec<impl TypeName + Send + Sync + 'static>,
	term: Term,
	record_batch_size: usize,
) -> BoxStream<'static, Result<FlightInfo, Status>> {
	stream::iter(domain_items.into_iter().map(|item| Ok::<_, tonic::Status>(item)))
		.then(move |item| {
			let pool = pool.clone();
			async move {
				let item = item?; // Handle the Result from the iterator
				let descriptor_path = vec![term.to_string(), item.as_type_name()];
				let metadata =
					get_domain_type_meta_from_cache(&descriptor_path).ok_or_else(|| {
						Status::from_error(Box::new(ChronicleArrowError::MissingSchemaError))
					})?;

				let count = calculate_count_by_metadata_term(
					&pool,
					&term,
					Some(item.as_type_name().to_string()),
				)
				.await?;

				let tickets = (0..count)
					.step_by(record_batch_size as _)
					.map(|start| {
						let end = std::cmp::min(start as usize + record_batch_size, count as usize);

						let ticket_metadata = ChronicleTicket::new(
							term,
							metadata.typ.as_ref().map(|x| x.as_domain_type_id()),
							start as _,
							(end - start as usize) as _,
						);
						Ticket::try_from(ticket_metadata)
							.map_err(|e| Status::from_error(Box::new(ChronicleArrowError::from(e))))
					})
					.collect::<Result<Vec<_>, _>>()?;

				let mut flight_info = FlightInfo::new();

				for ticket in tickets {
					flight_info =
						flight_info.with_endpoint(FlightEndpoint::new().with_ticket(ticket));
				}

				Ok(flight_info
					.with_descriptor(FlightDescriptor::new_path(descriptor_path))
					.try_with_schema(&metadata.schema)
					.map_err(|e| Status::from_error(Box::new(ChronicleArrowError::from(e))))?
					.with_total_records(count))
			}
		})
		.boxed()
}

#[derive(Clone)]
pub struct FlightServiceImpl {
	domain: common::domain::ChronicleDomainDef,
	pool: r2d2::Pool<ConnectionManager<PgConnection>>,
	api: ApiDispatch,
	record_batch_size: usize,
	security: EndpointSecurityConfiguration,
}

impl FlightServiceImpl {
	pub fn new(
		domain: &common::domain::ChronicleDomainDef,
		pool: &r2d2::Pool<ConnectionManager<PgConnection>>,
		api: &ApiDispatch,
		security: EndpointSecurityConfiguration,
		record_batch_size: usize,
	) -> Self {
		Self {
			domain: domain.clone(),
			pool: pool.clone(),
			api: api.clone(),
			security,
			record_batch_size,
		}
	}
}

#[derive(Debug, Serialize, serde::Deserialize)]
struct ChronicleTicket {
	term: Term,
	descriptor_path: Vec<String>,
	typ: Option<DomaintypeId>,
	start: u64,
	count: u64,
}

impl ChronicleTicket {
	pub fn new(term: Term, typ: Option<DomaintypeId>, start: u64, count: u64) -> Self {
		Self {
			term,
			descriptor_path: vec![
				term.to_string(),
				typ.as_ref()
					.map(|x| x.external_id_part().to_string())
					.unwrap_or_else(|| format!("Prov{}", term)),
			],
			typ,
			start,
			count,
		}
	}

	pub fn descriptor_path(&self) -> &Vec<String> {
		&self.descriptor_path
	}
}

impl TryFrom<ChronicleTicket> for Ticket {
	type Error = serde_json::Error;

	fn try_from(ticket: ChronicleTicket) -> Result<Self, Self::Error> {
		let ticket_bytes = serde_json::to_vec(&ticket)?;
		Ok(Ticket { ticket: ticket_bytes.into() })
	}
}

impl TryFrom<Ticket> for ChronicleTicket {
	type Error = serde_json::Error;

	fn try_from(ticket: Ticket) -> Result<Self, Self::Error> {
		let ticket_data = ticket.ticket.to_vec();
		serde_json::from_slice(&ticket_data)
	}
}

fn parse_flight_descriptor_path(descriptor: &FlightDescriptor) -> Result<(Term, String), Status> {
	let path = &descriptor.path;
	if path.is_empty() {
		return Err(Status::invalid_argument("FlightDescriptor path is empty"));
	}

	let term = path[0]
		.parse::<Term>()
		.map_err(|_| Status::invalid_argument("First element of the path must be a valid Term"))?;

	Ok((term, path[1].to_string()))
}

#[tonic::async_trait]
impl FlightService for FlightServiceImpl {
	type DoActionStream = BoxStream<'static, Result<arrow_flight::Result, Status>>;
	type DoExchangeStream = BoxStream<'static, Result<FlightData, Status>>;
	type DoGetStream = BoxStream<'static, Result<FlightData, Status>>;
	type DoPutStream = BoxStream<'static, Result<PutResult, Status>>;
	type HandshakeStream = BoxStream<'static, Result<HandshakeResponse, Status>>;
	type ListActionsStream = BoxStream<'static, Result<ActionType, Status>>;
	type ListFlightsStream = BoxStream<'static, Result<FlightInfo, Status>>;

	async fn handshake(
		&self,
		_request: Request<Streaming<HandshakeRequest>>,
	) -> Result<Response<Self::HandshakeStream>, Status> {
		Ok(Response::new(Box::pin(futures::stream::empty()) as Self::HandshakeStream))
	}

	#[instrument(skip(self, _request))]
	async fn list_flights(
		&self,
		_request: Request<Criteria>,
	) -> Result<Response<Self::ListFlightsStream>, Status> {
		let entity_flights_stream = create_flight_info_for_type(
			Arc::new(self.pool.clone()),
			self.domain.entities.to_vec(),
			Term::Entity,
			self.record_batch_size,
		)
		.await;
		let activities_flights_stream = create_flight_info_for_type(
			Arc::new(self.pool.clone()),
			self.domain.activities.to_vec(),
			Term::Activity,
			self.record_batch_size,
		)
		.await;
		let agents_flights_stream = create_flight_info_for_type(
			Arc::new(self.pool.clone()),
			self.domain.agents.to_vec(),
			Term::Agent,
			self.record_batch_size,
		)
		.await;

		let combined_stream = futures::stream::select_all(vec![
			entity_flights_stream,
			activities_flights_stream,
			agents_flights_stream,
		])
		.boxed();

		Ok(Response::new(combined_stream as Self::ListFlightsStream))
	}

	#[instrument(skip(self, request))]
	async fn get_flight_info(
		&self,
		request: Request<FlightDescriptor>,
	) -> Result<Response<FlightInfo>, Status> {
		let descriptor = request.into_inner();

		let (term, type_name) = parse_flight_descriptor_path(&descriptor)?;

		let mut flight_info_stream = match term {
			Term::Entity => {
				let definition = self
					.domain
					.entities
					.iter()
					.find(|&item| item.as_type_name() == type_name)
					.ok_or_else(|| {
						Status::not_found(format!(
							"Definition not found for term: {:?}, type_name: {}",
							term, type_name
						))
					})?;
				create_flight_info_for_type(
					Arc::new(self.pool.clone()),
					vec![definition.clone()],
					term,
					self.record_batch_size,
				)
				.boxed()
			},
			Term::Activity => {
				let definition = self
					.domain
					.activities
					.iter()
					.find(|&item| item.as_type_name() == type_name)
					.ok_or_else(|| {
						Status::not_found(format!(
							"Definition not found for term: {:?}, type_name: {}",
							term, type_name
						))
					})?;
				create_flight_info_for_type(
					Arc::new(self.pool.clone()),
					vec![definition.clone()],
					term,
					self.record_batch_size,
				)
				.boxed()
			},
			Term::Agent => {
				let definition = self
					.domain
					.agents
					.iter()
					.find(|&item| item.as_type_name() == type_name)
					.ok_or_else(|| {
						Status::not_found(format!(
							"Definition not found for term: {:?}, type_name: {}",
							term, type_name
						))
					})?;
				create_flight_info_for_type(
					Arc::new(self.pool.clone()),
					vec![definition.clone()],
					term,
					self.record_batch_size,
				)
				.boxed()
			},
			_ =>
				return Err(Status::not_found(format!(
					"Definition not found for term: {:?}, type_name: {}",
					term, type_name
				))),
		}
		.await;

		let flight_info = flight_info_stream
			.next()
			.await
			.ok_or(Status::not_found("No flight info for descriptor"))?
			.map_err(|e| Status::from_error(e.into()))?;

		Ok(Response::new(flight_info))
	}

	#[instrument(skip(self, request))]
	async fn get_schema(
		&self,
		request: Request<FlightDescriptor>,
	) -> Result<Response<SchemaResult>, Status> {
		let descriptor = request.into_inner();

		let schema = get_domain_type_meta_from_cache(&descriptor.path)
			.ok_or_else(|| ChronicleArrowError::MissingSchemaError)
			.map_err(|e| Status::internal(format!("Failed to get cached schema: {}", e)))?;

		let options = arrow_ipc::writer::IpcWriteOptions::default();
		let ipc_message_result = SchemaAsIpc::new(&schema.schema, &options).try_into();
		match ipc_message_result {
			Ok(IpcMessage(schema)) => Ok(Response::new(SchemaResult { schema })),
			Err(e) =>
				Err(Status::internal(format!("Failed to convert schema to IPC message: {}", e))),
		}
	}

	#[instrument(skip(self))]
	async fn do_get(
		&self,
		request: Request<Ticket>,
	) -> Result<Response<Self::DoGetStream>, Status> {
		let ticket = request.into_inner();
		let ticket: ChronicleTicket = ticket
			.try_into()
			.map_err(|e| Status::from_error(Box::new(ChronicleArrowError::from(e))))?;

		let meta = get_domain_type_meta_from_cache(&ticket.descriptor_path)
			.ok_or(Status::from_error(Box::new(ChronicleArrowError::InvalidDescriptorPath)))?;

		tracing::debug!(ticket = ?ticket);

		let terms_result = match ticket.term {
			Term::Entity => {
				let pool = self.pool.clone();
				let meta_clone = meta.clone();
				let result = tokio::task::spawn_blocking(move || {
					load_entities_by_type(
						&pool,
						&ticket.typ,
						&meta_clone.attributes,
						ticket.start,
						ticket.count,
					)
				})
				.await
				.map_err(|e| Status::from_error(Box::new(ChronicleArrowError::from(e))))?
				.map_err(|e| Status::from_error(Box::new(e)))?;

				let (entities, _returned_records, _total_records) = result;

				EntityAndReferences::to_record_batch(entities, &meta).map_err(|e| {
					Status::internal(format!("Failed to convert to record batch: {}", e))
				})?
			},
			Term::Activity => {
				let pool = self.pool.clone();
				let result = tokio::task::spawn_blocking(move || {
					load_activities_by_type(&pool, &ticket.typ, ticket.start, ticket.count)
				})
				.await
				.map_err(|e| Status::from_error(Box::new(ChronicleArrowError::from(e))))?
				.map_err(|e| Status::from_error(Box::new(e)))?;

				let (activities, _returned_records, _total_records) = result;

				ActivityAndReferences::to_record_batch(activities, &meta).map_err(|e| {
					Status::internal(format!("Failed to convert to record batch: {}", e))
				})?
			},
			Term::Agent => {
				let pool = self.pool.clone();
				let result = tokio::task::spawn_blocking(move || {
					load_agents_by_type(&pool, &ticket.typ, ticket.start, ticket.count)
				})
				.await
				.map_err(|e| Status::from_error(Box::new(ChronicleArrowError::from(e))))?
				.map_err(|e| Status::from_error(Box::new(e)))?;

				let (agents, _returned_records, _total_records) = result;

				AgentAndReferences::to_record_batch(agents, &meta).map_err(|e| {
					Status::internal(format!("Failed to convert to record batch: {}", e))
				})?
			},
			Term::Namespace => {
				tracing::error!("Attempted to put namespaces, which is not supported.");
				return Err(Status::internal("Cannot put namespaces"));
			},
		};

		let flight_data_result = batch_to_flight_data(
			&FlightDescriptor::new_path(ticket.descriptor_path),
			&meta,
			terms_result,
		);

		match flight_data_result {
			Ok(flight_data) => {
				let stream = futures::stream::iter(flight_data.into_iter().map(Ok)).boxed();
				Ok(Response::new(stream))
			},
			Err(e) => Err(Status::internal(e.to_string())),
		}
	}

	#[instrument(skip(self, request))]
	async fn do_put(
		&self,
		request: Request<Streaming<FlightData>>,
	) -> Result<Response<Self::DoPutStream>, Status> {
		let mut stream = request.map(PeekableFlightDataStream::new).into_inner();
		let first_item = stream.peek().await;

		let flight_descriptor = match &first_item {
			Some(Ok(flight_data)) => match flight_data.flight_descriptor.clone() {
				Some(descriptor) => descriptor,
				None => return Err(Status::invalid_argument("Flight data has no descriptor")),
			},
			Some(Err(e)) =>
				return Err(Status::internal(format!("Failed to get first item from stream: {}", e))),
			None => {
				return Err(Status::invalid_argument("Stream is empty"));
			},
		};

		let filtered_stream = stream.filter_map(|item| async move {
			match item {
				Ok(flight_data) => {
					tracing::trace!("Processing flight data item {:?}", flight_data);
					Some(Ok(flight_data))
				},
				Err(e) => {
					tracing::error!(error = %e, "Error processing stream item.");
					None
				},
			}
		});

		let mut decoder = FlightRecordBatchStream::new_from_flight_data(filtered_stream);
		while let Some(batch) = decoder.next().await {
			let batch = batch?;
			tracing::debug!("Processing batch of: {:?}", batch.num_rows());
			process_record_batch(&flight_descriptor.path, batch, &self.api)
				.await
				.map_err(|e| Status::from_error(e.into()))?;
		}
		Ok(Response::new(Box::pin(stream::empty()) as Self::DoPutStream))
	}

	#[tracing::instrument(skip(self, _request))]
	async fn do_action(
		&self,
		_request: Request<Action>,
	) -> Result<Response<Self::DoActionStream>, Status> {
		tracing::info!("No actions available, returning empty stream.");
		Ok(Response::new(Box::pin(stream::empty())))
	}

	#[tracing::instrument(skip(self, _request))]
	async fn list_actions(
		&self,
		_request: Request<Empty>,
	) -> Result<Response<Self::ListActionsStream>, Status> {
		tracing::info!("No actions available.");
		Ok(Response::new(Box::pin(stream::empty())))
	}

	async fn do_exchange(
		&self,
		_request: Request<Streaming<FlightData>>,
	) -> Result<Response<Self::DoExchangeStream>, Status> {
		Err(Status::unimplemented("Implement do_exchange"))
	}
}

lazy_static! {
	static ref SHUTDOWN_CHANNEL: (broadcast::Sender<()>, broadcast::Receiver<()>) =
		broadcast::channel(1);
}

/// Triggers a shutdown signal across the application.
pub fn trigger_shutdown() {
	let _ = SHUTDOWN_CHANNEL.0.send(());
}

/// Returns a receiver for the shutdown signal.
pub async fn await_shutdown() {
	SHUTDOWN_CHANNEL.0.subscribe().recv().await.ok();
}

#[instrument(skip(pool, api, security))]
pub async fn run_flight_service(
	domain: &common::domain::ChronicleDomainDef,
	pool: &Pool<ConnectionManager<PgConnection>>,
	api: &ApiDispatch,
	security: EndpointSecurityConfiguration,
	addrs: &Vec<SocketAddr>,
	record_batch_size: usize,
) -> Result<(), tonic::transport::Error> {
	meta::cache_domain_schemas(domain);
	let mut services = vec![];
	for addr in addrs {
		let flight_service =
			FlightServiceImpl::new(domain, pool, api, security.clone(), record_batch_size);

		info!("Starting flight service at {}", addr);

		let server = Server::builder()
			.add_service(arrow_flight::flight_service_server::FlightServiceServer::new(
				flight_service,
			))
			.serve_with_shutdown(*addr, await_shutdown());

		services.push(server);
	}

	let results: Result<Vec<_>, _> = join_all(services.into_iter()).await.into_iter().collect();
	results?;

	Ok(())
}

#[cfg(test)]
mod tests {
	use std::{collections::HashMap, net::SocketAddr, time::Duration};

	use arrow_array::RecordBatch;
	use arrow_flight::{
		decode::FlightRecordBatchStream, flight_service_client::FlightServiceClient, Criteria,
		FlightData, FlightDescriptor, FlightInfo, SchemaAsIpc,
	};
	use arrow_ipc::writer::{self, IpcWriteOptions};
	use arrow_schema::ArrowError;
	use chrono::{TimeZone, Utc};
	use futures::{pin_mut, stream, StreamExt};
	use portpicker::pick_unused_port;
	use tonic::{transport::Channel, Request, Status};
	use uuid::Uuid;

	use api::{
		chronicle_graphql::{authorization::TokenChecker, EndpointSecurityConfiguration},
		commands::{ApiCommand, ImportCommand},
	};
	use chronicle_test_infrastructure::substitutes::{test_api, TestDispatch};
	use common::{
		attributes::{Attribute, Attributes},
		domain::{ChronicleDomainDef, PrimitiveType},
		identity::AuthId,
		prov::{operations::ChronicleOperation, NamespaceId},
	};

	use crate::{
		meta::{cache_domain_schemas, get_domain_type_meta_from_cache, DomainTypeMeta},
		query::{
			ActedOnBehalfOfRef, ActivityAndReferences, ActivityAssociationRef, AgentAndReferences,
			AgentAttributionRef, AgentInteraction, DerivationRef, EntityAndReferences,
			EntityAttributionRef,
		},
	};

	async fn setup_test_environment<'a>(
		domain: &ChronicleDomainDef,
	) -> Result<
		(FlightServiceClient<tonic::transport::Channel>, TestDispatch<'a>),
		Box<dyn std::error::Error>,
	> {
		chronicle_telemetry::telemetry(chronicle_telemetry::ConsoleLogging::Pretty);
		let api = test_api().await;
		let port = pick_unused_port().expect("No ports free");
		let addr = SocketAddr::from(([127, 0, 0, 1], port));
		let pool = api.temporary_database().connection_pool().unwrap();
		let dispatch = api.api_dispatch().clone();
		let domain = domain.clone();
		tokio::spawn(async move {
			super::run_flight_service(
				&domain,
				&pool,
				&dispatch,
				EndpointSecurityConfiguration::new(
					TokenChecker::new(None, None, 30),
					HashMap::default(),
					true,
				),
				&vec![addr],
				10,
			)
			.await
			.unwrap();
		});

		tokio::time::sleep(Duration::from_secs(5)).await;

		let client = FlightServiceClient::connect(format!("http://{}", addr)).await?;
		Ok((client, api))
	}

	fn create_test_domain_def() -> ChronicleDomainDef {
		let yaml = r#"
name: Manufacturing
attributes:
  BatchID:
    type: String
  CertID:
    type: String
  CompanyName:
    type: String
  PartID:
    type: String
  Location:
    type: String
agents:
  Contractor:
    attributes:
      - CompanyName
      - Location
entities:
  Certificate:
    attributes:
      - CertID
  Item:
    attributes:
      - PartID
activities:
  ItemCertified:
    attributes: []
  ItemManufactured:
    attributes:
      - BatchID
roles:
  - CERTIFIER
  - MANUFACTURER
"#;

		ChronicleDomainDef::from_input_string(yaml).unwrap()
	}

	fn create_attributes(
		typ: Option<&(dyn common::domain::TypeName + Send + Sync)>,
		attributes: &[(String, PrimitiveType)],
	) -> Attributes {
		Attributes::new(
			typ.map(|x| x.as_domain_type_id()),
			attributes
				.iter()
				.map(|(name, typ)| {
					let value = match typ {
						PrimitiveType::String =>
							serde_json::Value::String(format!("{}-value", name)),
						PrimitiveType::Int =>
							serde_json::Value::Number(serde_json::Number::from(42)),
						PrimitiveType::Bool => serde_json::Value::Bool(true),
						PrimitiveType::JSON =>
							serde_json::Value::String(format!("{{\"{}\": \"example\"}}", name)),
					};
					Attribute::new(name, value)
				})
				.collect(),
		)
	}

	fn create_test_entity(
		attributes: Vec<(String, PrimitiveType)>,
		meta: &DomainTypeMeta,
		count: u32,
	) -> RecordBatch {
		let mut entities = Vec::new();
		for i in 0..count {
			let entity = EntityAndReferences {
				id: format!("{}-{}", meta.typ.as_ref().map(|x| x.as_type_name()).unwrap(), i),
				namespace_name: "default".to_string(),
				namespace_uuid: Uuid::default().into_bytes(),
				attributes: create_attributes(meta.typ.as_deref(), &attributes),
				was_generated_by: vec![format!("activity-{}", i), format!("activity-{}", i + 1)],
				was_attributed_to: vec![
					EntityAttributionRef {
						agent: format!("agent-{}", i),
						role: Some("CERTIFIER".to_string()),
					},
					EntityAttributionRef {
						agent: format!("agent-{}", i + 1),
						role: Some("MANUFACTURER".to_string()),
					},
				],
				was_derived_from: vec![
					DerivationRef {
						source: format!("entity-d-{}", i),
						activity: format!("activity-d-{}", i),
					},
					DerivationRef {
						source: format!("entity-d-{}", i),
						activity: format!("activity-d-{}", i),
					},
				],
				was_quoted_from: vec![
					DerivationRef {
						source: format!("entity-q-{}", i),
						activity: format!("activity-q-{}", i),
					},
					DerivationRef {
						source: format!("entity-q-{}", i),
						activity: format!("activity-q-{}", i),
					},
				],
				was_revision_of: vec![
					DerivationRef {
						source: format!("entity-r-{}", i),
						activity: format!("activity-r-{}", i),
					},
					DerivationRef {
						source: format!("entity-r-{}", i),
						activity: format!("activity-r-{}", i),
					},
				],
				had_primary_source: vec![
					DerivationRef {
						source: format!("entity-ps-{}", i),
						activity: format!("activity-ps-{}", i),
					},
					DerivationRef {
						source: format!("entity-ps-{}", i),
						activity: format!("activity-ps-{}", i),
					},
				],
			};
			entities.push(entity);
		}

		EntityAndReferences::to_record_batch(entities.into_iter(), meta)
			.expect("Failed to convert entities to record batch")
	}

	fn create_test_activity(
		attributes: Vec<(String, PrimitiveType)>,
		meta: &DomainTypeMeta,
		count: u32,
	) -> RecordBatch {
		let mut activities = Vec::new();
		for i in 0..count {
			let activity = ActivityAndReferences {
				id: format!("{}-{}", meta.typ.as_ref().map(|x| x.as_type_name()).unwrap(), i),
				namespace_name: "default".to_string(),
				namespace_uuid: Uuid::default().into_bytes(),
				attributes: create_attributes(meta.typ.as_deref(), &attributes),
				started: Some(Utc.with_ymd_and_hms(2022, 1, 1, 0, 0, 0).unwrap()),
				ended: Some(Utc.with_ymd_and_hms(2022, 1, 2, 0, 0, 0).unwrap()),
				generated: vec![format!("entity-{}", i), format!("entity-{}", i + 1)],
				was_informed_by: vec![format!("activity-{}", i), format!("activity-{}", i + 1)],
				was_associated_with: vec![ActivityAssociationRef {
					responsible: AgentInteraction {
						agent: format!("agent-{}", i),
						role: Some("ROLE_TYPE".to_string()),
					},
					delegated: vec![AgentInteraction {
						agent: format!("delegated-agent-{}", i),
						role: Some("DELEGATED_ROLE".to_string()),
					}],
				}],
				used: vec![format!("entity-{}", i), format!("entity-{}", i + 1)],
			};
			activities.push(activity);
		}

		ActivityAndReferences::to_record_batch(activities.into_iter(), meta)
			.expect("Failed to convert activities to record batch")
	}

	fn create_test_agent(
		attributes: Vec<(String, PrimitiveType)>,
		meta: &DomainTypeMeta,
		count: u32,
	) -> RecordBatch {
		let mut agents = Vec::new();
		for i in 0..count {
			let agent = AgentAndReferences {
				id: format!("{}-{}", meta.typ.as_ref().map(|x| x.as_type_name()).unwrap(), i),
				namespace_name: "default".to_string(),
				namespace_uuid: Uuid::default().into_bytes(),
				attributes: create_attributes(meta.typ.as_deref(), &attributes),
				acted_on_behalf_of: vec![ActedOnBehalfOfRef {
					agent: format!("agent-{}", i),
					role: Some("DELEGATED_CERTIFIER".to_string()),
					activity: format!("activity-{}", i),
				}],
				was_attributed_to: vec![AgentAttributionRef {
					entity: format!("entity-{}", i),
					role: Some("UNSPECIFIED_INTERACTION".to_string()),
				}],
			};
			agents.push(agent);
		}

		AgentAndReferences::to_record_batch(agents.into_iter(), meta)
			.expect("Failed to convert agents to record batch")
	}

	pub fn batches_to_flight_data(
		descriptor: &FlightDescriptor,
		meta: &DomainTypeMeta,
		batches: Vec<RecordBatch>,
	) -> Result<Vec<FlightData>, ArrowError> {
		let options = IpcWriteOptions::default();
		let schema_flight_data: FlightData =
			std::convert::Into::<FlightData>::into(SchemaAsIpc::new(&meta.schema, &options))
				.with_descriptor(descriptor.clone());
		let mut dictionaries = vec![];
		let mut flight_data = vec![];

		let data_gen = writer::IpcDataGenerator::default();
		let mut dictionary_tracker = writer::DictionaryTracker::new(false);

		for batch in batches.iter() {
			let (encoded_dictionaries, encoded_batch) =
				data_gen.encoded_batch(batch, &mut dictionary_tracker, &options)?;

			dictionaries.extend(encoded_dictionaries.into_iter().map(Into::into));
			let next: FlightData = encoded_batch.into();
			flight_data.push(next);
		}

		let mut stream = vec![schema_flight_data];
		stream.extend(dictionaries);
		stream.extend(flight_data);
		let flight_data: Vec<_> = stream.into_iter().collect();
		Ok(flight_data)
	}

	async fn create_test_flight_data(
		count: u32,
	) -> Result<Vec<Vec<FlightData>>, Box<dyn std::error::Error>> {
		let entity_meta = get_domain_type_meta_from_cache(&vec![
			"Entity".to_string(),
			"CertificateEntity".to_owned(),
		])
		.expect("Failed to get entity meta");
		let entity_batch = create_test_entity(
			vec![("certIDAttribute".to_string(), PrimitiveType::String)],
			&entity_meta,
			count,
		);
		let entity_flight_data = batches_to_flight_data(
			&FlightDescriptor::new_path(vec!["Entity".to_string(), "CertificateEntity".to_owned()]),
			&entity_meta,
			vec![entity_batch],
		)?;

		let activity_meta = get_domain_type_meta_from_cache(&vec![
			"Activity".to_string(),
			"ItemManufacturedActivity".to_owned(),
		])
		.expect("Failed to get activity meta");
		let activity_batch = create_test_activity(
			vec![("batchIDAttribute".to_string(), PrimitiveType::String)],
			&activity_meta,
			count,
		);
		let activity_flight_data = batches_to_flight_data(
			&FlightDescriptor::new_path(vec![
				"Activity".to_string(),
				"ItemManufacturedActivity".to_owned(),
			]),
			&activity_meta,
			vec![activity_batch],
		)?;

		let agent_meta = get_domain_type_meta_from_cache(&vec![
			"Agent".to_string(),
			"ContractorAgent".to_owned(),
		])
		.expect("Failed to get agent meta");
		let agent_batch = create_test_agent(
			vec![
				("companyNameAttribute".to_string(), PrimitiveType::String),
				("locationAttribute".to_string(), PrimitiveType::String),
			],
			&agent_meta,
			count,
		);
		let agent_flight_data = batches_to_flight_data(
			&FlightDescriptor::new_path(vec!["Agent".to_string(), "ContractorAgent".to_owned()]),
			&agent_meta,
			vec![agent_batch],
		)?;

		let combined_flight_data =
			vec![entity_flight_data, agent_flight_data, activity_flight_data];

		Ok(combined_flight_data)
	}

	async fn put_test_data(
		count: u32,
		client: &mut FlightServiceClient<Channel>,
		api: &mut TestDispatch<'_>,
	) -> Result<(), Box<dyn std::error::Error>> {
		let create_namespace_operation = ChronicleOperation::create_namespace(
			NamespaceId::from_external_id("default", Uuid::default()),
		);
		api.dispatch(
			ApiCommand::Import(ImportCommand { operations: vec![create_namespace_operation] }),
			AuthId::anonymous(),
		)
		.await
		.map_err(|e| Status::from_error(e.into()))?;

		for flight_data in create_test_flight_data(count).await? {
			client.do_put(stream::iter(flight_data)).await?;
		}

		Ok(())
	}

	async fn stable_sorted_flight_info(
		client: &mut FlightServiceClient<Channel>,
	) -> Result<Vec<FlightInfo>, Box<dyn std::error::Error>> {
		let list_flights_response = client.list_flights(Request::new(Criteria::default())).await?;

		let flights = list_flights_response.into_inner().collect::<Vec<_>>().await;
		let mut valid_flights: Vec<FlightInfo> =
			flights.into_iter().filter_map(Result::ok).collect();

		valid_flights.sort_by(|a, b| {
			a.flight_descriptor
				.as_ref()
				.map(|a| a.path.clone())
				.cmp(&b.flight_descriptor.as_ref().map(|b| b.path.clone()))
		});
		Ok(valid_flights)
	}

	async fn load_flights(
		flights: &[FlightInfo],
		client: &mut FlightServiceClient<Channel>,
	) -> Result<Vec<Vec<FlightData>>, Box<dyn std::error::Error>> {
		let mut all_flight_data_results = Vec::new();
		for flight_info in flights {
			for endpoint in &flight_info.endpoint {
				if let Some(ticket) = &endpoint.ticket {
					let request = Request::new(ticket.clone());
					let mut stream = client.do_get(request).await?.into_inner();
					let mut flight_data_results = Vec::new();
					while let Some(flight_data) = stream.message().await? {
						flight_data_results.push(flight_data);
					}
					all_flight_data_results.push(flight_data_results);
				}
			}
		}
		Ok(all_flight_data_results)
	}

	async fn decode_flight_data(
		flight_data: Vec<FlightData>,
	) -> Result<Vec<RecordBatch>, Box<dyn std::error::Error>> {
		let decoder = FlightRecordBatchStream::new_from_flight_data(stream::iter(
			flight_data.into_iter().map(Ok),
		));
		let mut record_batches = Vec::new();
		pin_mut!(decoder);
		while let Some(batch) = decoder.next().await.transpose()? {
			record_batches.push(batch);
		}
		Ok(record_batches)
	}

	#[tokio::test]
	//Test using a reasonably large data set, over the endpoint paging boundary size so we can
	// observe it
	async fn flight_service_info() {
		chronicle_telemetry::telemetry(
			chronicle_telemetry::ConsoleLogging::Pretty,
		);
		let domain = create_test_domain_def();
		let (mut client, mut api) = setup_test_environment(&domain).await.unwrap();
		cache_domain_schemas(&domain);
		put_test_data(22, &mut client, &mut api).await.unwrap();

		tokio::time::sleep(Duration::from_secs(10)).await;

		let flights = stable_sorted_flight_info(&mut client).await.unwrap();

		insta::assert_debug_snapshot!(flights, @r###"
  [
      FlightInfo {
          schema: b"\xff\xff\xff\xff8\x04\0\0\x10\0\0\0\0\0\n\0\x0c\0\n\0\t\0\x04\0\n\0\0\0\x10\0\0\0\0\x01\x04\0\x08\0\x08\0\0\0\x04\0\x08\0\0\0\x04\0\0\0\t\0\0\0\xb4\x03\0\0p\x03\0\0H\x03\0\0\x04\x03\0\0\xb8\x02\0\0`\x02\0\0\x04\x02\0\0\xa4\x01\0\0\x04\0\0\0\x80\xfc\xff\xff\x18\0\0\0\x0c\0\0\0\0\0\0\x0ct\x01\0\0\x01\0\0\0\x08\0\0\0t\xfc\xff\xffD\xfd\xff\xff\x1c\0\0\0\x0c\0\0\0\0\0\x01\rH\x01\0\0\x02\0\0\0\xbc\0\0\0\x08\0\0\0\x98\xfc\xff\xff\xc4\xfc\xff\xff\x18\0\0\0\x0c\0\0\0\0\0\0\x0c\x90\0\0\0\x01\0\0\0\x08\0\0\0\xb8\xfc\xff\xff\x88\xfd\xff\xff\x1c\0\0\0\x0c\0\0\0\0\0\x01\rd\0\0\0\x02\0\0\04\0\0\0\x08\0\0\0\xdc\xfc\xff\xff\xac\xfd\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\x01\x05\x0c\0\0\0\0\0\0\0\xf8\xfc\xff\xff\x04\0\0\0role\0\0\0\00\xfd\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\0\x05\x0c\0\0\0\0\0\0\0 \xfd\xff\xff\x05\0\0\0agent\0\0\0\x04\0\0\0item\0\0\0\0\t\0\0\0delegated\0\0\0t\xfd\xff\xff\x1c\0\0\0\x0c\0\0\0\0\0\0\rd\0\0\0\x02\0\0\04\0\0\0\x08\0\0\0l\xfd\xff\xff<\xfe\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\x01\x05\x0c\0\0\0\0\0\0\0\x88\xfd\xff\xff\x04\0\0\0role\0\0\0\0\xc0\xfd\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\0\x05\x0c\0\0\0\0\0\0\0\xb0\xfd\xff\xff\x05\0\0\0agent\0\0\0\x0b\0\0\0responsible\0\x04\0\0\0item\0\0\0\0\x13\0\0\0was_associated_with\0\x1c\xfe\xff\xff\x18\0\0\0\x0c\0\0\0\0\0\0\x0c8\0\0\0\x01\0\0\0\x08\0\0\0\x10\xfe\xff\xff<\xfe\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\0\x05\x0c\0\0\0\0\0\0\0,\xfe\xff\xff\x04\0\0\0item\0\0\0\0\x0f\0\0\0was_informed_by\0x\xfe\xff\xff\x18\0\0\0\x0c\0\0\0\0\0\0\x0c8\0\0\0\x01\0\0\0\x08\0\0\0l\xfe\xff\xff\x98\xfe\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\0\x05\x0c\0\0\0\0\0\0\0\x88\xfe\xff\xff\x04\0\0\0item\0\0\0\0\t\0\0\0generated\0\0\0\xd0\xfe\xff\xff\x18\0\0\0\x0c\0\0\0\0\0\0\x0c8\0\0\0\x01\0\0\0\x08\0\0\0\xc4\xfe\xff\xff\xf0\xfe\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\0\x05\x0c\0\0\0\0\0\0\0\xe0\xfe\xff\xff\x04\0\0\0item\0\0\0\0\x04\0\0\0used\0\0\0\0\xc8\xff\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\x01\n\x1c\0\0\0\0\0\0\0\xb8\xff\xff\xff\x08\0\0\0\0\0\x03\0\x03\0\0\0UTC\0\x05\0\0\0ended\0\0\0\x10\0\x14\0\x10\0\x0e\0\x0f\0\x04\0\0\0\x08\0\x10\0\0\0\x1c\0\0\0\x0c\0\0\0\0\0\x01\n$\0\0\0\0\0\0\0\x08\0\x0c\0\n\0\x04\0\x08\0\0\0\x08\0\0\0\0\0\x03\0\x03\0\0\0UTC\0\x07\0\0\0started\0\xac\xff\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\0\x05\x0c\0\0\0\0\0\0\0\x9c\xff\xff\xff\x02\0\0\0id\0\0\xd0\xff\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\0\x05\x0c\0\0\0\0\0\0\0\xc0\xff\xff\xff\x0e\0\0\0namespace_uuid\0\0\x10\0\x14\0\x10\0\0\0\x0f\0\x04\0\0\0\x08\0\x10\0\0\0\x18\0\0\0\x0c\0\0\0\0\0\0\x05\x10\0\0\0\0\0\0\0\x04\0\x04\0\x04\0\0\0\x0e\0\0\0namespace_name\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
          flight_descriptor: Some(
              FlightDescriptor {
                  r#type: Path,
                  cmd: b"",
                  path: [
                      "Activity",
                      "ItemCertifiedActivity",
                  ],
              },
          ),
          endpoint: [],
          total_records: 0,
          total_bytes: -1,
          ordered: false,
      },
      FlightInfo {
          schema: b"\xff\xff\xff\xffx\x04\0\0\x10\0\0\0\0\0\n\0\x0c\0\n\0\t\0\x04\0\n\0\0\0\x10\0\0\0\0\x01\x04\0\x08\0\x08\0\0\0\x04\0\x08\0\0\0\x04\0\0\0\n\0\0\0\xec\x03\0\0\xa8\x03\0\0\x80\x03\0\0H\x03\0\0\xf4\x02\0\0\xb8\x02\0\0`\x02\0\0\x04\x02\0\0\xa4\x01\0\0\x04\0\0\0L\xfc\xff\xff\x18\0\0\0\x0c\0\0\0\0\0\0\x0ct\x01\0\0\x01\0\0\0\x08\0\0\0@\xfc\xff\xff\x04\xfd\xff\xff\x1c\0\0\0\x0c\0\0\0\0\0\x01\rH\x01\0\0\x02\0\0\0\xbc\0\0\0\x08\0\0\0d\xfc\xff\xff\x90\xfc\xff\xff\x18\0\0\0\x0c\0\0\0\0\0\0\x0c\x90\0\0\0\x01\0\0\0\x08\0\0\0\x84\xfc\xff\xffH\xfd\xff\xff\x1c\0\0\0\x0c\0\0\0\0\0\x01\rd\0\0\0\x02\0\0\04\0\0\0\x08\0\0\0\xa8\xfc\xff\xffl\xfd\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\x01\x05\x0c\0\0\0\0\0\0\0\xc4\xfc\xff\xff\x04\0\0\0role\0\0\0\0\xfc\xfc\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\0\x05\x0c\0\0\0\0\0\0\0\xec\xfc\xff\xff\x05\0\0\0agent\0\0\0\x04\0\0\0item\0\0\0\0\t\0\0\0delegated\0\0\0@\xfd\xff\xff\x1c\0\0\0\x0c\0\0\0\0\0\0\rd\0\0\0\x02\0\0\04\0\0\0\x08\0\0\08\xfd\xff\xff\xfc\xfd\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\x01\x05\x0c\0\0\0\0\0\0\0T\xfd\xff\xff\x04\0\0\0role\0\0\0\0\x8c\xfd\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\0\x05\x0c\0\0\0\0\0\0\0|\xfd\xff\xff\x05\0\0\0agent\0\0\0\x0b\0\0\0responsible\0\x04\0\0\0item\0\0\0\0\x13\0\0\0was_associated_with\0\xe8\xfd\xff\xff\x18\0\0\0\x0c\0\0\0\0\0\0\x0c8\0\0\0\x01\0\0\0\x08\0\0\0\xdc\xfd\xff\xff\x08\xfe\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\0\x05\x0c\0\0\0\0\0\0\0\xf8\xfd\xff\xff\x04\0\0\0item\0\0\0\0\x0f\0\0\0was_informed_by\0D\xfe\xff\xff\x18\0\0\0\x0c\0\0\0\0\0\0\x0c8\0\0\0\x01\0\0\0\x08\0\0\08\xfe\xff\xffd\xfe\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\0\x05\x0c\0\0\0\0\0\0\0T\xfe\xff\xff\x04\0\0\0item\0\0\0\0\t\0\0\0generated\0\0\0\x9c\xfe\xff\xff\x18\0\0\0\x0c\0\0\0\0\0\0\x0c8\0\0\0\x01\0\0\0\x08\0\0\0\x90\xfe\xff\xff\xbc\xfe\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\0\x05\x0c\0\0\0\0\0\0\0\xac\xfe\xff\xff\x04\0\0\0item\0\0\0\0\x04\0\0\0used\0\0\0\0\x88\xff\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\x01\n\x1c\0\0\0\0\0\0\0\xc8\xff\xff\xff\x08\0\0\0\0\0\x03\0\x03\0\0\0UTC\0\x05\0\0\0ended\0\0\0\xc0\xff\xff\xff\x1c\0\0\0\x0c\0\0\0\0\0\x01\n$\0\0\0\0\0\0\0\x08\0\x0c\0\n\0\x04\0\x08\0\0\0\x08\0\0\0\0\0\x03\0\x03\0\0\0UTC\0\x07\0\0\0started\0\x10\0\x14\0\x10\0\x0e\0\x0f\0\x04\0\0\0\x08\0\x10\0\0\0\x14\0\0\0\x0c\0\0\0\0\0\x01\x05\x0c\0\0\0\0\0\0\0h\xff\xff\xff\x10\0\0\0batchIDAttribute\0\0\0\0\xac\xff\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\0\x05\x0c\0\0\0\0\0\0\0\x9c\xff\xff\xff\x02\0\0\0id\0\0\xd0\xff\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\0\x05\x0c\0\0\0\0\0\0\0\xc0\xff\xff\xff\x0e\0\0\0namespace_uuid\0\0\x10\0\x14\0\x10\0\0\0\x0f\0\x04\0\0\0\x08\0\x10\0\0\0\x18\0\0\0\x0c\0\0\0\0\0\0\x05\x10\0\0\0\0\0\0\0\x04\0\x04\0\x04\0\0\0\x0e\0\0\0namespace_name\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
          flight_descriptor: Some(
              FlightDescriptor {
                  r#type: Path,
                  cmd: b"",
                  path: [
                      "Activity",
                      "ItemManufacturedActivity",
                  ],
              },
          ),
          endpoint: [
              FlightEndpoint {
                  ticket: Some(
                      Ticket {
                          ticket: b"{\"term\":\"Activity\",\"descriptor_path\":[\"Activity\",\"ItemManufacturedActivity\"],\"typ\":\"ItemManufacturedActivity\",\"start\":0,\"count\":10}",
                      },
                  ),
                  location: [],
              },
              FlightEndpoint {
                  ticket: Some(
                      Ticket {
                          ticket: b"{\"term\":\"Activity\",\"descriptor_path\":[\"Activity\",\"ItemManufacturedActivity\"],\"typ\":\"ItemManufacturedActivity\",\"start\":10,\"count\":10}",
                      },
                  ),
                  location: [],
              },
              FlightEndpoint {
                  ticket: Some(
                      Ticket {
                          ticket: b"{\"term\":\"Activity\",\"descriptor_path\":[\"Activity\",\"ItemManufacturedActivity\"],\"typ\":\"ItemManufacturedActivity\",\"start\":20,\"count\":2}",
                      },
                  ),
                  location: [],
              },
          ],
          total_records: 22,
          total_bytes: -1,
          ordered: false,
      },
      FlightInfo {
          schema: b"\xff\xff\xff\xff8\x03\0\0\x10\0\0\0\0\0\n\0\x0c\0\n\0\t\0\x04\0\n\0\0\0\x10\0\0\0\0\x01\x04\0\x08\0\x08\0\0\0\x04\0\x08\0\0\0\x04\0\0\0\x07\0\0\0\x9c\x02\0\0X\x02\0\00\x02\0\0\xf4\x01\0\0\xac\x01\0\0\xc0\0\0\0\x04\0\0\0\x90\xfd\xff\xff\x18\0\0\0\x0c\0\0\0\0\0\0\x0c\x90\0\0\0\x01\0\0\0\x08\0\0\0\x84\xfd\xff\xff\xb0\xfd\xff\xff\x1c\0\0\0\x0c\0\0\0\0\0\0\rd\0\0\0\x02\0\0\04\0\0\0\x08\0\0\0\xa8\xfd\xff\xffp\xfe\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\x01\x05\x0c\0\0\0\0\0\0\0\xc4\xfd\xff\xff\x04\0\0\0role\0\0\0\0\xfc\xfd\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\0\x05\x0c\0\0\0\0\0\0\0\xec\xfd\xff\xff\x06\0\0\0entity\0\0\x04\0\0\0item\0\0\0\0\x11\0\0\0was_attributed_to\0\0\0H\xfe\xff\xff\x18\0\0\0\x0c\0\0\0\0\0\0\x0c\xc0\0\0\0\x01\0\0\0\x08\0\0\0<\xfe\xff\xffh\xfe\xff\xff \0\0\0\x0c\0\0\0\0\0\0\r\x94\0\0\0\x03\0\0\0d\0\0\04\0\0\0\x08\0\0\0d\xfe\xff\xff,\xff\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\x01\x05\x0c\0\0\0\0\0\0\0\x80\xfe\xff\xff\x04\0\0\0role\0\0\0\0\xb8\xfe\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\0\x05\x0c\0\0\0\0\0\0\0\xa8\xfe\xff\xff\x08\0\0\0activity\0\0\0\0\xe4\xfe\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\0\x05\x0c\0\0\0\0\0\0\0\xd4\xfe\xff\xff\x05\0\0\0agent\0\0\0\x04\0\0\0item\0\0\0\0\x12\0\0\0acted_on_behalf_of\0\0\xcc\xff\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\x01\x05\x0c\0\0\0\0\0\0\0 \xff\xff\xff\x11\0\0\0locationAttribute\0\0\0\x10\0\x14\0\x10\0\x0e\0\x0f\0\x04\0\0\0\x08\0\x10\0\0\0\x14\0\0\0\x0c\0\0\0\0\0\x01\x05\x0c\0\0\0\0\0\0\0d\xff\xff\xff\x14\0\0\0companyNameAttribute\0\0\0\0\xac\xff\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\0\x05\x0c\0\0\0\0\0\0\0\x9c\xff\xff\xff\x02\0\0\0id\0\0\xd0\xff\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\0\x05\x0c\0\0\0\0\0\0\0\xc0\xff\xff\xff\x0e\0\0\0namespace_uuid\0\0\x10\0\x14\0\x10\0\0\0\x0f\0\x04\0\0\0\x08\0\x10\0\0\0\x18\0\0\0\x0c\0\0\0\0\0\0\x05\x10\0\0\0\0\0\0\0\x04\0\x04\0\x04\0\0\0\x0e\0\0\0namespace_name\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
          flight_descriptor: Some(
              FlightDescriptor {
                  r#type: Path,
                  cmd: b"",
                  path: [
                      "Agent",
                      "ContractorAgent",
                  ],
              },
          ),
          endpoint: [
              FlightEndpoint {
                  ticket: Some(
                      Ticket {
                          ticket: b"{\"term\":\"Agent\",\"descriptor_path\":[\"Agent\",\"ContractorAgent\"],\"typ\":\"ContractorAgent\",\"start\":0,\"count\":10}",
                      },
                  ),
                  location: [],
              },
              FlightEndpoint {
                  ticket: Some(
                      Ticket {
                          ticket: b"{\"term\":\"Agent\",\"descriptor_path\":[\"Agent\",\"ContractorAgent\"],\"typ\":\"ContractorAgent\",\"start\":10,\"count\":10}",
                      },
                  ),
                  location: [],
              },
              FlightEndpoint {
                  ticket: Some(
                      Ticket {
                          ticket: b"{\"term\":\"Agent\",\"descriptor_path\":[\"Agent\",\"ContractorAgent\"],\"typ\":\"ContractorAgent\",\"start\":20,\"count\":2}",
                      },
                  ),
                  location: [],
              },
          ],
          total_records: 22,
          total_bytes: -1,
          ordered: false,
      },
      FlightInfo {
          schema: b"\xff\xff\xff\xff8\x05\0\0\x10\0\0\0\0\0\n\0\x0c\0\n\0\t\0\x04\0\n\0\0\0\x10\0\0\0\0\x01\x04\0\x08\0\x08\0\0\0\x04\0\x08\0\0\0\x04\0\0\0\n\0\0\0\xcc\x04\0\0\x88\x04\0\0`\x04\0\0,\x04\0\0\xb8\x03\0\0\xfc\x02\0\0<\x02\0\0|\x01\0\0\xc0\0\0\0\x04\0\0\0l\xfb\xff\xff\x18\0\0\0\x0c\0\0\0\0\0\0\x0c\x94\0\0\0\x01\0\0\0\x08\0\0\0`\xfb\xff\xff\x8c\xfb\xff\xff\x1c\0\0\0\x0c\0\0\0\0\0\0\rh\0\0\0\x02\0\0\08\0\0\0\x08\0\0\0\x84\xfb\xff\xff\xb0\xfb\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\0\x05\x0c\0\0\0\0\0\0\0\xa0\xfb\xff\xff\x08\0\0\0activity\0\0\0\0\xdc\xfb\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\0\x05\x0c\0\0\0\0\0\0\0\xcc\xfb\xff\xff\x06\0\0\0source\0\0\x04\0\0\0item\0\0\0\0\x0f\0\0\0was_revision_of\0$\xfc\xff\xff\x18\0\0\0\x0c\0\0\0\0\0\0\x0c\x94\0\0\0\x01\0\0\0\x08\0\0\0\x18\xfc\xff\xffD\xfc\xff\xff\x1c\0\0\0\x0c\0\0\0\0\0\0\rh\0\0\0\x02\0\0\08\0\0\0\x08\0\0\0<\xfc\xff\xffh\xfc\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\0\x05\x0c\0\0\0\0\0\0\0X\xfc\xff\xff\x08\0\0\0activity\0\0\0\0\x94\xfc\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\0\x05\x0c\0\0\0\0\0\0\0\x84\xfc\xff\xff\x06\0\0\0source\0\0\x04\0\0\0item\0\0\0\0\x0f\0\0\0was_quoted_from\0\xdc\xfc\xff\xff\x18\0\0\0\x0c\0\0\0\0\0\0\x0c\x94\0\0\0\x01\0\0\0\x08\0\0\0\xd0\xfc\xff\xff\xfc\xfc\xff\xff\x1c\0\0\0\x0c\0\0\0\0\0\0\rh\0\0\0\x02\0\0\08\0\0\0\x08\0\0\0\xf4\xfc\xff\xff \xfd\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\0\x05\x0c\0\0\0\0\0\0\0\x10\xfd\xff\xff\x08\0\0\0activity\0\0\0\0L\xfd\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\0\x05\x0c\0\0\0\0\0\0\0<\xfd\xff\xff\x06\0\0\0source\0\0\x04\0\0\0item\0\0\0\0\x12\0\0\0had_primary_source\0\0\x98\xfd\xff\xff\x18\0\0\0\x0c\0\0\0\0\0\0\x0c\x94\0\0\0\x01\0\0\0\x08\0\0\0\x8c\xfd\xff\xff\xb8\xfd\xff\xff\x1c\0\0\0\x0c\0\0\0\0\0\0\rh\0\0\0\x02\0\0\08\0\0\0\x08\0\0\0\xb0\xfd\xff\xff\xdc\xfd\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\0\x05\x0c\0\0\0\0\0\0\0\xcc\xfd\xff\xff\x08\0\0\0activity\0\0\0\0\x08\xfe\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\0\x05\x0c\0\0\0\0\0\0\0\xf8\xfd\xff\xff\x06\0\0\0source\0\0\x04\0\0\0item\0\0\0\0\x10\0\0\0was_derived_from\0\0\0\0T\xfe\xff\xff\x18\0\0\0\x0c\0\0\0\0\0\0\x0c\x90\0\0\0\x01\0\0\0\x08\0\0\0H\xfe\xff\xfft\xfe\xff\xff\x1c\0\0\0\x0c\0\0\0\0\0\0\rd\0\0\0\x02\0\0\04\0\0\0\x08\0\0\0l\xfe\xff\xff,\xff\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\x01\x05\x0c\0\0\0\0\0\0\0\x88\xfe\xff\xff\x04\0\0\0role\0\0\0\0\xc0\xfe\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\0\x05\x0c\0\0\0\0\0\0\0\xb0\xfe\xff\xff\x05\0\0\0agent\0\0\0\x04\0\0\0item\0\0\0\0\x11\0\0\0was_attributed_to\0\0\0\x0c\xff\xff\xff\x18\0\0\0\x0c\0\0\0\0\0\0\x0c8\0\0\0\x01\0\0\0\x08\0\0\0\0\xff\xff\xff,\xff\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\0\x05\x0c\0\0\0\0\0\0\0\x1c\xff\xff\xff\x04\0\0\0item\0\0\0\0\x10\0\0\0was_generated_by\0\0\0\0\x10\0\x14\0\x10\0\x0e\0\x0f\0\x04\0\0\0\x08\0\x10\0\0\0\x14\0\0\0\x0c\0\0\0\0\0\x01\x05\x0c\0\0\0\0\0\0\0l\xff\xff\xff\x0f\0\0\0certIDAttribute\0\xac\xff\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\0\x05\x0c\0\0\0\0\0\0\0\x9c\xff\xff\xff\x02\0\0\0id\0\0\xd0\xff\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\0\x05\x0c\0\0\0\0\0\0\0\xc0\xff\xff\xff\x0e\0\0\0namespace_uuid\0\0\x10\0\x14\0\x10\0\0\0\x0f\0\x04\0\0\0\x08\0\x10\0\0\0\x18\0\0\0\x0c\0\0\0\0\0\0\x05\x10\0\0\0\0\0\0\0\x04\0\x04\0\x04\0\0\0\x0e\0\0\0namespace_name\0\0\0\0\0\0\0\0\0\0",
          flight_descriptor: Some(
              FlightDescriptor {
                  r#type: Path,
                  cmd: b"",
                  path: [
                      "Entity",
                      "CertificateEntity",
                  ],
              },
          ),
          endpoint: [
              FlightEndpoint {
                  ticket: Some(
                      Ticket {
                          ticket: b"{\"term\":\"Entity\",\"descriptor_path\":[\"Entity\",\"CertificateEntity\"],\"typ\":\"CertificateEntity\",\"start\":0,\"count\":10}",
                      },
                  ),
                  location: [],
              },
              FlightEndpoint {
                  ticket: Some(
                      Ticket {
                          ticket: b"{\"term\":\"Entity\",\"descriptor_path\":[\"Entity\",\"CertificateEntity\"],\"typ\":\"CertificateEntity\",\"start\":10,\"count\":10}",
                      },
                  ),
                  location: [],
              },
              FlightEndpoint {
                  ticket: Some(
                      Ticket {
                          ticket: b"{\"term\":\"Entity\",\"descriptor_path\":[\"Entity\",\"CertificateEntity\"],\"typ\":\"CertificateEntity\",\"start\":20,\"count\":2}",
                      },
                  ),
                  location: [],
              },
          ],
          total_records: 22,
          total_bytes: -1,
          ordered: false,
      },
      FlightInfo {
          schema: b"\xff\xff\xff\xff8\x05\0\0\x10\0\0\0\0\0\n\0\x0c\0\n\0\t\0\x04\0\n\0\0\0\x10\0\0\0\0\x01\x04\0\x08\0\x08\0\0\0\x04\0\x08\0\0\0\x04\0\0\0\n\0\0\0\xcc\x04\0\0\x88\x04\0\0`\x04\0\0,\x04\0\0\xb8\x03\0\0\xfc\x02\0\0<\x02\0\0|\x01\0\0\xc0\0\0\0\x04\0\0\0l\xfb\xff\xff\x18\0\0\0\x0c\0\0\0\0\0\0\x0c\x94\0\0\0\x01\0\0\0\x08\0\0\0`\xfb\xff\xff\x8c\xfb\xff\xff\x1c\0\0\0\x0c\0\0\0\0\0\0\rh\0\0\0\x02\0\0\08\0\0\0\x08\0\0\0\x84\xfb\xff\xff\xb0\xfb\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\0\x05\x0c\0\0\0\0\0\0\0\xa0\xfb\xff\xff\x08\0\0\0activity\0\0\0\0\xdc\xfb\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\0\x05\x0c\0\0\0\0\0\0\0\xcc\xfb\xff\xff\x06\0\0\0source\0\0\x04\0\0\0item\0\0\0\0\x0f\0\0\0was_revision_of\0$\xfc\xff\xff\x18\0\0\0\x0c\0\0\0\0\0\0\x0c\x94\0\0\0\x01\0\0\0\x08\0\0\0\x18\xfc\xff\xffD\xfc\xff\xff\x1c\0\0\0\x0c\0\0\0\0\0\0\rh\0\0\0\x02\0\0\08\0\0\0\x08\0\0\0<\xfc\xff\xffh\xfc\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\0\x05\x0c\0\0\0\0\0\0\0X\xfc\xff\xff\x08\0\0\0activity\0\0\0\0\x94\xfc\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\0\x05\x0c\0\0\0\0\0\0\0\x84\xfc\xff\xff\x06\0\0\0source\0\0\x04\0\0\0item\0\0\0\0\x0f\0\0\0was_quoted_from\0\xdc\xfc\xff\xff\x18\0\0\0\x0c\0\0\0\0\0\0\x0c\x94\0\0\0\x01\0\0\0\x08\0\0\0\xd0\xfc\xff\xff\xfc\xfc\xff\xff\x1c\0\0\0\x0c\0\0\0\0\0\0\rh\0\0\0\x02\0\0\08\0\0\0\x08\0\0\0\xf4\xfc\xff\xff \xfd\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\0\x05\x0c\0\0\0\0\0\0\0\x10\xfd\xff\xff\x08\0\0\0activity\0\0\0\0L\xfd\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\0\x05\x0c\0\0\0\0\0\0\0<\xfd\xff\xff\x06\0\0\0source\0\0\x04\0\0\0item\0\0\0\0\x12\0\0\0had_primary_source\0\0\x98\xfd\xff\xff\x18\0\0\0\x0c\0\0\0\0\0\0\x0c\x94\0\0\0\x01\0\0\0\x08\0\0\0\x8c\xfd\xff\xff\xb8\xfd\xff\xff\x1c\0\0\0\x0c\0\0\0\0\0\0\rh\0\0\0\x02\0\0\08\0\0\0\x08\0\0\0\xb0\xfd\xff\xff\xdc\xfd\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\0\x05\x0c\0\0\0\0\0\0\0\xcc\xfd\xff\xff\x08\0\0\0activity\0\0\0\0\x08\xfe\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\0\x05\x0c\0\0\0\0\0\0\0\xf8\xfd\xff\xff\x06\0\0\0source\0\0\x04\0\0\0item\0\0\0\0\x10\0\0\0was_derived_from\0\0\0\0T\xfe\xff\xff\x18\0\0\0\x0c\0\0\0\0\0\0\x0c\x90\0\0\0\x01\0\0\0\x08\0\0\0H\xfe\xff\xfft\xfe\xff\xff\x1c\0\0\0\x0c\0\0\0\0\0\0\rd\0\0\0\x02\0\0\04\0\0\0\x08\0\0\0l\xfe\xff\xff,\xff\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\x01\x05\x0c\0\0\0\0\0\0\0\x88\xfe\xff\xff\x04\0\0\0role\0\0\0\0\xc0\xfe\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\0\x05\x0c\0\0\0\0\0\0\0\xb0\xfe\xff\xff\x05\0\0\0agent\0\0\0\x04\0\0\0item\0\0\0\0\x11\0\0\0was_attributed_to\0\0\0\x0c\xff\xff\xff\x18\0\0\0\x0c\0\0\0\0\0\0\x0c8\0\0\0\x01\0\0\0\x08\0\0\0\0\xff\xff\xff,\xff\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\0\x05\x0c\0\0\0\0\0\0\0\x1c\xff\xff\xff\x04\0\0\0item\0\0\0\0\x10\0\0\0was_generated_by\0\0\0\0\x10\0\x14\0\x10\0\x0e\0\x0f\0\x04\0\0\0\x08\0\x10\0\0\0\x14\0\0\0\x0c\0\0\0\0\0\x01\x05\x0c\0\0\0\0\0\0\0l\xff\xff\xff\x0f\0\0\0partIDAttribute\0\xac\xff\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\0\x05\x0c\0\0\0\0\0\0\0\x9c\xff\xff\xff\x02\0\0\0id\0\0\xd0\xff\xff\xff\x14\0\0\0\x0c\0\0\0\0\0\0\x05\x0c\0\0\0\0\0\0\0\xc0\xff\xff\xff\x0e\0\0\0namespace_uuid\0\0\x10\0\x14\0\x10\0\0\0\x0f\0\x04\0\0\0\x08\0\x10\0\0\0\x18\0\0\0\x0c\0\0\0\0\0\0\x05\x10\0\0\0\0\0\0\0\x04\0\x04\0\x04\0\0\0\x0e\0\0\0namespace_name\0\0\0\0\0\0\0\0\0\0",
          flight_descriptor: Some(
              FlightDescriptor {
                  r#type: Path,
                  cmd: b"",
                  path: [
                      "Entity",
                      "ItemEntity",
                  ],
              },
          ),
          endpoint: [],
          total_records: 0,
          total_bytes: -1,
          ordered: false,
      },
  ]
  "###);
	}

	#[tokio::test]
	async fn get_and_put_are_isomorphic() {
		chronicle_telemetry::telemetry( chronicle_telemetry::ConsoleLogging::Pretty);
		let domain = create_test_domain_def();
		let (mut client, mut api) = setup_test_environment(&domain).await.unwrap();
		cache_domain_schemas(&domain);
		put_test_data(8, &mut client, &mut api).await.unwrap();

		tokio::time::sleep(Duration::from_secs(2)).await;

		let flights = stable_sorted_flight_info(&mut client).await.unwrap();
		let flight_data = load_flights(&flights, &mut client).await.unwrap();

		let mut decoded_flight_data = vec![];

		for flight_data in flight_data.into_iter() {
			decoded_flight_data
				.push(decode_flight_data(flight_data).await.expect("Failed to decode flight data"));
		}

		let json_arrays = decoded_flight_data
			.into_iter()
			.map(|batch| {
				let batch_refs: Vec<&RecordBatch> = batch.iter().collect();
				arrow::json::writer::record_batches_to_json_rows(&batch_refs)
					.expect("Failed to convert record batches to JSON")
			})
			.collect::<Vec<_>>();

		insta::assert_debug_snapshot!(json_arrays, @r###"
  [
      [
          {
              "ended": String("2022-01-02T00:00:00Z"),
              "generated": Array [
                  String("entity-0"),
                  String("entity-1"),
              ],
              "id": String("ItemManufacturedActivity-0"),
              "namespace_name": String("default"),
              "namespace_uuid": String("00000000-0000-0000-0000-000000000000"),
              "started": String("2022-01-01T00:00:00Z"),
              "used": Array [
                  String("entity-0"),
                  String("entity-1"),
              ],
              "was_associated_with": Array [
                  Object {
                      "delegated": Array [],
                      "responsible": Object {
                          "agent": String("agent-0"),
                          "role": String("ROLE_TYPE"),
                      },
                  },
              ],
              "was_informed_by": Array [],
          },
          {
              "ended": String("2022-01-02T00:00:00Z"),
              "generated": Array [
                  String("entity-1"),
                  String("entity-2"),
              ],
              "id": String("ItemManufacturedActivity-1"),
              "namespace_name": String("default"),
              "namespace_uuid": String("00000000-0000-0000-0000-000000000000"),
              "started": String("2022-01-01T00:00:00Z"),
              "used": Array [
                  String("entity-1"),
                  String("entity-2"),
              ],
              "was_associated_with": Array [
                  Object {
                      "delegated": Array [],
                      "responsible": Object {
                          "agent": String("agent-1"),
                          "role": String("ROLE_TYPE"),
                      },
                  },
              ],
              "was_informed_by": Array [],
          },
          {
              "ended": String("2022-01-02T00:00:00Z"),
              "generated": Array [
                  String("entity-2"),
                  String("entity-3"),
              ],
              "id": String("ItemManufacturedActivity-2"),
              "namespace_name": String("default"),
              "namespace_uuid": String("00000000-0000-0000-0000-000000000000"),
              "started": String("2022-01-01T00:00:00Z"),
              "used": Array [
                  String("entity-2"),
                  String("entity-3"),
              ],
              "was_associated_with": Array [
                  Object {
                      "delegated": Array [],
                      "responsible": Object {
                          "agent": String("agent-2"),
                          "role": String("ROLE_TYPE"),
                      },
                  },
              ],
              "was_informed_by": Array [],
          },
          {
              "ended": String("2022-01-02T00:00:00Z"),
              "generated": Array [
                  String("entity-3"),
                  String("entity-4"),
              ],
              "id": String("ItemManufacturedActivity-3"),
              "namespace_name": String("default"),
              "namespace_uuid": String("00000000-0000-0000-0000-000000000000"),
              "started": String("2022-01-01T00:00:00Z"),
              "used": Array [
                  String("entity-3"),
                  String("entity-4"),
              ],
              "was_associated_with": Array [
                  Object {
                      "delegated": Array [],
                      "responsible": Object {
                          "agent": String("agent-3"),
                          "role": String("ROLE_TYPE"),
                      },
                  },
              ],
              "was_informed_by": Array [],
          },
          {
              "ended": String("2022-01-02T00:00:00Z"),
              "generated": Array [
                  String("entity-4"),
                  String("entity-5"),
              ],
              "id": String("ItemManufacturedActivity-4"),
              "namespace_name": String("default"),
              "namespace_uuid": String("00000000-0000-0000-0000-000000000000"),
              "started": String("2022-01-01T00:00:00Z"),
              "used": Array [
                  String("entity-4"),
                  String("entity-5"),
              ],
              "was_associated_with": Array [
                  Object {
                      "delegated": Array [],
                      "responsible": Object {
                          "agent": String("agent-4"),
                          "role": String("ROLE_TYPE"),
                      },
                  },
              ],
              "was_informed_by": Array [],
          },
          {
              "ended": String("2022-01-02T00:00:00Z"),
              "generated": Array [
                  String("entity-5"),
                  String("entity-6"),
              ],
              "id": String("ItemManufacturedActivity-5"),
              "namespace_name": String("default"),
              "namespace_uuid": String("00000000-0000-0000-0000-000000000000"),
              "started": String("2022-01-01T00:00:00Z"),
              "used": Array [
                  String("entity-5"),
                  String("entity-6"),
              ],
              "was_associated_with": Array [
                  Object {
                      "delegated": Array [],
                      "responsible": Object {
                          "agent": String("agent-5"),
                          "role": String("ROLE_TYPE"),
                      },
                  },
              ],
              "was_informed_by": Array [],
          },
          {
              "ended": String("2022-01-02T00:00:00Z"),
              "generated": Array [
                  String("entity-6"),
                  String("entity-7"),
              ],
              "id": String("ItemManufacturedActivity-6"),
              "namespace_name": String("default"),
              "namespace_uuid": String("00000000-0000-0000-0000-000000000000"),
              "started": String("2022-01-01T00:00:00Z"),
              "used": Array [
                  String("entity-6"),
                  String("entity-7"),
              ],
              "was_associated_with": Array [
                  Object {
                      "delegated": Array [],
                      "responsible": Object {
                          "agent": String("agent-6"),
                          "role": String("ROLE_TYPE"),
                      },
                  },
              ],
              "was_informed_by": Array [],
          },
          {
              "ended": String("2022-01-02T00:00:00Z"),
              "generated": Array [
                  String("entity-7"),
                  String("entity-8"),
              ],
              "id": String("ItemManufacturedActivity-7"),
              "namespace_name": String("default"),
              "namespace_uuid": String("00000000-0000-0000-0000-000000000000"),
              "started": String("2022-01-01T00:00:00Z"),
              "used": Array [
                  String("entity-7"),
                  String("entity-8"),
              ],
              "was_associated_with": Array [
                  Object {
                      "delegated": Array [],
                      "responsible": Object {
                          "agent": String("agent-7"),
                          "role": String("ROLE_TYPE"),
                      },
                  },
              ],
              "was_informed_by": Array [],
          },
      ],
      [
          {
              "acted_on_behalf_of": Array [
                  Object {
                      "activity": String("activity-0"),
                      "agent": String("agent-0"),
                      "role": String("DELEGATED_CERTIFIER"),
                  },
              ],
              "id": String("ContractorAgent-0"),
              "namespace_name": String("default"),
              "namespace_uuid": String("00000000-0000-0000-0000-000000000000"),
              "was_attributed_to": Array [
                  Object {
                      "entity": String("entity-0"),
                      "role": String("UNSPECIFIED_INTERACTION"),
                  },
              ],
          },
          {
              "acted_on_behalf_of": Array [
                  Object {
                      "activity": String("activity-1"),
                      "agent": String("agent-1"),
                      "role": String("DELEGATED_CERTIFIER"),
                  },
              ],
              "id": String("ContractorAgent-1"),
              "namespace_name": String("default"),
              "namespace_uuid": String("00000000-0000-0000-0000-000000000000"),
              "was_attributed_to": Array [
                  Object {
                      "entity": String("entity-1"),
                      "role": String("UNSPECIFIED_INTERACTION"),
                  },
              ],
          },
          {
              "acted_on_behalf_of": Array [
                  Object {
                      "activity": String("activity-2"),
                      "agent": String("agent-2"),
                      "role": String("DELEGATED_CERTIFIER"),
                  },
              ],
              "id": String("ContractorAgent-2"),
              "namespace_name": String("default"),
              "namespace_uuid": String("00000000-0000-0000-0000-000000000000"),
              "was_attributed_to": Array [
                  Object {
                      "entity": String("entity-2"),
                      "role": String("UNSPECIFIED_INTERACTION"),
                  },
              ],
          },
          {
              "acted_on_behalf_of": Array [
                  Object {
                      "activity": String("activity-3"),
                      "agent": String("agent-3"),
                      "role": String("DELEGATED_CERTIFIER"),
                  },
              ],
              "id": String("ContractorAgent-3"),
              "namespace_name": String("default"),
              "namespace_uuid": String("00000000-0000-0000-0000-000000000000"),
              "was_attributed_to": Array [
                  Object {
                      "entity": String("entity-3"),
                      "role": String("UNSPECIFIED_INTERACTION"),
                  },
              ],
          },
          {
              "acted_on_behalf_of": Array [
                  Object {
                      "activity": String("activity-4"),
                      "agent": String("agent-4"),
                      "role": String("DELEGATED_CERTIFIER"),
                  },
              ],
              "id": String("ContractorAgent-4"),
              "namespace_name": String("default"),
              "namespace_uuid": String("00000000-0000-0000-0000-000000000000"),
              "was_attributed_to": Array [
                  Object {
                      "entity": String("entity-4"),
                      "role": String("UNSPECIFIED_INTERACTION"),
                  },
              ],
          },
          {
              "acted_on_behalf_of": Array [
                  Object {
                      "activity": String("activity-5"),
                      "agent": String("agent-5"),
                      "role": String("DELEGATED_CERTIFIER"),
                  },
              ],
              "id": String("ContractorAgent-5"),
              "namespace_name": String("default"),
              "namespace_uuid": String("00000000-0000-0000-0000-000000000000"),
              "was_attributed_to": Array [
                  Object {
                      "entity": String("entity-5"),
                      "role": String("UNSPECIFIED_INTERACTION"),
                  },
              ],
          },
          {
              "acted_on_behalf_of": Array [
                  Object {
                      "activity": String("activity-6"),
                      "agent": String("agent-6"),
                      "role": String("DELEGATED_CERTIFIER"),
                  },
              ],
              "id": String("ContractorAgent-6"),
              "namespace_name": String("default"),
              "namespace_uuid": String("00000000-0000-0000-0000-000000000000"),
              "was_attributed_to": Array [
                  Object {
                      "entity": String("entity-6"),
                      "role": String("UNSPECIFIED_INTERACTION"),
                  },
              ],
          },
          {
              "acted_on_behalf_of": Array [
                  Object {
                      "activity": String("activity-7"),
                      "agent": String("agent-7"),
                      "role": String("DELEGATED_CERTIFIER"),
                  },
              ],
              "id": String("ContractorAgent-7"),
              "namespace_name": String("default"),
              "namespace_uuid": String("00000000-0000-0000-0000-000000000000"),
              "was_attributed_to": Array [
                  Object {
                      "entity": String("entity-7"),
                      "role": String("UNSPECIFIED_INTERACTION"),
                  },
              ],
          },
      ],
      [
          {
              "certIDAttribute": String("certIDAttribute-value"),
              "had_primary_source": Array [
                  Object {
                      "activity": String("activity-ps-0"),
                      "source": String("CertificateEntity-0"),
                  },
              ],
              "id": String("CertificateEntity-0"),
              "namespace_name": String("default"),
              "namespace_uuid": String("00000000-0000-0000-0000-000000000000"),
              "was_attributed_to": Array [
                  Object {
                      "agent": String("agent-0"),
                      "role": String("CERTIFIER"),
                  },
              ],
              "was_derived_from": Array [
                  Object {
                      "activity": String("activity-d-0"),
                      "source": String("CertificateEntity-0"),
                  },
              ],
              "was_generated_by": Array [
                  String("activity-0"),
                  String("activity-1"),
              ],
              "was_quoted_from": Array [
                  Object {
                      "activity": String("activity-q-0"),
                      "source": String("CertificateEntity-0"),
                  },
              ],
              "was_revision_of": Array [
                  Object {
                      "activity": String("activity-r-0"),
                      "source": String("CertificateEntity-0"),
                  },
              ],
          },
          {
              "certIDAttribute": String("certIDAttribute-value"),
              "had_primary_source": Array [
                  Object {
                      "activity": String("activity-ps-1"),
                      "source": String("CertificateEntity-1"),
                  },
              ],
              "id": String("CertificateEntity-1"),
              "namespace_name": String("default"),
              "namespace_uuid": String("00000000-0000-0000-0000-000000000000"),
              "was_attributed_to": Array [
                  Object {
                      "agent": String("agent-1"),
                      "role": String("CERTIFIER"),
                  },
                  Object {
                      "agent": String("agent-1"),
                      "role": String("MANUFACTURER"),
                  },
              ],
              "was_derived_from": Array [
                  Object {
                      "activity": String("activity-d-1"),
                      "source": String("CertificateEntity-1"),
                  },
              ],
              "was_generated_by": Array [
                  String("activity-1"),
                  String("activity-2"),
              ],
              "was_quoted_from": Array [
                  Object {
                      "activity": String("activity-q-1"),
                      "source": String("CertificateEntity-1"),
                  },
              ],
              "was_revision_of": Array [
                  Object {
                      "activity": String("activity-r-1"),
                      "source": String("CertificateEntity-1"),
                  },
              ],
          },
          {
              "certIDAttribute": String("certIDAttribute-value"),
              "had_primary_source": Array [
                  Object {
                      "activity": String("activity-ps-2"),
                      "source": String("CertificateEntity-2"),
                  },
              ],
              "id": String("CertificateEntity-2"),
              "namespace_name": String("default"),
              "namespace_uuid": String("00000000-0000-0000-0000-000000000000"),
              "was_attributed_to": Array [
                  Object {
                      "agent": String("agent-2"),
                      "role": String("CERTIFIER"),
                  },
                  Object {
                      "agent": String("agent-2"),
                      "role": String("MANUFACTURER"),
                  },
              ],
              "was_derived_from": Array [
                  Object {
                      "activity": String("activity-d-2"),
                      "source": String("CertificateEntity-2"),
                  },
              ],
              "was_generated_by": Array [
                  String("activity-2"),
                  String("activity-3"),
              ],
              "was_quoted_from": Array [
                  Object {
                      "activity": String("activity-q-2"),
                      "source": String("CertificateEntity-2"),
                  },
              ],
              "was_revision_of": Array [
                  Object {
                      "activity": String("activity-r-2"),
                      "source": String("CertificateEntity-2"),
                  },
              ],
          },
          {
              "certIDAttribute": String("certIDAttribute-value"),
              "had_primary_source": Array [
                  Object {
                      "activity": String("activity-ps-3"),
                      "source": String("CertificateEntity-3"),
                  },
              ],
              "id": String("CertificateEntity-3"),
              "namespace_name": String("default"),
              "namespace_uuid": String("00000000-0000-0000-0000-000000000000"),
              "was_attributed_to": Array [
                  Object {
                      "agent": String("agent-3"),
                      "role": String("CERTIFIER"),
                  },
                  Object {
                      "agent": String("agent-3"),
                      "role": String("MANUFACTURER"),
                  },
              ],
              "was_derived_from": Array [
                  Object {
                      "activity": String("activity-d-3"),
                      "source": String("CertificateEntity-3"),
                  },
              ],
              "was_generated_by": Array [
                  String("activity-3"),
                  String("activity-4"),
              ],
              "was_quoted_from": Array [
                  Object {
                      "activity": String("activity-q-3"),
                      "source": String("CertificateEntity-3"),
                  },
              ],
              "was_revision_of": Array [
                  Object {
                      "activity": String("activity-r-3"),
                      "source": String("CertificateEntity-3"),
                  },
              ],
          },
          {
              "certIDAttribute": String("certIDAttribute-value"),
              "had_primary_source": Array [
                  Object {
                      "activity": String("activity-ps-4"),
                      "source": String("CertificateEntity-4"),
                  },
              ],
              "id": String("CertificateEntity-4"),
              "namespace_name": String("default"),
              "namespace_uuid": String("00000000-0000-0000-0000-000000000000"),
              "was_attributed_to": Array [
                  Object {
                      "agent": String("agent-4"),
                      "role": String("CERTIFIER"),
                  },
                  Object {
                      "agent": String("agent-4"),
                      "role": String("MANUFACTURER"),
                  },
              ],
              "was_derived_from": Array [
                  Object {
                      "activity": String("activity-d-4"),
                      "source": String("CertificateEntity-4"),
                  },
              ],
              "was_generated_by": Array [
                  String("activity-4"),
                  String("activity-5"),
              ],
              "was_quoted_from": Array [
                  Object {
                      "activity": String("activity-q-4"),
                      "source": String("CertificateEntity-4"),
                  },
              ],
              "was_revision_of": Array [
                  Object {
                      "activity": String("activity-r-4"),
                      "source": String("CertificateEntity-4"),
                  },
              ],
          },
          {
              "certIDAttribute": String("certIDAttribute-value"),
              "had_primary_source": Array [
                  Object {
                      "activity": String("activity-ps-5"),
                      "source": String("CertificateEntity-5"),
                  },
              ],
              "id": String("CertificateEntity-5"),
              "namespace_name": String("default"),
              "namespace_uuid": String("00000000-0000-0000-0000-000000000000"),
              "was_attributed_to": Array [
                  Object {
                      "agent": String("agent-5"),
                      "role": String("CERTIFIER"),
                  },
                  Object {
                      "agent": String("agent-5"),
                      "role": String("MANUFACTURER"),
                  },
              ],
              "was_derived_from": Array [
                  Object {
                      "activity": String("activity-d-5"),
                      "source": String("CertificateEntity-5"),
                  },
              ],
              "was_generated_by": Array [
                  String("activity-5"),
                  String("activity-6"),
              ],
              "was_quoted_from": Array [
                  Object {
                      "activity": String("activity-q-5"),
                      "source": String("CertificateEntity-5"),
                  },
              ],
              "was_revision_of": Array [
                  Object {
                      "activity": String("activity-r-5"),
                      "source": String("CertificateEntity-5"),
                  },
              ],
          },
          {
              "certIDAttribute": String("certIDAttribute-value"),
              "had_primary_source": Array [
                  Object {
                      "activity": String("activity-ps-6"),
                      "source": String("CertificateEntity-6"),
                  },
              ],
              "id": String("CertificateEntity-6"),
              "namespace_name": String("default"),
              "namespace_uuid": String("00000000-0000-0000-0000-000000000000"),
              "was_attributed_to": Array [
                  Object {
                      "agent": String("agent-6"),
                      "role": String("CERTIFIER"),
                  },
                  Object {
                      "agent": String("agent-6"),
                      "role": String("MANUFACTURER"),
                  },
              ],
              "was_derived_from": Array [
                  Object {
                      "activity": String("activity-d-6"),
                      "source": String("CertificateEntity-6"),
                  },
              ],
              "was_generated_by": Array [
                  String("activity-6"),
                  String("activity-7"),
              ],
              "was_quoted_from": Array [
                  Object {
                      "activity": String("activity-q-6"),
                      "source": String("CertificateEntity-6"),
                  },
              ],
              "was_revision_of": Array [
                  Object {
                      "activity": String("activity-r-6"),
                      "source": String("CertificateEntity-6"),
                  },
              ],
          },
          {
              "certIDAttribute": String("certIDAttribute-value"),
              "had_primary_source": Array [
                  Object {
                      "activity": String("activity-ps-7"),
                      "source": String("CertificateEntity-7"),
                  },
              ],
              "id": String("CertificateEntity-7"),
              "namespace_name": String("default"),
              "namespace_uuid": String("00000000-0000-0000-0000-000000000000"),
              "was_attributed_to": Array [
                  Object {
                      "agent": String("agent-7"),
                      "role": String("CERTIFIER"),
                  },
                  Object {
                      "agent": String("agent-7"),
                      "role": String("MANUFACTURER"),
                  },
              ],
              "was_derived_from": Array [
                  Object {
                      "activity": String("activity-d-7"),
                      "source": String("CertificateEntity-7"),
                  },
              ],
              "was_generated_by": Array [
                  String("activity-7"),
                  String("activity-8"),
              ],
              "was_quoted_from": Array [
                  Object {
                      "activity": String("activity-q-7"),
                      "source": String("CertificateEntity-7"),
                  },
              ],
              "was_revision_of": Array [
                  Object {
                      "activity": String("activity-r-7"),
                      "source": String("CertificateEntity-7"),
                  },
              ],
          },
      ],
  ]
  "###);
	}
}
