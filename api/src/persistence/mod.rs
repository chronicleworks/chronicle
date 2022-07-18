use std::collections::BTreeMap;
use std::{collections::HashMap, str::FromStr, time::Duration};

use chrono::DateTime;

use chrono::Utc;
use common::{
    attributes::Attribute,
    ledger::Offset,
    prov::{
        Activity, ActivityId, Agent, AgentId, Association, Attachment, ChronicleTransactionId,
        ChronicleTransactionIdError, Delegation, Derivation, DomaintypeId, Entity, EntityId,
        EvidenceId, Generation, Identity, IdentityId, Name, NamePart, Namespace, NamespaceId,
        ProvModel, PublicKeyPart, SignaturePart, Usage,
    },
};
use custom_error::custom_error;
use derivative::*;

use diesel::{
    connection::SimpleConnection,
    dsl::max,
    prelude::*,
    r2d2::{ConnectionManager, Pool, PooledConnection},
    sqlite::SqliteConnection,
};
use diesel_migrations::{embed_migrations, EmbeddedMigrations};
use tracing::{debug, instrument, trace, warn};
use uuid::Uuid;

use crate::QueryCommand;

mod query;
pub(crate) mod schema;
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

custom_error! {pub StoreError
    Db{source: diesel::result::Error}                           = "Database operation failed",
    DbConnection{source: diesel::ConnectionError}               = "Database connection failed",
    DbMigration{migration: Box<dyn custom_error::Error + Send + Sync>} = "Database migration failed {:?}",
    DbPool{source: r2d2::Error}                                 = "Connection pool error",
    Uuid{source: uuid::Error}                                   = "Invalid UUID",
    Json{source: serde_json::Error}                             = "Unreadable Attribute",
    TransactionId{source: ChronicleTransactionIdError }         = "Invalid transaction Id",
    RecordNotFound{}                                            = "Could not locate record in store",
    InvalidNamespace{}                                          = "Could not find namespace",
}

#[derive(Debug)]
pub struct ConnectionOptions {
    pub enable_wal: bool,
    pub enable_foreign_keys: bool,
    pub busy_timeout: Option<Duration>,
}

#[instrument]
fn sleeper(attempts: i32) -> bool {
    warn!(attempts, "SQLITE_BUSY, retrying");
    std::thread::sleep(std::time::Duration::from_millis(250));
    true
}

impl diesel::r2d2::CustomizeConnection<SqliteConnection, diesel::r2d2::Error>
    for ConnectionOptions
{
    fn on_acquire(&self, conn: &mut SqliteConnection) -> Result<(), diesel::r2d2::Error> {
        (|| {
            if self.enable_wal {
                conn.batch_execute(
                    r#"PRAGMA journal_mode = WAL2;
                PRAGMA synchronous = NORMAL;
                PRAGMA wal_autocheckpoint = 1000;
                PRAGMA wal_checkpoint(TRUNCATE);"#,
                )?;
            }
            if self.enable_foreign_keys {
                conn.batch_execute("PRAGMA foreign_keys = ON;")?;
            }
            if let Some(d) = self.busy_timeout {
                conn.batch_execute(&format!("PRAGMA busy_timeout = {};", d.as_millis()))?;
            }

            Ok(())
        })()
        .map_err(diesel::r2d2::Error::QueryError)
    }
}

#[derive(Derivative)]
#[derivative(Debug, Clone)]
pub struct Store {
    #[derivative(Debug = "ignore")]
    pool: Pool<ConnectionManager<SqliteConnection>>,
}

impl Store {
    /// Fetch the activity record for the IRI
    pub fn activity_by_activity_name_and_namespace(
        &self,
        connection: &mut SqliteConnection,
        name: &Name,
        namespaceid: &NamespaceId,
    ) -> Result<query::Activity, StoreError> {
        let (_namespaceid, nsid) = self.namespace_by_name(connection, namespaceid.name_part())?;
        use schema::activity::dsl;

        Ok(schema::activity::table
            .filter(dsl::name.eq(name).and(dsl::namespace_id.eq(nsid)))
            .first::<query::Activity>(connection)?)
    }

    /// Fetch the agent record for the IRI
    pub(crate) fn agent_by_agent_name_and_namespace(
        &self,
        connection: &mut SqliteConnection,
        name: &Name,
        namespaceid: &NamespaceId,
    ) -> Result<query::Agent, StoreError> {
        let (_namespaceid, nsid) = self.namespace_by_name(connection, namespaceid.name_part())?;
        use schema::agent::dsl;

        Ok(schema::agent::table
            .filter(dsl::name.eq(name).and(dsl::namespace_id.eq(nsid)))
            .first::<query::Agent>(connection)?)
    }

