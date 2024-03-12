-- This file should undo anything in `up.sql`

create table attachment (
    id serial primary key,
    namespace_id integer not null,
    signature_time timestamp not null,
    signature text not null,
    signer_id integer not null,
    locator text,
    foreign key(namespace_id) references namespace(id),
    foreign key(signer_id) references identity(id)
);

create index attachment_signature_idx on attachment(signature);

create table hadattachment (
    entity_id integer not null,
    attachment_id integer not null,
    foreign key(entity_id) references entity(id),
    foreign key(attachment_id) references attachment(id),
    primary key(entity_id,attachment_id)
);

alter table entity
    add column attachment_id integer,
    add constraint entity_attachment_id_fkey
        foreign key (attachment_id) references attachment(id);
