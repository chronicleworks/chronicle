use std::{
    collections::{BTreeMap, HashMap},
    str::FromStr,
    time::Duration,
};

use chrono::DateTime;

use chrono::Utc;
use common::{
    attributes::Attribute,
    ledger::Offset,
    prov::{
        Activity, ActivityId, Agent, AgentId, Association, Attachment, ChronicleTransactionId,
        ChronicleTransactionIdError, Delegation, Derivation, DomaintypeId, Entity, EntityId,
        EvidenceId, ExternalId, ExternalIdPart, Generation, Identity, IdentityId, Namespace,
        NamespaceId, ProvModel, PublicKeyPart, SignaturePart, Usage,
    },
};
use custom_error::custom_error;
use derivative::*;

use diesel::{
    connection::SimpleConnection,
    prelude::*,
    r2d2::{ConnectionManager, Pool, PooledConnection},
    PgConnection,
};
use diesel_migrations::{embed_migrations, EmbeddedMigrations};
use tracing::{debug, instrument, warn};
use uuid::Uuid;

use crate::QueryCommand;

mod query;
pub(crate) mod schema;
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

custom_error! {pub StoreError
    Db{source: diesel::result::Error}                           = "Database operation failed {source}",
    DbConnection{source: diesel::ConnectionError}               = "Database connection failed {source}",
    DbMigration{migration: Box<dyn custom_error::Error + Send + Sync>} = "Database migration failed {migration}",
    DbPool{source: r2d2::Error}                                 = "Connection pool error {source}",
    Uuid{source: uuid::Error}                                   = "Invalid UUID {source}",
    Json{source: serde_json::Error}                             = "Unreadable Attribute {source}",
    TransactionId{source: ChronicleTransactionIdError }         = "Invalid transaction Id {source}",
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

impl diesel::r2d2::CustomizeConnection<PgConnection, diesel::r2d2::Error>
    for ConnectionOptions
{
    fn on_acquire(&self, conn: &mut PgConnection) -> Result<(), diesel::r2d2::Error> {
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
    pool: Pool<ConnectionManager<PgConnection>>,
}

impl Store {
    #[instrument(name = "Bind namespace", skip(self))]
    pub(crate) fn namespace_binding(
        &self,
        external_id: &str,
        uuid: Uuid,
    ) -> Result<(), StoreError> {
        use schema::namespace::dsl;

        let uuid = uuid.to_string();
        self.connection()?.build_transaction().run(|conn| {
            diesel::insert_into(dsl::namespace)
                .values((dsl::external_id.eq(external_id), dsl::uuid.eq(&uuid)))
                .on_conflict(dsl::external_id)
                .do_update()
                .set(dsl::uuid.eq(&uuid))
                .execute(conn)
        })?;

        Ok(())
    }

    /// Fetch the activity record for the IRI
    fn activity_by_activity_external_id_and_namespace(
        &self,
        connection: &mut PgConnection,
        external_id: &ExternalId,
        namespaceid: &NamespaceId,
    ) -> Result<query::Activity, StoreError> {
        let (_namespaceid, nsid) =
            self.namespace_by_external_id(connection, namespaceid.external_id_part())?;
        use schema::activity::dsl;

        Ok(schema::activity::table
            .filter(
                dsl::external_id
                    .eq(external_id)
                    .and(dsl::namespace_id.eq(nsid)),
            )
            .first::<query::Activity>(connection)?)
    }

    /// Fetch the entity record for the IRI
    fn entity_by_entity_external_id_and_namespace(
        &self,
        connection: &mut PgConnection,
        external_id: &ExternalId,
        namespace_id: &NamespaceId,
    ) -> Result<query::Entity, StoreError> {
        let (_, ns_id) =
            self.namespace_by_external_id(connection, namespace_id.external_id_part())?;
        use schema::entity::dsl;

        Ok(schema::entity::table
            .filter(
                dsl::external_id
                    .eq(external_id)
                    .and(dsl::namespace_id.eq(ns_id)),
            )
            .first::<query::Entity>(connection)?)
    }

    /// Fetch the agent record for the IRI
    pub(crate) fn agent_by_agent_external_id_and_namespace(
        &self,
        connection: &mut PgConnection,
        external_id: &ExternalId,
        namespaceid: &NamespaceId,
    ) -> Result<query::Agent, StoreError> {
        let (_namespaceid, nsid) =
            self.namespace_by_external_id(connection, namespaceid.external_id_part())?;
        use schema::agent::dsl;

        Ok(schema::agent::table
            .filter(
                dsl::external_id
                    .eq(external_id)
                    .and(dsl::namespace_id.eq(nsid)),
            )
            .first::<query::Agent>(connection)?)
    }

    /// Apply an activity to persistent storage, name + namespace are a key, so we update times + domaintype on conflict
    #[instrument(level = "trace", skip(self, connection), ret(Debug))]
    fn apply_activity(
        &self,
        connection: &mut PgConnection,
        Activity {
            ref external_id,
            namespaceid,
            started,
            ended,
            domaintypeid,
            attributes,
            ..
        }: &Activity,
        ns: &HashMap<NamespaceId, Namespace>,
    ) -> Result<(), StoreError> {
        use schema::activity as dsl;
        let _namespace = ns.get(namespaceid).ok_or(StoreError::InvalidNamespace {})?;
        let (_, nsid) =
            self.namespace_by_external_id(connection, namespaceid.external_id_part())?;

        let existing = self
            .activity_by_activity_external_id_and_namespace(connection, external_id, namespaceid)
            .ok();

        let resolved_domain_type = domaintypeid
            .as_ref()
            .map(|x| x.external_id_part().clone())
            .or_else(|| {
                existing
                    .as_ref()
                    .and_then(|x| x.domaintype.as_ref().map(ExternalId::from))
            });

        let resolved_started = started
            .map(|x| x.naive_utc())
            .or_else(|| existing.as_ref().and_then(|x| x.started));

        let resolved_ended = ended
            .map(|x| x.naive_utc())
            .or_else(|| existing.as_ref().and_then(|x| x.ended));

        diesel::insert_into(schema::activity::table)
            .values((
                dsl::external_id.eq(external_id),
                dsl::namespace_id.eq(nsid),
                dsl::started.eq(started.map(|t| t.naive_utc())),
                dsl::ended.eq(ended.map(|t| t.naive_utc())),
                dsl::domaintype.eq(domaintypeid.as_ref().map(|x| x.external_id_part())),
            ))
            .on_conflict((dsl::external_id, dsl::namespace_id))
            .do_update()
            .set((
                dsl::domaintype.eq(resolved_domain_type),
                dsl::started.eq(resolved_started),
                dsl::ended.eq(resolved_ended),
            ))
            .execute(connection)?;

        let query::Activity { id, .. } = self.activity_by_activity_external_id_and_namespace(
            connection,
            external_id,
            namespaceid,
        )?;

        diesel::insert_into(schema::activity_attribute::table)
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
            .on_conflict_do_nothing()
            .execute(connection)?;

        Ok(())
    }

    /// Apply an agent to persistent storage, external_id + namespace are a key, so we update publickey + domaintype on conflict
    /// current is a special case, only relevant to local CLI context. A possibly improved design would be to store this in another table given its scope
    #[instrument(level = "trace", skip(self, connection), ret(Debug))]
    fn apply_agent(
        &self,
        connection: &mut PgConnection,
        Agent {
            ref external_id,
            namespaceid,
            domaintypeid,
            attributes,
            ..
        }: &Agent,
        ns: &HashMap<NamespaceId, Namespace>,
    ) -> Result<(), StoreError> {
        use schema::agent::dsl;
        let _namespace = ns.get(namespaceid).ok_or(StoreError::InvalidNamespace {})?;
        let (_, nsid) =
            self.namespace_by_external_id(connection, namespaceid.external_id_part())?;

        let existing = self
            .agent_by_agent_external_id_and_namespace(connection, external_id, namespaceid)
            .ok();

        let resolved_domain_type = domaintypeid
            .as_ref()
            .map(|x| x.external_id_part().clone())
            .or_else(|| {
                existing
                    .as_ref()
                    .and_then(|x| x.domaintype.as_ref().map(ExternalId::from))
            });

        diesel::insert_into(schema::agent::table)
            .values((
                dsl::external_id.eq(external_id),
                dsl::namespace_id.eq(nsid),
                dsl::current.eq(0),
                dsl::domaintype.eq(domaintypeid.as_ref().map(|x| x.external_id_part())),
            ))
            .on_conflict((dsl::namespace_id, dsl::external_id))
            .do_update()
            .set(dsl::domaintype.eq(resolved_domain_type))
            .execute(connection)?;

        let query::Agent { id, .. } =
            self.agent_by_agent_external_id_and_namespace(connection, external_id, namespaceid)?;

        diesel::insert_into(schema::agent_attribute::table)
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
            .on_conflict_do_nothing()
            .execute(connection)?;

        Ok(())
    }

    #[instrument(level = "trace", skip(self, connection), ret(Debug))]
    fn apply_attachment(
        &self,
        connection: &mut PgConnection,
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
        let (_, nsid) =
            self.namespace_by_external_id(connection, namespaceid.external_id_part())?;
        let (agent_external_id, public_key) = (signer.external_id_part(), signer.public_key_part());

        use schema::{agent::dsl as agentdsl, identity::dsl as identitydsl};
        let signer_id = agentdsl::agent
            .inner_join(identitydsl::identity)
            .filter(
                agentdsl::external_id
                    .eq(agent_external_id)
                    .and(agentdsl::namespace_id.eq(nsid))
                    .and(identitydsl::public_key.eq(public_key)),
            )
            .select(identitydsl::id)
            .first::<i32>(connection)?;

        use schema::attachment::dsl;

        diesel::insert_into(schema::attachment::table)
            .values((
                dsl::namespace_id.eq(nsid),
                dsl::signature.eq(signature),
                dsl::signer_id.eq(signer_id),
                dsl::locator.eq(locator),
                dsl::signature_time.eq(Utc::now().naive_utc()),
            ))
            .on_conflict_do_nothing()
            .execute(connection)?;

        Ok(())
    }

    #[instrument(level = "trace", skip(self, connection), ret(Debug))]
    fn apply_entity(
        &self,
        connection: &mut PgConnection,
        Entity {
            namespaceid,
            id,
            external_id,
            domaintypeid,
            attributes,
        }: &Entity,
        ns: &HashMap<NamespaceId, Namespace>,
    ) -> Result<(), StoreError> {
        use schema::entity::dsl;
        let _namespace = ns.get(namespaceid).ok_or(StoreError::InvalidNamespace {})?;
        let (_, nsid) =
            self.namespace_by_external_id(connection, namespaceid.external_id_part())?;

        let existing = self
            .entity_by_entity_external_id_and_namespace(connection, external_id, namespaceid)
            .ok();

        let resolved_domain_type = domaintypeid
            .as_ref()
            .map(|x| x.external_id_part().clone())
            .or_else(|| {
                existing
                    .as_ref()
                    .and_then(|x| x.domaintype.as_ref().map(ExternalId::from))
            });

        diesel::insert_into(schema::entity::table)
            .values((
                dsl::external_id.eq(&external_id),
                dsl::namespace_id.eq(nsid),
                dsl::domaintype.eq(domaintypeid.as_ref().map(|x| x.external_id_part())),
            ))
            .on_conflict((dsl::namespace_id, dsl::external_id))
            .do_update()
            .set(dsl::domaintype.eq(resolved_domain_type))
            .execute(connection)?;

        let query::Entity { id, .. } =
            self.entity_by_entity_external_id_and_namespace(connection, external_id, namespaceid)?;

        diesel::insert_into(schema::entity_attribute::table)
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
            .on_conflict_do_nothing()
            .execute(connection)?;

        Ok(())
    }

    #[instrument(level = "trace", skip(self, connection), ret(Debug))]
    fn apply_has_evidence(
        &self,
        connection: &mut PgConnection,
        model: &ProvModel,
        namespaceid: &NamespaceId,
        entity: &EntityId,
        evidence: &EvidenceId,
    ) -> Result<(), StoreError> {
        let (_, nsid) =
            self.namespace_by_external_id(connection, namespaceid.external_id_part())?;
        let attachment = self.attachment_by(connection, namespaceid, evidence)?;
        use schema::entity::dsl;

        diesel::update(schema::entity::table)
            .filter(
                dsl::external_id
                    .eq(entity.external_id_part())
                    .and(dsl::namespace_id.eq(nsid)),
            )
            .set(dsl::attachment_id.eq(attachment.id))
            .execute(connection)?;

        Ok(())
    }

    #[instrument(level = "trace", skip(self, connection), ret(Debug))]
    fn apply_had_evidence(
        &self,
        connection: &mut PgConnection,
        model: &ProvModel,
        namespaceid: &NamespaceId,
        entity: &EntityId,
        attachment: &EvidenceId,
    ) -> Result<(), StoreError> {
        let attachment = self.attachment_by(connection, namespaceid, attachment)?;
        let entity = self.entity_by_entity_external_id_and_namespace(
            connection,
            entity.external_id_part(),
            namespaceid,
        )?;
        use schema::hadattachment::dsl;

        diesel::insert_into(schema::hadattachment::table)
            .values((
                dsl::entity_id.eq(entity.id),
                dsl::attachment_id.eq(attachment.id),
            ))
            .on_conflict_do_nothing()
            .execute(connection)?;

        Ok(())
    }

    #[instrument(level = "trace", skip(self, connection), ret(Debug))]
    fn apply_has_identity(
        &self,
        connection: &mut PgConnection,
        model: &ProvModel,
        namespaceid: &NamespaceId,
        agent: &AgentId,
        identity: &IdentityId,
    ) -> Result<(), StoreError> {
        let (_, nsid) =
            self.namespace_by_external_id(connection, namespaceid.external_id_part())?;
        let identity = self.identity_by(connection, namespaceid, identity)?;
        use schema::agent::dsl;

        diesel::update(schema::agent::table)
            .filter(
                dsl::external_id
                    .eq(agent.external_id_part())
                    .and(dsl::namespace_id.eq(nsid)),
            )
            .set(dsl::identity_id.eq(identity.id))
            .execute(connection)?;

        Ok(())
    }

    #[instrument(level = "trace", skip(self, connection), ret(Debug))]
    fn apply_had_identity(
        &self,
        connection: &mut PgConnection,
        model: &ProvModel,
        namespaceid: &NamespaceId,
        agent: &AgentId,
        identity: &IdentityId,
    ) -> Result<(), StoreError> {
        let identity = self.identity_by(connection, namespaceid, identity)?;
        let agent = self.agent_by_agent_external_id_and_namespace(
            connection,
            agent.external_id_part(),
            namespaceid,
        )?;
        use schema::hadidentity::dsl;

        diesel::insert_into(schema::hadidentity::table)
            .values((dsl::agent_id.eq(agent.id), dsl::identity_id.eq(identity.id)))
            .on_conflict_do_nothing()
            .execute(connection)?;

        Ok(())
    }

    #[instrument(level = "trace", skip(self, connection), ret(Debug))]
    fn apply_identity(
        &self,
        connection: &mut PgConnection,
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
        let (_, nsid) =
            self.namespace_by_external_id(connection, namespaceid.external_id_part())?;

        diesel::insert_into(schema::identity::table)
            .values((dsl::namespace_id.eq(nsid), dsl::public_key.eq(public_key)))
            .on_conflict_do_nothing()
            .execute(connection)?;

        Ok(())
    }

    fn apply_model(
        &self,
        connection: &mut PgConnection,
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

        for ((namespaceid, activity_id), was_informed_by) in model.was_informed_by.iter() {
            for (_, informing_activity_id) in was_informed_by.iter() {
                self.apply_was_informed_by(
                    connection,
                    namespaceid,
                    activity_id,
                    informing_activity_id,
                )?;
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

    #[instrument(level = "trace", skip(self, connection), ret(Debug))]
    fn apply_namespace(
        &self,
        connection: &mut PgConnection,
        Namespace {
            ref external_id,
            ref uuid,
            ..
        }: &Namespace,
    ) -> Result<(), StoreError> {
        use schema::namespace::dsl;
        diesel::insert_into(schema::namespace::table)
            .values((
                dsl::external_id.eq(external_id),
                dsl::uuid.eq(uuid.to_string()),
            ))
            .on_conflict_do_nothing()
            .execute(connection)?;

        Ok(())
    }

    pub(crate) fn apply_prov(&self, prov: &ProvModel) -> Result<(), StoreError> {
        self.connection()?
            .build_transaction().run(|connection| self.apply_model(connection, prov))?;

        Ok(())
    }

    #[instrument(skip(connection))]
    fn apply_used(
        &self,
        connection: &mut PgConnection,
        namespace: &NamespaceId,
        usage: &Usage,
    ) -> Result<(), StoreError> {
        let storedactivity = self.activity_by_activity_external_id_and_namespace(
            connection,
            usage.activity_id.external_id_part(),
            namespace,
        )?;

        let storedentity = self.entity_by_entity_external_id_and_namespace(
            connection,
            usage.entity_id.external_id_part(),
            namespace,
        )?;

        use schema::usage::dsl as link;
        diesel::insert_into(schema::usage::table)
            .values((
                &link::activity_id.eq(storedactivity.id),
                &link::entity_id.eq(storedentity.id),
            ))
            .on_conflict_do_nothing()
            .execute(connection)?;

        Ok(())
    }

    #[instrument(skip(connection))]
    fn apply_was_informed_by(
        &self,
        connection: &mut PgConnection,
        namespace: &NamespaceId,
        activity_id: &ActivityId,
        informing_activity_id: &ActivityId,
    ) -> Result<(), StoreError> {
        let storedactivity = self.activity_by_activity_external_id_and_namespace(
            connection,
            activity_id.external_id_part(),
            namespace,
        )?;

        let storedinformingactivity = self.activity_by_activity_external_id_and_namespace(
            connection,
            informing_activity_id.external_id_part(),
            namespace,
        )?;

        use schema::wasinformedby::dsl as link;
        diesel::insert_into(schema::wasinformedby::table)
            .values((
                &link::activity_id.eq(storedactivity.id),
                &link::informing_activity_id.eq(storedinformingactivity.id),
            ))
            .on_conflict_do_nothing()
            .execute(connection)?;

        Ok(())
    }

    #[instrument(skip(self, connection))]
    fn apply_was_associated_with(
        &self,
        connection: &mut PgConnection,
        namespaceid: &common::prov::NamespaceId,
        association: &Association,
    ) -> Result<(), StoreError> {
        let storedactivity = self.activity_by_activity_external_id_and_namespace(
            connection,
            association.activity_id.external_id_part(),
            namespaceid,
        )?;

        let storedagent = self.agent_by_agent_external_id_and_namespace(
            connection,
            association.agent_id.external_id_part(),
            namespaceid,
        )?;

        use schema::association::dsl as asoc;
        diesel::insert_into(schema::association::table)
            .values((
                &asoc::activity_id.eq(storedactivity.id),
                &asoc::agent_id.eq(storedagent.id),
                &asoc::role.eq(association.role.as_ref()),
            ))
            .on_conflict_do_nothing()
            .execute(connection)?;

        Ok(())
    }

    #[instrument(skip(self, connection, namespace))]
    fn apply_delegation(
        &self,
        connection: &mut PgConnection,
        namespace: &common::prov::NamespaceId,
        delegation: &Delegation,
    ) -> Result<(), StoreError> {
        let responsible = self.agent_by_agent_external_id_and_namespace(
            connection,
            delegation.responsible_id.external_id_part(),
            namespace,
        )?;

        let delegate = self.agent_by_agent_external_id_and_namespace(
            connection,
            delegation.delegate_id.external_id_part(),
            namespace,
        )?;

        let activity = {
            if let Some(ref activity_id) = delegation.activity_id {
                Some(
                    self.activity_by_activity_external_id_and_namespace(
                        connection,
                        activity_id.external_id_part(),
                        namespace,
                    )?
                    .id,
                )
            } else {
                None
            }
        };

        use schema::delegation::dsl as link;
        diesel::insert_into(schema::delegation::table)
            .values((
                &link::responsible_id.eq(responsible.id),
                &link::delegate_id.eq(delegate.id),
                &link::activity_id.eq(activity),
                &link::role.eq(delegation.role.as_ref()),
            ))
            .on_conflict_do_nothing()
            .execute(connection)?;

        Ok(())
    }

    #[instrument(skip(self, connection, namespace))]
    fn apply_derivation(
        &self,
        connection: &mut PgConnection,
        namespace: &common::prov::NamespaceId,
        derivation: &Derivation,
    ) -> Result<(), StoreError> {
        let stored_generated = self.entity_by_entity_external_id_and_namespace(
            connection,
            derivation.generated_id.external_id_part(),
            namespace,
        )?;

        let stored_used = self.entity_by_entity_external_id_and_namespace(
            connection,
            derivation.used_id.external_id_part(),
            namespace,
        )?;

        let stored_activity = derivation
            .activity_id
            .as_ref()
            .map(|activity_id| {
                self.activity_by_activity_external_id_and_namespace(
                    connection,
                    activity_id.external_id_part(),
                    namespace,
                )
            })
            .transpose()?;

        use schema::derivation::dsl as link;
        diesel::insert_into(schema::derivation::table)
            .values((
                &link::used_entity_id.eq(stored_used.id),
                &link::generated_entity_id.eq(stored_generated.id),
                &link::typ.eq(derivation.typ),
                &link::activity_id.eq(stored_activity.map(|activity| activity.id)),
            ))
            .on_conflict_do_nothing()
            .execute(connection)?;

        Ok(())
    }

    #[instrument(skip(connection))]
    fn apply_was_generated_by(
        &self,
        connection: &mut PgConnection,
        namespace: &common::prov::NamespaceId,
        generation: &Generation,
    ) -> Result<(), StoreError> {
        let storedactivity = self.activity_by_activity_external_id_and_namespace(
            connection,
            generation.activity_id.external_id_part(),
            namespace,
        )?;

        let storedentity = self.entity_by_entity_external_id_and_namespace(
            connection,
            generation.generated_id.external_id_part(),
            namespace,
        )?;

        use schema::generation::dsl as link;
        diesel::insert_into(schema::generation::table)
            .values((
                &link::activity_id.eq(storedactivity.id),
                &link::generated_entity_id.eq(storedentity.id),
            ))
            .on_conflict_do_nothing()
            .execute(connection)?;

        Ok(())
    }

    pub(crate) fn connection(
        &self,
    ) -> Result<PooledConnection<ConnectionManager<PgConnection>>, StoreError> {
        Ok(self.pool.get()?)
    }

    #[instrument(skip(connection))]
    pub(crate) fn get_current_agent(
        &self,
        connection: &mut PgConnection,
    ) -> Result<query::Agent, StoreError> {
        use schema::agent::dsl;
        Ok(schema::agent::table
            .filter(dsl::current.ne(0))
            .first::<query::Agent>(connection)?)
    }

    /// Get the last fully synchronized offset
    #[instrument]
    pub(crate) fn get_last_offset(&self) -> Result<Option<(Offset, String)>, StoreError> {
        use schema::ledgersync::dsl;
        self.connection()?.build_transaction().run(|connection| {
            schema::ledgersync::table
                .order_by(dsl::sync_time)
                .select((dsl::offset, dsl::tx_id))
                .first::<(Option<String>, String)>(connection)
                .map_err(StoreError::from)
                .map(|(offset, tx_id)| offset.map(|offset| (Offset::from(&*offset), tx_id)))
        })
    }

    #[instrument(skip(connection))]
    pub(crate) fn namespace_by_external_id(
        &self,
        connection: &mut PgConnection,
        namespace: &ExternalId,
    ) -> Result<(NamespaceId, i32), StoreError> {
        use self::schema::namespace::dsl;

        let ns = dsl::namespace
            .filter(dsl::external_id.eq(namespace))
            .select((dsl::id, dsl::external_id, dsl::uuid))
            .first::<(i32, String, String)>(connection)
            .optional()?
            .ok_or(StoreError::RecordNotFound {})?;

        Ok((
            NamespaceId::from_external_id(ns.1, Uuid::from_str(&ns.2)?),
            ns.0,
        ))
    }

    #[instrument(skip(connection))]
    pub(crate) fn attachment_by(
        &self,
        connection: &mut PgConnection,
        namespaceid: &NamespaceId,
        attachment: &EvidenceId,
    ) -> Result<query::Attachment, StoreError> {
        use self::schema::attachment::dsl;
        let (_, nsid) =
            self.namespace_by_external_id(connection, namespaceid.external_id_part())?;
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
        connection: &mut PgConnection,
        namespaceid: &NamespaceId,
        identity: &IdentityId,
    ) -> Result<query::Identity, StoreError> {
        use self::schema::identity::dsl;
        let (_, nsid) =
            self.namespace_by_external_id(connection, namespaceid.external_id_part())?;
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
    pub(crate) fn new(pool: Pool<ConnectionManager<PgConnection>>) -> Result<Self, StoreError> {
        Ok(Store { pool })
    }

    #[instrument(skip(connection))]
    pub(crate) fn prov_model_for_namespace(
        &self,
        connection: &mut PgConnection,
        query: QueryCommand,
    ) -> Result<ProvModel, StoreError> {
        let mut model = ProvModel::default();
        let (namespaceid, nsid) =
            self.namespace_by_external_id(connection, &ExternalId::from(&query.namespace))?;

        let agents = schema::agent::table
            .filter(schema::agent::namespace_id.eq(&nsid))
            .load::<query::Agent>(connection)?;

        for agent in agents {
            let attributes = schema::agent_attribute::table
                .filter(schema::agent_attribute::agent_id.eq(&agent.id))
                .load::<query::AgentAttribute>(connection)?;

            debug!(?agent, "Map agent to prov");
            let agentid: AgentId = AgentId::from_external_id(&agent.external_id);
            model.agents.insert(
                (namespaceid.clone(), agentid.clone()),
                Agent {
                    id: agentid.clone(),
                    namespaceid: namespaceid.clone(),
                    external_id: ExternalId::from(&agent.external_id),
                    domaintypeid: agent.domaintype.map(|x| DomaintypeId::from_external_id(&x)),
                    attributes: attributes
                        .into_iter()
                        .map(|attr| {
                            serde_json::from_str(&attr.value).map(|value| {
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

            let id: ActivityId = ActivityId::from_external_id(&activity.external_id);
            model.activities.insert(
                (namespaceid.clone(), id.clone()),
                Activity {
                    id: id.clone(),
                    namespaceid: namespaceid.clone(),
                    external_id: activity.external_id.into(),
                    started: activity.started.map(|x| DateTime::from_utc(x, Utc)),
                    ended: activity.ended.map(|x| DateTime::from_utc(x, Utc)),
                    domaintypeid: activity
                        .domaintype
                        .map(|x| DomaintypeId::from_external_id(&x)),
                    attributes: attributes
                        .into_iter()
                        .map(|attr| {
                            serde_json::from_str(&attr.value).map(|value| {
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
                .select(schema::entity::external_id)
                .load::<String>(connection)?
            {
                model.was_generated_by(
                    namespaceid.clone(),
                    &EntityId::from_external_id(&generation),
                    &id,
                );
            }

            for used in schema::usage::table
                .filter(schema::usage::activity_id.eq(activity.id))
                .order(schema::usage::activity_id.asc())
                .inner_join(schema::entity::table)
                .select(schema::entity::external_id)
                .load::<String>(connection)?
            {
                let used = used;
                model.used(namespaceid.clone(), &id, &EntityId::from_external_id(&used));
            }

            for wasinformedby in schema::wasinformedby::table
                .filter(schema::wasinformedby::activity_id.eq(activity.id))
                .inner_join(
                    schema::activity::table
                        .on(schema::wasinformedby::informing_activity_id.eq(schema::activity::id)),
                )
                .select(schema::activity::external_id)
                .load::<String>(connection)?
            {
                let wasinformedby = wasinformedby;
                model.was_informed_by(
                    namespaceid.clone(),
                    &id,
                    &ActivityId::from_external_id(wasinformedby),
                );
            }
        }

        let entities = schema::entity::table
            .filter(schema::entity::namespace_id.eq(nsid))
            .load::<query::Entity>(connection)?;

        for query::Entity {
            id,
            namespace_id: _,
            domaintype,
            external_id,
            attachment_id: _,
        } in entities
        {
            let attributes = schema::entity_attribute::table
                .filter(schema::entity_attribute::entity_id.eq(&id))
                .load::<query::EntityAttribute>(connection)?;

            let id: EntityId = EntityId::from_external_id(&external_id);
            model.entities.insert(
                (namespaceid.clone(), id.clone()),
                Entity {
                    id,
                    namespaceid: namespaceid.clone(),
                    external_id: external_id.into(),
                    domaintypeid: domaintype.map(|x| DomaintypeId::from_external_id(&x)),
                    attributes: attributes
                        .into_iter()
                        .map(|attr| {
                            serde_json::from_str(&attr.value).map(|value| {
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
    pub(crate) fn set_last_offset(
        &self,
        offset: Offset,
        tx_id: ChronicleTransactionId,
    ) -> Result<(), StoreError> {
        use schema::ledgersync as dsl;

        if let Offset::Identity(offset) = offset {
            Ok(self.connection()?.build_transaction().run(|connection| {
                diesel::insert_into(dsl::table)
                    .values((
                        dsl::offset.eq(offset),
                        dsl::tx_id.eq(&*tx_id.to_string()),
                        (dsl::sync_time.eq(Utc::now().naive_utc())),
                    ))
                    .on_conflict(dsl::tx_id)
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
        connection: &mut PgConnection,
        external_id: &ExternalId,
        namespace: &ExternalId,
    ) -> Result<(), StoreError> {
        let (_, nsid) = self.namespace_by_external_id(connection, namespace)?;
        use schema::agent::dsl;

        diesel::update(schema::agent::table.filter(dsl::current.ne(0)))
            .set(dsl::current.eq(0))
            .execute(connection)?;

        diesel::update(
            schema::agent::table.filter(
                dsl::external_id
                    .eq(external_id)
                    .and(dsl::namespace_id.eq(nsid)),
            ),
        )
        .set(dsl::current.eq(1))
        .execute(connection)?;

        Ok(())
    }
}
