mod peekablestream;
mod query;

use api::commands::{ApiCommand, ImportCommand, NamespaceCommand};
use api::{ApiDispatch, ApiError};
use arrow_array::cast::AsArray;
use arrow_array::{ArrayRef, BooleanArray, Int64Array, NullArray, RecordBatch, StringArray};
use arrow_flight::decode::FlightRecordBatchStream;

use arrow_flight::{
	flight_service_server::FlightService, Action, ActionType, Criteria, Empty, FlightData,
	FlightDescriptor, FlightInfo, HandshakeRequest, HandshakeResponse, PutResult, SchemaResult,
	Ticket,
};

use arrow_flight::{FlightEndpoint, IpcMessage, SchemaAsIpc};
use arrow_schema::{ArrowError, Schema, SchemaBuilder};

use common::attributes::{Attribute, Attributes};
use common::domain::{
	ActivityDef, AgentDef, AttributesTypeName, ChronicleDomainDef, EntityDef, PrimitiveType,
	TypeName,
};

use common::identity::AuthId;
use common::prov::operations::{ChronicleOperation, SetAttributes};
use common::prov::{
	ActivityId, AgentId, DomaintypeId, EntityId, ExternalIdPart, NamespaceId, ParseIriError,
};
use diesel::r2d2::ConnectionManager;
use diesel::PgConnection;
use futures::future::ok;
use futures::{stream, TryStreamExt};
use futures::{stream::BoxStream, StreamExt};
use lazy_static::lazy_static;
use r2d2::Pool;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use tokio::task::spawn_blocking;

use std::sync::Arc;
use std::sync::Mutex;
use std::vec::Vec;
use tonic::transport::Server;
use tonic::{Request, Response, Status, Streaming};
use tracing::{info, instrument};

use thiserror::Error;

use crate::peekablestream::PeekableFlightDataStream;

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

	#[error("Missing column: {0}")]
	MissingColumn(String),

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
}

#[derive(Clone)]
pub struct FlightServiceImpl {
	domain: common::domain::ChronicleDomainDef,
	pool: r2d2::Pool<ConnectionManager<PgConnection>>,
	api: ApiDispatch,
}

fn field_for_domain_primitive(prim: &PrimitiveType) -> Option<arrow_schema::DataType> {
	match prim {
		PrimitiveType::String => Some(arrow_schema::DataType::Utf8),
		PrimitiveType::Int => Some(arrow_schema::DataType::Int64),
		PrimitiveType::Bool => Some(arrow_schema::DataType::Boolean),
		_ => None,
	}
}

#[tracing::instrument]
fn schema_for_namespace() -> Schema {
	let mut builder = SchemaBuilder::new();

	builder.push(arrow_schema::Field::new("name", arrow_schema::DataType::Utf8, false));
	builder.push(arrow_schema::Field::new(
		"uuid",
		arrow_schema::DataType::FixedSizeBinary(16),
		false,
	));

	builder.finish()
}

fn schema_for_entity(entity: &EntityDef) -> Schema {
	let mut builder = SchemaBuilder::new();

	builder.push(arrow_schema::Field::new("namespace_id", arrow_schema::DataType::Utf8, false));

	builder.push(arrow_schema::Field::new("id", arrow_schema::DataType::Utf8, false));
	for attribute in &entity.attributes {
		if let Some(data_type) = field_for_domain_primitive(&attribute.primitive_type) {
			builder.push(arrow_schema::Field::new(
				&attribute.preserve_inflection(),
				data_type,
				true,
			));
		}
	}
	builder.finish()
}

