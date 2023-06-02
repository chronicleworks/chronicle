# Access Control Policy with OPA

## Default Policy in Chronicle

The default policy in Chronicle defines the access control rules for
authorization decisions. It consists of the `allow_transactions` and
`common_rules` packages, which define the authorization rules for
transaction.

### Default Policy Description

The default policy in Chronicle is defined using the
[Rego policy language](https://www.openpolicyagent.org/docs/latest/policy-language/)
of the Open Policy Agent (OPA). It consists of two main packages:
`allow_transactions` and `common_rules`.

#### `allow_transactions` Package

The `allow_transactions` package is responsible for defining authorization rules
related to transaction processing in Chronicle. It depends on the rules
specified in the `common_rules` package.

The following rules are defined in the `allow_transactions` package:

```rego
package allow_transactions

import data.common_rules
import future.keywords.in
import input

default allowed_users = false
allowed_users {
  common_rules.allowed_users
}

default allow_defines = false
allow_defines {
  common_rules.allow_defines
}

default deny_all = false
```

- `allowed_users`: This rule determines whether users are allowed to access
  resources or perform actions. It relies on the `allowed_users` rule defined
  in the `common_rules` package.

- `allow_defines`: This rule specifies whether users are allowed to execute
  Chronicle's `define` Mutation and Submission operations. It also relies on
  the `allow_defines` rule defined in the `common_rules` package.

- `deny_all`: This rule denies access to all resources or actions by
  default. It ensures that if no other rules match, access is denied.

#### `common_rules` Package

The `common_rules` package defines authorization rules that are common to
various parts of Chronicle's functionality.

The following rules are defined in the `common_rules` package:

```rego
package common_rules

import future.keywords.in
import input

allowed := {"chronicle", "anonymous"}

allowed_users {
  input.type in allowed
}

allow_defines {
  data.context.operation in ["Mutation", "Submission"]
  startswith(data.context.state[0], "define")
}
```

- `allowed_users`: This rule determines whether users of specific types
  are allowed to access resources or perform actions. In the current
  implementation, users with the types `"chronicle"` and `"anonymous"` are
  allowed.

- `allow_defines`: This rule specifies whether users can execute
  Chronicle's `define` Mutation and Submission operations. It checks the
  operation type and verifies if the state starts with the string `"define"`.

### Default Policy and JWKS Authorization

The default policy implemented in Chronicle does not allow access to
users authenticated using JWKS authorization. Users of type `"jwt"`
will be denied access under the default policy. This is because the
default policy does not include rules specific to JWKS authorization. If
you want to enable access for JWKS-authorized users, you need to modify
the policy accordingly.

### Modifying the Default Policy

To modify the default policy in Chronicle, you can follow these steps:

- Update the Rego policy files to define the desired access control
  rules.

- Use the `opactl` command-line tool to load the updated policy bundle
  into the OPA Transaction Processor.

- Configure Chronicle's settings to match the policy by setting the
  appropriate Sawtooth settings entries.

Detailed instructions on modifying the default policy can be found in
the [Configuration section](#configuring-chronicle-to-use-opa) of this
documentation.

## OPA Standard

The OPA standard, or [Open Policy Agent](https://www.openpolicyagent.org/docs/latest/),
is a policy engine that enables fine-grained control over authorization decisions
within an application or infrastructure. It provides a declarative language,
[Rego](https://www.openpolicyagent.org/docs/latest/policy-language/), for expressing
policies, and can be integrated with various applications and services via APIs.
The OPA standard allows policies to be enforced at various stages of the application
stack, and can be used to enforce compliance with regulations or to implement
customized security policies, based on user identity, roles, and permissions.

## OPA Execution in Chronicle

Chronicle integrates OPA execution at its GraphQL API request point and in its Transaction
Processor to ensure access control and policy compliance. OPA verifies user permissions
against policies that define which actions are allowed and denied to whom. Integrating
OPA execution in the transaction processor ensures that only transactions that meet
specific criteria are accepted and processed by the network.

## Authorization Rules in OPA

Authorization rules in Open Policy Agent (OPA) determine whether a user or client
is authorized to access a resource or perform an action.

As an example of how to define an authorization rule in OPA, suppose we have a REST
API endpoint that returns a list of users but we want to restrict access to this
endpoint only to users who have the "admin" role. We can define an OPA policy like
this:

```rego
default allow_access = false

allow_access {
  input.user.roles[_] = "admin"
}
```

See [here](https://www.openpolicyagent.org/docs/latest/policy-language/) for the
OPA documentation on the Rego policy language.

Here are two examples of Rego policies used in Chronicle's development:

The `common_rules` package defines two authorization rules: `allowed_users` and
`allow_defines`. The former allows users who belong to the `allowed` set to access
resources or perform actions, while the latter allows users to execute Chronicle
`define` Mutation and Submission operations.

```rego
package common_rules

import future.keywords.in
import input

allowed := {"chronicle", "anonymous"}

allowed_users {
  input.type in allowed
}

allow_defines {
  data.context.operation in ["Mutation", "Submission"]
  startswith(data.context.state[0], "define")
}
```

The `allow_transactions` package uses the rules specified in `common_rules` (above)
by default.

```rego
package allow_transactions

import data.common_rules
import future.keywords.in
import input

default allowed_users = false
allowed_users {
  common_rules.allowed_users
}

default allow_defines = false
allow_defines {
  common_rules.allow_defines
}

default deny_all = false
```

The [discussion of authorization
tokens](./auth.md#constructing-chronicle-identity-from-bearer-token-claims)
mentions OAuth scope which is an important means by which an authorization
server can assert that the authenticated user is authorized to perform
specific actions within Chronicle. If, for example, in the above rules, we
wish to allow defines to only those users whose role grants them the
`write:instance` scope, we may note the granted scopes in a variable:

```rego
oauth_scopes := split(input.claims.scope, " ")
```

then, among the rules for `allow_defines`, include,

```rego
"write:instance" in oauth_scopes
```

Additionally, for this scenario, the allowed `input.type` values should
include `"jwt"`, corresponding to users whose identity and access rights are
demonstrated by their presentation of a token issued by an authorization
server. Users' scopes are typically defined in that server's settings for
role-based access control.

## `opa-tp`

The opa-tp command-line interface (CLI) is used to interact with the Chronicle OPA-TP
(Transaction Processor) for Public Key Infrastructure (PKI) and OPA rule storage.

The available options are:

- `--connect` (`-C`): Sets the sawtooth validator address. This option takes a URL
  as its argument.

- `--completions`: Generate shell completions for the opa-tp command. This option
  takes a shell type as its argument (either bash, zsh, or fish).

- `--instrument`: Instrument the opa-tp process using the `RUST_LOG` environment
  variable. This option takes a URL as its argument.

- `--console-logging`: Enable console logging for `opa-tp` using the `RUST_LOG`
  environment variable. This option takes a string argument, which can be set to
  `pretty` or `json` to enable pretty-printed or JSON-formatted logging output, respectively.

`OpaTransactionHandler` holds the configuration for the handler, including the family
name, family versions, and namespaces. Its `apply` method works as follows.

The `verify_signed_operation` function takes a `submission` and optional `root_keys`
argument and verifies that the `submission` is valid. If the payload is a bootstrap
root operation, the function returns `Ok`. If the payload is a signed operation,
the function checks that there are root keys available and that the signature
matches the public key associated with the operation. If the signature is valid and
the verifying key matches the current key in the root keys, the function returns
`Ok`. Otherwise, the function returns an error.

The `apply_signed_operation` function takes a `payload`, `request`, and `context`
and applies the signed operation or bootstrap root operation. If the payload is a
bootstrap root operation, the function checks that the OPA TP has not already been
bootstrapped and sets the root key. If the payload is a signed operation, the
function calls `apply_signed_operation_payload` with the operation payload.

The `apply_signed_operation_payload` function takes a `request`, `payload`, and `context`
and applies the operation payload. If the payload is a register key operation, the
function checks that the key ID is not "root", the key is not already registered,
and sets the key. If the payload is not a register key operation, the function
returns an error.

## `opactl`

`opactl` allows users to submit transactions to the Sawtooth network that specify
operations on OPA (Open Policy Agent) policies stored on the ledger. The tool provides
various commands for querying, listing, and modifying OPA policy states on the Sawtooth
network.

`opactl` provides a command-line interface for managing keys and transactions in
the OPA Transaction Processor. It has the following commands:

### `bootstrap`

The `bootstrap` command initializes the OPA Transaction Processor with a root key.

`bootstrap`'s arguments include:

- `--root-key` (`-r`): A required argument that specifies the path to the PEM-encoded
  private key to be used as the root key.

- `--transactor-key` (`-t`): An optional argument that specifies the path to the
  PEM-encoded private key to be used for signing a transaction. If not specified,
  an ephemeral key will be generated.

#### `bootstrap` Example

```bash
opactl bootstrap -r path/to/root/key.pem
```

### `generate`

The `generate` command generates a new private key and writes it to stdout.

`generate`'s arguments include:

- `--output` (`-o`): An optional argument that specifies the path to write the
  private key. If not specified, the key is written to stdout.

#### `generate` Example

```bash
opactl generate --output path/to/new/key.pem
```

### `rotate-root`

The `rotate-root` command rotates the root key for the OPA Transaction Processor.

- `--current-root-key` (`-c`): A required argument that specifies the path to
  the PEM-encoded private key currently registered as the root key.

- `--new-root-key` (`-n`): A required argument that specifies the path to the
  PEM-encoded private key to be registered as the new root key.

- `--transactor-key` (`-t`): An optional argument that specifies the path to the
  PEM-encoded private key to be used for signing the transaction. If not
  specified, an ephemeral key will be generated.

#### `rotate-root` Example

```bash
opactl rotate-root -c path/to/current/root/key.pem -n path/to/new/root/key.pem
```

### `register-key`

The `register-key` command registers a new non-root key with the OPA transaction
processor.

`register-key`'s arguments include:

- `--new-key` (`-k`): A required argument that specifies the path to the
  PEM-encoded private key to be registered.

- `--root-key` (`-r`): A required argument that specifies the path to the
  PEM-encoded private key currently registered as the root key.

- `--id` (`-i`): A required argument that specifies the name to associate with
  the new key.

- `--transactor-key` (`-t`): An optional argument that specifies the path to
  the PEM-encoded private key to be used for signing the transaction. If not
  specified, an ephemeral key will be generated.

- `--overwrite` (`-o`): An optional flag allowing re-registration of a
  non-root key.

#### `register-key` Example

```bash
opactl register-key -k path/to/new/key.pem -r path/to/root/key.pem -i my_key_name
```

### `rotate-key`

The `rotate-key` command rotates the key with the specified ID for the OPA
Transaction Processor.

`rotate-key`'s arguments include:

- `--current-key` (`-c`): A required argument that specifies the path to the
  PEM-encoded private key currently registered for the given ID.

- `--root-key` (`-r`): A required argument that specifies the path to the
  PEM-encoded private key currently registered as the root key.

- `--new-key` (`-n`): A required argument that specifies the path to the
  PEM-encoded private key to be registered for the given ID.

- `--id` (`-i`): A required argument that specifies the ID of the key to be
  rotated.

- `--transactor-key` (`-t`): An optional argument that specifies the path to
  the PEM-encoded private key to be used for signing the transaction. If not
  specified, an ephemeral key will be generated.

#### `rotate-key` Example

```bash
opactl rotate-key -c path/to/current/key
```

### `set-policy`

Sets a policy with a given ID, using a specified policy compiled to `.bundle.tar.gz`
and requiring access to the root private key. The command takes the following
arguments:

- `--id` (`-i`): A required argument that specifies the ID of the new policy.

- `--policy` (`-p`): A required argument that specifies the path of the policy.

- `--root-key` (`-k`): A required argument that specifies the path of a
  PEM-encoded private key that has access to the root.

- `--transactor-key` (`-t`): An optional argument that specifies the path of
  a PEM-encoded private key for the transactor.

#### `set-policy` Example

```bash
opactl set-policy -i my_policy \
                 -p /path/to/policy.bundle.tar.gz \
                 -k /path/to/private/key
```

### `get-key`

Gets the currently registered public key, with an option to specify the key ID
and an option to write the key to a file. The command takes the following
arguments:

- `--id` (`-i`): An optional argument that specifies the ID of the key. If not
  specified, the root key is returned.

- `--output` (`-o`): An optional argument that specifies the path to write the
  key to. If not specified, the key is written to stdout.

#### `get-key` Example

```bash
opactl get-key -i my_key -o /path/to/output
```

### `get-policy`

Gets the currently registered policy, with an option to specify the policy ID
and an option to write the policy to a file. The command takes the following
arguments:

- `--id` (`-i`): A required argument that specifies the ID of the policy. If
  not specified, the default policy is returned.

- `--output` (`-o`): An optional argument that specifies the path where the
  policy will be written.

#### `get-policy` Example

```bash
opactl get-policy -i my_policy -o /path/to/output
```

## Configuring Chronicle to use OPA

By default, an embedded policy is used that allows all graphql operations and
transactions. To use a custom policy you must:

### Bundle your policy to target WASM

Build your custom Rego files into a bundle. For example, using the `opa`
command-line tool that can be downloaded from the
[Open Policy Agent](https://www.openpolicyagent.org/) project:

```bash
opa build -t wasm -o policies/bundle.tar.gz -b policies -e allow_transactions
```

Listed entry points (after `-e`) must be defined in your custom policy.

### Load a policy bundle using opactl

The policy id here must match the rego.

```bash
opactl set-policy -i allow_transactions -p /path/to/bundle.tar.gz
```

### Configure Sawtooth with settings that match the policy

2 settings entries are required, you should use
[sawset](https://sawtooth.hyperledger.org/docs/1.2/cli/sawset.html) with the
following 2 settings keys, using the policy name and entrypoint you have defined
 and previously uploaded to the `opa-tp`.

For the example rego we are using, these entries will be:

```text
chronicle.opa.policy_name=allow_transactions
chronicle.opa.entrypoint=allow_transactions.allowed_users
```

Once this has been set, you should restart Chronicle for them to be applied, the
transaction processor should not need to be restarted.

### Load OPA Policy Bundle from a URL or File Path

To configure Chronicle to use an OPA policy bundle loaded from a URL or a file
path, follow these steps:

Start Chronicle with the `--opa-bundle-address` option, providing the URL or file
path as the argument value.

For example:

```text
--opa-bundle-address https://example.com/policy-bundle.tar.gz
```

or

```text
--opa-bundle-address /path/to/policy-bundle.tar.gz
```

Make sure to replace `https://example.com/policy-bundle.tar.gz` with the actual
URL or `/path/to/policy-bundle.tar.gz` with the actual file path of the OPA policy
bundle you want to load.

Note that when using `--opa-bundle-address` option, the `--opa-policy-name` and
`--opa-policy-entrypoint` options must be provided.

For example:

```text
--opa-policy-name my_policy --opa-policy-entrypoint entrypoint1
```
