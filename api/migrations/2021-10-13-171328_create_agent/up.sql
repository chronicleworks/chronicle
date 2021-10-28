create table agent (
    id integer primary key not null,
    name text key not null,
    namespace text not null,
    publickey text,
    privatekeypath text,
    current integer not null,
    foreign key(namespace) references namespace(name)
);