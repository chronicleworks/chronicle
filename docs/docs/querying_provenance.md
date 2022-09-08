# Querying Provenance

Currently Chronicle has 4 root queries.

``` graphql
type Query {
    activityTimeline(activityTypes: [ActivityType!]!, forEntity: [EntityID!]!, from: DateTime, to: DateTime, namespace: ID, after: String, before: String, first: Int, last: Int): ActivityConnection!
    agentsByType(agentType: AgentType!, namespace: String, after: String, before: String, first: Int, last: Int): AgentConnection!
    agentById(id: AgentID!, namespace: String): Agent
    entityById(id: EntityID!, namespace: String): Entity
}
```

The majority of the work for provenance retrieval will be with the [activity timeline](#activity-timeline) query.

Familiarizing yourself with GraphQL is necessary to make good use of Chronicle. Chronicle makes extensive use of [relay cursors](https://relay.dev/graphql/connections.htm) and [union types](https://www.apollographql.com/docs/apollo-server/schema/unions-interfaces/).


## Activity timeline

### Parameters

#### activityTypes

A list of ActivityTypes to filter the returned timeline by, leaving this empty will return all activity types. The `PROV_ACTIVITY` activity type can be used to return activities that are not currently specified in the Chronicle domain.

``` graphql
enum ActivityType {
  PROV_ACTIVITY
  PUBLISHED
  QUESTION_ASKED
  RESEARCHED
  REVISED
}

```
#### forEntity

A list of EntityIDs to filter activities by - leaving this empty will return all activity types.

#### from

The time in RFC3889 format to return activities from. Not specifying this will return all activity types before the time specified in[to](#to).

#### to

The time in RFC3889 format to return activities until. Nor specifying this will return all activity types after the time specified in[from](#from).


#### after

Relay cursor control, returning a page after the cursor you supply to this argument - for forwards pagination.

#### before

Relay cursor control, returning items before the cursor you supply to this argument - for reverse pagination.

#### first

An integer controlling page size for forward pagination. Defaults to 20

#### last

An integer controlling page size for reverse pagination. Defaults to 20


## agentsByType

## agentById

## entityById


## Returned objects

### Entity subtypes

All Chronicle Entity subtypes follow a similar pattern, we will use the Guidance entity from our example domain as a sample.

``` graphql
type Guidance {
  id: EntityID!
  namespace: Namespace!
  name: String!
  type: DomaintypeID
  evidence: ChronicleEvidence
  wasGeneratedBy: [Activity!]!
  wasDerivedFrom: [Entity!]!
  hadPrimarySource: [Entity!]!
  wasRevisionOf: [Entity!]!
  wasQuotedFrom: [Entity!]!
  titleAttribute: TitleAttribute
  versionAttribute: VersionAttribute
}

```

#### id

The EntityID of the entity. This is derived from name, but clients should not attempt to synthesize it themselves.

#### namespace

The Namespace of the entity, only of interest for Chronicle domains that span multiple namespaces.

### name

The name of the entity, determined when defined

### type

A DomainTypeID derived from the Entity subtype, the built in graphql field `__TypeName` should be used for union queries

### evidence

See [chronicle evidence](#chronicle-evidence)

### wasGeneratedBy

A list of the Activities that generated this entity. See [generation](./provenance_concepts.md#generation).

### wasRevisionOf

A list of the Entities that this entity is a revision of. See [revision](./provenance_concepts.md#revision). This currently only returns the immediate entity that the current entity is derived from and will require recursive enumeration to retrieve a deep hierarchy.

### wasQuotedFrom

A list of the Entities that this entity was quoted from. See [quotation](./provenance_concepts.md#quotation). This currently only returns the immediate entity that the current entity is derived from and will require recursive enumeration to retrieve a deep hierarchy.

### wasDerivedFrom

A list of the Entities that this entity is derived from. See [derivation](./provenance_concepts.md#derivation). This currently only returns the immediate entity that the current entity is derived from and will require recursive enumeration to retrieve a deep hierarchy.

### Attributes

Attribute values for the attributes associated with the entity subtype, as determined by the [domain model](./domain_modelling.md)


### Activity subtypes


``` graphql
type Published {
  id: ActivityID!
  namespace: Namespace!
  name: String!
  started: DateTime
  ended: DateTime
  type: DomaintypeID
  wasAssociatedWith: [Association!]!
  used: [Entity!]!
  versionAttribute: VersionAttribute
}
```

#### id

The EntityID of the entity. This is derived from name, but clients should not attempt to synthesize it themselves.

#### namespace

The Namespace of the entity, only of interest for Chronicle domains that span multiple namespaces.

### name

The name of the entity, determined when defined

### type

A DomainTypeID derived from the Entity subtype, the built in graphql field `__TypeName` should be used for union queries
