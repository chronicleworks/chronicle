use arrow::array::AsArray;
use arrow_array::{Array, BooleanArray, Int64Array, RecordBatch, StringArray};
use arrow_flight::{FlightData, FlightDescriptor, SchemaAsIpc};
use arrow_ipc::writer::{DictionaryTracker, IpcDataGenerator, IpcWriteOptions};
use arrow_schema::ArrowError;
use uuid::Uuid;

pub(crate) use activity::*;
pub(crate) use agent::*;
use api::ApiDispatch;
use api::commands::{ApiCommand, ImportCommand};
use common::{
	attributes::{Attribute, Attributes},
	domain::TypeName,
	identity::AuthId,
	prov::{NamespaceId, operations::ChronicleOperation},
};
pub(crate) use entity::*;

use crate::{
	ChronicleArrowError,
	meta::{DomainTypeMeta, get_domain_type_meta_from_cache, Term},
};

mod activity;
mod agent;
mod entity;

#[tracing::instrument(skip(record_batch, api))]
pub async fn process_record_batch(
    descriptor_path: &Vec<String>,
    record_batch: RecordBatch,
    api: &ApiDispatch,
) -> Result<(), ChronicleArrowError> {
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

    match domain_type_meta.term {
        Term::Entity =>
            create_chronicle_entity(&domain_type_meta.typ, &record_batch, &attribute_columns, api)
                .await?,
        Term::Activity =>
            create_chronicle_activity(&domain_type_meta.typ, &record_batch, &attribute_columns, api)
                .await?,
        Term::Agent =>
            create_chronicle_agent(&domain_type_meta.typ, &record_batch, &attribute_columns, api)
                .await?,
        Term::Namespace => create_chronicle_namespace(&record_batch, api).await?,
    }
    Ok(())
}

#[tracing::instrument(skip(descriptor, meta, batch))]
pub fn batch_to_flight_data(
    descriptor: &FlightDescriptor,
    meta: &DomainTypeMeta,
    batch: RecordBatch,
) -> Result<Vec<FlightData>, ArrowError> {
    let options = IpcWriteOptions::default();

    let schema_flight_data: FlightData =
        std::convert::Into::<FlightData>::into(SchemaAsIpc::new(&meta.schema, &options))
            .with_descriptor(descriptor.clone());

    let data_gen = IpcDataGenerator::default();
    let mut dictionary_tracker = DictionaryTracker::new(false);

    let (encoded_dictionaries, encoded_batch) =
        data_gen.encoded_batch(&batch, &mut dictionary_tracker, &options)?;

    let dictionaries: Vec<FlightData> = encoded_dictionaries.into_iter().map(Into::into).collect();
    let flight_data: FlightData = encoded_batch.into();

    let mut stream = vec![schema_flight_data];
    stream.extend(dictionaries);
    stream.push(flight_data);

    Ok(stream)
}

async fn create_chronicle_namespace(
    record_batch: &RecordBatch,
    _api: &ApiDispatch,
) -> Result<(), ChronicleArrowError> {
    let _uuid = record_batch
        .column_by_name("uuid")
        .ok_or(ChronicleArrowError::MissingColumn("uuid".to_string()))?;
    let _name = record_batch
        .column_by_name("name")
        .ok_or(ChronicleArrowError::MissingColumn("name".to_string()))?;

    Ok(())
}

pub async fn create_chronicle_entity(
    domain_type: &Option<Box<dyn TypeName + Send + Sync>>,
    record_batch: &RecordBatch,
    attribute_columns: &Vec<String>,
    api: &ApiDispatch,
) -> Result<(), ChronicleArrowError> {
    create_chronicle_terms(record_batch, Term::Entity, domain_type, attribute_columns, api).await
}

pub async fn create_chronicle_activity(
    domain_type: &Option<Box<dyn TypeName + Send + Sync>>,
    record_batch: &RecordBatch,
    attribute_columns: &Vec<String>,
    api: &ApiDispatch,
) -> Result<(), ChronicleArrowError> {
    create_chronicle_terms(record_batch, Term::Activity, domain_type, attribute_columns, api).await
}

pub async fn create_chronicle_agent(
    domain_type: &Option<Box<dyn TypeName + Send + Sync>>,
    record_batch: &RecordBatch,
    attribute_columns: &Vec<String>,
    api: &ApiDispatch,
) -> Result<(), ChronicleArrowError> {
    create_chronicle_terms(record_batch, Term::Agent, domain_type, attribute_columns, api).await
}

