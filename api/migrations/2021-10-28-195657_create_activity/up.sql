create table activity (
    id integer primary key,
    name text not null,
    namespace text not null,
    started text,
    ended text,
    foreign key(namespace) references namespace(name)
);

create table entity (
    id integer primary key,
    name text not null,
    namespace text not null,
    started text,
    ended text,
    foreign key(namespace) references namespace(name)
);

create table wasattributedto (
    id integer primary key,
    agent integer,
    activity integer,
    foreign key(agent) references agent(id),
    foreign key(activity) references activity(id)
);

create table wasgeneratedby (
    id integer primary key,
    agent integer,
    entity integer,
    foreign key(agent) references agent(id),
    foreign key(entity) references entity(id)
);

create table uses (
    id integer primary key,
    agent integer,
    entity integer,
    foreign key(agent) references agent(id),
    foreign key(entity) references entity(id)
);

create table wasasociatedwith (
    id integer primary key,
    agent integer,
    activity integer,
    foreign key(agent) references agent(id),
    foreign key(activity) references activity(id)
);

 