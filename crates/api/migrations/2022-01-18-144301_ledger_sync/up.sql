create table namespace (
    id serial primary key,
    external_id text not null,
    uuid text not null,
    unique(external_id)
);

create unique index namespace_idx on namespace(external_id,uuid);

create table ledgersync (
    tx_id text primary key,
    bc_offset text,
    sync_time timestamp
);

create index ledger_index on ledgersync(sync_time,bc_offset);

create table identity (
    id serial primary key,
    namespace_id integer not null,
    public_key text not null,
    foreign key(namespace_id) references namespace(id)
);

create index identity_public_key_idx on identity(public_key);

create table agent (
    id serial primary key,
    external_id text not null,
    namespace_id integer not null,
    domaintype text,
    current integer not null,
    identity_id integer,
    foreign key(identity_id) references identity(id),
    foreign key(namespace_id) references namespace(id),
    unique(external_id,namespace_id)
);

create index agent_external_id_idx on agent(external_id,namespace_id);

create table attachment (
    id serial primary key,
    namespace_id integer not null,
    signature_time timestamp not null,
    signature text not null,
    signer_id integer not null,
    locator text,
    foreign key(namespace_id) references namespace(id),
    foreign key(signer_id) references identity(id)
);

create index attachment_signature_idx on attachment(signature);

create table activity (
    id serial primary key,
    external_id text not null,
    namespace_id integer not null,
    domaintype text,
    started timestamp,
    ended timestamp,
    foreign key(namespace_id) references namespace(id),
    unique(external_id,namespace_id)
);

create table entity (
    id serial primary key,
    external_id text not null,
    namespace_id integer not null,
    domaintype text,
    attachment_id integer,
    foreign key(attachment_id) references attachment(id),
    foreign key(namespace_id) references namespace(id),
    unique(external_id,namespace_id)
);

create table delegation (
    delegate_id integer not null,
    responsible_id integer not null,
    activity_id integer not null default -1,
    role text not null default '',
    foreign key(delegate_id) references agent(id),
    foreign key(responsible_id) references agent(id),
    foreign key(activity_id) references activity(id),
    primary key(responsible_id,delegate_id,activity_id,role)
);

create table derivation (
    activity_id integer,
    generated_entity_id integer not null,
    used_entity_id integer not null,
    typ integer not null default -1,
    foreign key(activity_id) references activity(id),
    foreign key(generated_entity_id) references entity(id),
    foreign key(used_entity_id) references entity(id),
    primary key(activity_id,used_entity_id,generated_entity_id,typ)
);

create table generation (
    activity_id integer not null,
    generated_entity_id integer not null,
    typ text,
    foreign key(activity_id) references activity(id),
    foreign key(generated_entity_id) references entity(id),
    primary key(activity_id,generated_entity_id)
);

create table association (
    agent_id integer not null,
    activity_id integer not null,
    role text not null default '',
    foreign key(agent_id) references agent(id),
    foreign key(activity_id) references activity(id),
    primary key(agent_id, activity_id, role)
);

create table usage (
    activity_id integer not null,
    entity_id integer not null,
    foreign key(entity_id) references entity(id),
    foreign key(activity_id) references activity(id),
    primary key(activity_id,entity_id)
);

create table wasinformedby (
    activity_id integer not null,
    informing_activity_id integer not null,
    foreign key(activity_id) references activity(id),
    foreign key(informing_activity_id) references activity(id),
    primary key(activity_id,informing_activity_id)
);

create table hadidentity (
    agent_id integer not null,
    identity_id integer not null,
    foreign key(agent_id) references agent(id),
    foreign key(identity_id) references identity(id),
    primary key(agent_id,identity_id)
);

create table hadattachment (
    entity_id integer not null,
    attachment_id integer not null,
    foreign key(entity_id) references entity(id),
    foreign key(attachment_id) references attachment(id),
    primary key(entity_id,attachment_id)
);

create table entity_attribute (
    entity_id integer not null,
    typename text not null,
    value text not null,
    foreign key(entity_id) references entity(id),
    primary key(entity_id,typename)
);

create table agent_attribute (
    agent_id integer not null,
    typename text not null,
    value text not null,
    foreign key(agent_id) references agent(id),
    primary key(agent_id,typename)
);

create table activity_attribute (
    activity_id integer not null,
    typename text not null,
    value text not null,
    foreign key(activity_id) references activity(id),
    primary key(activity_id,typename)
);

insert into namespace(id, external_id, uuid)
    values (-1, 'hidden entry for Option None', '00000000-0000-0000-0000-000000000000');

insert into activity(id, external_id, namespace_id)
    values (-1, 'hidden entry for Option None', -1);