pub async fn create_chronicle_terms(
    record_batch: &RecordBatch,
    record_type: Term,
    domain_type: &Option<Box<dyn TypeName + Send + Sync>>,
    attribute_columns: &Vec<String>,
    api: &ApiDispatch,
) -> Result<(), ChronicleArrowError> {
    let ns_name_column = record_batch
        .column_by_name("namespace_name")
        .ok_or(ChronicleArrowError::MissingColumn("namespace_name".to_string()))?;

    let ns_uuid_column = record_batch
        .column_by_name("namespace_uuid")
        .ok_or(ChronicleArrowError::MissingColumn("namespace_uuid".to_string()))?;

    let id_column = record_batch
        .column_by_name("id")
        .ok_or(ChronicleArrowError::MissingColumn("id".to_string()))?;

    let attribute_columns_refs: Vec<&String> = attribute_columns.iter().collect();
    let attribute_values = attribute_columns_refs
        .iter()
        .map(|column_name| (column_name.to_string(), record_batch.column_by_name(column_name)))
        .filter_map(|(column_name, array_ref)| array_ref.map(|array_ref| (column_name, array_ref)))
        .collect::<Vec<(_, _)>>();

    tracing::trace!(?attribute_columns, "Processing attribute columns");

    let mut operations = Vec::new();
    for row_index in 0..record_batch.num_rows() {
        let ns_name = ns_name_column.as_string::<i32>().value(row_index);
        let ns_uuid = ns_uuid_column.as_string::<i32>().value(row_index);
        let ns_uuid = Uuid::parse_str(ns_uuid).map_err(ChronicleArrowError::from)?;
        let ns: NamespaceId = NamespaceId::from_external_id(ns_name, ns_uuid);

        let id = id_column.as_string::<i32>().value(row_index);

        let mut attributes: Vec<Attribute> = Vec::new();

        for (attribute_name, attribute_array) in attribute_values.iter() {
            tracing::trace!(%attribute_name, row_index, "Appending to attributes");
            if let Some(array) = attribute_array.as_any().downcast_ref::<StringArray>() {
                let value = array.value(row_index);
                attributes.push(Attribute::new(
                    attribute_name.clone(),
                    serde_json::Value::String(value.to_string()),
                ));
            } else if let Some(array) = attribute_array.as_any().downcast_ref::<Int64Array>() {
                let value = array.value(row_index);
                attributes.push(Attribute::new(
                    attribute_name.clone(),
                    serde_json::Value::Number(value.into()),
                ));
            } else if let Some(array) = attribute_array.as_any().downcast_ref::<BooleanArray>() {
                let value = array.value(row_index);
                attributes
                    .push(Attribute::new(attribute_name.clone(), serde_json::Value::Bool(value)));
            } else {
                tracing::warn!(%attribute_name, row_index, "Unsupported attribute type");
            }
        }
        let attributes =
            Attributes::new(domain_type.as_ref().map(|x| x.as_domain_type_id()), attributes);

        match record_type {
            Term::Entity => {
                operations.extend(entity_operations(&ns, id, attributes, row_index, record_batch)?);
            }
            Term::Activity => {
                operations.extend(activity_operations(
                    &ns,
                    id,
                    attributes,
                    row_index,
                    record_batch,
                )?);
            }
            Term::Agent => {
                operations.extend(agent_operations(&ns, id, attributes, row_index, record_batch)?);
            }
            Term::Namespace => {
                // Noop / unreachable
            }
        }
    }

    api.dispatch(ApiCommand::Import(ImportCommand { operations }), AuthId::anonymous())
        .await?;

    Ok(())
}

fn string_list_column(
    record_batch: &RecordBatch,
    column_name: &str,
    row_index: usize,
) -> Result<Vec<String>, ChronicleArrowError> {
    let column_index = record_batch
        .schema()
        .index_of(column_name)
        .map_err(|_| ChronicleArrowError::MissingColumn(column_name.to_string()))?;
    let column = record_batch.column(column_index);
    if let Some(list_array) = column.as_any().downcast_ref::<arrow_array::ListArray>() {
        if let Some(string_array) =
            list_array.value(row_index).as_any().downcast_ref::<arrow_array::StringArray>()
        {
            Ok((0..string_array.len()).map(|i| string_array.value(i).to_string()).collect())
        } else {
            Ok(vec![])
        }
    } else {
        Ok(vec![])
    }
}

