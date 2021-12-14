use std::{collections::HashMap, str::FromStr};

use chrono::DateTime;

use chrono::Utc;
use common::prov::{
    vocab::Chronicle, Activity, ActivityId, Agent, AgentId, ChronicleTransaction, Entity, EntityId,
    Namespace, NamespaceId, ProvModel,
};
use custom_error::custom_error;
use derivative::*;
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use diesel::{dsl::max, prelude::*, sqlite::SqliteConnection};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use tracing::debug;
use tracing::{instrument, trace};
use uuid::Uuid;

use crate::QueryCommand;

mod query;
pub(crate) mod schema;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

custom_error! {pub StoreError
    Db{source: diesel::result::Error}                           = "Database operation failed",
    DbConnection{source: diesel::ConnectionError}               = "Database connection failed",
    DbMigration{source: diesel_migrations::MigrationError}      = "Database migration failed",
    DbPool{source: r2d2::Error}                         = "Connection pool error",
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
    pool: Pool<ConnectionManager<SqliteConnection>>,
}

impl Store {
    pub fn connection(
        &self,
    ) -> Result<PooledConnection<ConnectionManager<SqliteConnection>>, StoreError> {
        Ok(self.pool.get()?)
    }

    pub fn new(pool: Pool<ConnectionManager<SqliteConnection>>) -> Result<Self, StoreError> {
        pool.get()?.run_pending_migrations(MIGRATIONS).unwrap();

        Ok(Store { pool })
    }

