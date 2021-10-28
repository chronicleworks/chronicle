create table activity (
    id integer primary key not null,
    name text not null,
    namespace text not null,
    started text,
    ended text,
    foreign key(namespace) references namespace(name)
);

create table entity (
    id integer primary key not null,
    name text not null,
    namespace text not null,
    started text,
    ended text,
    foreign key(namespace) references namespace(name)
);

create table wasattributedto (
    id integer primary key not null,
    agent integer not null,
    activity integer not null,
    foreign key(agent) references agent(id),
    foreign key(activity) references activity(id)
);

create table wasgeneratedby (
    id integer primary key not null,
    agent integer not null,
    entity integer not null,
    foreign key(agent) references agent(id),
    foreign key(entity) references entity(id)
);

create table uses (
    id integer primary key not null,
    agent integer not null,
    entity integer not null,
    foreign key(agent) references agent(id),
    foreign key(entity) references entity(id)
);

create table wasasociatedwith (
    id integer primary key not null,
    agent integer not null,
    activity integer not null,
    foreign key(agent) references agent(id),
    foreign key(activity) references activity(id)
);

 