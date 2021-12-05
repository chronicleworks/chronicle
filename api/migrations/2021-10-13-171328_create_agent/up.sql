create table agent (
    id integer primary key not null,
    name text key not null,
    namespace text not null,
    domaintype text,
    publickey text,
    current integer not null,
    foreign key(namespace) references namespace(name)
    unique(name,namespace)
);