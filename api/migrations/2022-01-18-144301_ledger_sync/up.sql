-- Your SQL goes here
create table ledgersync(
    offset text primary key not null,
    sync_time timestamp
);