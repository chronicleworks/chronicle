# Upgrading

## To 0.3x

The 0.3 release font loads a number of breaking changes to address usability and
consistency:

### New completion mechanism

### Contradiction notification

Before 0.3, contradictions would result in a no-operation, but with no
notification to clients. 0.3 contains various protocol changes to notify
Chronicle and connected clients of contradictions.

See: [subscriptions](recording_provenance.md#commit-notification-subscriptions)

### Submission notification

Before 0.3, submission was not independently notified from commit, and had no
ability to carry an error result. There is now
a 2 stage notification for chronicle operations.

See: [subscriptions](recording_provenance.md#commit-notification-subscriptions)

### External ID

`name` is now `externalId` and will no longer be disambiguated by Chronicle when
creating entities, activities, or agents. Previously, re-using a name in these operations
would result in it being postfixed with an index. We discovered that stable
external identifiers are far more useful in practice and will enable batching
operations and convenience methods for revision in future releases.

### Short form IRIs

Chronicle ids will now be written in their short form with a prefix of
'chronicle:' vs 'http//blockchaintp.com/chronicle#ns', operations will continue
to accept the long form for backwards compatibility.

### JSON attribute

In addition to `String`, `Int`, and `Bool`, Chronicle users can now
[add generic JSON data](domain_modelling.md#inputting-a-json-attribute)
to activities, agents, and entities.
