# Configuration

## Configuring Chronicle

### Chronicle Configuration File

Chronicle's `config.toml` file contains settings for where to store secret
keys, how to connect to the Sawtooth validator, and any namespace bindings. If
a configuration file does not yet exist, Chronicle will create one before its
API service starts up.

You may also specify an existing configuration file to use via the `--config`
cli argument.

```bash
--config /path/to/config.toml
```

Or via the `CHRONICLE_CONFIG` environment variable.

```bash
CHRONICLE_CONFIG=/path/to/config.toml
```

### Chronicle Configuration Options

Additionally, the [command-line interface](./cli.md) offers various arguments,
many of which have an associated environment variable that can be used to set
their value. The [opactl](./opa.md) utility is important in setting access
controls.

## Remote PostgreSQL Database

### Setup

Install PostgreSQL with a download from the
[official website](https://www.postgresql.org/).
Alternatively, it is offered through many popular package managers, so you may
find it more conveniently available via whichever means you typically install
other software on your system. In production, we recommend running Chronicle
and PostgreSQL on separate systems, whether virtual or physical.

To learn more about installing PostgreSQL, and about how to set up a database,
user and password for Chronicle, refer to the
[Server Administration](https://www.postgresql.org/docs/current/admin.html)
part of PostgreSQL's documentation. After Chronicle successfully connects to
an empty database then it will automatically create the tables, etc. that it
requires.

### Use with Chronicle

The standard PostgreSQL variables `PGHOST`, `PGPORT`, `PGUSER`, `PGPASSWORD`,
`PGDATABASE` are recognized by Chronicle which uses them in attempting to
connect to a remote database on startup. As described in the `--help`,
values for the database connection configuration can be provided via
`--database-*` options at the command line, except for `PGPASSWORD` for
reasons of security.

## Authentication and Authorization

Separate sections describe how [identity is established](./auth.md) and
[access is controlled](./opa.md). The [command-line interface](./cli.md)
and associated environment variables include options relating to these.

## In-Memory Operation for Development

Passing the `--features inmem` argument to `cargo run` has Chronicle use an
in-memory transaction processor rather than attempting to connect to Sawtooth.