    /// Apply an activity to persistent storage, name + namespace are a key, so we update times + domaintype on conflict
    #[instrument(name = "Apply activity", skip(self, connection, ns))]
    fn apply_activity(
        &self,
        connection: &mut SqliteConnection,
        Activity {
            ref name,
            namespaceid,
            started,
            ended,
            domaintypeid,
            attributes,
            ..
        }: &Activity,
        ns: &HashMap<NamespaceId, Namespace>,
    ) -> Result<(), StoreError> {
        use schema::activity::{self as dsl};
        let _namespace = ns.get(namespaceid).ok_or(StoreError::InvalidNamespace {})?;
        let (_, nsid) = self.namespace_by_name(connection, namespaceid.name_part())?;

        let existing = self
            .activity_by_activity_name_and_namespace(connection, &*name, namespaceid)
            .ok();

        let resolved_domain_type = domaintypeid
            .as_ref()
            .map(|x| x.name_part().clone())
            .or_else(|| {
                existing
                    .as_ref()
                    .and_then(|x| x.domaintype.as_ref().map(Name::from))
            });

        let resolved_started = started
            .map(|x| x.naive_utc())
            .or_else(|| existing.as_ref().and_then(|x| x.started));

        let resolved_ended = ended
            .map(|x| x.naive_utc())
            .or_else(|| existing.as_ref().and_then(|x| x.ended));

        diesel::insert_into(schema::activity::table)
            .values((
                dsl::name.eq(name),
                dsl::namespace_id.eq(nsid),
                dsl::started.eq(started.map(|t| t.naive_utc())),
                dsl::ended.eq(ended.map(|t| t.naive_utc())),
                dsl::domaintype.eq(domaintypeid.as_ref().map(|x| x.name_part())),
            ))
            .on_conflict((dsl::name, dsl::namespace_id))
            .do_update()
            .set((
                dsl::domaintype.eq(resolved_domain_type),
                dsl::started.eq(resolved_started),
                dsl::ended.eq(resolved_ended),
            ))
            .execute(connection)?;

        let query::Activity { id, .. } =
            self.activity_by_activity_name_and_namespace(connection, &*name, namespaceid)?;

        diesel::insert_or_ignore_into(schema::activity_attribute::table)
            .values(
                attributes
                    .iter()
                    .map(
                        |(_, Attribute { typ, value, .. })| query::ActivityAttribute {
                            activity_id: id,
                            typename: typ.to_owned(),
                            value: value.to_string(),
                        },
                    )
                    .collect::<Vec<_>>(),
            )
            .execute(connection)?;

        Ok(())
    }

    /// Apply an agent to persistent storage, name + namespace are a key, so we update publickey + domaintype on conflict
    /// current is a special case, only relevant to local CLI context. A possibly improved design would be to store this in another table given its scope
    #[instrument(name = "Apply agent", skip(self, connection, ns))]
    fn apply_agent(
        &self,
        connection: &mut SqliteConnection,
        Agent {
            ref name,
            namespaceid,
            domaintypeid,
            attributes,
            ..
        }: &Agent,
        ns: &HashMap<NamespaceId, Namespace>,
    ) -> Result<(), StoreError> {
        use schema::agent::dsl;
        let _namespace = ns.get(namespaceid).ok_or(StoreError::InvalidNamespace {})?;
        let (_, nsid) = self.namespace_by_name(connection, namespaceid.name_part())?;

        let existing = self
            .agent_by_agent_name_and_namespace(connection, name, namespaceid)
            .ok();

        let resolved_domain_type = domaintypeid
            .as_ref()
            .map(|x| x.name_part().clone())
            .or_else(|| {
                existing
                    .as_ref()
                    .and_then(|x| x.domaintype.as_ref().map(Name::from))
            });

        diesel::insert_into(schema::agent::table)
            .values((
                dsl::name.eq(name),
                dsl::namespace_id.eq(nsid),
                dsl::current.eq(0),
                dsl::domaintype.eq(domaintypeid.as_ref().map(|x| x.name_part())),
            ))
            .on_conflict((dsl::namespace_id, dsl::name))
            .do_update()
            .set(dsl::domaintype.eq(resolved_domain_type))
            .execute(connection)?;

        let query::Agent { id, .. } =
            self.agent_by_agent_name_and_namespace(connection, name, namespaceid)?;

        diesel::insert_or_ignore_into(schema::agent_attribute::table)
            .values(
                attributes
                    .iter()
                    .map(|(_, Attribute { typ, value, .. })| query::AgentAttribute {
                        agent_id: id,
                        typename: typ.to_owned(),
                        value: value.to_string(),
                    })
                    .collect::<Vec<_>>(),
            )
            .execute(connection)?;

        Ok(())
    }

