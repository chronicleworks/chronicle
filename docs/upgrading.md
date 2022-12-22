# Upgrading

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
`chronicle:` vs `http://blockchaintp.com/chronicle#ns`, operations will continue
to accept the long form for backwards compatibility.
