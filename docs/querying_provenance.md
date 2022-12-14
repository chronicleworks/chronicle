# Querying Provenance

Currently Chronicle has 7 root queries.

```graphql
type Query {
  activityTimeline(
    activityTypes: [ActivityType!]!
    forEntity: [EntityIdOrExternal!]!
    forAgent: [AgentIdOrExternal!]!
    from: DateTime
    to: DateTime
    order: TimelineOrder
    namespace: ID
    after: String
    before: String
    first: Int
    last: Int
  ): ActivityConnection!
  agentsByType(
    agentType: AgentType!
    namespace: ID
    after: String
    before: String
    first: Int
    last: Int
  ): AgentConnection!
  activitiesByType(
    activityType: ActivityType!
    namespace: ID
    after: String
    before: String
    first: Int
    last: Int
  ): ActivityConnection!
  entitiesByType(
    entityType: EntityType!
    namespace: ID
    after: String
    before: String
    first: Int
    last: Int
  ): EntityConnection!
  agentById(id: AgentIdOrExternal!, namespace: String): Agent
  activityById(id: ActivityIdOrExternal!, namespace: String): Activity
  entityById(id: EntityIdOrExternal!, namespace: String): Entity
}
```

The majority of the work for provenance retrieval will be with the [activity
timeline](#activity-timeline) query.

Familiarizing yourself with GraphQL is necessary to make good use of Chronicle.
Chronicle makes extensive use of
[relay cursors](https://relay.dev/graphql/connections.htm) and [union types](https://www.apollographql.com/docs/apollo-server/schema/unions-interfaces/).

## Activity Timeline

### Parameters

#### activityTypes

A list of ActivityTypes to filter the returned timeline by, leaving this empty
will return all activity types. The `ProvActivity` activity type can be used to
return activities that are not currently specified in the Chronicle domain.

```graphql
enum ActivityType {
  ProvActivity
  PublishedActivity
  QuestionAskedActivity
  ResearchedActivity
  RevisedActivity
}

```

#### EntityIdOrExternal

A list of EntityIDs or externalIds to filter activities by - leaving this empty
will return all activity types.

#### from

The time in RFC3339 format to return activities from. Not specifying this will
return all activity types before the time specified in [to](#to).

#### to

The time in RFC3339 format to return activities until. Nor specifying this will
return all activity types after the time specified in [from](#from).

#### after

Relay cursor control, returning a page after the cursor you supply to this
argument - for forwards pagination.

#### before

Relay cursor control, returning items before the cursor you supply to this
argument - for reverse pagination.

#### first

An integer controlling page size for forward pagination. Defaults to 20

#### last

An integer controlling page size for reverse pagination. Defaults to 20

## activitiesByType

An activity could be defined like so:

```graphql
  mutation {
    defineItemCertifiedActivity(externalId:"certification1", attributes: { certIdAttribute: "123" }) {
        context
    }
  }
```

A user could query all activities of that type as in the following example:

```graphql
  query {
    activitiesByType(activityType: ItemCertifiedActivity) {
      nodes {
        ...on ItemCertifiedActivity {
          id
          certIdAttribute
        }
      }
    }
  }
```

## agentsByType

An agent could be defined like so:

```graphql
  mutation {
    defineContractorAgent(externalId:"contractor1", attributes: { locationAttribute: "Shenzhen" }) {
        context
    }
  }
```

A user could query all agents of that type as in the following example:

```graphql
  query {
    agentsByType(agentType: ContractorAgent) {
      nodes {
        ...on ContractorAgent {
          id
          location
        }
      }
    }
  }
```

## entitiesByType

An entity could be defined like so:

```graphql
mutation {
    defineCertificateEntity(externalId:"testentity1", attributes: { certIdAttribute: "something" }) {
        context
    }
  }
```

A user could query all entities of that type as in the following example:

```graphql
query {
    entitiesByType(entityType: CertificateEntity) {
      nodes {
        ...on CertificateEntity {
          id
        }
      }
    }
}
```

## activityById

An activity could be defined like so:

```graphql
defineItemCertifiedActivity(externalId: "externalid", attributes: { certIdAttribute: "234" }
  ) {
    context
  }
```

A user could query that activity in the following way:

```graphql
activityById(id: {externalId: "externalid" }) {
    ... on ItemCertifiedActivity {
      id
      externalId
      certIdAttribute
    }
  }
```

## agentById

An agent could be defined like so:

```graphql
defineContractorAgent(externalId: "externalid", attributes: { locationAttribute: "location" }
  ) {
    context
  }
```

A user could query that agent in the following way:

```graphql
agentById(id: {externalId: "externalid" }) {
    ... on ContractorAgent {
      id
      externalId
      locationAttribute
    }
  }
```

## entityById

An entity could be defined like so:

```graphql
defineItemEntity(externalId: "externalid", attributes: { partIdAttribute: "432" }
  ) {
    context
  }
```

A user could query that entity in the following way:

```graphql
entityById(id: {externalId: "externalid" }) {
    ... on ItemEntity {
      id
      externalId
      partIdAttribute
    }
  }
```

## Returned Objects

### Entity Subtypes

All Chronicle Entity subtypes follow a similar pattern, we will use the Guidance
entity from our example domain as a sample.

```graphql
type GuidanceEntity {
  id: EntityID!
  namespace: Namespace!
  externalId: String!
  type: DomaintypeID
  evidence: Evidence
  wasGeneratedBy: [Activity!]!
  wasDerivedFrom: [Entity!]!
  hadPrimarySource: [Entity!]!
  wasRevisionOf: [Entity!]!
  wasQuotedFrom: [Entity!]!
  titleAttribute: TitleAttribute
  versionAttribute: VersionAttribute
}

```

#### Entity: id

The EntityID of the entity. This is derived from externalId, but clients should
not attempt to synthesize it themselves.

#### Entity: namespace

The Namespace of the entity, only of interest for Chronicle domains that span
multiple namespaces.

#### Entity: externalId

The externalId of the entity, determined when defined.

#### Entity: type

A DomainTypeID derived from the Entity subtype. The built-in GraphQL field
`__TypeName` should be used for union queries.

#### Entity: evidence

See [chronicle evidence](#chronicle-evidence)

#### Entity: wasGeneratedBy

A list of the Activities that generated this entity. See
[generation](./provenance_concepts.md#generation).

#### Entity: wasRevisionOf

A list of the Entities that this entity is a revision of. See
[revision](./provenance_concepts.md#revision). This currently only returns the
immediate entity that the current entity is derived from and will require
recursive enumeration to retrieve a deep hierarchy.

#### Entity: wasQuotedFrom

A list of the Entities that this entity was quoted from. See
[quotation](./provenance_concepts.md#quotation). This currently only returns the
immediate entity that the current entity is derived from and will require
recursive enumeration to retrieve a deep hierarchy.

#### Entity: wasDerivedFrom

A list of the Entities that this entity is derived from. See
[derivation](./provenance_concepts.md#derivation). This currently only returns
the immediate entity that the current entity is derived from and will require
recursive enumeration to retrieve a deep hierarchy.

### Attributes

Attribute values for the attributes associated with the entity subtype, as
determined by the [domain model](./domain_modelling.md).

### Activity Subtypes

```graphql
type PublishedActivity {
  id: ActivityID!
  namespace: Namespace!
  externalId: String!
  started: DateTime
  ended: DateTime
  type: DomaintypeID
  wasAssociatedWith: [Association!]!
  used: [Entity!]!
  wasInformedBy: [Activity!]!
  generated: [Entity!]!
  versionAttribute: VersionAttribute
}
```

#### Activity: id

The ActivityID of the activity. This is derived from externalId, but clients
should not attempt to synthesize it themselves.

#### Activity: namespace

The Namespace of the activity, only of interest for Chronicle domains that span
multiple namespaces.

#### Activity: externalId

The externalId of the activity, determined when defined.

#### Activity: type

A DomainTypeID derived from the Activity subtype. The built-in GraphQL field
`__TypeName` should be used for union queries
