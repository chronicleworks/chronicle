use arrow_array::RecordBatch;

use common::{
    attributes::Attributes,
    prov::{
        ActivityId,
        AgentId, EntityId, NamespaceId, operations::{ChronicleOperation, SetAttributes}, Role,
    },
};

use crate::{
    ChronicleArrowError,
    query::{ActedOnBehalfOfRef, AgentAttributionRef},
};

use super::{struct_2_list_column_opt_string, struct_3_list_column_opt_string, with_implied};

fn get_agent_attribution(
    record_batch: &RecordBatch,
    row_index: usize,
) -> Result<Vec<AgentAttributionRef>, ChronicleArrowError> {
    Ok(struct_2_list_column_opt_string(
        record_batch,
        "was_attributed_to",
        row_index,
        "entity",
        "role",
    )?
        .into_iter()
        .map(|(entity, role)| AgentAttributionRef { entity, role })
        .collect())
}

fn get_acted_on_behalf_of(
    record_batch: &RecordBatch,
    row_index: usize,
) -> Result<Vec<ActedOnBehalfOfRef>, ChronicleArrowError> {
    Ok(struct_3_list_column_opt_string(
        record_batch,
        "acted_on_behalf_of",
        row_index,
        "agent",
        "activity",
        "role",
    )?
        .into_iter()
        .map(|(agent, activity, role)| ActedOnBehalfOfRef { agent, role, activity })
        .collect())
}

pub fn agent_operations(
    ns: &NamespaceId,
    id: &str,
    attributes: Attributes,
    row_index: usize,
    record_batch: &RecordBatch,
) -> Result<Vec<ChronicleOperation>, ChronicleArrowError> {
    let mut operations = vec![
        ChronicleOperation::agent_exists(ns.clone(), AgentId::from_external_id(id)),
        ChronicleOperation::set_attributes(SetAttributes::agent(
            ns.clone(),
            AgentId::from_external_id(id),
            attributes,
        )),
    ];

    let was_attributed_to_refs = get_agent_attribution(record_batch, row_index)?;

    for was_attributed_to_ref in was_attributed_to_refs {
        operations.push(ChronicleOperation::was_attributed_to(
            ns.clone(),
            EntityId::from_external_id(was_attributed_to_ref.entity),
            AgentId::from_external_id(id),
            was_attributed_to_ref.role.map(Role::from),
        ));
    }

    let acted_on_behalf_of_refs = get_acted_on_behalf_of(record_batch, row_index)?;

    for acted_on_behalf_of_ref in acted_on_behalf_of_refs {
        operations.push(ChronicleOperation::agent_acts_on_behalf_of(
            ns.clone(),
            AgentId::from_external_id(id),
            AgentId::from_external_id(acted_on_behalf_of_ref.agent),
            Some(ActivityId::from_external_id(acted_on_behalf_of_ref.activity)),
            acted_on_behalf_of_ref.role.map(Role::from),
        ));
    }

    Ok(with_implied(operations))
}
