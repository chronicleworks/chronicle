create table activity (
    id integer primary key not null,
    name text not null,
    namespace text not null,
    domaintype text,
    started timestamp,
    ended timestamp,
    foreign key(namespace) references namespace(name)
    unique(name,namespace)
);

create table entity (
    id integer primary key not null,
    name text not null,
    namespace text not null,
    domaintype text,
    signature_time timestamp,
    signature text,
    locator text,
    foreign key(namespace) references namespace(name)
    unique(name,namespace)
);

create table wasgeneratedby (
    activity integer not null,
    entity integer not null,
    foreign key(activity) references activity(id),
    foreign key(entity) references entity(id),
    primary key(activity,entity)
);

create table used (
    activity integer not null,
    entity integer not null,
    foreign key(activity) references activity(id),
    foreign key(entity) references entity(id),
    primary key(activity,entity)
);

create table wasassociatedwith (
    agent integer not null,
    activity integer not null,
    foreign key(agent) references agent(id),
    foreign key(activity) references activity(id),
    primary key(agent,activity)
);

 