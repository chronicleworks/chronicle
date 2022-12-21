# Command line options

## Subcommands

### `serve-graphql`

Run Chronicle as a GraphQL server.

#### Arguments

##### `--interface <interface>`

The GraphQL server socket address - defaults to 127.0.0.1:9982.

#### `--open`

Serve the GraphQL playground IDE on the GraphQL server interface and open a web
browser to it.

### `export-schema`

Write the GraphQL SDL for Chronicle to stdout and exit.

### `completions`

Installs shell completions for bash, zsh, or fish.

## Other Subcommands

Chronicle will also generate subcommands for recording provenance, derived from
your [domain configuration](./domain_modeling.md).
