create table agent (
    name text primary key not null,
    namespace text not null,
    publickey text,
    privatekeypath text,
    current integer not null,
    foreign key(namespace) references namespace(name)
);