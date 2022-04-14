create table namespace (
    id integer primary key not null,
    name text not null,
    uuid text not null,
    unique(name,uuid)
);

create unique index namespace_idx on namespace(name,uuid);

create table ledgersync (
    correlation_id text primary key not null,
    offset text,
    sync_time timestamp
);

create index ledger_index on ledgersync(sync_time,offset);

create table agent (
    id integer primary key not null,
    name text key not null,
    namespace_id integer not null,
    domaintype text,
    current integer not null,
    identity_id integer,
    foreign key(identity_id) references identity(id),
    foreign key(namespace_id) references namespace(id),
    unique(name,namespace_id)
);

create index agent_name_idx on agent(name,namespace_id);

create table identity (
    id integer primary key not null,
    namespace_id integer not null,
    public_key text not null,
    foreign key(namespace_id) references namespace(id)
);

create index identity_public_key_idx on identity(public_key);

create table activity (
    id integer primary key not null,
    name text not null,
    namespace_id integer not null,
    domaintype text,
    started timestamp,
    ended timestamp,
    foreign key(namespace_id) references namespace(id),
    unique(name,namespace_id)
);

create table entity (
    id integer primary key not null,
    name text not null,
    namespace_id integer not null,
    domaintype text,
    attachment_id integer,
    foreign key(attachment_id) references attachment(id),
    foreign key(namespace_id) references namespace(id),
    unique(name,namespace_id)
);

create table attachment (
    id integer primary key not null,
    namespace_id integer not null,
    signature_time timestamp not null,
    signature text not null,
    signer_id integer not null,
    locator text,
    foreign key(namespace_id) references namespace(id),
    foreign key(signer_id) references identity(id)
);

create index attachment_signature_idx on attachment(signature);

create table delegation (
    offset integer not null,
    delegate_id integer not null,
    responsible_id integer not null,
    activity_id integer,
    typ text,
    foreign key(delegate_id) references agent(id),
    foreign key(responsible_id) references agent(id),
    foreign key(activity_id) references activity(id),
    primary key(offset,responsible_id,delegate_id)
);

create table derivation (
    offset integer not null,
    activity_id integer,
    generated_entity_id integer not null,
    used_entity_id integer not null,
    typ integer,
    foreign key(activity_id) references activity(id),
    foreign key(generated_entity_id) references entity(id),
    foreign key(used_entity_id) references entity(id),
    primary key(offset,generated_entity_id)
);

create table generation (
    offset integer not null,
    activity_id integer not null,
    generated_entity_id integer not null,
    typ text,
    foreign key(activity_id) references activity(id),
    foreign key(generated_entity_id) references entity(id),
    primary key(offset,generated_entity_id)
);

create table association (
    offset integer not null,
    agent_id integer not null,
    activity_id integer not null,
    foreign key(agent_id) references agent(id),
    foreign key(activity_id) references activity(id),
    primary key(offset,agent_id)
);

create table useage (
    offset integer not null,
    activity_id integer not null,
    entity_id integer not null,
    foreign key(entity_id) references entity(id),
    foreign key(activity_id) references activity(id),
    primary key(offset,entity_id)
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
    foreign key(entity_id ) references entity(id),
    foreign key(attachment_id) references attachment(id),
    primary key(entity_id,attachment_id)
);




