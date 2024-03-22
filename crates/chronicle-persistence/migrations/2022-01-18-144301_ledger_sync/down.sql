-- This file should undo anything in `up.sql`

drop table activity_attribute;
drop table agent_attribute;
drop table entity_attribute;
drop table hadattachment;
drop table hadidentity;
drop table wasinformedby;
drop table usage;
drop table association;
drop table if exists attribution;
drop table generation;
drop table derivation;
drop table delegation;
drop table entity;
drop table activity;
drop index attachment_signature_idx;
drop table attachment;
drop index agent_external_id_idx;
drop table agent;
drop index identity_public_key_idx;
drop table identity;
drop index ledger_index;
drop table ledgersync;
drop index namespace_idx;
drop table namespace;
