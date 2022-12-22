# Configuration

## Configuration File

Chronicle's `config.toml` file contains settings for where to store secret
keys, how to connect to the Sawtooth validator, and any namespace bindings. If
a configuration file does not yet exist, Chronicle will create one before its
API service starts up.

## Remote PostgreSQL Database

The standard PostgreSQL variables `PGHOST`, `PGPORT`, `PGUSER`, `PGPASSWORD`,
`PGDATABASE` are recognized by Chronicle which uses them in attempting to
connect to a remote database on startup. To prevent fallback to an embedded
database on connection failure, use `--remote-database`. As described in the
`--help`, values for the database connection configuration can be provided via
`--database-*` options at the command line, except for `PGPASSWORD` for
reasons of security.

## In-Memory Operation for Development

Passing the `--features inmem` argument to `cargo run` has Chronicle use an
in-memory transaction processor rather than attempting to connect to Sawtooth.

Passing the `--embedded-database` argument to Chronicle itself has it use an
embedded PostgreSQL server instead of attempting to connect to a database
provided in the deployment environment.

The combination of these options is useful for obtaining a simple development
environment wherein provenance information need not persist between sessions.

An example for GraphQL interaction:

```bash
cargo run --features inmem --bin chronicle -- --embedded-database serve-graphql --open
```