    ///TODO: any sort of query design, this should fundamentally be for export / streaming into external data pipelines or we will end up with an embedded triple store
    #[instrument(skip(connection))]
    pub fn prov_model_from(
        &self,
        connection: &mut SqliteConnection,
        query: QueryCommand,
    ) -> Result<ProvModel, StoreError> {
        let mut model = ProvModel::default();

        let agents = schema::agent::table
            .filter(schema::agent::namespace.eq(&query.namespace))
            .load::<query::Agent>(&mut self.connection()?)?;

        for agent in agents {
            debug!(?agent, "Map agent to prov");
            let agentid: AgentId = Chronicle::agent(&agent.name).into();
            let namespaceid = self.namespace_by_name(connection, &agent.namespace)?;
            model.agents.insert(
                (namespaceid.clone(), agentid.clone()),
                Agent {
                    id: agentid.clone(),
                    namespaceid: namespaceid.clone(),
                    name: agent.name,
                    publickey: agent.publickey,
                    domaintypeid: agent.domaintype.map(|x| Chronicle::domaintype(&x).into()),
                },
            );

            for asoc in schema::wasassociatedwith::table
                .filter(schema::wasassociatedwith::agent.eq(agent.id))
                .inner_join(schema::activity::table)
                .select(schema::activity::name)
                .load_iter::<String>(&mut self.connection()?)?
            {
                let asoc = asoc?;
                model.associate_with(&namespaceid, &Chronicle::activity(&asoc).into(), &agentid);
            }
        }

        let activities = schema::activity::table
            .filter(schema::activity::namespace.eq(&query.namespace))
            .load::<query::Activity>(&mut self.connection()?)?;

        for activity in activities {
            debug!(?activity, "Map activity to prov");

            let id: ActivityId = Chronicle::activity(&activity.name).into();
            let namespaceid = self.namespace_by_name(connection, &activity.namespace)?;
            model.activities.insert(
                (namespaceid.clone(), id.clone()),
                Activity {
                    id: id.clone(),
                    namespaceid: namespaceid.clone(),
                    name: activity.name,
                    started: activity.started.map(|x| DateTime::from_utc(x, Utc)),
                    ended: activity.ended.map(|x| DateTime::from_utc(x, Utc)),
                    domaintypeid: activity
                        .domaintype
                        .map(|x| Chronicle::domaintype(&x).into()),
                },
            );

            for asoc in schema::wasgeneratedby::table
                .filter(schema::wasgeneratedby::activity.eq(activity.id))
                .inner_join(schema::entity::table)
                .select(schema::entity::name)
                .load_iter::<String>(&mut self.connection()?)?
            {
                let asoc = asoc?;
                model.generate_by(namespaceid.clone(), Chronicle::entity(&asoc).into(), &id);
            }

            for used in schema::used::table
                .filter(schema::used::activity.eq(activity.id))
                .inner_join(schema::entity::table)
                .select(schema::entity::name)
                .load_iter::<String>(&mut self.connection()?)?
            {
                let used = used?;
                model.used(
                    namespaceid.clone(),
                    id.clone(),
                    &Chronicle::entity(&used).into(),
                );
            }
        }

        let entites = schema::entity::table
            .filter(schema::entity::namespace.eq(query.namespace))
            .load::<query::Entity>(&mut self.connection()?)?;

        for entity in entites {
            debug!(?entity, "Map entity to prov");
            let id: EntityId = Chronicle::entity(&entity.name).into();
            let namespaceid = self.namespace_by_name(connection, &entity.namespace)?;
            model.entities.insert((namespaceid.clone(), id.clone()), {
                match entity {
                    query::Entity {
                        name,
                        signature: Some(signature),
                        locator,
                        signature_time: Some(signature_time),
                        domaintype,
                        ..
                    } => Entity::Signed {
                        id,
                        namespaceid,
                        name,
                        signature,
                        signature_time: DateTime::from_utc(signature_time, Utc),
                        locator,
                        domaintypeid: domaintype.map(|x| Chronicle::domaintype(&x).into()),
                    },
                    query::Entity {
                        name, domaintype, ..
                    } => Entity::Unsigned {
                        id,
                        namespaceid,
                        name,
                        domaintypeid: domaintype.map(|x| Chronicle::domaintype(&x).into()),
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

        self.connection()?.immediate_transaction(|connection| {
            self.idempotently_apply_model(connection, &model)
        })?;

        Ok(model)
    }

    fn idempotently_apply_model(
        &self,
        connection: &mut SqliteConnection,
        model: &ProvModel,
    ) -> Result<(), StoreError> {
        for (_, ns) in model.namespaces.iter() {
            self.apply_namespace(connection, ns)?
        }
        for (_, agent) in model.agents.iter() {
            self.apply_agent(connection, agent, &model.namespaces)?
        }
        for (_, activity) in model.activities.iter() {
            self.apply_activity(connection, activity, &model.namespaces)?
        }

        for (_, entity) in model.entities.iter() {
            self.apply_entity(connection, entity, &model.namespaces)?
        }

        for ((namespaceid, activityid), agentid) in model.was_associated_with.iter() {
            for (_, agentid) in agentid {
                self.apply_was_associated_with(
                    connection,
                    model,
                    namespaceid,
                    activityid,
                    agentid,
                )?;
            }
        }

        for ((namespaceid, activityid), entityid) in model.used.iter() {
            for (_, entityid) in entityid {
                self.apply_used(connection, model, namespaceid, activityid, entityid)?;
            }
        }

        for ((namespaceid, entityid), activityid) in model.was_generated_by.iter() {
            for (_, activityid) in activityid {
                self.apply_was_generated_by(connection, model, namespaceid, entityid, activityid)?;
            }
        }

        Ok(())
    }

    #[instrument(skip(connection))]
    fn apply_namespace(
        &self,
        connection: &mut SqliteConnection,
        Namespace {
            ref name, ref uuid, ..
        }: &Namespace,
    ) -> Result<(), StoreError> {
        diesel::insert_or_ignore_into(schema::namespace::table)
            .values(&query::NewNamespace {
                name,
                uuid: &uuid.to_string(),
            })
            .execute(connection)?;

        Ok(())
    }

    #[instrument(skip(connection))]
    pub(crate) fn namespace_by_name(
        &self,
        connection: &mut SqliteConnection,
        namespace: &str,
    ) -> Result<NamespaceId, StoreError> {
        use self::schema::namespace::dsl as ns;

        let ns = ns::namespace
            .filter(ns::name.eq(namespace))
            .first::<query::Namespace>(connection)
            .optional()?
            .ok_or(StoreError::RecordNotFound {})?;

        Ok(Chronicle::namespace(&ns.name, &Uuid::from_str(&ns.uuid)?).into())
    }

    #[instrument(skip(connection))]
    fn apply_agent(
        &self,
        connection: &mut SqliteConnection,
        Agent {
            ref name,
            namespaceid,
            publickey,
            id: _,
            domaintypeid,
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
            .execute(connection)?;

        Ok(())
    }

    #[instrument(skip(connection))]
    fn apply_activity(
        &self,
        connection: &mut SqliteConnection,
        Activity {
            ref name,
            id,
            namespaceid,
            started,
            ended,
            domaintypeid,
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
            .execute(connection)?;

        Ok(())
    }

    #[instrument(skip(connection))]
    pub(crate) fn use_agent(
        &self,
        connection: &mut SqliteConnection,
        name: String,
        namespace: String,
    ) -> Result<(), StoreError> {
        use schema::agent::dsl;

        diesel::update(schema::agent::table.filter(dsl::current.ne(0)))
            .set(dsl::current.eq(0))
            .execute(connection)?;

        diesel::update(
            schema::agent::table.filter(dsl::name.eq(name).and(dsl::namespace.eq(namespace))),
        )
        .set(dsl::current.eq(1))
        .execute(connection)?;

        Ok(())
    }

    #[instrument(skip(connection))]
    pub(crate) fn get_current_agent(
        &self,
        connection: &mut SqliteConnection,
    ) -> Result<query::Agent, StoreError> {
        use schema::agent::dsl;
        Ok(schema::agent::table
            .filter(dsl::current.ne(0))
            .first::<query::Agent>(connection)?)
    }

    /// Ensure the name is unique within the namespace, if not, then postfix the rowid
    pub(crate) fn disambiguate_entity_name(
        &self,
        connection: &mut SqliteConnection,
        name: &str,
    ) -> Result<String, StoreError> {
        use schema::entity::dsl;

        let collision = schema::entity::table
            .filter(dsl::name.eq(name))
            .count()
            .first::<i64>(connection)?;

        if collision == 0 {
            return Ok(name.to_owned());
        }

        let ambiguous = schema::entity::table
            .select(max(dsl::id))
            .first::<Option<i32>>(connection)?;

        Ok(format!("{}_{}", name, ambiguous.unwrap_or_default()))
    }

    /// Ensure the name is unique within the namespace, if not, then postfix the rowid
    pub(crate) fn disambiguate_agent_name(
        &self,
        connection: &mut SqliteConnection,
        name: &str,
    ) -> Result<String, StoreError> {
        use schema::agent::dsl;

        let collision = schema::agent::table
            .filter(dsl::name.eq(name))
            .count()
            .first::<i64>(connection)?;

        if collision == 0 {
            return Ok(name.to_owned());
        }

        let ambiguous = schema::agent::table
            .select(max(dsl::id))
            .first::<Option<i32>>(connection)?;

        Ok(format!("{}_{}", name, ambiguous.unwrap_or_default()))
    }

    /// Ensure the name is unique within the namespace, if not, then postfix the rowid
    pub(crate) fn disambiguate_activity_name(
        &self,
        connection: &mut SqliteConnection,
        name: &str,
    ) -> Result<String, StoreError> {
        use schema::activity::dsl;

        let collision = schema::activity::table
            .filter(dsl::name.eq(name))
            .count()
            .first::<i64>(connection)?;

        if collision == 0 {
            return Ok(name.to_owned());
        }

        let ambiguous = schema::activity::table
            .select(max(dsl::id))
            .first::<Option<i32>>(connection)?;

        Ok(format!("{}_{}", name, ambiguous.unwrap_or_default()))
    }

    /// Fetch the activity record for the IRI
    fn activity_by_activity_name_and_namespace(
        &self,
        connection: &mut SqliteConnection,
        name: &str,
        namespace: &str,
    ) -> Result<query::Activity, StoreError> {
        use schema::activity::dsl;

        Ok(schema::activity::table
            .filter(dsl::name.eq(name).and(dsl::namespace.eq(namespace)))
            .first::<query::Activity>(connection)?)
    }

    /// Fetch the agent record for the IRI
    pub(crate) fn agent_by_agent_name_and_namespace(
        &self,
        connection: &mut SqliteConnection,
        name: &str,
        namespace: &str,
    ) -> Result<query::Agent, StoreError> {
        use schema::agent::dsl;

        Ok(schema::agent::table
            .filter(dsl::name.eq(name).and(dsl::namespace.eq(namespace)))
            .first::<query::Agent>(connection)?)
    }

    pub(crate) fn entity_by_entity_name_and_namespace(
        &self,
        connection: &mut SqliteConnection,
        name: &str,
        namespace: &str,
    ) -> Result<query::Entity, StoreError> {
        use schema::entity::dsl;

        Ok(schema::entity::table
            .filter(dsl::name.eq(name).and(dsl::namespace.eq(namespace)))
            .first::<query::Entity>(connection)?)
    }

    /// Get the named acitvity or the last started one, a useful context aware shortcut for the CLI
    pub(crate) fn get_activity_by_name_or_last_started(
        &self,
        connection: &mut SqliteConnection,
        name: Option<String>,
        namespace: Option<String>,
    ) -> Result<query::Activity, StoreError> {
        use schema::activity::dsl;

        match (name, namespace) {
            (Some(name), Some(namespace)) => {
                Ok(self.activity_by_activity_name_and_namespace(connection, &name, &namespace)?)
            }
            _ => Ok(schema::activity::table
                .order(dsl::started)
                .first::<query::Activity>(connection)?),
        }
    }

    #[instrument(skip(connection))]
    fn apply_entity(
        &self,
        connection: &mut SqliteConnection,
        entity: &common::prov::Entity,
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
            .execute(connection)?;

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
                .execute(&mut self.connection()?)?;
        }

        Ok(())
    }

    #[instrument(skip(connection))]
    fn apply_was_associated_with(
        &self,
        connection: &mut SqliteConnection,
        model: &ProvModel,
        namespaceid: &common::prov::NamespaceId,
        activityid: &common::prov::ActivityId,
        agentid: &common::prov::AgentId,
    ) -> Result<(), StoreError> {
        let provagent = model
            .agents
            .get(&(namespaceid.to_owned(), agentid.to_owned()))
            .ok_or_else(|| StoreError::ModelDoesNotContainAgent {
                agentid: agentid.clone(),
            })?;
        let provactivity = model
            .activities
            .get(&(namespaceid.to_owned(), activityid.to_owned()))
            .ok_or_else(|| StoreError::ModelDoesNotContainActivity {
                activityid: activityid.clone(),
            })?;

        let storedactivity = self.activity_by_activity_name_and_namespace(
            connection,
            &provactivity.name,
            provactivity.namespaceid.decompose().0,
        )?;

        let storedagent = self.agent_by_agent_name_and_namespace(
            connection,
            &provagent.name,
            provagent.namespaceid.decompose().0,
        )?;

        use schema::wasassociatedwith::dsl as link;
        diesel::insert_or_ignore_into(schema::wasassociatedwith::table)
            .values((
                &link::activity.eq(storedactivity.id),
                &link::agent.eq(storedagent.id),
            ))
            .execute(&mut self.connection()?)?;

        Ok(())
    }

    #[instrument(skip(connection))]
    fn apply_was_generated_by(
        &self,
        connection: &mut SqliteConnection,
        model: &ProvModel,
        namespace: &common::prov::NamespaceId,
        entity: &common::prov::EntityId,
        activity: &ActivityId,
    ) -> Result<(), StoreError> {
        let proventity = model
            .entities
            .get(&(namespace.to_owned(), entity.to_owned()))
            .ok_or_else(|| StoreError::ModelDoesNotContainEntity {
                entityid: entity.clone(),
            })?;
        let provactivity = model
            .activities
            .get(&(namespace.to_owned(), activity.to_owned()))
            .ok_or_else(|| StoreError::ModelDoesNotContainActivity {
                activityid: activity.clone(),
            })?;

        let storedactivity = self.activity_by_activity_name_and_namespace(
            connection,
            &provactivity.name,
            provactivity.namespaceid.decompose().0,
        )?;

        let storedentity = self.entity_by_entity_name_and_namespace(
            connection,
            proventity.name(),
            proventity.namespaceid().decompose().0,
        )?;

        use schema::wasgeneratedby::dsl as link;
        diesel::insert_or_ignore_into(schema::wasgeneratedby::table)
            .values((
                &link::activity.eq(storedactivity.id),
                &link::entity.eq(storedentity.id),
            ))
            .execute(&mut self.connection()?)?;

        Ok(())
    }

    #[instrument(skip(connection))]
    fn apply_used(
        &self,
        connection: &mut SqliteConnection,
        model: &ProvModel,
        namespace: &NamespaceId,
        activity: &ActivityId,
        entity: &common::prov::EntityId,
    ) -> Result<(), StoreError> {
        let proventity = model
            .entities
            .get(&(namespace.to_owned(), entity.to_owned()))
            .ok_or_else(|| StoreError::ModelDoesNotContainEntity {
                entityid: entity.clone(),
            })?;
        let provactivity = model
            .activities
            .get(&(namespace.to_owned(), activity.to_owned()))
            .ok_or_else(|| StoreError::ModelDoesNotContainActivity {
                activityid: activity.clone(),
            })?;

        let storedactivity = self.activity_by_activity_name_and_namespace(
            connection,
            &provactivity.name,
            provactivity.namespaceid.decompose().0,
        )?;

        let storedentity = self.entity_by_entity_name_and_namespace(
            connection,
            proventity.name(),
            proventity.namespaceid().decompose().0,
        )?;

        use schema::used::dsl as link;
        diesel::insert_or_ignore_into(schema::used::table)
            .values((
                &link::activity.eq(storedactivity.id),
                &link::entity.eq(storedentity.id),
            ))
            .execute(&mut self.connection()?)?;

        Ok(())
    }
}
