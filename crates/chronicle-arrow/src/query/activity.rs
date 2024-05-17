use std::{collections::HashMap, sync::Arc};

use arrow::array::{ArrayBuilder, ListBuilder, StringBuilder, StructBuilder};
use arrow_array::{
    Array, BooleanArray, Int64Array, ListArray, RecordBatch, StringArray, TimestampNanosecondArray,
};
use arrow_schema::{DataType, Field};
use chrono::{DateTime, Utc};
use diesel::{
    pg::PgConnection,
    prelude::*,
    r2d2::{ConnectionManager, Pool},
};
use uuid::Uuid;

use chronicle_persistence::{
    query::{Activity, Association, Delegation, Generation, Namespace, Usage, WasInformedBy},
    schema::{
        activity, agent, association, delegation, entity, generation, namespace, usage,
        wasinformedby,
    },
};
use common::{
    attributes::Attributes,
    domain::PrimitiveType,
    prov::{DomaintypeId, ExternalIdPart},
};

use crate::{ChronicleArrowError, DomainTypeMeta};

use super::vec_vec_string_to_list_array;

#[tracing::instrument(skip(pool))]
pub fn activity_count_by_type(
    pool: &Pool<ConnectionManager<PgConnection>>,
    typ: Vec<&str>,
) -> Result<i64, ChronicleArrowError> {
    let mut connection = pool.get()?;
    let count = activity::table
        .filter(activity::domaintype.eq_any(typ))
        .count()
        .get_result(&mut connection)?;
    Ok(count)
}

#[derive(Default)]
pub struct AgentInteraction {
    pub(crate) agent: String,
    pub(crate) role: Option<String>,
}

#[derive(Default)]
pub struct ActivityAssociationRef {
    pub(crate) responsible: AgentInteraction,
    pub(crate) delegated: Vec<AgentInteraction>,
}

#[derive(Default)]
pub struct ActivityAndReferences {
    pub(crate) id: String,
    pub(crate) namespace_name: String,
    pub(crate) namespace_uuid: [u8; 16],
    pub(crate) started: Option<DateTime<Utc>>,
    pub(crate) ended: Option<DateTime<Utc>>,
    pub(crate) attributes: Attributes,
    pub(crate) used: Vec<String>,
    pub(crate) generated: Vec<String>,
    pub(crate) was_informed_by: Vec<String>,
    pub(crate) was_associated_with: Vec<ActivityAssociationRef>,
}

