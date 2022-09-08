# Command line options

## Sub commands

### serve-graphql

Run chronicle as a graphql server.

#### Arguments

##### `--interface <interface>`

The graphql server socket address - defaults to 127.0.0.1:9982.

#### `--open`

Serve the graphql playground IDE on the graphql server interface and open a web
browser to it.

### export-schema

Write the graphql SDL for Chronicle to stdout and exit

### completions

Installs shell completions for  bash, zsh or fish

## Other sub commands

Chronicle will also generate sub commands for recording provenance, derived from
your [domain configuration](./domain_modelling.md).
