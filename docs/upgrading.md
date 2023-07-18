# Upgrading

## To 0.7x

### Accept and Verify JSON Web Tokens

We made changes to API authentication. Access to the API is now authenticated, and
users can provide a URI to the keyset used to sign the JWT bearer tokens. Users can
also provide a JSON pointer into the JWT claims for the Chronicle identity. A Chronicle
`AuthId` with a verified external ID can then be found in the GraphQL context.

We also converted the `JWT` `AuthId` variant to be a structure of both a json-pointer
-derived scalar and JWT claims.

### Anonymous Identity Variant

By default, Chronicle allows unauthenticated access to the GraphQL API, classifying
such a user as "anonymous". Adds an identity variant emanating from GraphQL or CLI,
not authorized, allowing for the option to pass a transactor key as a CLI argument,
or, if not, to generate an ephemeral transaction signing key for the anonymous agent.

Chronicle's GraphQL API will accept anonymous access as default unless disallowed
with the new `--require-auth` option. Only with that option must a JWKS server be
specified for JWT verification.

### OPA Execution

We introduced new interfaces for OPA policy execution and loading. The `OpaExecutor`
and `PolicyLoader` interfaces provide a flexible and straightforward way to evaluate
OPA policies.

The `OpaExecutor` interface takes a Chronicle `AuthId` identity and an arbitrary
JSON context, which it evaluates against an OPA policy. The `WasmtimeOpaExecutor`
evaluates OPA policies compiled to .wasm for execution.

The `PolicyLoader` interface offers two implementations. The `SawtoothPolicyLoader`
implementation loads an OPA policy rule that is compiled to .wasm from a Sawtooth
address. The `CliPolicyLoader` implementation provides support for loading an OPA
policy .wasm file via CLI arguments for a file path and policy entrypoint.

Chronicle integrates OPA execution at its GraphQL API endpoint and in its Transaction
Processor to ensure access control and policy compliance.

Chronicle's `opa-tp` command-line interface (CLI) is used to interact with the Chronicle
OPA-TP (Transaction Processor) for Public Key Infrastructure (PKI) and OPA rule storage.

Chronicle now has `opactl`, which enables users to submit transactions to the Sawtooth
network, specifying operations on OPA (Open Policy Agent) policies stored on the
ledger. With this tool, users can inspect, modify, and query OPA policy states on
the network, provided they have the policy ID.

## No Changes, No Dispatch

We added API methods and responses to check whether Chronicle operations resulting
from API calls will result in changes in state. If they don't, they won't be submitted
to the Transaction Processor, instead returning `Submission::AlreadyRecorded` along
with GraphQL data containing `context` and a `null` `tx_id`.

## In-Domain Documentation Capabilities

Chronicle now provides users with the ability to generate a Chronicle domain with
documentation by using a domain.yaml file and adding documentation comments to it.

## `async-stl-client`

Chronicle now uses `async-stl-client` in this update, which contains generic client
types used by `opa-tp-protocol`. The transition to this new SDK has led to several
improvements.

The `LedgerReader` and `LedgerWriter` client abstractions are now in common, with
specialization for transactions and events in the `chronicle-protocol` and `opa-tp-protocol`
modules. The simulated transaction processors now use the actual Chronicle and OPA
TransactionHandlers during tests and in `devmode`.

## `prov:wasAttributedTo`

Chronicle now supports the `wasAttributedTo` PROV-O relationship.

## Endpoint offering JSON by IRI

Now, running Chronicle with the `serve-api` command, users can visit
<http://localhost:9982/data/chronicle:agent:my-agent-id> and see a JSON summary,
which they can also do for entity and activity.

## To 0.6x

### OPA TP, Protocol, and Skeletal CLI

We introduced an OPA policy and key registration transaction processor in this update.
The OPA policy and key registration transaction processor share similarities in design
with the Chronicle TP. Tested at the protobuf level, this feature is designed to
operate independently of the rest of the Chronicle project.

## To 0.5x

### Move from SQLite to PostgreSQL

Chronicle now utilizes PostgreSQL instead of SQLite as its local database.
This change brings several benefits, such as increased scalability and better
performance for large-scale data management.

### `AuthId` Identity Type

We introduced an identity union in this update, which offers a versatile and flexible
approach to identity management. This new feature can be created from any of the
following options:

- Chronicle root key, identified by the agent name "chronicle"
- an agent along with its corresponding key pair
- a pointer into a JWT structure, for retrieving the external id to use for the agent
  and its key pair

The identity union includes the identity, public key, and signature, providing a
comprehensive solution for managing identities in the Chronicle application.

## To 0.4x

### Chronicle Domain Linter

Chronicle now includes a domain linter from which Chronicle users can get helpful
comments on fixing any formatting issues in their domain.yaml domain files.

### Input Either ExternalId or Id in Relational Mutations and *ById Queries

Where relational mutations and queries previously required `ActivityId`, `AgentId`,
and/or `EntityId` inputs, Chronicle now makes available the `ActivityIdOrExternal`,
`AgentIdOrExternal`, and `EntityIdOrExternal` input types. In such cases, Chronicle
users can now use either simply the `ExternalId` or the full `Id`.
See [Recording Provenance](./recording_provenance.md) for examples.

### Query Agents, Entities, and Activities by Type

Chronicle users can now query [agents](./querying_provenance.md#agentsbytype),
[entities](./querying_provenance.md#entitiesbytype), and
[activities](./querying_provenance.md#activitiesbytype) by type.

### JSON Attribute

In addition to `String`, `Int`, and `Bool`, Chronicle users can now
[add generic JSON data](./domain_modeling.md#inputting-a-json-attribute)
to activities, agents, and entities.

## To 0.3x

The 0.3 release font loads a number of breaking changes to address usability and
consistency:

### New Completion Mechanism

### Contradiction Notification

Before 0.3, contradictions would result in a no-operation, but with no
notification to clients. 0.3 contains various protocol changes to notify
Chronicle and connected clients of contradictions.

See: [subscriptions](./recording_provenance.md#commit-notification-subscriptions).

### Submission Notification

Before 0.3, submission was not independently notified from commit, and had no
ability to carry an error result. There is now
a 2 stage notification for chronicle operations.

See: [subscriptions](./recording_provenance.md#commit-notification-subscriptions).

### External ID

`name` is now `externalId` and will no longer be disambiguated by Chronicle when
creating entities, activities, or agents. Previously, re-using a name in these operations
would result in it being postfixed with an index. We discovered that stable
external identifiers are far more useful in practice and will enable batching
operations and convenience methods for revision in future releases.

### Short Form IRIs

Chronicle ids will now be written in their short form with a prefix of
`chronicle:` vs `http://btp.works/chronicle#ns`, operations will continue
to accept the long form for backwards compatibility.