impl ActivityAndReferences {
    #[tracing::instrument(skip(items, meta))]
    pub fn to_record_batch(
        items: impl Iterator<Item=ActivityAndReferences>,
        meta: &DomainTypeMeta,
    ) -> Result<RecordBatch, ChronicleArrowError> {
        let mut attributes_map: HashMap<String, (PrimitiveType, Vec<Option<serde_json::Value>>)> =
            HashMap::new();

        for (attribute_name, primitive_type) in meta.attributes.iter() {
            attributes_map.insert(attribute_name.to_string(), (*primitive_type, vec![]));
        }

        let mut id_vec = Vec::new();
        let mut namespace_name_vec = Vec::new();
        let mut namespace_uuid_vec = Vec::new();
        let mut started_vec = Vec::new();
        let mut ended_vec = Vec::new();
        let mut used_vec = Vec::new();
        let mut generated_vec = Vec::new();
        let mut was_informed_by_vec = Vec::new();
        let mut was_associated_with_vec = Vec::new();

        for item in items {
            id_vec.push(item.id);
            namespace_name_vec.push(item.namespace_name);
            namespace_uuid_vec.push(Uuid::from_bytes(item.namespace_uuid).to_string());
            started_vec.push(item.started.map(|dt| dt.timestamp_nanos_opt().unwrap_or_default()));
            ended_vec.push(item.ended.map(|dt| dt.timestamp_nanos_opt().unwrap_or_default()));
            used_vec.push(item.used);
            generated_vec.push(item.generated);
            was_informed_by_vec.push(item.was_informed_by);
            was_associated_with_vec.push(item.was_associated_with);

            for (key, (_primitive_type, values)) in attributes_map.iter_mut() {
                if let Some(attribute) = item.attributes.get_attribute(key) {
                    values.push(Some(attribute.value.clone().into()));
                } else {
                    values.push(None);
                }
            }
        }

        let used_array = vec_vec_string_to_list_array(used_vec)?;
        let generated_array = vec_vec_string_to_list_array(generated_vec)?;
        let was_informed_by_array = vec_vec_string_to_list_array(was_informed_by_vec)?;
        let was_associated_with_array = associations_to_list_array(was_associated_with_vec)?;

        let mut fields = vec![
            (
                "namespace_name".to_string(),
                Arc::new(StringArray::from(namespace_name_vec)) as Arc<dyn arrow_array::Array>,
            ),
            (
                "namespace_uuid".to_string(),
                Arc::new(StringArray::from(namespace_uuid_vec)) as Arc<dyn arrow_array::Array>,
            ),
            ("id".to_string(), Arc::new(StringArray::from(id_vec)) as Arc<dyn arrow_array::Array>),
        ];

        // Dynamically generate fields for attribute key/values based on their primitive type
        for (key, (primitive_type, values)) in attributes_map {
            let array: Arc<dyn arrow_array::Array> = match primitive_type {
                PrimitiveType::String => {
                    tracing::debug!("Converting String attribute values for key: {}", key);
                    Arc::new(StringArray::from(
                        values
                            .iter()
                            .map(|v| v.as_ref().map(|v| v.as_str()).unwrap_or_default())
                            .collect::<Vec<_>>(),
                    )) as Arc<dyn arrow_array::Array>
                }
                PrimitiveType::Int => {
                    tracing::debug!("Converting Int attribute values for key: {}", key);
                    Arc::new(Int64Array::from(
                        values
                            .iter()
                            .map(|v| v.as_ref().map(|v| v.as_i64()).unwrap_or_default())
                            .collect::<Vec<_>>(),
                    )) as Arc<dyn arrow_array::Array>
                }
                PrimitiveType::Bool => {
                    tracing::debug!("Converting Bool attribute values for key: {}", key);
                    Arc::new(BooleanArray::from(
                        values
                            .iter()
                            .map(|v| v.as_ref().map(|v| v.as_bool()).unwrap_or_default())
                            .collect::<Vec<_>>(),
                    )) as Arc<dyn arrow_array::Array>
                }
                _ => {
                    tracing::warn!("Unsupported attribute primitive type for key: {}", key);
                    continue;
                }
            };
            fields.push((key, array as Arc<dyn arrow_array::Array>));
        }

        fields.extend(vec![
            (
                "started".to_string(),
                Arc::new(TimestampNanosecondArray::with_timezone_opt(
                    started_vec.into(),
                    Some("UTC".to_string()),
                )) as Arc<dyn arrow_array::Array>,
            ),
            (
                "ended".to_string(),
                Arc::new(TimestampNanosecondArray::with_timezone_opt(
                    ended_vec.into(),
                    Some("UTC".to_string()),
                )) as Arc<dyn arrow_array::Array>,
            ),
            ("used".to_string(), Arc::new(used_array) as Arc<dyn arrow_array::Array>),
            ("generated".to_string(), Arc::new(generated_array) as Arc<dyn arrow_array::Array>),
            (
                "was_informed_by".to_string(),
                Arc::new(was_informed_by_array) as Arc<dyn arrow_array::Array>,
            ),
            (
                "was_associated_with".to_string(),
                Arc::new(was_associated_with_array) as Arc<dyn arrow_array::Array>,
            ),
        ]);

        let hashed_fields = fields.into_iter().collect::<HashMap<_, _>>();

        let mut columns = Vec::new();
        for field in meta.schema.fields() {
            let field_name = field.name();
            match hashed_fields.get(field_name) {
                Some(array) => columns.push(array.clone()),
                None =>
                    return Err(ChronicleArrowError::SchemaFieldNotFound(field_name.to_string())),
            }
        }

        RecordBatch::try_new(meta.schema.clone(), columns).map_err(ChronicleArrowError::from)
    }
}

