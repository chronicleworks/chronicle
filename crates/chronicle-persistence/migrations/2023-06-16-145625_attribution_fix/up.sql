create table if not exists attribution (
    agent_id integer not null,
    entity_id integer not null,
    role text not null default '',
    foreign key(agent_id) references agent(id),
    foreign key(entity_id) references entity(id),
    primary key(agent_id, entity_id, role)
);
