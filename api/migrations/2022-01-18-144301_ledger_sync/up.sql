-- Your SQL goes here
create table ledgersync(
    offset text not null,
    sync_time timestamp,
    primary key (offset, sync_time)
);