fn struct_2_list_column_opt_string(
    record_batch: &RecordBatch,
    column_name: &str,
    row_index: usize,
    field1_name: &str,
    field2_name: &str,
) -> Result<Vec<(String, Option<String>)>, ChronicleArrowError> {
    let column_index = record_batch
        .schema()
        .index_of(column_name)
        .map_err(|_| ChronicleArrowError::MissingColumn(column_name.to_string()))?;
    let column = record_batch.column(column_index);
    if let Some(list_array) = column.as_any().downcast_ref::<arrow_array::ListArray>() {
        if let Some(struct_array) =
            list_array.value(row_index).as_any().downcast_ref::<arrow_array::StructArray>()
        {
            let field1_index = struct_array
                .column_by_name(field1_name)
                .ok_or_else(|| ChronicleArrowError::MissingColumn(field1_name.to_string()))?;
            let field2_index = struct_array
                .column_by_name(field2_name)
                .ok_or_else(|| ChronicleArrowError::MissingColumn(field2_name.to_string()))?;

            let field1_array = field1_index
                .as_any()
                .downcast_ref::<arrow_array::StringArray>()
                .ok_or_else(|| ChronicleArrowError::ColumnTypeMismatch(field1_name.to_string()))?;
            let field2_array = field2_index.as_any().downcast_ref::<arrow_array::StringArray>();

            Ok((0..struct_array.len())
                .map(|i| {
                    (
                        field1_array.value(i).to_string(),
                        field2_array.map(|arr| arr.value(i).to_string()),
                    )
                })
                .collect())
        } else {
            Ok(vec![])
        }
    } else {
        Ok(vec![])
    }
}

fn struct_3_list_column_opt_string(
    record_batch: &RecordBatch,
    column_name: &str,
    row_index: usize,
    field1_name: &str,
    field2_name: &str,
    field3_name: &str,
) -> Result<Vec<(String, String, Option<String>)>, ChronicleArrowError> {
    let column_index = record_batch
        .schema()
        .index_of(column_name)
        .map_err(|_| ChronicleArrowError::MissingColumn(column_name.to_string()))?;
    let column = record_batch.column(column_index);
    if let Some(list_array) = column.as_any().downcast_ref::<arrow_array::ListArray>() {
        if let Some(struct_array) =
            list_array.value(row_index).as_any().downcast_ref::<arrow_array::StructArray>()
        {
            let field1_index = struct_array
                .column_by_name(field1_name)
                .ok_or_else(|| ChronicleArrowError::MissingColumn(field1_name.to_string()))?;
            let field2_index = struct_array
                .column_by_name(field2_name)
                .ok_or_else(|| ChronicleArrowError::MissingColumn(field2_name.to_string()))?;
            let field3_index = struct_array
                .column_by_name(field3_name)
                .ok_or_else(|| ChronicleArrowError::MissingColumn(field3_name.to_string()))?;

            let field1_array = field1_index
                .as_any()
                .downcast_ref::<arrow_array::StringArray>()
                .ok_or_else(|| ChronicleArrowError::ColumnTypeMismatch(field1_name.to_string()))?;
            let field2_array = field2_index
                .as_any()
                .downcast_ref::<arrow_array::StringArray>()
                .ok_or_else(|| ChronicleArrowError::ColumnTypeMismatch(field2_name.to_string()))?;
            let field3_array = field3_index.as_any().downcast_ref::<arrow_array::StringArray>();

            Ok((0..struct_array.len())
                .map(|i| {
                    (
                        field1_array.value(i).to_string(),
                        field2_array.value(i).to_string(),
                        field3_array.map(|arr| arr.value(i).to_string()),
                    )
                })
                .collect::<Vec<(String, String, Option<String>)>>())
        } else {
            Ok(vec![])
        }
    } else {
        Ok(vec![])
    }
}

fn struct_2_list_column(
    record_batch: &RecordBatch,
    column_name: &str,
    row_index: usize,
    field1_name: &str,
    field2_name: &str,
) -> Result<Vec<(String, String)>, ChronicleArrowError> {
    let column_index = record_batch
        .schema()
        .index_of(column_name)
        .map_err(|_| ChronicleArrowError::MissingColumn(column_name.to_string()))?;
    let column = record_batch.column(column_index);
    if let Some(list_array) = column.as_any().downcast_ref::<arrow_array::ListArray>() {
        if let Some(struct_array) =
            list_array.value(row_index).as_any().downcast_ref::<arrow_array::StructArray>()
        {
            let field1_index = struct_array
                .column_by_name(field1_name)
                .ok_or_else(|| ChronicleArrowError::MissingColumn(field1_name.to_string()))?;
            let field2_index = struct_array
                .column_by_name(field2_name)
                .ok_or_else(|| ChronicleArrowError::MissingColumn(field2_name.to_string()))?;

            if let (Some(field1_array), Some(field2_array)) = (
                field1_index.as_any().downcast_ref::<arrow_array::StringArray>(),
                field2_index.as_any().downcast_ref::<arrow_array::StringArray>(),
            ) {
                Ok((0..struct_array.len())
                    .map(|i| (field1_array.value(i).to_string(), field2_array.value(i).to_string()))
                    .collect())
            } else {
                Ok(vec![])
            }
        } else {
            Ok(vec![])
        }
    } else {
        Ok(vec![])
    }
}

fn with_implied(operations: Vec<ChronicleOperation>) -> Vec<ChronicleOperation> {
    operations
        .into_iter()
        .flat_map(|op| {
            let mut implied_ops = op.implied_by();
            implied_ops.push(op);
            implied_ops
        })
        .collect()
}
