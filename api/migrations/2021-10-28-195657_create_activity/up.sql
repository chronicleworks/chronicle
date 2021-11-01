create table activity (
    id integer primary key not null,
    name text not null,
    namespace text not null,
    started timestamp,
    ended timestamp,
    foreign key(namespace) references namespace(name)
);

create table entity (
    id integer primary key not null,
    name text not null,
    namespace text not null,
    started timestamp,
    ended timestamp,
    foreign key(namespace) references namespace(name)
);

create table wasattributedto (
    agent integer not null,
    activity integer not null,
    foreign key(agent) references agent(id),
    foreign key(activity) references activity(id),
    primary key (agent,activity)
);

create table wasgeneratedby (
    agent integer not null,
    entity integer not null,
    foreign key(agent) references agent(id),
    foreign key(entity) references entity(id),
    primary key(agent,entity)
);

create table uses (
    agent integer not null,
    entity integer not null,
    foreign key(agent) references agent(id),
    foreign key(entity) references entity(id),
    primary key(agent,entity)
);

create table wasassociatedwith (
    agent integer not null,
    activity integer not null,
    foreign key(agent) references agent(id),
    foreign key(activity) references activity(id),
    primary key(agent,activity)
);

 