    #[instrument(name = "Apply attachment", skip(self, connection, ns))]
    fn apply_attachment(
        &self,
        connection: &mut SqliteConnection,
        Attachment {
            namespaceid,
            signature,
            signer,
            locator,
            signature_time,
            ..
        }: &Attachment,
        ns: &HashMap<NamespaceId, Namespace>,
    ) -> Result<(), StoreError> {
        let _namespace = ns.get(namespaceid).ok_or(StoreError::InvalidNamespace {})?;
        let (_, nsid) = self.namespace_by_name(connection, namespaceid.name_part())?;
        let (agent_name, public_key) = (signer.name_part(), signer.public_key_part());

        use schema::{agent::dsl as agentdsl, identity::dsl as identitydsl};
        let signer_id = agentdsl::agent
            .inner_join(identitydsl::identity)
            .filter(
                agentdsl::name
                    .eq(agent_name)
                    .and(agentdsl::namespace_id.eq(nsid))
                    .and(identitydsl::public_key.eq(public_key)),
            )
            .select(identitydsl::id)
            .first::<i32>(connection)?;

        use schema::attachment::dsl;

        diesel::insert_or_ignore_into(schema::attachment::table)
            .values((
                dsl::namespace_id.eq(nsid),
                dsl::signature.eq(signature),
                dsl::signer_id.eq(signer_id),
                dsl::locator.eq(locator),
                dsl::signature_time.eq(Utc::now().naive_utc()),
            ))
            .execute(connection)?;

        Ok(())
    }

    #[instrument(name = "Apply entity", skip(self, connection, ns))]
    fn apply_entity(
        &self,
        connection: &mut SqliteConnection,
        Entity {
            namespaceid,
            id,
            name,
            domaintypeid,
            attributes,
        }: &Entity,
        ns: &HashMap<NamespaceId, Namespace>,
    ) -> Result<(), StoreError> {
        use schema::entity::dsl;
        let _namespace = ns.get(namespaceid).ok_or(StoreError::InvalidNamespace {})?;
        let (_, nsid) = self.namespace_by_name(connection, namespaceid.name_part())?;

        let existing = self
            .entity_by_entity_name_and_namespace(connection, name, namespaceid)
            .ok();

        let resolved_domain_type = domaintypeid
            .as_ref()
            .map(|x| x.name_part().clone())
            .or_else(|| {
                existing
                    .as_ref()
                    .and_then(|x| x.domaintype.as_ref().map(Name::from))
            });

        diesel::insert_into(schema::entity::table)
            .values((
                dsl::name.eq(&name),
                dsl::namespace_id.eq(nsid),
                dsl::domaintype.eq(domaintypeid.as_ref().map(|x| x.name_part())),
            ))
            .on_conflict((dsl::namespace_id, dsl::name))
            .do_update()
            .set(dsl::domaintype.eq(resolved_domain_type))
            .execute(connection)?;

        let query::Entity { id, .. } =
            self.entity_by_entity_name_and_namespace(connection, name, namespaceid)?;

        diesel::insert_or_ignore_into(schema::entity_attribute::table)
            .values(
                attributes
                    .iter()
                    .map(|(_, Attribute { typ, value, .. })| query::EntityAttribute {
                        entity_id: id,
                        typename: typ.to_owned(),
                        value: value.to_string(),
                    })
                    .collect::<Vec<_>>(),
            )
            .execute(connection)?;

        Ok(())
    }

    #[instrument(name = "Apply has evidence", skip(self, connection))]
    fn apply_has_evidence(
        &self,
        connection: &mut SqliteConnection,
        model: &ProvModel,
        namespaceid: &NamespaceId,
        entity: &EntityId,
        evidence: &EvidenceId,
    ) -> Result<(), StoreError> {
        let (_, nsid) = self.namespace_by_name(connection, namespaceid.name_part())?;
        let attachment = self.attachment_by(connection, namespaceid, evidence)?;
        use schema::entity::dsl;

        diesel::update(schema::entity::table)
            .filter(
                dsl::name
                    .eq(entity.name_part())
                    .and(dsl::namespace_id.eq(nsid)),
            )
            .set(dsl::attachment_id.eq(attachment.id))
            .execute(connection)?;

        Ok(())
    }

    #[instrument(name = "Apply had evidence", skip(self, connection))]
    fn apply_had_evidence(
        &self,
        connection: &mut SqliteConnection,
        model: &ProvModel,
        namespaceid: &NamespaceId,
        entity: &EntityId,
        attachment: &EvidenceId,
    ) -> Result<(), StoreError> {
        let attachment = self.attachment_by(connection, namespaceid, attachment)?;
        let entity =
            self.entity_by_entity_name_and_namespace(connection, entity.name_part(), namespaceid)?;
        use schema::hadattachment::dsl;

        diesel::insert_or_ignore_into(schema::hadattachment::table)
            .values((
                dsl::entity_id.eq(entity.id),
                dsl::attachment_id.eq(attachment.id),
            ))
            .execute(connection)?;

        Ok(())
    }

