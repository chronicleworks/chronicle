# Command line options

## Subcommands

### `serve-graphql`

Run Chronicle as a GraphQL server.

#### Arguments

##### `--interface <interface>`

The GraphQL server socket address - defaults to 127.0.0.1:9982.

##### `--anonymous-api`

Ignore any JSON Web Tokens that are provided to the GraphQL API,
regard all access as being anonymous from unauthenticated users.
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

##### `--jwt-must-claim name value`

For security, the GraphQL server can be set to ignore JSON Web Tokens that
do not include the expected claims. It may be appropriate to set required
values for names such as `iss`, `aud`, or `azp` depending on the claims
expected in the JWTs issued by the authorization server.

This option may be given multiple times. To set via environment variables
instead, prefix each variable name with `JWT_MUST_CLAIM_`.

Ignored if `--anonymous-api` is given.

##### `--opa-rule <opa-rule>`

A path or a Sawtooth address of an OPA policy compiled to Wasm. Required
unless `--features devmode` is selected, in which case Chronicle uses
an embedded default-allow OPA policy. This devmode feature can be overridden
by providing `--opa-rule` and `--opa-entrypoint`.

##### `--opa-entrypoint <opa-entrypoint>`

An entrypoint for the OPA policy indexed by `--opa-rule`. Required if
`--opa-rule` is given.

### `export-schema`

Write the GraphQL SDL for Chronicle to stdout and exit.

### `completions`

Installs shell completions for bash, zsh, or fish.

## Other Subcommands

Chronicle will also generate subcommands for recording provenance, derived from
your [domain configuration](./domain_modeling.md).
