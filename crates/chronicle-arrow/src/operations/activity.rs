use arrow_array::{Array, RecordBatch};
use futures::StreamExt;

use common::{
    attributes::Attributes,
    prov::{
        ActivityId,
        AgentId, EntityId, NamespaceId, operations::{ChronicleOperation, SetAttributes}, Role,
    },
};

use crate::{
    ChronicleArrowError,
    query::{ActivityAssociationRef, AgentInteraction},
};

use super::{string_list_column, with_implied};

fn get_used(
    record_batch: &RecordBatch,
    row_index: usize,
) -> Result<Vec<String>, ChronicleArrowError> {
    string_list_column(record_batch, "used", row_index)
}

fn get_generated(
    record_batch: &RecordBatch,
    row_index: usize,
) -> Result<Vec<String>, ChronicleArrowError> {
    string_list_column(record_batch, "generated", row_index)
}

fn get_was_informed_by(
    record_batch: &RecordBatch,
    row_index: usize,
) -> Result<Vec<String>, ChronicleArrowError> {
    string_list_column(record_batch, "was_informed_by", row_index)
}

fn opt_time_column(
    record_batch: &RecordBatch,
    column_name: &str,
    row_index: usize,
) -> Result<Option<chrono::DateTime<chrono::Utc>>, ChronicleArrowError> {
    let column_index = record_batch
        .schema()
        .index_of(column_name)
        .map_err(|_| ChronicleArrowError::MissingColumn(column_name.to_string()))?;
    let column = record_batch.column(column_index);

    if let Some(timestamp_array) =
        column.as_any().downcast_ref::<arrow_array::TimestampNanosecondArray>()
    {
        let naive_time = timestamp_array.value_as_datetime(row_index);
        let time = naive_time
            .map(|nt| chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(nt, chrono::Utc));
        Ok(time)
    } else {
        Ok(None)
    }
}

fn get_started(
    record_batch: &RecordBatch,
    row_index: usize,
) -> Result<Option<chrono::DateTime<chrono::Utc>>, ChronicleArrowError> {
    opt_time_column(record_batch, "started", row_index)
}

fn get_ended(
    record_batch: &RecordBatch,
    row_index: usize,
) -> Result<Option<chrono::DateTime<chrono::Utc>>, ChronicleArrowError> {
    opt_time_column(record_batch, "ended", row_index)
}

fn get_was_associated_with(
    record_batch: &RecordBatch,
    row_index: usize,
) -> Result<Vec<ActivityAssociationRef>, ChronicleArrowError> {
    use arrow_array::{ListArray, StringArray, StructArray};

    let column_index = record_batch
        .schema()
        .index_of("was_associated_with")
        .map_err(|_| ChronicleArrowError::MissingColumn("was_associated_with".to_string()))?;
    let column = record_batch.column(column_index);
    let list_array = column
        .as_any()
        .downcast_ref::<ListArray>()
        .ok_or(ChronicleArrowError::ColumnTypeMismatch("Expected ListArray".to_string()))?;
    let binding = list_array.value(row_index);
    let struct_array = binding
        .as_any()
        .downcast_ref::<StructArray>()
        .ok_or(ChronicleArrowError::ColumnTypeMismatch("Expected StructArray".to_string()))?;

    let mut associations = Vec::new();
    for i in 0..struct_array.len() {
        let responsible_struct_array =
            struct_array.column(0).as_any().downcast_ref::<StructArray>().ok_or(
                ChronicleArrowError::ColumnTypeMismatch(
                    "Expected StructArray for responsible".to_string(),
                ),
            )?;

        let agent_array = responsible_struct_array
            .column(0)
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or(ChronicleArrowError::ColumnTypeMismatch(
                "Expected StringArray for agent".to_string(),
            ))?;
        let role_array = responsible_struct_array
            .column(1)
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or(ChronicleArrowError::ColumnTypeMismatch(
                "Expected StringArray for role".to_string(),
            ))?;

        let agent = agent_array.value(i).to_string();
        let role = Some(role_array.value(i).to_string());

        // Handling the delegated field, which is a ListArray of StructArray
        let delegated_list_array =
            struct_array.column(1).as_any().downcast_ref::<ListArray>().ok_or(
                ChronicleArrowError::ColumnTypeMismatch(
                    "Expected ListArray for delegated".to_string(),
                ),
            )?;
        let delegated_binding = delegated_list_array.value(i);
        let delegated_struct_array = delegated_binding
            .as_any()
            .downcast_ref::<StructArray>()
            .ok_or(ChronicleArrowError::ColumnTypeMismatch(
                "Expected StructArray for delegated".to_string(),
            ))?;

        let mut delegated_agents = Vec::new();
        for j in 0..delegated_struct_array.len() {
            let delegated_agent_array =
                delegated_struct_array.column(0).as_any().downcast_ref::<StringArray>().ok_or(
                    ChronicleArrowError::ColumnTypeMismatch(
                        "Expected StringArray for delegated agent".to_string(),
                    ),
                )?;
            let delegated_role_array =
                delegated_struct_array.column(1).as_any().downcast_ref::<StringArray>().ok_or(
                    ChronicleArrowError::ColumnTypeMismatch(
                        "Expected StringArray for delegated role".to_string(),
                    ),
                )?;

            let delegated_agent = delegated_agent_array.value(j).to_string();
            let delegated_role = Some(delegated_role_array.value(j).to_string());

            delegated_agents
                .push(AgentInteraction { agent: delegated_agent, role: delegated_role });
        }

        associations.push(ActivityAssociationRef {
            responsible: AgentInteraction { agent, role },
            delegated: delegated_agents,
        });
    }

    Ok(associations)
}