    #[instrument(name = "Apply has identity", skip(self, connection))]
    fn apply_has_identity(
        &self,
        connection: &mut SqliteConnection,
        model: &ProvModel,
        namespaceid: &NamespaceId,
        agent: &AgentId,
        identity: &IdentityId,
    ) -> Result<(), StoreError> {
        let (_, nsid) = self.namespace_by_name(connection, namespaceid.name_part())?;
        let identity = self.identity_by(connection, namespaceid, identity)?;
        use schema::agent::dsl;

        diesel::update(schema::agent::table)
            .filter(
                dsl::name
                    .eq(agent.name_part())
                    .and(dsl::namespace_id.eq(nsid)),
            )
            .set(dsl::identity_id.eq(identity.id))
            .execute(connection)?;

        Ok(())
    }

    #[instrument(name = "Apply had identity", skip(self, connection))]
    fn apply_had_identity(
        &self,
        connection: &mut SqliteConnection,
        model: &ProvModel,
        namespaceid: &NamespaceId,
        agent: &AgentId,
        identity: &IdentityId,
    ) -> Result<(), StoreError> {
        let identity = self.identity_by(connection, namespaceid, identity)?;
        let agent =
            self.agent_by_agent_name_and_namespace(connection, agent.name_part(), namespaceid)?;
        use schema::hadidentity::dsl;

        diesel::insert_or_ignore_into(schema::hadidentity::table)
            .values((dsl::agent_id.eq(agent.id), dsl::identity_id.eq(identity.id)))
            .execute(connection)?;

        Ok(())
    }

    #[instrument(name = "Apply identity", skip(self, connection, ns))]
    fn apply_identity(
        &self,
        connection: &mut SqliteConnection,
        Identity {
            id,
            namespaceid,
            public_key,
            ..
        }: &Identity,
        ns: &HashMap<NamespaceId, Namespace>,
    ) -> Result<(), StoreError> {
        use schema::identity::dsl;
        let _namespace = ns.get(namespaceid).ok_or(StoreError::InvalidNamespace {})?;
        let (_, nsid) = self.namespace_by_name(connection, namespaceid.name_part())?;

        diesel::insert_or_ignore_into(schema::identity::table)
            .values((dsl::namespace_id.eq(nsid), dsl::public_key.eq(public_key)))
            .execute(connection)?;

        Ok(())
    }