fn associations_to_list_array(
    associations: Vec<Vec<ActivityAssociationRef>>,
) -> Result<ListArray, ChronicleArrowError> {
    let fields =
        vec![Field::new("agent", DataType::Utf8, false), Field::new("role", DataType::Utf8, true)];

    let agent_struct = DataType::Struct(fields.clone().into());

    let mut builder = ListBuilder::new(StructBuilder::new(
        vec![
            Field::new("responsible", agent_struct.clone(), false),
            Field::new(
                "delegated",
                DataType::List(Arc::new(Field::new("item", agent_struct, true))),
                false,
            ),
        ],
        vec![
            Box::new(StructBuilder::from_fields(fields.clone(), 0)),
            Box::new(ListBuilder::new(StructBuilder::from_fields(fields, 0))),
        ],
    ));

    for association_vec in associations {
        let struct_builder = builder.values();

        for association in association_vec {
            // Build the responsible field
            let responsible_builder = struct_builder.field_builder::<StructBuilder>(0).unwrap();
            responsible_builder
                .field_builder::<StringBuilder>(0)
                .unwrap()
                .append_value(&association.responsible.agent);
            if let Some(role) = &association.responsible.role {
                responsible_builder
                    .field_builder::<StringBuilder>(1)
                    .unwrap()
                    .append_value(role);
            } else {
                responsible_builder.field_builder::<StringBuilder>(1).unwrap().append_null();
            }
            responsible_builder.append(true);

            // Build the delegated field
            let delegated_builder =
                struct_builder.field_builder::<ListBuilder<StructBuilder>>(1).unwrap();
            for agent_interaction in &association.delegated {
                let interaction_builder = delegated_builder.values();
                interaction_builder
                    .field_builder::<StringBuilder>(0)
                    .unwrap()
                    .append_value(&agent_interaction.agent);
                if let Some(role) = &agent_interaction.role {
                    interaction_builder
                        .field_builder::<StringBuilder>(1)
                        .unwrap()
                        .append_value(role);
                } else {
                    interaction_builder.field_builder::<StringBuilder>(1).unwrap().append_null();
                }
                interaction_builder.append(true);
            }
            delegated_builder.append(true);

            struct_builder.append(true);
        }

        builder.append(true);
    }

    Ok(builder.finish())
}