pub fn activity_operations(
    ns: &NamespaceId,
    id: &str,
    attributes: Attributes,
    row_index: usize,
    record_batch: &RecordBatch,
) -> Result<Vec<ChronicleOperation>, ChronicleArrowError> {
    let mut operations = vec![
        ChronicleOperation::activity_exists(ns.clone(), ActivityId::from_external_id(id)),
        ChronicleOperation::set_attributes(SetAttributes::activity(
            ns.clone(),
            ActivityId::from_external_id(id),
            attributes,
        )),
    ];

    let generated_ids = get_generated(record_batch, row_index)?;

    for entity_id in generated_ids {
        operations.push(ChronicleOperation::was_generated_by(
            ns.clone(),
            EntityId::from_external_id(&entity_id),
            ActivityId::from_external_id(id),
        ));
    }

    let used_ids = get_used(record_batch, row_index)?;

    for used_id in used_ids {
        operations.push(ChronicleOperation::activity_used(
            ns.clone(),
            ActivityId::from_external_id(id),
            EntityId::from_external_id(&used_id),
        ));
    }

    let was_informed_by_ids = get_was_informed_by(record_batch, row_index)?;

    for informed_by_id in was_informed_by_ids {
        operations.push(ChronicleOperation::was_informed_by(
            ns.clone(),
            ActivityId::from_external_id(id),
            ActivityId::from_external_id(&informed_by_id),
        ));
    }

    let started = get_started(record_batch, row_index)?;

    if let Some(started) = started {
        operations.push(ChronicleOperation::start_activity(
            ns.clone(),
            ActivityId::from_external_id(id),
            started,
        ));
    }

    let ended = get_ended(record_batch, row_index)?;

    if let Some(ended) = ended {
        operations.push(ChronicleOperation::end_activity(
            ns.clone(),
            ActivityId::from_external_id(id),
            ended,
        ));
    }

    let was_associated_with_refs = get_was_associated_with(record_batch, row_index)?;

    for association_ref in was_associated_with_refs {
        operations.push(ChronicleOperation::was_associated_with(
            ns.clone(),
            ActivityId::from_external_id(id),
            AgentId::from_external_id(&association_ref.responsible.agent),
            association_ref.responsible.role.map(Role),
        ));

        for delegated in &association_ref.delegated {
            operations.push(ChronicleOperation::agent_acts_on_behalf_of(
                ns.clone(),
                AgentId::from_external_id(id),
                AgentId::from_external_id(&association_ref.responsible.agent),
                Some(ActivityId::from_external_id(id)),
                delegated.role.as_ref().map(|role| Role(role.clone())),
            ));
        }
    }

    Ok(with_implied(operations))
}
