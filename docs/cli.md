# Command-Line Options

## Subcommands

### `serve-api`

Run Chronicle as an API server.

#### Arguments

##### Client Connection Settings

###### `--interface <interface> ...`

The API server socket address. If more than one value is provided then the
Chronicle API will listen on all the specified sockets.

###### `--offer-endpoints <name> <name> ...`

Which endpoints to listen at for serving requests. By default, all are served.
Options are:

- `data` for IRIs encoded in URIs (at `/context` and `/data`)
- `graphql` for GraphQL requests (at `/` and `/ws`)

##### Authentication

###### `--id-claims <JWT field names>`

The names of the JSON Web Token claim fields whose values should be used to
determine the external ID of the authenticated user's Chronicle identity. All
listed fields must be present among the user's claims and their values must
be strings.

The order in which the fields are specified does not change the resulting
external ID.

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

### `import` <`namespace-id`> <`namespace-uuid`> <`path`>

The import command is used to load data from a JSON-LD file containing an
array of Chronicle Operations. This command requires two arguments:
`namespace-id` and `namespace-uuid`, which are used to create a unique
identifier for the namespace that the data is being imported to. The
`NamespaceId` is a required argument for the import command because it
allows the program to differentiate between different namespaces and avoid
conflicts when importing data. Data not matching the given namespace is
ignored.

By default, `import` will use standard input as its data source. Users can
also use an optional `path` argument to specify the file path of a JSON-LD
file to be imported.

Once the data has been successfully imported, the Chronicle Operations will
be added to the Chronicle database under the specified namespace.

To import to namespace `testns`, UUID 6803790d-5891-4dfa-b773-41827d2c630b
from standard input:

```bash
< import.json cargo run --bin chronicle \
    -- import \
    testns \
    6803790d-5891-4dfa-b773-41827d2c630b
```

To import from a file named import.json, run the following command:

```bash
chronicle \
    --bin chronicle \
    -- import \
    testns \
    6803790d-5891-4dfa-b773-41827d2c630b \
    import.json
```

## Other Subcommands

Chronicle will also generate subcommands for recording provenance, derived from
your [domain configuration](./domain_modeling.md).

## Load OPA Policy from URL or File Arguments

### `--opa-bundle-address <address>`

To load an OPA policy from a provided HTTP URL, file URL, or file path, you can
use the `--opa-bundle-address` option. The `--opa-bundle-address` argument must
be a valid URL or a valid file path.

Note that `--opa-bundle-address` requires both the `--opa-policy-name` and
`--opa-policy-entrypoint` arguments to be provided.

#### `--opa-policy-name <name>`

The `--opa-policy-name` option specifies the name of the OPA policy.

#### `--opa-policy-entrypoint <entrypoint>`

The `--opa-policy-entrypoint` option specifies the entrypoint within the policy.

#### Examples

Loading from an HTTP URL:

```text
--opa-bundle-address https://example.com/bundle.tar.gz --opa-policy-name my_policy --opa-policy-entrypoint entrypoint1
```

Replace `https://example.com/bundle.tar.gz` with the actual URL of the OPA policy
bundle you want to load.

Loading from a file URL:

```text
--opa-bundle-address file:///example.com/bundle.tar.gz --opa-policy-name my_policy --opa-policy-entrypoint entrypoint1
```

Replace `file:///example.com/bundle.tar.gz` with the actual file URL of the OPA
policy bundle you want to load.

Loading from a file path:

```text
--opa-bundle-address /path/to/bundle.tar.gz --opa-policy-name my_policy --opa-policy-entrypoint entrypoint1
```

Replace `/path/to/bundle.tar.gz` with the actual file path of the OPA policy
bundle you want to load.