    #[instrument(skip(connection, model))]
    fn apply_model(
        &self,
        connection: &mut SqliteConnection,
        model: &ProvModel,
    ) -> Result<(), StoreError> {
        debug!(model=?model);

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
        for (_, identity) in model.identities.iter() {
            self.apply_identity(connection, identity, &model.namespaces)?
        }
        for (_, attachment) in model.attachments.iter() {
            self.apply_attachment(connection, attachment, &model.namespaces)?
        }

        for ((namespaceid, agent_id), (_, identity_id)) in model.has_identity.iter() {
            self.apply_has_identity(connection, model, namespaceid, agent_id, identity_id)?;
        }

        for ((namespaceid, agent_id), identity_id) in model.had_identity.iter() {
            for (_, identity_id) in identity_id {
                self.apply_had_identity(connection, model, namespaceid, agent_id, identity_id)?;
            }
        }

        for ((namespaceid, entity_id), (_, evidence_id)) in model.has_evidence.iter() {
            self.apply_has_evidence(connection, model, namespaceid, entity_id, evidence_id)?;
        }

        for ((namespaceid, entity_id), attachment_id) in model.had_attachment.iter() {
            for (_, attachment_id) in attachment_id {
                self.apply_had_evidence(connection, model, namespaceid, entity_id, attachment_id)?;
            }
        }

        for ((namespaceid, _), association) in model.association.iter() {
            for association in association.iter() {
                self.apply_was_associated_with(connection, namespaceid, association)?;
            }
        }

        for ((namespaceid, _), usage) in model.usage.iter() {
            for usage in usage.iter() {
                self.apply_used(connection, namespaceid, usage)?;
            }
        }

        for ((namespaceid, _), generation) in model.generation.iter() {
            for generation in generation.iter() {
                self.apply_was_generated_by(connection, namespaceid, generation)?;
            }
        }

        for ((namespaceid, _), derivation) in model.derivation.iter() {
            for derivation in derivation.iter() {
                self.apply_derivation(connection, namespaceid, derivation)?;
            }
        }

        for ((namespaceid, _), delegation) in model.delegation.iter() {
            for delegation in delegation.iter() {
                self.apply_delegation(connection, namespaceid, delegation)?;
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
        use schema::namespace::dsl;
        diesel::insert_or_ignore_into(schema::namespace::table)
            .values((dsl::name.eq(name), dsl::uuid.eq(uuid.to_string())))
            .execute(connection)?;

        Ok(())
    }

    #[instrument(skip(self, prov))]
    pub fn apply_prov(&self, prov: &ProvModel) -> Result<(), StoreError> {
        debug!("Enter transaction");

        self.connection()?.immediate_transaction(|connection| {
            debug!("Entered transaction");
            self.apply_model(connection, prov)
        })?;
        debug!("Completed transaction");

        Ok(())
    }

    #[instrument(skip(connection))]
    fn apply_used(
        &self,
        connection: &mut SqliteConnection,
        namespace: &NamespaceId,
        usage: &Usage,
    ) -> Result<(), StoreError> {
        let storedactivity = self.activity_by_activity_name_and_namespace(
            connection,
            usage.activity_id.name_part(),
            namespace,
        )?;

        let storedentity = self.entity_by_entity_name_and_namespace(
            connection,
            usage.entity_id.name_part(),
            namespace,
        )?;

        use schema::usage::dsl as link;
        diesel::insert_or_ignore_into(schema::usage::table)
            .values((
                &link::activity_id.eq(storedactivity.id),
                &link::entity_id.eq(storedentity.id),
            ))
            .execute(connection)?;

        Ok(())
    }

    #[instrument(skip(self, connection))]
    fn apply_was_associated_with(
        &self,
        connection: &mut SqliteConnection,
        namespaceid: &common::prov::NamespaceId,
        association: &Association,
    ) -> Result<(), StoreError> {
        let storedactivity = self.activity_by_activity_name_and_namespace(
            connection,
            association.activity_id.name_part(),
            namespaceid,
        )?;

        let storedagent = self.agent_by_agent_name_and_namespace(
            connection,
            association.agent_id.name_part(),
            namespaceid,
        )?;

        use schema::association::dsl as asoc;
        diesel::insert_or_ignore_into(schema::association::table)
            .values((
                &asoc::activity_id.eq(storedactivity.id),
                &asoc::agent_id.eq(storedagent.id),
                &asoc::role.eq(association.role.as_ref()),
            ))
            .execute(connection)?;

        Ok(())
    }

    #[instrument(skip(self, connection, namespace))]
    fn apply_delegation(
        &self,
        connection: &mut SqliteConnection,
        namespace: &common::prov::NamespaceId,
        delegation: &Delegation,
    ) -> Result<(), StoreError> {
        let responsible = self.agent_by_agent_name_and_namespace(
            connection,
            delegation.responsible_id.name_part(),
            namespace,
        )?;

        let delegate = self.agent_by_agent_name_and_namespace(
            connection,
            delegation.delegate_id.name_part(),
            namespace,
        )?;

        let activity = {
            if let Some(ref activity_id) = delegation.activity_id {
                Some(
                    self.activity_by_activity_name_and_namespace(
                        connection,
                        activity_id.name_part(),
                        namespace,
                    )?
                    .id,
                )
            } else {
                None
            }
        };

        use schema::delegation::dsl as link;
        diesel::insert_or_ignore_into(schema::delegation::table)
            .values((
                &link::responsible_id.eq(responsible.id),
                &link::delegate_id.eq(delegate.id),
                &link::activity_id.eq(activity),
                &link::role.eq(delegation.role.as_ref()),
            ))
            .execute(connection)?;

        Ok(())
    }

    #[instrument(skip(self, connection, namespace))]
    fn apply_derivation(
        &self,
        connection: &mut SqliteConnection,
        namespace: &common::prov::NamespaceId,
        derivation: &Derivation,
    ) -> Result<(), StoreError> {
        let stored_generated = self.entity_by_entity_name_and_namespace(
            connection,
            derivation.generated_id.name_part(),
            namespace,
        )?;

        let stored_used = self.entity_by_entity_name_and_namespace(
            connection,
            derivation.used_id.name_part(),
            namespace,
        )?;

        let stored_activity = derivation
            .activity_id
            .as_ref()
            .map(|activity_id| {
                self.activity_by_activity_name_and_namespace(
                    connection,
                    activity_id.name_part(),
                    namespace,
                )
            })
            .transpose()?;

        use schema::derivation::dsl as link;
        diesel::insert_or_ignore_into(schema::derivation::table)
            .values((
                &link::used_entity_id.eq(stored_used.id),
                &link::generated_entity_id.eq(stored_generated.id),
                &link::typ.eq(derivation.typ),
                &link::activity_id.eq(stored_activity.map(|activity| activity.id)),
            ))
            .execute(connection)?;

        Ok(())
    }

    #[instrument(skip(connection))]
    fn apply_was_generated_by(
        &self,
        connection: &mut SqliteConnection,
        namespace: &common::prov::NamespaceId,
        generation: &Generation,
    ) -> Result<(), StoreError> {
        let storedactivity = self.activity_by_activity_name_and_namespace(
            connection,
            generation.activity_id.name_part(),
            namespace,
        )?;

        let storedentity = self.entity_by_entity_name_and_namespace(
            connection,
            generation.generated_id.name_part(),
            namespace,
        )?;

        use schema::generation::dsl as link;
        diesel::insert_or_ignore_into(schema::generation::table)
            .values((
                &link::activity_id.eq(storedactivity.id),
                &link::generated_entity_id.eq(storedentity.id),
            ))
            .execute(connection)?;

        Ok(())
    }

    pub fn connection(
        &self,
    ) -> Result<PooledConnection<ConnectionManager<SqliteConnection>>, StoreError> {
        Ok(self.pool.get()?)
    }

    /// Ensure the name is unique within the namespace, if not, then postfix the rowid
    pub(crate) fn disambiguate_activity_name(
        &self,
        connection: &mut SqliteConnection,
        name: &Name,
        namespaceid: &NamespaceId,
    ) -> Result<Name, StoreError> {
        use schema::{activity::dsl, namespace::dsl as nsdsl};

        let collision = schema::activity::table
            .inner_join(schema::namespace::table)
            .filter(
                dsl::name
                    .eq(name)
                    .and(nsdsl::name.eq(namespaceid.name_part())),
            )
            .count()
            .first::<i64>(connection)?;

        if collision == 0 {
            return Ok(name.to_owned());
        }

        let ambiguous = schema::activity::table
            .select(max(dsl::id))
            .first::<Option<i32>>(connection)?;

        Ok(format!("{}-{}", name, ambiguous.unwrap_or_default()).into())
    }

    /// Ensure the name is unique within the namespace, if not, then postfix the rowid
    pub(crate) fn disambiguate_agent_name(
        &self,
        connection: &mut SqliteConnection,
        name: &Name,
        namespaceid: &NamespaceId,
    ) -> Result<Name, StoreError> {
        use schema::{agent::dsl, namespace::dsl as nsdsl};

        let collision = schema::agent::table
            .inner_join(schema::namespace::table)
            .filter(
                dsl::name
                    .eq(name)
                    .and(nsdsl::name.eq(namespaceid.name_part())),
            )
            .count()
            .first::<i64>(connection)?;

        if collision == 0 {
            return Ok(name.to_owned());
        }

        let ambiguous = schema::agent::table
            .select(max(dsl::id))
            .first::<Option<i32>>(connection)?;

        Ok(format!("{}-{}", name, ambiguous.unwrap_or_default()).into())
    }

    /// Ensure the name is unique within the namespace, if not, then postfix the rowid
    #[instrument(skip(connection))]
    pub(crate) fn disambiguate_entity_name(
        &self,
        connection: &mut SqliteConnection,
        name: Name,
        namespaceid: NamespaceId,
    ) -> Result<Name, StoreError> {
        use schema::{entity::dsl, namespace::dsl as nsdsl};

        let collision = schema::entity::table
            .inner_join(schema::namespace::table)
            .filter(
                dsl::name
                    .eq(&name)
                    .and(nsdsl::name.eq(namespaceid.name_part())),
            )
            .count()
            .first::<i64>(connection)?;

        if collision == 0 {
            trace!(
                ?name,
                "Entity name is unique within namespace, so use directly"
            );
            return Ok(name.to_owned());
        }

        let ambiguous = schema::entity::table
            .select(max(dsl::id))
            .first::<Option<i32>>(connection)?;

        trace!(?name, "Is not unique, postfix with last rowid");

        Ok(format!("{}-{}", name, ambiguous.unwrap_or_default()).into())
    }

    pub(crate) fn entity_by_entity_name_and_namespace(
        &self,
        connection: &mut SqliteConnection,
        name: &Name,
        namespaceid: &NamespaceId,
    ) -> Result<query::Entity, StoreError> {
        let (_namespaceid, nsid) = self.namespace_by_name(connection, namespaceid.name_part())?;
        use schema::entity::dsl;

        Ok(schema::entity::table
            .filter(dsl::name.eq(name).and(dsl::namespace_id.eq(nsid)))
            .first::<query::Entity>(connection)?)
    }

    /// Get the named activity or the last started one, a useful context aware shortcut for the CLI
    #[instrument(skip(connection))]
    pub(crate) fn get_activity_by_name_or_last_started(
        &self,
        connection: &mut SqliteConnection,
        name: Option<Name>,
        namespace: NamespaceId,
    ) -> Result<query::Activity, StoreError> {
        use schema::activity::dsl;

        if let Some(name) = name {
            trace!(%name, "Use existing");
            Ok(self.activity_by_activity_name_and_namespace(connection, &name, &namespace)?)
        } else {
            trace!("Use last started");
            Ok(schema::activity::table
                .order(dsl::started)
                .first::<query::Activity>(connection)?)
        }
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

    /// Get the last fully synchronized offset
    #[instrument]
    pub fn get_last_offset(&self) -> Result<Option<(Offset, String)>, StoreError> {
        use schema::ledgersync::dsl;
        self.connection()?.immediate_transaction(|connection| {
            schema::ledgersync::table
                .order_by(dsl::sync_time)
                .select((dsl::offset, dsl::correlation_id))
                .first::<(Option<String>, String)>(connection)
                .map_err(StoreError::from)
                .map(|(offset, correlation_id)| {
                    offset.map(|offset| (Offset::from(&*offset), correlation_id))
                })
        })
    }

    #[instrument(skip(connection))]
    pub(crate) fn namespace_by_name(
        &self,
        connection: &mut SqliteConnection,
        namespace: &Name,
    ) -> Result<(NamespaceId, i32), StoreError> {
        use self::schema::namespace::dsl;

        let ns = dsl::namespace
            .filter(dsl::name.eq(namespace))
            .select((dsl::id, dsl::name, dsl::uuid))
            .first::<(i32, String, String)>(connection)
            .optional()?
            .ok_or(StoreError::RecordNotFound {})?;

        Ok((NamespaceId::from_name(ns.1, Uuid::from_str(&ns.2)?), ns.0))
    }

    #[instrument(skip(connection))]
    pub(crate) fn attachment_by(
        &self,
        connection: &mut SqliteConnection,
        namespaceid: &NamespaceId,
        attachment: &EvidenceId,
    ) -> Result<query::Attachment, StoreError> {
        use self::schema::attachment::dsl;
        let (_, nsid) = self.namespace_by_name(connection, namespaceid.name_part())?;
        let id_signature = attachment.signature_part();

        Ok(dsl::attachment
            .filter(
                dsl::signature
                    .eq(id_signature)
                    .and(dsl::namespace_id.eq(nsid)),
            )
            .first::<query::Attachment>(connection)?)
    }

    #[instrument(skip(connection))]
    pub(crate) fn identity_by(
        &self,
        connection: &mut SqliteConnection,
        namespaceid: &NamespaceId,
        identity: &IdentityId,
    ) -> Result<query::Identity, StoreError> {
        use self::schema::identity::dsl;
        let (_, nsid) = self.namespace_by_name(connection, namespaceid.name_part())?;
        let public_key = identity.public_key_part();

        Ok(dsl::identity
            .filter(
                dsl::public_key
                    .eq(public_key)
                    .and(dsl::namespace_id.eq(nsid)),
            )
            .first::<query::Identity>(connection)?)
    }

    #[instrument]
    pub fn new(pool: Pool<ConnectionManager<SqliteConnection>>) -> Result<Self, StoreError> {
        Ok(Store { pool })
    }

    #[instrument(skip(connection))]
    pub fn prov_model_for_namespace(
        &self,
        connection: &mut SqliteConnection,
        query: QueryCommand,
    ) -> Result<ProvModel, StoreError> {
        let mut model = ProvModel::default();
        let (namespaceid, nsid) =
            self.namespace_by_name(connection, &Name::from(&query.namespace))?;

        let agents = schema::agent::table
            .filter(schema::agent::namespace_id.eq(&nsid))
            .load::<query::Agent>(connection)?;

        for agent in agents {
            let attributes = schema::agent_attribute::table
                .filter(schema::agent_attribute::agent_id.eq(&agent.id))
                .load::<query::AgentAttribute>(connection)?;

            debug!(?agent, "Map agent to prov");
            let agentid: AgentId = AgentId::from_name(&agent.name);
            model.agents.insert(
                (namespaceid.clone(), agentid.clone()),
                Agent {
                    id: agentid.clone(),
                    namespaceid: namespaceid.clone(),
                    name: Name::from(&agent.name),
                    domaintypeid: agent.domaintype.map(|x| DomaintypeId::from_name(&x)),
                    attributes: attributes
                        .into_iter()
                        .map(|attr| {
                            serde_json::from_str(&*attr.value).map(|value| {
                                (
                                    attr.typename.clone(),
                                    Attribute {
                                        typ: attr.typename,
                                        value,
                                    },
                                )
                            })
                        })
                        .collect::<Result<BTreeMap<_, _>, _>>()?,
                },
            );
        }

        let activities = schema::activity::table
            .filter(schema::activity::namespace_id.eq(nsid))
            .load::<query::Activity>(connection)?;

        for activity in activities {
            debug!(?activity, "Map activity to prov");
            let attributes = schema::activity_attribute::table
                .filter(schema::activity_attribute::activity_id.eq(&activity.id))
                .load::<query::ActivityAttribute>(connection)?;

            let id: ActivityId = ActivityId::from_name(&activity.name);
            model.activities.insert(
                (namespaceid.clone(), id.clone()),
                Activity {
                    id: id.clone(),
                    namespaceid: namespaceid.clone(),
                    name: activity.name.into(),
                    started: activity.started.map(|x| DateTime::from_utc(x, Utc)),
                    ended: activity.ended.map(|x| DateTime::from_utc(x, Utc)),
                    domaintypeid: activity.domaintype.map(|x| DomaintypeId::from_name(&x)),
                    attributes: attributes
                        .into_iter()
                        .map(|attr| {
                            serde_json::from_str(&*attr.value).map(|value| {
                                (
                                    attr.typename.clone(),
                                    Attribute {
                                        typ: attr.typename,
                                        value,
                                    },
                                )
                            })
                        })
                        .collect::<Result<BTreeMap<_, _>, _>>()?,
                },
            );

            for generation in schema::generation::table
                .filter(schema::generation::activity_id.eq(activity.id))
                .order(schema::generation::activity_id.asc())
                .inner_join(schema::entity::table)
                .select(schema::entity::name)
                .load::<String>(connection)?
            {
                model.was_generated_by(namespaceid.clone(), &EntityId::from_name(&generation), &id);
            }

            for used in schema::usage::table
                .filter(schema::usage::activity_id.eq(activity.id))
                .order(schema::usage::activity_id.asc())
                .inner_join(schema::entity::table)
                .select(schema::entity::name)
                .load::<String>(connection)?
            {
                let used = used;
                model.used(namespaceid.clone(), &id, &EntityId::from_name(&used));
            }
        }

        let entities = schema::entity::table
            .filter(schema::entity::namespace_id.eq(nsid))
            .load::<query::Entity>(connection)?;

        for query::Entity {
            id,
            namespace_id: _,
            domaintype,
            name,
            attachment_id: _,
        } in entities
        {
            let attributes = schema::entity_attribute::table
                .filter(schema::entity_attribute::entity_id.eq(&id))
                .load::<query::EntityAttribute>(connection)?;

            let id: EntityId = EntityId::from_name(&name);
            model.entities.insert(
                (namespaceid.clone(), id.clone()),
                Entity {
                    id,
                    namespaceid: namespaceid.clone(),
                    name: name.into(),
                    domaintypeid: domaintype.map(|x| DomaintypeId::from_name(&x)),
                    attributes: attributes
                        .into_iter()
                        .map(|attr| {
                            serde_json::from_str(&*attr.value).map(|value| {
                                (
                                    attr.typename.clone(),
                                    Attribute {
                                        typ: attr.typename,
                                        value,
                                    },
                                )
                            })
                        })
                        .collect::<Result<BTreeMap<_, _>, _>>()?,
                },
            );
        }

        Ok(model)
    }

    /// Set the last fully synchronized offset
    #[instrument]
    pub fn set_last_offset(
        &self,
        offset: Offset,
        correlation_id: ChronicleTransactionId,
    ) -> Result<(), StoreError> {
        use schema::ledgersync::{self as dsl};

        if let Offset::Identity(offset) = offset {
            Ok(self.connection()?.immediate_transaction(|connection| {
                diesel::insert_into(dsl::table)
                    .values((
                        dsl::offset.eq(offset),
                        dsl::correlation_id.eq(&*correlation_id.to_string()),
                        (dsl::sync_time.eq(Utc::now().naive_utc())),
                    ))
                    .on_conflict(dsl::correlation_id)
                    .do_update()
                    .set(dsl::sync_time.eq(Utc::now().naive_utc()))
                    .execute(connection)
                    .map(|_| ())
            })?)
        } else {
            Ok(())
        }
    }

    #[instrument(skip(connection))]
    pub(crate) fn use_agent(
        &self,
        connection: &mut SqliteConnection,
        name: &Name,
        namespace: &Name,
    ) -> Result<(), StoreError> {
        let (_, nsid) = self.namespace_by_name(connection, &*namespace)?;
        use schema::agent::dsl;

        diesel::update(schema::agent::table.filter(dsl::current.ne(0)))
            .set(dsl::current.eq(0))
            .execute(connection)?;

        diesel::update(
            schema::agent::table.filter(dsl::name.eq(name).and(dsl::namespace_id.eq(nsid))),
        )
        .set(dsl::current.eq(1))
        .execute(connection)?;

        Ok(())
    }
}
