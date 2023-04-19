# Command-Line Options

## Subcommands

### `serve-api`

Run Chronicle as an API server.

#### Arguments

##### Client Connection Settings

###### `--interface <interface>`

The API server socket address - defaults to 127.0.0.1:9982.

###### `--offer-endpoints <name> <name> ...`

Which endpoints to listen at for serving requests. By default, all are served.
Options are:

- `data` for IRIs encoded in URIs (at `/context` and `/data`)
- `graphql` for GraphQL requests (at `/` and `/ws`)

##### Authentication

###### `--id-pointer <JSON ptr>`

A JSON pointer into the JSON Web Token claims, specifying the location of
a string value corresponding to the external ID that should be used in
constructing the authenticated user's Chronicle identity.

###### `--jwks-address <URI>`

The URI from which the JSON Web Key Set (JWKS) can be obtained.
This typically has a suffix of `jwks.json`.
The JWKS is used for verifying JSON Web Tokens provided via HTTP
`Authorized: Bearer ...`.

###### `--jwt-must-claim <name> <value>`

For security, the GraphQL server can be set to ignore JSON Web Tokens that
do not include the expected claims. It may be appropriate to set required
values for names such as `iss`, `aud`, or `azp` depending on the claims
expected in the JWTs issued by the authorization server.

This option may be given multiple times. To set via environment variables
instead, prefix each variable name with `JWT_MUST_CLAIM_`.

###### `--require-auth`

Reject anonymous requests. Requires `--jwks-address` because identity for
every request must be established verifiably.

###### `--userinfo-address`

The URI that should be used to exchange access tokens for user information,
typically suffixed `/userinfo`. If this option is supplied then the endpoint
must supply any additional claims in response to the same `Authorization:`
header that was provided by a user in making their API request.

##### Deprecated Options

Options may be removed in the next release of Chronicle.

###### `--playground` / `--open`

Make the GraphQL Playground available.

### `export-schema`

Write the GraphQL SDL for Chronicle to stdout and exit.

### `completions`

Installs shell completions for bash, zsh, or fish.

## Other Subcommands

Chronicle will also generate subcommands for recording provenance, derived from
your [domain configuration](./domain_modeling.md).
