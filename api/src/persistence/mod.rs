use std::{cell::RefCell, collections::HashMap, str::FromStr};

use chrono::DateTime;

use chrono::Utc;
use common::models::EntityId;
use common::{
    models::{
        Activity, ActivityId, Agent, AgentId, ChronicleTransaction, Entity, Namespace, NamespaceId,
        ProvModel,
    },
    vocab::Chronicle,
};
use custom_error::custom_error;
use derivative::*;
use diesel::{dsl::max, prelude::*, sqlite::SqliteConnection};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use tracing::debug;
use tracing::{instrument, trace};
use uuid::Uuid;

use crate::QueryCommand;

mod query;
mod schema;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

custom_error! {pub StoreError
    Db{source: diesel::result::Error}                           = "Database operation failed",
    DbConnection{source: diesel::ConnectionError}               = "Database connection failed",
    DbMigration{source: diesel_migrations::MigrationError}      = "Database migration failed",
    Uuid{source: uuid::Error}                                   = "Invalid UUID string",
    RecordNotFound{}                                            = "Could not locate record in store",
    InvalidNamespace{}                                          = "Could not find namespace",
    ModelDoesNotContainActivity{activityid: ActivityId}         = "Could not locate {} in activities",
    ModelDoesNotContainAgent{agentid: AgentId}                  = "Could not locate {} in agents",
    ModelDoesNotContainEntity{entityid: EntityId}               = "Could not locate {} in entities",
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct Store {
    #[derivative(Debug = "ignore")]
    connection: RefCell<SqliteConnection>,
}

impl Store {
    pub fn new(database_url: &str) -> Result<Self, StoreError> {
        let mut connection = SqliteConnection::establish(database_url)?;
        connection.run_pending_migrations(MIGRATIONS).unwrap();

        Ok(Store {
            connection: connection.into(),
        })
    }

    ///TODO: any sort of query design, this should fundamentally be for export / streaming into external data pipelines or we will end up with an embedded triple store
    #[instrument]
    pub fn prov_model_from(&self, query: QueryCommand) -> Result<ProvModel, StoreError> {
        let mut model = ProvModel::default();

        let agents = schema::agent::table
            .filter(schema::agent::namespace.eq(&query.namespace))
            .load::<query::Agent>(&mut *self.connection.borrow_mut())?;

        for agent in agents {
            debug!(?agent, "Map agent to prov");
            let agentid: AgentId = Chronicle::agent(&agent.name).into();
            let namespaceid = self.namespace_by_name(&agent.namespace)?;
            model.agents.insert(
                agentid.clone(),
                Agent {
                    id: agentid.clone(),
                    namespaceid,
                    name: agent.name,
                    publickey: agent.publickey,
                },
            );

            for asoc in schema::wasassociatedwith::table
                .filter(schema::wasassociatedwith::agent.eq(agent.id))
                .inner_join(schema::activity::table)
                .select(schema::activity::name)
                .load_iter::<String>(&mut *self.connection.borrow_mut())?
            {
                let asoc = asoc?;
                model.associate_with(Chronicle::activity(&asoc).into(), &agentid);
            }
        }

        let activities = schema::activity::table
            .filter(schema::activity::namespace.eq(&query.namespace))
            .load::<query::Activity>(&mut *self.connection.borrow_mut())?;

        for activity in activities {
            debug!(?activity, "Map activity to prov");

            let id: ActivityId = Chronicle::activity(&activity.name).into();
            let namespaceid = self.namespace_by_name(&activity.namespace)?;
            model.activities.insert(
                id.clone(),
                Activity {
                    id: id.clone(),
                    namespaceid,
                    name: activity.name,
                    started: activity.started.map(|x| DateTime::from_utc(x, Utc)),
                    ended: activity.ended.map(|x| DateTime::from_utc(x, Utc)),
                },
            );

            for asoc in schema::wasgeneratedby::table
                .filter(schema::wasgeneratedby::activity.eq(activity.id))
                .inner_join(schema::entity::table)
                .select(schema::entity::name)
                .load_iter::<String>(&mut *self.connection.borrow_mut())?
            {
                let asoc = asoc?;
                model.generate_by(Chronicle::entity(&asoc).into(), &id);
            }

            for used in schema::used::table
                .filter(schema::used::activity.eq(activity.id))
                .inner_join(schema::entity::table)
                .select(schema::entity::name)
                .load_iter::<String>(&mut *self.connection.borrow_mut())?
            {
                let used = used?;
                model.used(id.clone(), &Chronicle::entity(&used).into());
            }
        }

        let entites = schema::entity::table
            .filter(schema::entity::namespace.eq(query.namespace))
            .load::<query::Entity>(&mut *self.connection.borrow_mut())?;

        for entity in entites {
            debug!(?entity, "Map entity to prov");
            let id: EntityId = Chronicle::entity(&entity.name).into();
            let namespaceid = self.namespace_by_name(&entity.namespace)?;
            model.entities.insert(id.clone(), {
                match entity {
                    query::Entity {
                        name,
                        signature: Some(signature),
                        locator,
                        signature_time: Some(signature_time),
                        ..
                    } => Entity::Signed {
                        id,
                        namespaceid,
                        name,
                        signature,
                        signature_time: DateTime::from_utc(signature_time, Utc),
                        locator,
                    },
                    query::Entity { name, .. } => Entity::Unsigned {
                        id,
                        namespaceid,
                        name,
                    },
                }
            });
        }

        Ok(model)
    }

    /// Apply a chronicle transaction to the store idempotently and return a prov model relevant to the transaction
    #[instrument]
    pub fn apply(&self, tx: &ChronicleTransaction) -> Result<ProvModel, StoreError> {
        let model = ProvModel::from_tx(vec![tx]);

        trace!(?model);

        self.idempotently_apply_model(&model)?;

        Ok(model)
    }

    fn idempotently_apply_model(&self, model: &ProvModel) -> Result<(), StoreError> {
        for (_, ns) in model.namespaces.iter() {
            self.apply_namespace(ns)?
        }
        for (_, agent) in model.agents.iter() {
            self.apply_agent(agent, &model.namespaces)?
        }
        for (_, activity) in model.activities.iter() {
            self.apply_activity(activity, &model.namespaces)?
        }

        for (_, entity) in model.entities.iter() {
            self.apply_entity(entity, &model.namespaces)?
        }

        for (activityid, agentid) in model.was_associated_with.iter() {
            for agentid in agentid {
                self.apply_was_associated_with(model, activityid, agentid)?;
            }
        }

        for (activityid, entityid) in model.used.iter() {
            for entityid in entityid {
                self.apply_used(model, activityid, entityid)?;
            }
        }

        for (entityid, activityid) in model.was_generated_by.iter() {
            for activityid in activityid {
                self.apply_was_generated_by(model, entityid, activityid)?;
            }
        }

        Ok(())
    }

    #[instrument]
    fn apply_namespace(
        &self,
        Namespace {
            ref name, ref uuid, ..
        }: &Namespace,
    ) -> Result<(), StoreError> {
        diesel::insert_or_ignore_into(schema::namespace::table)
            .values(&query::NewNamespace {
                name,
                uuid: &uuid.to_string(),
            })
            .execute(&mut *self.connection.borrow_mut())?;

        Ok(())
    }

    #[instrument]
    pub(crate) fn namespace_by_name(&self, namespace: &str) -> Result<NamespaceId, StoreError> {
        use self::schema::namespace::dsl as ns;
        let ns = ns::namespace
            .filter(ns::name.eq(namespace))
            .first::<query::Namespace>(&mut *self.connection.borrow_mut())
            .optional()?
            .ok_or(StoreError::RecordNotFound {})?;

        Ok(Chronicle::namespace(&ns.name, &Uuid::from_str(&ns.uuid)?).into())
    }

    #[instrument]
    fn apply_agent(
        &self,
        Agent {
            ref name,
            namespaceid,
            publickey,
            id: _,
        }: &Agent,
        ns: &HashMap<NamespaceId, Namespace>,
    ) -> Result<(), StoreError> {
        let namespace = ns.get(namespaceid).ok_or(StoreError::InvalidNamespace {})?;
        diesel::insert_or_ignore_into(schema::agent::table)
            .values(&query::NewAgent {
                name,
                namespace: &namespace.name,
                current: 0,
                publickey: publickey.as_deref(),
            })
            .execute(&mut *self.connection.borrow_mut())?;

        Ok(())
    }

    #[instrument]
    fn apply_activity(
        &self,
        Activity {
            ref name,
            id,
            namespaceid,
            started,
            ended,
        }: &Activity,
        ns: &HashMap<NamespaceId, Namespace>,
    ) -> Result<(), StoreError> {
        let namespace = ns.get(namespaceid).ok_or(StoreError::InvalidNamespace {})?;
        diesel::insert_or_ignore_into(schema::activity::table)
            .values(&query::NewActivity {
                name,
                namespace: &namespace.name,
                started: started.map(|t| t.naive_utc()),
                ended: ended.map(|t| t.naive_utc()),
            })
            .execute(&mut *self.connection.borrow_mut())?;

        Ok(())
    }

    pub(crate) fn use_agent(&self, name: String, namespace: String) -> Result<(), StoreError> {
        use schema::agent::dsl;
        diesel::update(schema::agent::table.filter(dsl::current.ne(0)))
            .set(dsl::current.eq(0))
            .execute(&mut *self.connection.borrow_mut())?;

        diesel::update(
            schema::agent::table.filter(dsl::name.eq(name).and(dsl::namespace.eq(namespace))),
        )
        .set(dsl::current.eq(1))
        .execute(&mut *self.connection.borrow_mut())?;

        Ok(())
    }

    pub(crate) fn get_current_agent(&self) -> Result<query::Agent, StoreError> {
        use schema::agent::dsl;
        Ok(schema::agent::table
            .filter(dsl::current.ne(0))
            .first::<query::Agent>(&mut *self.connection.borrow_mut())?)
    }

    /// Ensure the name is unique within the namespace, if not, then postfix the rowid
    pub(crate) fn disambiguate_entity_name(&self, name: &str) -> Result<String, StoreError> {
        use schema::entity::dsl;

        let collision = schema::entity::table
            .filter(dsl::name.eq(name))
            .count()
            .first::<i64>(&mut *self.connection.borrow_mut())?;

        if collision == 0 {
            return Ok(name.to_owned());
        }

        let ambiguous = schema::entity::table
            .select(max(dsl::id))
            .first::<Option<i32>>(&mut *self.connection.borrow_mut())?;

        Ok(format!("{}_{}", name, ambiguous.unwrap_or_default()))
    }

    /// Ensure the name is unique within the namespace, if not, then postfix the rowid
    pub(crate) fn disambiguate_agent_name(&self, name: &str) -> Result<String, StoreError> {
        use schema::agent::dsl;

        let collision = schema::agent::table
            .filter(dsl::name.eq(name))
            .count()
            .first::<i64>(&mut *self.connection.borrow_mut())?;

        if collision == 0 {
            return Ok(name.to_owned());
        }

        let ambiguous = schema::agent::table
            .select(max(dsl::id))
            .first::<Option<i32>>(&mut *self.connection.borrow_mut())?;

        Ok(format!("{}_{}", name, ambiguous.unwrap_or_default()))
    }

    /// Ensure the name is unique within the namespace, if not, then postfix the rowid
    pub(crate) fn disambiguate_activity_name(&self, name: &str) -> Result<String, StoreError> {
        use schema::activity::dsl;

        let collision = schema::activity::table
            .filter(dsl::name.eq(name))
            .count()
            .first::<i64>(&mut *self.connection.borrow_mut())?;

        if collision == 0 {
            return Ok(name.to_owned());
        }

        let ambiguous = schema::activity::table
            .select(max(dsl::id))
            .first::<Option<i32>>(&mut *self.connection.borrow_mut())?;

        Ok(format!("{}_{}", name, ambiguous.unwrap_or_default()))
    }

    /// Fetch the activity record for the IRI
    fn activity_by_activity_name_and_namespace(
        &self,
        name: &str,
        namespace: &str,
    ) -> Result<query::Activity, StoreError> {
        use schema::activity::dsl;

        Ok(schema::activity::table
            .filter(dsl::name.eq(name).and(dsl::namespace.eq(namespace)))
            .first::<query::Activity>(&mut *self.connection.borrow_mut())?)
    }

    /// Fetch the agent record for the IRI
    pub(crate) fn agent_by_agent_name_and_namespace(
        &self,
        name: &str,
        namespace: &str,
    ) -> Result<query::Agent, StoreError> {
        use schema::agent::dsl;

        Ok(schema::agent::table
            .filter(dsl::name.eq(name).and(dsl::namespace.eq(namespace)))
            .first::<query::Agent>(&mut *self.connection.borrow_mut())?)
    }

    pub(crate) fn entity_by_entity_name_and_namespace(
        &self,
        name: &str,
        namespace: &str,
    ) -> Result<query::Entity, StoreError> {
        use schema::entity::dsl;

        Ok(schema::entity::table
            .filter(dsl::name.eq(name).and(dsl::namespace.eq(namespace)))
            .first::<query::Entity>(&mut *self.connection.borrow_mut())?)
    }

    /// Get the named acitvity or the last started one, a useful context aware shortcut for the CLI
    pub(crate) fn get_activity_by_name_or_last_started(
        &self,
        name: Option<String>,
        namespace: Option<String>,
    ) -> Result<query::Activity, StoreError> {
        use schema::activity::dsl;

        match (name, namespace) {
            (Some(name), Some(namespace)) => {
                Ok(self.activity_by_activity_name_and_namespace(&name, &namespace)?)
            }
            _ => Ok(schema::activity::table
                .order(dsl::started)
                .first::<query::Activity>(&mut *self.connection.borrow_mut())?),
        }
    }

    #[instrument]
    fn apply_entity(
        &self,
        entity: &common::models::Entity,
        ns: &HashMap<NamespaceId, Namespace>,
    ) -> Result<(), StoreError> {
        use schema::entity::dsl;
        let namespace = ns
            .get(entity.namespaceid())
            .ok_or(StoreError::InvalidNamespace {})?;

        diesel::insert_into(schema::entity::table)
            .values((
                dsl::name.eq(entity.name()),
                dsl::namespace.eq(&namespace.name),
            ))
            .on_conflict((dsl::name, dsl::namespace))
            .do_nothing()
            .execute(&mut *self.connection.borrow_mut())?;

        if let Entity::Signed {
            signature,
            signature_time,
            locator,
            ..
        } = entity
        {
            diesel::update(schema::entity::table)
                .filter(
                    dsl::name
                        .eq(entity.name())
                        .and(dsl::namespace.eq(&namespace.name)),
                )
                .set((
                    dsl::locator.eq(locator),
                    dsl::signature.eq(signature),
                    dsl::signature_time.eq(signature_time.naive_utc()),
                ))
                .execute(&mut *self.connection.borrow_mut())?;
        }

        Ok(())
    }

    #[instrument]
    fn apply_was_associated_with(
        &self,
        model: &ProvModel,
        activityid: &common::models::ActivityId,
        agentid: &common::models::AgentId,
    ) -> Result<(), StoreError> {
        let provagent =
            model
                .agents
                .get(agentid)
                .ok_or_else(|| StoreError::ModelDoesNotContainAgent {
                    agentid: agentid.clone(),
                })?;
        let provactivity = model.activities.get(activityid).ok_or_else(|| {
            StoreError::ModelDoesNotContainActivity {
                activityid: activityid.clone(),
            }
        })?;

        let storedactivity = self.activity_by_activity_name_and_namespace(
            &provactivity.name,
            provactivity.namespaceid.decompose().0,
        )?;

        let storedagent = self.agent_by_agent_name_and_namespace(
            &provagent.name,
            provagent.namespaceid.decompose().0,
        )?;

        use schema::wasassociatedwith::dsl as link;
        diesel::insert_or_ignore_into(schema::wasassociatedwith::table)
            .values((
                &link::activity.eq(storedactivity.id),
                &link::agent.eq(storedagent.id),
            ))
            .execute(&mut *self.connection.borrow_mut())?;

        Ok(())
    }

    #[instrument]
    fn apply_was_generated_by(
        &self,
        model: &ProvModel,
        entityid: &common::models::EntityId,
        activityid: &ActivityId,
    ) -> Result<(), StoreError> {
        let proventity =
            model
                .entities
                .get(entityid)
                .ok_or_else(|| StoreError::ModelDoesNotContainEntity {
                    entityid: entityid.clone(),
                })?;
        let provactivity = model.activities.get(activityid).ok_or_else(|| {
            StoreError::ModelDoesNotContainActivity {
                activityid: activityid.clone(),
            }
        })?;

        let storedactivity = self.activity_by_activity_name_and_namespace(
            &provactivity.name,
            provactivity.namespaceid.decompose().0,
        )?;

        let storedentity = self.entity_by_entity_name_and_namespace(
            proventity.name(),
            proventity.namespaceid().decompose().0,
        )?;

        use schema::wasgeneratedby::dsl as link;
        diesel::insert_or_ignore_into(schema::wasgeneratedby::table)
            .values((
                &link::activity.eq(storedactivity.id),
                &link::entity.eq(storedentity.id),
            ))
            .execute(&mut *self.connection.borrow_mut())?;

        Ok(())
    }

    #[instrument]
    fn apply_used(
        &self,
        model: &ProvModel,
        activityid: &ActivityId,
        entityid: &common::models::EntityId,
    ) -> Result<(), StoreError> {
        let proventity =
            model
                .entities
                .get(entityid)
                .ok_or_else(|| StoreError::ModelDoesNotContainEntity {
                    entityid: entityid.clone(),
                })?;
        let provactivity = model.activities.get(activityid).ok_or_else(|| {
            StoreError::ModelDoesNotContainActivity {
                activityid: activityid.clone(),
            }
        })?;

        let storedactivity = self.activity_by_activity_name_and_namespace(
            &provactivity.name,
            provactivity.namespaceid.decompose().0,
        )?;

        let storedentity = self.entity_by_entity_name_and_namespace(
            proventity.name(),
            proventity.namespaceid().decompose().0,
        )?;

        use schema::used::dsl as link;
        diesel::insert_or_ignore_into(schema::used::table)
            .values((
                &link::activity.eq(storedactivity.id),
                &link::entity.eq(storedentity.id),
            ))
            .execute(&mut *self.connection.borrow_mut())?;

        Ok(())
    }
}
