# Command line options

## Subcommands

### `serve-graphql`

Run Chronicle as a GraphQL server.

#### Arguments

##### `--interface <interface>`

The GraphQL server socket address - defaults to 127.0.0.1:9982.

##### `--anonymous-api`

Allow unauthenticated access to the GraphQL API.
Otherwise, the following two arguments are relevant and required.

##### `--jwks-address <URI>`

The URI from which the JSON Web Key Set (JWKS) can be obtained.
This typically has a suffix of `jwks.json`.
The JWKS is used for verifying JSON Web Tokens provided via HTTP
`Authorized: Bearer ...`.

Ignored if `--anonymous-api` is given.

##### `id-pointer <JSON ptr>`

A JSON pointer into the JSON Web Token claims, specifying the location of
a string value corresponding to the external ID that should be used in
constructing the authenticated user's Chronicle identity.

Ignored if `--anonymous-api` is given.

### `export-schema`

Write the GraphQL SDL for Chronicle to stdout and exit.

### `completions`

Installs shell completions for bash, zsh, or fish.

## Other Subcommands

Chronicle will also generate subcommands for recording provenance, derived from
your [domain configuration](./domain_modeling.md).