fn schema_for_activity(activity: &ActivityDef) -> Schema {
	let mut builder = SchemaBuilder::new();

	builder.push(arrow_schema::Field::new("namespace_id", arrow_schema::DataType::Utf8, false));
	builder.push(arrow_schema::Field::new("id", arrow_schema::DataType::Utf8, false));
	builder.push(arrow_schema::Field::new(
		"started",
		arrow_schema::DataType::Timestamp(arrow_schema::TimeUnit::Millisecond, None),
		true,
	));
	builder.push(arrow_schema::Field::new(
		"ended",
		arrow_schema::DataType::Timestamp(arrow_schema::TimeUnit::Millisecond, None),
		true,
	));

	for attribute in &activity.attributes {
		if let Some(typ) = field_for_domain_primitive(&attribute.primitive_type) {
			builder.push(arrow_schema::Field::new(&attribute.preserve_inflection(), typ, true));
		}
	}

	builder.push(arrow_schema::Field::new(
		"was_associated_with",
		arrow_schema::DataType::List(Arc::new(arrow_schema::Field::new(
			"agent",
			arrow_schema::DataType::Utf8,
			false,
		))),
		false,
	));
	builder.push(arrow_schema::Field::new(
		"used",
		arrow_schema::DataType::List(Arc::new(arrow_schema::Field::new(
			"entity",
			arrow_schema::DataType::Utf8,
			false,
		))),
		true,
	));
	builder.push(arrow_schema::Field::new(
		"was_informed_by",
		arrow_schema::DataType::List(Arc::new(arrow_schema::Field::new(
			"activity",
			arrow_schema::DataType::Utf8,
			false,
		))),
		false,
	));
	builder.push(arrow_schema::Field::new(
		"generated",
		arrow_schema::DataType::List(Arc::new(arrow_schema::Field::new(
			"entity",
			arrow_schema::DataType::Utf8,
			true,
		))),
		true,
	));
	builder.finish()
}

