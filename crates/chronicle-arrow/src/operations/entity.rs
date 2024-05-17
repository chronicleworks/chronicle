use arrow_array::RecordBatch;

use common::{
    attributes::Attributes,
    prov::{
        ActivityId,
        AgentId, EntityId, NamespaceId, operations::{ChronicleOperation, DerivationType, SetAttributes}, Role,
    },
};

use crate::{
    ChronicleArrowError,
    query::{DerivationRef, EntityAttributionRef},
};

use super::{
    string_list_column, struct_2_list_column, struct_2_list_column_opt_string, with_implied,
};

fn get_was_generated_by(
    record_batch: &RecordBatch,
    row_index: usize,
) -> Result<Vec<String>, ChronicleArrowError> {
    string_list_column(record_batch, "was_generated_by", row_index)
}

fn get_entity_was_attributed_to(
    record_batch: &RecordBatch,
    row_index: usize,
) -> Result<Vec<EntityAttributionRef>, ChronicleArrowError> {
    Ok(struct_2_list_column_opt_string(
        record_batch,
        "was_attributed_to",
        row_index,
        "agent",
        "role",
    )?
        .into_iter()
        .map(|(agent, role)| EntityAttributionRef { agent, role })
        .collect())
}

fn get_derivation(
    column_name: &str,
    record_batch: &RecordBatch,
    row_index: usize,
) -> Result<Vec<DerivationRef>, ChronicleArrowError> {
    Ok(struct_2_list_column(record_batch, column_name, row_index, "source", "activity")?
        .into_iter()
        .map(|(target, activity)| DerivationRef { source: target, activity })
        .collect())
}

pub fn entity_operations(
    ns: &NamespaceId,
    id: &str,
    attributes: Attributes,
    row_index: usize,
    record_batch: &RecordBatch,
) -> Result<Vec<ChronicleOperation>, ChronicleArrowError> {
    let mut operations = vec![
        ChronicleOperation::entity_exists(ns.clone(), EntityId::from_external_id(id)),
        ChronicleOperation::set_attributes(SetAttributes::entity(
            ns.clone(),
            EntityId::from_external_id(id),
            attributes,
        )),
    ];

    let was_generated_by_ids = get_was_generated_by(record_batch, row_index)?;

    for generated_by_id in was_generated_by_ids {
        operations.push(ChronicleOperation::was_generated_by(
            ns.clone(),
            EntityId::from_external_id(id),
            ActivityId::from_external_id(&generated_by_id),
        ));
    }

    let was_attributed_to_refs = get_entity_was_attributed_to(record_batch, row_index)?;

    for was_attributed_to_ref in was_attributed_to_refs {
        operations.push(ChronicleOperation::was_attributed_to(
            ns.clone(),
            EntityId::from_external_id(id),
            AgentId::from_external_id(was_attributed_to_ref.agent),
            was_attributed_to_ref.role.map(Role::from),
        ))
    }

    let was_derived_from_refs = get_derivation("was_derived_from", record_batch, row_index)?;

    for was_derived_from_ref in was_derived_from_refs {
        operations.push(ChronicleOperation::entity_derive(
            ns.clone(),
            EntityId::from_external_id(was_derived_from_ref.source),
            EntityId::from_external_id(id),
            Some(ActivityId::from_external_id(was_derived_from_ref.activity)),
            DerivationType::None,
        ))
    }

    let had_primary_source_refs = get_derivation("had_primary_source", record_batch, row_index)?;

    for had_primary_source_ref in had_primary_source_refs {
        operations.push(ChronicleOperation::entity_derive(
            ns.clone(),
            EntityId::from_external_id(had_primary_source_ref.source),
            EntityId::from_external_id(id),
            Some(ActivityId::from_external_id(had_primary_source_ref.activity)),
            DerivationType::PrimarySource,
        ))
    }

    let was_quoted_from_refs = get_derivation("was_quoted_from", record_batch, row_index)?;

    for was_quoted_from_ref in was_quoted_from_refs {
        operations.push(ChronicleOperation::entity_derive(
            ns.clone(),
            EntityId::from_external_id(was_quoted_from_ref.source),
            EntityId::from_external_id(id),
            Some(ActivityId::from_external_id(was_quoted_from_ref.activity)),
            DerivationType::Quotation,
        ))
    }

    let was_revision_of_refs = get_derivation("was_revision_of", record_batch, row_index)?;

    for was_revision_of_ref in was_revision_of_refs {
        operations.push(ChronicleOperation::entity_derive(
            ns.clone(),
            EntityId::from_external_id(was_revision_of_ref.source),
            EntityId::from_external_id(id),
            Some(ActivityId::from_external_id(was_revision_of_ref.activity)),
            DerivationType::Revision,
        ))
    }

    Ok(with_implied(operations))
}