pub fn load_activities_by_type(
    pool: &Pool<ConnectionManager<PgConnection>>,
    typ: &Option<DomaintypeId>,
    position: u64,
    max_records: u64,
) -> Result<(impl Iterator<Item=ActivityAndReferences>, u64, u64), ChronicleArrowError> {
    let mut connection = pool.get().map_err(ChronicleArrowError::PoolError)?;

    let activities_and_namespaces: Vec<(Activity, Namespace)> = match typ {
        Some(typ_value) => activity::table
            .inner_join(namespace::table.on(activity::namespace_id.eq(namespace::id)))
            .filter(activity::domaintype.eq(typ_value.external_id_part()))
            .order(activity::id)
            .select((Activity::as_select(), Namespace::as_select()))
            .offset(position as i64)
            .limit(max_records as i64)
            .load(&mut connection)?,
        None => activity::table
            .inner_join(namespace::table.on(activity::namespace_id.eq(namespace::id)))
            .filter(activity::domaintype.is_null())
            .order(activity::id)
            .select((Activity::as_select(), Namespace::as_select()))
            .offset(position as i64)
            .limit(max_records as i64)
            .load(&mut connection)?,
    };

    let (activities, namespaces): (Vec<Activity>, Vec<Namespace>) =
        activities_and_namespaces.into_iter().unzip();

    let mut was_informed_by_map: HashMap<i32, Vec<String>> =
        WasInformedBy::belonging_to(&activities)
            .inner_join(activity::table.on(wasinformedby::informing_activity_id.eq(activity::id)))
            .select((wasinformedby::activity_id, activity::external_id))
            .load::<(i32, String)>(&mut connection)?
            .into_iter()
            .fold(HashMap::new(), |mut acc: HashMap<i32, Vec<String>>, (id, external_id)| {
                acc.entry(id).or_default().push(external_id);
                acc
            });

    let mut used_map: HashMap<i32, Vec<String>> = Usage::belonging_to(&activities)
        .inner_join(entity::table.on(usage::entity_id.eq(entity::id)))
        .select((usage::activity_id, entity::external_id))
        .load::<(i32, String)>(&mut connection)?
        .into_iter()
        .fold(HashMap::new(), |mut acc: HashMap<i32, Vec<String>>, (id, external_id)| {
            acc.entry(id).or_default().push(external_id);
            acc
        });

    let mut generated_map: HashMap<i32, Vec<String>> = Generation::belonging_to(&activities)
        .inner_join(entity::table.on(generation::generated_entity_id.eq(entity::id)))
        .select((generation::activity_id, entity::external_id))
        .load::<(i32, String)>(&mut connection)?
        .into_iter()
        .fold(HashMap::new(), |mut acc: HashMap<i32, Vec<String>>, (id, external_id)| {
            acc.entry(id).or_default().push(external_id);
            acc
        });

    let associations_map: HashMap<i32, HashMap<i32, (String, String)>> =
        Association::belonging_to(&activities)
            .inner_join(agent::table.on(association::agent_id.eq(agent::id)))
            .select((association::activity_id, (agent::id, agent::external_id, association::role)))
            .load::<(i32, (i32, String, String))>(&mut connection)?
            .into_iter()
            .fold(
                HashMap::new(),
                |mut acc: HashMap<i32, HashMap<i32, (String, String)>>,
                 (activity_id, (agent_id, agent_external_id, role_external_id))| {
                    acc.entry(activity_id)
                        .or_default()
                        .insert(agent_id, (agent_external_id, role_external_id));
                    acc
                },
            );

    let delegations_map: HashMap<i32, HashMap<i32, (String, String)>> =
        Delegation::belonging_to(&activities)
            .inner_join(agent::table.on(delegation::delegate_id.eq(agent::id)))
            .select((
                delegation::activity_id,
                (delegation::responsible_id, agent::external_id, delegation::role),
            ))
            .load::<(i32, (i32, String, String))>(&mut connection)?
            .into_iter()
            .fold(
                HashMap::new(),
                |mut acc: HashMap<i32, HashMap<i32, (String, String)>>,
                 (activity_id, (agent_id, agent_external_id, role_external_id))| {
                    acc.entry(activity_id)
                        .or_default()
                        .insert(agent_id, (agent_external_id, role_external_id));
                    acc
                },
            );

    let mut activity_associations: HashMap<i32, Vec<ActivityAssociationRef>> = HashMap::new();

    for (activity_id, agent_map) in associations_map.into_iter() {
        let mut association_refs = Vec::new();
        for (agent_id, (agent_external_id, role_external_id)) in agent_map.into_iter() {
            let mut delegated_agents = Vec::new();
            if let Some(delegations) = delegations_map.get(&activity_id) {
                if let Some((delegated_agent_external_id, delegated_role_external_id)) =
                    delegations.get(&agent_id)
                {
                    delegated_agents.push(AgentInteraction {
                        agent: delegated_agent_external_id.clone(),
                        role: Some(delegated_role_external_id.clone()),
                    });
                }
            }
            association_refs.push(ActivityAssociationRef {
                responsible: AgentInteraction {
                    agent: agent_external_id,
                    role: Some(role_external_id),
                },
                delegated: delegated_agents,
            });
        }
        activity_associations.insert(activity_id, association_refs);
    }
    let fetched_records = activities.len() as u64;

    let mut activities_and_references = vec![];

    for (activity, ns) in activities.into_iter().zip(namespaces) {
        activities_and_references.push(ActivityAndReferences {
            id: activity.external_id,
            namespace_name: ns.external_id,
            namespace_uuid: Uuid::parse_str(&ns.uuid)?.into_bytes(),
            attributes: Attributes::new(
                activity.domaintype.map(DomaintypeId::from_external_id),
                vec![],
            ), // Placeholder for attribute loading logic
            started: activity.started.map(|dt| dt.and_utc()),
            ended: activity.ended.map(|dt| dt.and_utc()),
            was_informed_by: was_informed_by_map.remove(&activity.id).unwrap_or_default(),
            used: used_map.remove(&activity.id).unwrap_or_default(),
            generated: generated_map.remove(&activity.id).unwrap_or_default(),
            was_associated_with: activity_associations.remove(&activity.id).unwrap_or_default(),
        });
    }
    Ok((activities_and_references.into_iter(), fetched_records, fetched_records))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_associations_to_list_array_empty() {
        let associations = Vec::new();
        let result = associations_to_list_array(associations);
        assert!(result.is_ok());
        let array = result.unwrap();
        assert_eq!(array.len(), 0);
    }

    #[test]
    fn test_associations_to_list_array_single() {
        let associations = vec![ActivityAssociationRef {
            responsible: AgentInteraction {
                agent: "agent1".to_string(),
                role: Some("role1".to_string()),
            },
            delegated: vec![AgentInteraction {
                agent: "delegated1".to_string(),
                role: Some("role3".to_string()),
            }],
        }];
        let result = associations_to_list_array(vec![associations]).unwrap();

        let json = arrow::json::writer::array_to_json_array(&result).unwrap();

        insta::assert_debug_snapshot!(&json, @r###"
  [
      Array [
          Object {
              "delegated": Array [
                  Object {
                      "agent": String("delegated1"),
                      "role": String("role3"),
                  },
              ],
              "responsible": Object {
                  "agent": String("agent1"),
                  "role": String("role1"),
              },
          },
      ],
  ]
  "### );
    }

    #[test]
    fn test_associations_to_list_array_multiple() {
        let associations = vec![
            ActivityAssociationRef {
                responsible: AgentInteraction {
                    agent: "agent1".to_string(),
                    role: Some("role1".to_string()),
                },
                delegated: vec![],
            },
            ActivityAssociationRef {
                responsible: AgentInteraction {
                    agent: "agent2".to_string(),
                    role: Some("role2".to_string()),
                },
                delegated: vec![
                    AgentInteraction {
                        agent: "delegated1".to_string(),
                        role: Some("role3".to_string()),
                    },
                    AgentInteraction {
                        agent: "delegated2".to_string(),
                        role: Some("role3".to_string()),
                    },
                ],
            },
        ];
        let result = associations_to_list_array(vec![associations]).unwrap();

        let json = arrow::json::writer::array_to_json_array(&result).unwrap();

        insta::assert_debug_snapshot!(&json, @r###"
  [
      Array [
          Object {
              "delegated": Array [],
              "responsible": Object {
                  "agent": String("agent1"),
                  "role": String("role1"),
              },
          },
          Object {
              "delegated": Array [
                  Object {
                      "agent": String("delegated1"),
                      "role": String("role3"),
                  },
                  Object {
                      "agent": String("delegated2"),
                      "role": String("role3"),
                  },
              ],
              "responsible": Object {
                  "agent": String("agent2"),
                  "role": String("role2"),
              },
          },
      ],
  ]
  "### );
    }
}
