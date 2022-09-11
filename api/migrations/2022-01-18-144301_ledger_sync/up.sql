create table namespace (
    id integer primary key not null,
    name text not null,
    uuid text not null,
    unique(name)
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
    delegate_id integer not null,
    responsible_id integer not null,
    activity_id integer,
    role text,
    foreign key(delegate_id) references agent(id),
    foreign key(responsible_id) references agent(id),
    foreign key(activity_id) references activity(id),
    primary key(responsible_id,delegate_id,activity_id,role)
    unique(responsible_id,delegate_id,activity_id,role)
);

create table derivation (
    activity_id integer,
    generated_entity_id integer not null,
    used_entity_id integer not null,
    typ integer,
    foreign key(activity_id) references activity(id),
    foreign key(generated_entity_id) references entity(id),
    foreign key(used_entity_id) references entity(id),
    primary key(activity_id,used_entity_id,generated_entity_id,typ)
    unique(activity_id,used_entity_id,generated_entity_id,typ)
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
    role text,
    foreign key(agent_id) references agent(id),
    foreign key(activity_id) references activity(id),
    primary key(agent_id, activity_id, role)
    unique(agent_id, activity_id, role)
);

create table usage (
    activity_id integer not null,
    entity_id integer not null,
    foreign key(entity_id) references entity(id),
    foreign key(activity_id) references activity(id),
    primary key(activity_id,entity_id)
);

create table wasinformedby (
    namespace_id integer not null,
    activity_id integer not null,
    informing_activity_id integer not null,
    foreign key (namespace_id) references namespace(id),
    foreign key(activity_id) references activity(id),
    primary key(activity_id,namespace_id)
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


create table entity_attribute (
    entity_id integer not null,
    typename text not null,
    value text not null,
    foreign key(entity_id ) references entity(id),
    primary key(entity_id,typename)
);

create table agent_attribute (
    agent_id integer not null,
    typename text not null,
    value text not null,
    foreign key(agent_id ) references agent(id),
    primary key(agent_id,typename)
);

create table activity_attribute (
    activity_id integer not null,
    typename text not null,
    value text not null,
    foreign key(activity_id ) references activity(id),
    primary key(activity_id,typename)
);