fn schema_for_agent(agent: &AgentDef) -> Schema {
	let mut builder = SchemaBuilder::new();
	builder.push(arrow_schema::Field::new("namespace_id", arrow_schema::DataType::Utf8, false));
	builder.push(arrow_schema::Field::new("id", arrow_schema::DataType::Utf8, false));
	for attribute in &agent.attributes {
		if let Some(typ) = field_for_domain_primitive(&attribute.primitive_type) {
			builder.push(arrow_schema::Field::new(&attribute.preserve_inflection(), typ, true));
		}
	}

	builder.finish()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Term {
	Namespace,
	Entity,
	Activity,
	Agent,
}

struct DomainTypeMeta {
	pub schema: Schema,
	pub term: Term,
	pub typ: Option<DomaintypeId>,
}

#[derive(Debug, Serialize, serde::Deserialize)]
struct ChronicleTicket {
	term: Term,
	typ: Option<DomaintypeId>,
	start: u64,
	count: u64,
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

lazy_static! {
	static ref SCHEMA_CACHE: Mutex<HashMap<Vec<String>, Arc<DomainTypeMeta>>> =
		Mutex::new(HashMap::new());
}

fn get_domain_type_meta_from_cache(descriptor_path: &Vec<String>) -> Option<Arc<DomainTypeMeta>> {
	let cache = SCHEMA_CACHE.lock().unwrap();
	cache.get(descriptor_path).cloned()
}

fn cache_metadata(
	term: Term,
	domain_type_id: DomaintypeId,
	descriptor_path: Vec<String>,
	schema: Schema,
) {
	let mut cache = SCHEMA_CACHE.lock().unwrap();
	let domain_type_meta = Arc::new(DomainTypeMeta { schema, term, typ: Some(domain_type_id) });
	cache.insert(descriptor_path, domain_type_meta);
}

fn cache_namespace_schema() {
	let mut cache = SCHEMA_CACHE.lock().unwrap();
	cache.insert(
		vec!["Namespace".to_string()],
		Arc::new(DomainTypeMeta {
			schema: schema_for_namespace(),
			term: Term::Namespace,
			typ: None,
		}),
	);
}

#[tracing::instrument(skip(domain_def))]
fn cache_domain_schemas(domain_def: &ChronicleDomainDef) {
	for entity in &domain_def.entities {
		let schema = schema_for_entity(entity);
		let descriptor_path = vec![entity.as_type_name()];
		cache_metadata(
			Term::Entity,
			DomaintypeId::from_external_id(entity.as_type_name()),
			descriptor_path,
			schema,
		);
	}

	for agent in &domain_def.agents {
		let schema = schema_for_agent(agent);
		let descriptor_path = vec![agent.as_type_name()];
		cache_metadata(
			Term::Agent,
			DomaintypeId::from_external_id(agent.as_type_name()),
			descriptor_path,
			schema,
		);
	}

	for activity in &domain_def.activities {
		let schema = schema_for_activity(activity);
		let descriptor_path = vec![activity.as_type_name()];
		cache_metadata(
			Term::Activity,
			DomaintypeId::from_external_id(activity.as_type_name()),
			descriptor_path,
			schema,
		);
	}
}

#[tracing::instrument(skip(record_batch))]
async fn process_record_batch(
	descriptor_path: &Vec<String>,
	record_batch: RecordBatch,
	api: &ApiDispatch,
) -> Result<(), ChronicleArrowError> {
	if descriptor_path.is_empty() {
		return Err(ChronicleArrowError::InvalidDescriptorPath);
	}

	let domain_type_meta = get_domain_type_meta_from_cache(descriptor_path)
		.ok_or(ChronicleArrowError::MetadataNotFound)?;

	let attribute_columns = domain_type_meta
		.schema
		.fields()
		.iter()
		.filter_map(|field| {
			if field.name().ends_with("Attribute") {
				Some(field.name().clone())
			} else {
				None
			}
		})
		.collect::<Vec<String>>();

	tracing::debug!(?attribute_columns, "Extracted attribute column names");
	match domain_type_meta.term {
		Term::Entity => {
			create_chronicle_entity(&domain_type_meta.typ, &record_batch, &attribute_columns, api)
				.await?
		},
		Term::Activity => {
			create_chronicle_activity(&domain_type_meta.typ, &record_batch, &attribute_columns, api)
				.await?
		},
		Term::Agent => {
			create_chronicle_agent(&domain_type_meta.typ, &record_batch, &attribute_columns, api)
				.await?
		},
		Term::Namespace => create_chronicle_namespace(&record_batch, api).await?,
	}
	Ok(())
}

async fn create_chronicle_namespace(
	record_batch: &RecordBatch,
	api: &ApiDispatch,
) -> Result<(), ChronicleArrowError> {
	let uuid = record_batch
		.column_by_name("uuid")
		.ok_or(ChronicleArrowError::MissingColumn("uuid".to_string()))?;
	let name = record_batch
		.column_by_name("name")
		.ok_or(ChronicleArrowError::MissingColumn("name".to_string()))?;

	Ok(())
}

async fn create_chronicle_entity(
	domain_type: &Option<DomaintypeId>,
	record_batch: &RecordBatch,
	attribute_columns: &Vec<String>,
	api: &ApiDispatch,
) -> Result<(), ChronicleArrowError> {
	create_chronicle_terms(record_batch, Term::Entity, domain_type, attribute_columns, api).await
}

async fn create_chronicle_activity(
	domain_type: &Option<DomaintypeId>,
	record_batch: &RecordBatch,
	attribute_columns: &Vec<String>,
	api: &ApiDispatch,
) -> Result<(), ChronicleArrowError> {
	create_chronicle_terms(record_batch, Term::Activity, domain_type, attribute_columns, api).await
}

async fn create_chronicle_agent(
	domain_type: &Option<DomaintypeId>,
	record_batch: &RecordBatch,
	attribute_columns: &Vec<String>,
	api: &ApiDispatch,
) -> Result<(), ChronicleArrowError> {
	create_chronicle_terms(record_batch, Term::Agent, domain_type, attribute_columns, api).await
}

async fn create_chronicle_terms(
	record_batch: &RecordBatch,
	record_type: Term,
	domain_type: &Option<DomaintypeId>,
	attribute_columns: &Vec<String>,
	api: &ApiDispatch,
) -> Result<(), ChronicleArrowError> {
	let ns_column = record_batch
		.column_by_name("namespace_id")
		.ok_or(ChronicleArrowError::MissingColumn("namespace_id".to_string()))?;

	let id_column = record_batch
		.column_by_name("id")
		.ok_or(ChronicleArrowError::MissingColumn("id".to_string()))?;

	let attribute_columns_refs: Vec<&String> = attribute_columns.iter().collect();
	let attribute_values = attribute_columns_refs
		.iter()
		.map(|column_name| (column_name.to_string(), record_batch.column_by_name(column_name)))
		.filter_map(|(column_name, array_ref)| array_ref.map(|array_ref| (column_name, array_ref)))
		.collect::<Vec<(_, _)>>();

	tracing::debug!(?attribute_columns, "Processing attribute columns");

	let total_rows = record_batch.num_rows();
	for batch_start in (0..total_rows).step_by(100) {
		let batch_end = std::cmp::min(batch_start + 100, total_rows);
		tracing::debug!(batch_start, batch_end, "Processing batch");

		let mut operations = Vec::new();
		for row_index in batch_start..batch_end {
			let ns: NamespaceId =
				NamespaceId::try_from(ns_column.as_string::<i32>().value(row_index))?;
			let id = id_column.as_string::<i32>().value(row_index);

			let mut attributes: Vec<(String, Attribute)> = vec![];

			for (attribute_name, attribute_array) in attribute_values.iter() {
				tracing::trace!(%attribute_name, row_index, "Appending to attributes");
				if let Some(array) = attribute_array.as_any().downcast_ref::<StringArray>() {
					let value = array.value(row_index);
					attributes.push((
						attribute_name.clone(),
						Attribute::new(
							attribute_name.clone(),
							serde_json::Value::String(value.to_string()),
						),
					));
				} else if let Some(array) = attribute_array.as_any().downcast_ref::<Int64Array>() {
					let value = array.value(row_index);
					attributes.push((
						attribute_name.clone(),
						Attribute::new(
							attribute_name.clone(),
							serde_json::Value::Number(value.into()),
						),
					));
				} else if let Some(array) = attribute_array.as_any().downcast_ref::<BooleanArray>()
				{
					let value = array.value(row_index);
					attributes.push((
						attribute_name.clone(),
						Attribute::new(attribute_name.clone(), serde_json::Value::Bool(value)),
					));
				} else {
					tracing::warn!(%attribute_name, row_index, "Unsupported attribute type");
				}
			}

			let attributes = Attributes::new(domain_type.clone(), attributes);

			match record_type {
				Term::Entity => {
					operations.push(ChronicleOperation::entity_exists(
						ns.clone(),
						EntityId::from_external_id(id),
					));
					operations.push(ChronicleOperation::set_attributes(SetAttributes::entity(
						ns.clone(),
						EntityId::from_external_id(id),
						attributes,
					)));
				},
				Term::Activity => {
					operations.push(ChronicleOperation::activity_exists(
						ns.clone(),
						ActivityId::from_external_id(id),
					));
					operations.push(ChronicleOperation::set_attributes(SetAttributes::activity(
						ns.clone(),
						ActivityId::from_external_id(id),
						attributes,
					)));
				},
				Term::Agent => {
					operations.push(ChronicleOperation::agent_exists(
						ns.clone(),
						AgentId::from_external_id(id),
					));
					operations.push(ChronicleOperation::set_attributes(SetAttributes::agent(
						ns.clone(),
						AgentId::from_external_id(id),
						attributes,
					)));
				},
				Term::Namespace => {
					// Noop / unreachable
				},
			}
		}

		api.dispatch(ApiCommand::Import(ImportCommand { operations }), AuthId::anonymous())
			.await?;
	}

	Ok(())
}

#[instrument(skip(pool, term, domaintype))]
async fn calculate_count_by_metadata_term(
	pool: &Pool<ConnectionManager<PgConnection>>,
	term: &Term,
	domaintype: Option<String>,
) -> Result<i64, Status> {
	let pool = pool.clone();
	match term {
		Term::Entity => {
			spawn_blocking(move || {
				query::entity_count_by_type(
					&pool,
					domaintype.map(|x| x.to_string()).iter().map(|s| s.as_str()).collect(),
				)
			})
			.await
		},
		Term::Agent => {
			spawn_blocking(move || {
				query::agent_count_by_type(
					&pool,
					domaintype.map(|x| x.to_string()).iter().map(|s| s.as_str()).collect(),
				)
			})
			.await
		},
		Term::Activity => {
			spawn_blocking(move || {
				query::activity_count_by_type(
					&pool,
					domaintype.map(|x| x.to_string()).iter().map(|s| s.as_str()).collect(),
				)
			})
			.await
		},
		_ => Ok(Ok(0)),
	}
	.map_err(|e| Status::from_error(e.into()))
	.and_then(|res| res.map_err(|e| Status::from_error(e.into())))
}

async fn create_flight_info_for_type(
	pool: Arc<Pool<ConnectionManager<PgConnection>>>,
	domain_items: Vec<impl TypeName + Send + Sync + 'static>,
	term: Term,
) -> BoxStream<'static, Result<Vec<FlightInfo>, Status>> {
	stream::iter(domain_items.into_iter().map(|item| Ok::<_, tonic::Status>(item)))
		.then(move |item| {
			let pool = pool.clone();
			async move {
				let item = item?; // Handle the Result from the iterator
				let descriptor_path = vec![item.as_type_name()];
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

				let flight_infos = (0..count)
					.step_by(FLIGHT_MAX_SIZE)
					.map(|start| {
						let end = std::cmp::min(start as usize + FLIGHT_MAX_SIZE, count as usize);

						let chunk_descriptor_path = descriptor_path.clone();
						let ticket_metadata = ChronicleTicket {
							term: term.clone(),
							typ: metadata.typ.clone(),
							start: start as _,
							count: (end - start as usize) as _,
						};
						let ticket = Ticket::try_from(ticket_metadata).map_err(|e| {
							Status::from_error(Box::new(ChronicleArrowError::from(e)))
						})?;

						FlightInfo::new()
							.with_endpoint(FlightEndpoint::new().with_ticket(ticket))
							.with_descriptor(FlightDescriptor::new_path(chunk_descriptor_path))
							.try_with_schema(&metadata.schema)
							.map_err(|e| Status::from_error(Box::new(ChronicleArrowError::from(e))))
					})
					.collect::<Result<Vec<_>, _>>();

				flight_infos
			}
		})
		.boxed()
}

/// Maximum number of records in a flight
const FLIGHT_MAX_SIZE: usize = 1024 * 10;

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

	async fn list_flights(
		&self,
		_request: Request<Criteria>,
	) -> Result<Response<Self::ListFlightsStream>, Status> {
		let entity_flights_stream = create_flight_info_for_type(
			Arc::new(self.pool.clone()),
			self.domain.entities.iter().map(|e| e.clone()).collect(),
			Term::Entity,
		)
		.await;
		let activities_flights_stream = create_flight_info_for_type(
			Arc::new(self.pool.clone()),
			self.domain.activities.iter().map(|a| a.clone()).collect(),
			Term::Activity,
		)
		.await;
		let agents_flights_stream = create_flight_info_for_type(
			Arc::new(self.pool.clone()),
			self.domain.agents.iter().map(|a| a.clone()).collect(),
			Term::Agent,
		)
		.await;

		let combined_flights = futures::stream::select_all(vec![
			entity_flights_stream,
			activities_flights_stream,
			agents_flights_stream,
		])
		.flat_map(|result| match result {
			Ok(vec_flight_info) => {
				futures::stream::iter(vec_flight_info.into_iter().map(Ok)).boxed()
			},
			Err(e) => futures::stream::once(async move { Err(e) }).boxed(),
		})
		.boxed();

		Ok(Response::new(Box::pin(combined_flights) as Self::ListFlightsStream))
	}
	#[instrument(skip(self, request))]
	async fn get_flight_info(
		&self,
		request: Request<FlightDescriptor>,
	) -> Result<Response<FlightInfo>, Status> {
		let descriptor = request.into_inner();
		let path = descriptor.path;
		if path.is_empty() {
			return Err(Status::invalid_argument("Descriptor path is empty"));
		}

		let type_name = &path[0];
		let descriptor_path = vec![type_name.to_string()];
		let metadata = get_domain_type_meta_from_cache(&descriptor_path)
			.ok_or_else(|| ChronicleArrowError::MissingSchemaError)
			.map_err(|e| Status::internal(format!("Failed to get cached schema: {}", e)))?;

		let mut flight_info = FlightInfo::new()
			.with_descriptor(FlightDescriptor::new_path(descriptor_path))
			.try_with_schema(&metadata.schema)
			.map_err(|e| Status::from_error(e.into()))?;

		let count = calculate_count_by_metadata_term(
			&self.pool,
			&metadata.term,
			metadata.typ.as_ref().map(|x| x.external_id_part().to_string()),
		)
		.await
		.map_err(|e| Status::internal(format!("Failed to get count: {}", e)))?;

		flight_info = flight_info.with_total_records(count);

		Ok(Response::new(flight_info))
	}

	#[instrument(skip(self, _request))]
	async fn get_schema(
		&self,
		_request: Request<FlightDescriptor>,
	) -> Result<Response<SchemaResult>, Status> {
		let descriptor = _request.into_inner();
		let path = descriptor.path;
		if path.is_empty() {
			return Err(Status::invalid_argument("Descriptor path is empty"));
		}

		let type_name = &path[0];
		let descriptor_path = vec![type_name.to_string()];
		let schema = get_domain_type_meta_from_cache(&descriptor_path)
			.ok_or_else(|| ChronicleArrowError::MissingSchemaError)
			.map_err(|e| Status::internal(format!("Failed to get cached schema: {}", e)))?;

		let options = arrow_ipc::writer::IpcWriteOptions::default();
		let ipc_message_result = SchemaAsIpc::new(&schema.schema, &options).try_into();
		match ipc_message_result {
			Ok(IpcMessage(schema)) => Ok(Response::new(SchemaResult { schema })),
			Err(e) => {
				Err(Status::internal(format!("Failed to convert schema to IPC message: {}", e)))
			},
		}
	}
	async fn do_get(
		&self,
		request: Request<Ticket>,
	) -> Result<Response<Self::DoGetStream>, Status> {
		let ticket = request.into_inner();
		let ticket: ChronicleTicket = ticket
			.try_into()
			.map_err(|e| Status::from_error(Box::new(ChronicleArrowError::from(e))))?;

		let entities = match ticket.term {
			Term::Entity => {
				let domain_type_str =
					ticket.typ.as_ref().map(|typ| typ.external_id_part().to_string());
				let domain_types =
					domain_type_str.as_ref().map_or_else(Vec::new, |s| vec![s.as_str()]);
				query::load_entities_by_type(&self.pool, domain_types, ticket.start, ticket.count)
					.await
			},
			Term::Activity => {
				let domain_type_str =
					ticket.typ.as_ref().map(|typ| typ.external_id_part().to_string());
				let domain_types =
					domain_type_str.as_ref().map_or_else(Vec::new, |s| vec![s.as_str()]);
				query::load_activities_by_type(&self.pool, domain_types, ticket.start, ticket.count)
					.await
			},
			Term::Agent => {
				let domain_type_str =
					ticket.typ.as_ref().map(|typ| typ.external_id_part().to_string());
				let domain_types =
					domain_type_str.as_ref().map_or_else(Vec::new, |s| vec![s.as_str()]);
				query::load_agents_by_type(&self.pool, domain_types, ticket.start, ticket.count)
					.await
			},
			Term::Namespace => {
				tracing::warn!("Namespace query not implemented. Returning empty stream.");
				futures::stream::empty()
			},
		};

		let entities =
			entities.map_err(|e| Status::internal(format!("Failed to load entities: {}", e)))?;

		let stream = stream::iter(entities.into_iter().map(Ok::<_, Status>));
		let response = Response::new(Box::pin(stream) as Self::DoGetStream);

		Ok(response)
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
			Some(Err(e)) => {
				return Err(Status::internal(format!(
					"Failed to get first item from stream: {}",
					e
				)))
			},
			None => {
				return Err(Status::invalid_argument("Stream is empty"));
			},
		};

		tracing::debug!(
			descriptor_type = %flight_descriptor.r#type,
			descriptor_cmd = %String::from_utf8_lossy(&flight_descriptor.cmd),
			descriptor_path = ?flight_descriptor.path,
			"Flight descriptor properties"
		);

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
			tracing::debug!("Processing batch: {:?}", batch);
			let batch = batch?;

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

#[instrument(skip(pool, api))]
pub async fn run_flight_service(
	pool: &Pool<ConnectionManager<PgConnection>>,
	domain: common::domain::ChronicleDomainDef,
	api: ApiDispatch,
	addr: SocketAddr,
) -> Result<(), Box<dyn std::error::Error>> {
	cache_domain_schemas(&domain);
	let flight_service = FlightServiceImpl { pool: pool.clone(), domain, api };

	info!("Starting flight service at {}", addr);

	Server::builder()
		.add_service(arrow_flight::flight_service_server::FlightServiceServer::new(flight_service))
		.serve(addr)
		.await?;

	Ok(())
}

#[cfg(test)]
mod tests {

	use api::commands::{ApiCommand, ImportCommand};
	use arrow_array::RecordBatch;
	use arrow_flight::{
		flight_service_client::FlightServiceClient, Criteria, FlightData, FlightDescriptor,
		SchemaAsIpc,
	};

	use arrow_ipc::writer::{self, IpcWriteOptions};
	use arrow_schema::ArrowError;
	use chronicle_persistence::Store;
	use chronicle_test_infrastructure::substitutes::{test_api, TestDispatch};
	use common::{
		domain::ChronicleDomainDef,
		identity::AuthId,
		prov::{operations::ChronicleOperation, NamespaceId},
	};
	use futures::{stream, StreamExt};
	use portpicker::pick_unused_port;
	use serde::{Deserialize, Serialize};

	use std::{net::SocketAddr, sync::Arc, time::Duration};
	use tonic::{Request, Status};
	use uuid::Uuid;

	use crate::{cache_domain_schemas, get_domain_type_meta_from_cache, DomainTypeMeta};

	async fn setup_test_environment<'a>(
		domain: &ChronicleDomainDef,
	) -> Result<
		(FlightServiceClient<tonic::transport::Channel>, TestDispatch<'a>),
		Box<dyn std::error::Error>,
	> {
		chronicle_telemetry::telemetry(None, chronicle_telemetry::ConsoleLogging::Pretty);
		let api = test_api().await;
		let port = pick_unused_port().expect("No ports free");
		let addr = SocketAddr::from(([127, 0, 0, 1], port));
		let pool = api.temporary_database().connection_pool().unwrap();
		let dispatch = api.api_dispatch().clone();
		let domain = domain.clone();
		tokio::spawn(async move {
			super::run_flight_service(&pool, domain, dispatch, addr).await.unwrap();
		});

		tokio::time::sleep(Duration::from_secs(5)).await;

		let client = FlightServiceClient::connect(format!("http://{}", addr)).await?;
		Ok((client, api))
	}

	#[tracing::instrument]
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

	#[derive(Serialize, Deserialize)]
	struct Certificate {
		namespace_id: String,
		id: String,
		certIDAttribute: String,
	}

	#[derive(Serialize, Deserialize)]
	struct Item {
		namespace_id: String,
		id: String,
		partIDAttribute: String,
	}

	fn create_test_entity_certificate(meta: &DomainTypeMeta, count: u32) -> RecordBatch {
		let fields = meta.schema.all_fields().into_iter().cloned().collect::<Vec<_>>();

		let mut certs = Vec::new();
		for i in 0..count {
			certs.push(Certificate {
				namespace_id: NamespaceId::from_external_id("default", Uuid::default()).to_string(),
				id: format!("certificate-{}", i),
				certIDAttribute: format!("CERT-{}", i),
			});
		}

		let arrays = serde_arrow::to_arrow(&fields, &certs).unwrap();

		RecordBatch::try_new(Arc::new(meta.schema.clone()), arrays).unwrap()
	}

	fn create_test_entity_item(meta: &DomainTypeMeta, count: u32) -> RecordBatch {
		let fields = meta.schema.all_fields().into_iter().cloned().collect::<Vec<_>>();

		let mut certs = Vec::new();
		for i in 0..count {
			certs.push(Item {
				namespace_id: NamespaceId::from_external_id("default", Uuid::default()).to_string(),
				id: format!("item-{}", i),
				partIDAttribute: format!("PART-{}", i),
			});
		}

		let arrays = serde_arrow::to_arrow(&fields, &certs).unwrap();

		RecordBatch::try_new(Arc::new(meta.schema.clone()), arrays).unwrap()
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

	async fn create_test_flight_data() -> Result<Vec<FlightData>, Box<dyn std::error::Error>> {
		let path = vec!["CertificateEntity".to_owned()];
		let meta = get_domain_type_meta_from_cache(&path).unwrap();
		let batch = create_test_entity_certificate(
			&get_domain_type_meta_from_cache(&path).unwrap().clone(),
			1000,
		);

		let flight_data =
			batches_to_flight_data(&FlightDescriptor::new_path(path.clone()), &meta, vec![batch])
				.unwrap();

		Ok(flight_data)
	}

	#[tokio::test]
	async fn flight_service_is_isomorphic() -> Result<(), Box<dyn std::error::Error>> {
		cache_domain_schemas(&create_test_domain_def());
		let domain = create_test_domain_def();
		let (mut client, mut api) = setup_test_environment(&domain).await?;

		let create_namespace_operation = ChronicleOperation::create_namespace(
			NamespaceId::from_external_id("default", Uuid::default()),
		);
		api.dispatch(
			ApiCommand::Import(ImportCommand { operations: vec![create_namespace_operation] }),
			AuthId::anonymous(),
		)
		.await
		.map_err(|e| Status::from_error(e.into()))?;

		client
			.do_put(stream::iter(create_test_flight_data().await.unwrap()).boxed())
			.await
			.unwrap();

		Ok(())
	}
}
