# Access Control Policy with OPA

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
