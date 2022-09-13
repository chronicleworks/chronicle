# Recording Provenance using Chronicle

## Immutability

Chronicle provenance is immutable. Once recorded you cannot contradict it - only
record additional provenance.

## Open-world model

Chronicle does not use relational constraints in the way you may expect a
relational database to, or allow free-form data in the manner of a document
store. It is useful to think of Chronicle as recording facts, and it will make
simple inferences to keep your provenance data consistent.

For example:

```graphql title="'homer' in the activity 'writing-the-iliad' as a writer"
mutation {
    wasAssociatedWith(
        activity: "chronicle:activity:writing-the-iliad",
        agent: "chronicle:agent:homer",
        role: writer
    )
}
```

```graphql title="An activity 'writing-the-iliad' that took place in Ionia"
mutation {
    writing(
        name: "writing-the-iliad",
        attributes: {
            location: "ionia"
        }
    )
}
```

```graphql title="An agent 'homer' exists and was a person"
mutation {
    person(
        name: "homer",
        attributes: {
            email: "homer@ionia.gr"
        }
    )
}
```

These three Chronicle mutations can be executed in *any* order - Chronicle will
assume the existence of the Agent and Activities referred to even if they have
not yet been recorded by their `person` and `writing` mutation.

## Contradiction

Chronicle will not allow you to record provenance that contradicts previously
recorded provenance. For example, you cannot change the start / end dates of
activities once set, or alter the value of an attribute. You can however record
identical provenance any number of times, as this does not cause contradiction,
or change the state of Chronicle meaningfully.

### Contradiction of attribute time

Here we attempt to change the end date of an activity after it has been
recorded - one of these mutations will fail.

```graphql
    mutation {
        endActivity(id:"chronicle:activity:birthday",
            time: "2022-07-29T12:41:52.433Z"
        )

        endActivity(id:"chronicle:activity:birthday",
            time: "2020-07-29T12:41:52.433Z"
        )
    }
```

### Contradiction of attribute value

Here we attempt to change the value of an attribute - one of these operations
will fail.

```graphql
    mutation {
        writer(id: "chronicle:agent:poet", attributes: {
            name: "Tennyson"
        })
    }

    mutation {
        writer(id: "chronicle:agent:poet", attributes: {
            name: "Kipling"
        })
    }
```

### Extending attributes is not a contradiction

Where a provenance term has been recorded with attributes, and the domain has
been extended to include new attributes, it is valid to append new attributes,
as long as the values of the already recorded ones are unchanged.

```graphql

    mutation {
        writer(id: "chronicle:agent:poet", attributes: {
            name: "Tennyson"
        })
    }

    mutation {
        writer(id: "chronicle:agent:poet", attributes: {
            name: "Tennyson",
            heightInCentimeters: "185"
        })
    }
```

## Operations

### Example domain

For this section we will use our simplified [domain model for recording the
provenance of medical guidance](domain_modelling). We have a number of
people who may be any combination of authors, editors or researchers
collaborating to produce documents from evidence produced by a search process.
An external CMS is being used that has identifiers for documents and users.

### The `name` parameter

The definition mutations for prov terms -  Entity, Activity and Agent - are all
supplied with a `name` parameter. This should be something meaningful to the
domain you are recording provenance for - a unique identifier from an external
system or a natural key. This will form part of the identity of the term.

### A note on identities

Chronicle identities will contain an encoded form of the name parameter, but
should be treated as opaque. Under no circumstances should you attempt to
synthesize an identity from Chronicle in client code as the scheme may be
subject to change.

### GraphQL mutation result - `Submission`

All Chronicle mutations return a `Submission` type defined by:

```graphql
type Submission {
  context: String!
  correlationId: String!
}
```

The `context` property here will be the identity of the Chronicle term you have
changed in the mutation, i.e calling `agent(..)` will return the identity of the
agent, calling `startActivity(..)` the identity of the started activity.

The `correlationId` property corresponds to the transaction id when running
Chronicle with a backend ledger, or a randomly generated uuid when used in
[in-memory](chronicle_architecture#development) mode.

### Commit notification subscriptions

Chronicle provides a [GraphQL
subscription](https://graphql.org/blog/subscriptions-in-graphql-and-relay/) to
notify clients when a Chronicle operation has completed.

```graphql

type Submission {
  context: String!
  correlationId: String!
}

subscription {
    commitNotifications {
        correlationId
    }
}

```

The `correlationId` on this subscription will match the correlationId from the
[Submission](#graphql-mutation-result---submission). Clients that wish to know
the result of an operation should await both the
[Submission](#graphql-mutation-result---submission) and the corresponding
correlation id from a commit notification.

### Define an Entity

> In PROV, things we want to describe the provenance of are called entities and
> have some fixed aspects. The term "things" encompasses a broad diversity of
> notions, including digital objects such as a file or web page, physical things
> such as a mountain, a building, a printed book, or a car as well as abstract
> concepts and ideas. An entity is a physical, digital, conceptual, or other
> kind of thing with some fixed aspects; entities may be real or imaginary.

See [provenance concepts](provenance_concepts#entity)

Using our example domain, Chronicle will have generated four entity subtypes for
us, `Question`, `Guidance`, `PublishedGuidance` and `Evidence`, as a GraphQL
union called `Entity`. The union also contains an untyped entity `ProvEntity`.
The untyped entity can be potentially returned where the domain definition has
evolved, see [evolving your domain](domain_modelling#evolution).

The definition mutations `question`, `guidance`, `publishedGuidance`, and
`evidence` will also have been created to allow you to define an instance of
each subtype and their attributes. The generated GraphQL mutations and their
associated types will look like this:

```graphql

scalar EntityID
scalar DomaintypeID
type Evidence {
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
  searchParametersAttribute: SearchParameterAttribute
  referenceAttribute: ReferenceAttribute
}
input EvidenceAttributes {
  searchParametersAttribute: String!
  referenceAttribute: String!
}
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
input GuidanceAttributes {
  titleAttribute: String!
  versionAttribute: Int!
}

input PublishedAttributes {
  versionAttribute: Int!
}
type PublishedGuidance {
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
}

type Question {
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
  cmsIdAttribute: CmsIdAttribute
  contentAttribute: ContentAttribute
}

input QuestionAttributes {
  cmsIdAttribute: String!
  contentAttribute: String!
}

type ProvEntity {
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
}

input ProvEntityAttributes {
  type: String
}

union Entity = | ProvEntity | Evidence | Guidance | PublishedGuidance | Question

mutation {
  entity(name: String!, namespace: String, attributes: ProvEntityAttributes!): Submission!
  evidence(name: String!, namespace: String, attributes: EvidenceAttributes!): Submission!
  guidance(name: String!, namespace: String, attributes: GuidanceAttributes!): Submission!
  publishedGuidance(name: String!, namespace: String): Submission!
  question(name: String!, namespace: String, attributes: QuestionAttributes!): Submission!
}

```

Executing the following example mutation `question` will define an `Entity` of
subtype `Question`, along with its attributes.

```graphql
mutation {
    question(name: "anaphylaxis-referral", attributes: {
        cmsIdAttribute: "0c6fa8c5-69da-43d1-95d6-726f5f671b30",
        contentAttribute: "How to assess and refer patients needing emergency treatment for Anaphylaxis",
    })
}
```

And the equivalent operation using the command line interface is:

```bash
chronicle question define anaphylaxis-referral --cms-id-attribute \
  "How to assess and refer patients needing emergency treatment for Anaphylaxis" --cms-id-attr "0c6fa8c5-69da-43d1-95d6-726f5f671b30"

```

Either operation will return the ID of the newly defined question.

### Define an Activity

> An activity is something that occurs over a period of time and acts upon or
> with entities; it may include consuming, processing, transforming, modifying,
> relocating, using, or generating entities. Just as entities cover a broad
> range of notions, activities can cover a broad range of notions: information
> processing activities may for example move, copy, or duplicate digital
> entities; physical activities can include driving a car between two locations
> or printing a book.

See [provenance concepts](provenance_concepts#activity)

Chronicle will have generated four `Activity` subtypes for us, `QuestionAsked`,
`Researched`, `Revised` and `Published`, as a GraphQL union called `Activity`.
The union also contains an untyped activity `ProvActivity`. The untyped activity
can be potentially returned where the domain definition has evolved, see
[evolving your domain](domain_modelling#evolution).

The definition mutations `questionAsked`, `researched`, `revised` and
`published` will also have been created to allow you to define an instance of
each subtype and their attributes. The generated GraphQL mutations and their
associated types will look like this:

```graphql
union Activity = | ProvActivity | Published | QuestionAsked | Researched | Revised

type ProvEntity {
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
}
input ProvEntityAttributes {
  type: String
}
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
input PublishedAttributes {
  versionAttribute: Int!
}
type PublishedGuidance {
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
}

type Researched {
  id: ActivityID!
  namespace: Namespace!
  name: String!
  started: DateTime
  ended: DateTime
  type: DomaintypeID
  wasAssociatedWith: [Association!]!
  used: [Entity!]!
  searchParametersAttribute: SearchParameterAttribute
}
input ResearchedAttributes {
  searchParametersAttribute: String!
}
type Revised {
  id: ActivityID!
  namespace: Namespace!
  name: String!
  started: DateTime
  ended: DateTime
  type: DomaintypeID
  wasAssociatedWith: [Association!]!
  used: [Entity!]!
  cmsIdAttribute: CmsIdAttribute
  versionAttribute: VersionAttribute
}
input RevisedAttributes {
  cmsIdAttribute: String!
  versionAttribute: Int!
}

mutation {
  activity(name: String!, namespace: String, attributes: ProvActivityAttributes!): Submission!
  published(name: String!, namespace: String, attributes: PublishedAttributes!): Submission!
  questionAsked(name: String!, namespace: String, attributes: QuestionAskedAttributes!): Submission!
  researched(name: String!, namespace: String, attributes: ResearchedAttributes!): Submission!
  revised(name: String!, namespace: String, attributes: RevisedAttributes!): Submission!
}
```

The following example mutation `revised` will define an `Activity` of subtype
`Revised`:

```graphql title="Define a revised activity with graphql"
mutation {
    revised(name: "september-2018-review", attributes: {
        versionAttribute: 14,
    })
}
```

And the equivalent operation using the command line interface is:

```bash title="Define a document entity with the CLI"
chronicle revised define september-2018-review --version-attr 14
```

### Define an Agent

> An agent is something that bears some form of responsibility for an activity
> taking place, for the existence of an entity, or for another agent's activity.

See [provenance concepts](provenance_concepts#agent)

Chronicle will have generated two `Agent` subtypes for us, `Person` and
`Organization` as a GraphQL union called `Agent`. The union also contains an
untyped activity `ProvAgent`. The untyped agent can be potentially returned
where the domain definition has evolved, see [evolving your
domain](domain_modelling#evolution).

The definition mutations `person` and `organization` will also have been
created. See [domain modelling](domain_modelling#graphql_generation) for
details on the generated GraphQL SDL.

```graphql title="Define an organization agent with graphql"
mutation {
    organization(name: "health-trust", attributes: {})
}
```

And the equivalent operation using the command line interface is:

```bash title="Define an organization entity with the CLI"
chronicle organization define health-trust
```

### Used

> Usage is the beginning of utilizing an entity by an activity. Before usage,
> the activity had not begun to utilize this entity and could not have been
> affected by the entity.

See [provenance concepts](provenance_concepts#used)

Usage operations in Chronicle can applied to all subtypes of Entity and
Activity.

To apply using GraphQL:

```graphql
mutation {
  used(activity: "chronicle:activity:september-2018-review", entity: "chronicle:entity:anaphylaxis-evidence-12114")
}
```

And the equivalent operation using the command line interface is:

```bash
chronicle revised use "chronicle:entity:anaphylaxis-evidence-12114" "chronicle:activity:september-2018-review"
```

### Generation

> Generation is the completion of production of a new entity by an activity.
> This entity did not exist before generation and becomes available for usage
> after this generation.

See [provenance concepts](provenance_concepts#generation)

Generation operations in Chronicle can be applied to all subtypes of Entity and
Activity.

To apply using GraphQL:

```graphql
mutation {
  wasGeneratedBy(activity: "chronicle:activity:september-2018-review", entity: "chronicle:entity:anaphylaxis-guidance-9-2018")
}
```

And the equivalent operation using the command line interface is:

```bash
chronicle revised generate "chronicle:entity:anaphylaxis-guidance-9-2018" "chronicle:activity:september-2018-review"
```

### Started at time

> Start is when an activity is deemed to have been started by an entity, known
> as trigger. The activity did not exist before its start. Any usage,
> generation, or invalidation involving an activity follows the activity's
> start. A start may refer to a trigger entity that set off the activity, or to
> an activity, known as starter, that generated the trigger.

See [provenance concepts](provenance_concepts#started-at-time)

Chronicle allows you to specify the start time of an activity when you need to
model a time range. If you want an instantaneous activity, simply call [ended at
time](#ended-at-time). Eliding the time parameter will use the current system
time. Time stamps should be in
[RFC3339](https://www.rfc-editor.org/rfc/rfc3339.html) format.

Started at time operations also take an optional `AgentId`, to associate the
activity with the agent - there is no current way to record a role with this
however, so prefer [wasAssociatedWith](#association) if you need role-based
modelling.

```graphql
mutation {
  startActivity(id: "chronicle:activity:september-2018-review",time:"2002-10-02T15:00:00Z")
}
```

```bash
chronicle revision start "chronicle:activity:september-2018-review" --time "2002-10-02T15:00:00Z"

```

### Ended at time

> End is when an activity is deemed to have been ended by an entity, known as
> trigger. The activity no longer exists after its end. Any usage, generation,
> or invalidation involving an activity precedes the activity's end. An end may
> refer to a trigger entity that terminated the activity, or to an activity,
> known as ender that generated the trigger.

See [provenance concepts](provenance_concepts#ended-at-time)

Chronicle allows you to specify the end time of an activity when you need to
model a time range. If you want an instantaneous activity, simply call this
operation. Eliding the time parameter will use the current system time. Time
stamps should be in [RFC3339](https://www.rfc-editor.org/rfc/rfc3339.html)
format.

Ended at time operations also take an optional `AgentId`, to associate the
activity with the agent - there is no current way to record a role with this
however, so prefer [wasAssociatedWith](#association) if you need role-based
modelling.

```graphql
mutation {
  endActivity(id: "chronicle:activity:september-2018-review",time:"2002-10-02T15:00:00Z")
}
```

And the equivalent command line operation:

```bash
chronicle revision end "chronicle:activity:september-2018-review" --time "2002-10-02T15:00:00Z"

```

### Association

> An activity association is an assignment of responsibility to an agent for an
> activity, indicating that the agent had a role in the activity.

See [provenance concepts](provenance_concepts#association)

Association and [delegation](#delegation) accept an optional `Role`. These will
have been generated from the domain model, and using the example domain results
in:

```graphql
enum RoleType {
  UNSPECIFIED
  STAKEHOLDER
  AUTHOR
  RESEARCHER
  EDITOR
}
```

To record the asking of a question, we relate an Organization to a
`QuestionAsked` activity, using the `Role` `STAKEHOLDER`.

```graphql
mutation {
  wasAssociatedWith(
    responsible: "chronicle:agent:ncsa",
    activity: "chronicle:activity:anaphylaxis-assessment",
    role: STAKEHOLDER
  )
}

```

### Delegation

> Delegation is the assignment of authority and responsibility to an agent (by
> itself or by another agent) to carry out a specific activity as a delegate or
> representative, while the agent it acts on behalf of retains some
> responsibility for the outcome of the delegated work. For example, a student
> acted on behalf of his supervisor, who acted on behalf of the department
> chair, who acted on behalf of the university; all those agents are responsible
> in some way for the activity that took place but we do not say explicitly who
> bears responsibility and to what degree.

See [provenance concepts](provenance_concepts#delegation)

Delegation and [association](#association) accept an optional `Role`. These will
have been generated from the domain model, and using the example domain results
in:

```graphql
enum RoleType {
  UNSPECIFIED
  STAKEHOLDER
  AUTHOR
  RESEARCHER
  EDITOR
}

```

To record the responsibility of an `Editor` supervising an `Author`, we relate a
responsible `Person` to another delegate `Person` Agent, using the `Role`
`EDITOR`, and specify a particular `Revision` activity. The activity is not a
required parameter - generic delegation between agents can be recorded.

```graphql
mutation {
  actedOnBehalfOf(
    responsible: "chronicle:agent:john-roberts",
    delegate: "chronicle:agent:janet-flynn",
    activity: "chronicle:activity:september-2018-review",
    role: EDITOR
  )
}

```

### Derivation

#### Primary Source

> A primary source for a topic refers to something produced by some agent with
> direct experience and knowledge about the topic, at the time of the topic's
> study, without benefit from hindsight. Because of the directness of primary
> sources, they 'speak for themselves' in ways that cannot be captured through
> the filter of secondary sources. As such, it is important for secondary
> sources to reference those primary sources from which they were derived, so
> that their reliability can be investigated. A primary source relation is a
> particular case of derivation of secondary materials from their primary
> sources. It is recognized that the determination of primary sources can be up
> to interpretation, and should be done according to conventions accepted within
> the application's domain.

See [provenance concepts](provenance_concepts#primary-source)

Primary sources can be recorded in Chronicle using the `hadPrimarySource`
mutation, which takes two entities - the generatedEntity having a primary source
of the useEntity.

```graphql
mutation {
  hadPrimarySource {
    usedEntity: "chronicle:entity:anaphylaxis-assessment-question",
    generatedEntity: "chronicle:entity:anaphylaxis-guidance-revision-1",
  }
}
```

#### Revision

> A revision is a derivation for which the resulting entity is a revised version
> of some original. The implication here is that the resulting entity contains
> substantial content from the original. Revision is a particular case of
> derivation.

See [provenance concepts](provenance_concepts#revision)

Revision can be recorded in Chronicle using the `wasRevisionOf` mutation, which
takes two entities - the generatedEntity being a revision of the usedEntity.

```graphql
mutation {
  revision {
    usedEntity: "chronicle:entity:anaphylaxis-guidance-revision-1",
    generatedEntity: "chronicle:entity:anaphylaxis-guidance-revision-2",
  }
}
```

#### Quotation

> A quotation is the repeat of (some or all of) an entity, such as text or
> image, by someone who may or may not be its original author. Quotation is a
> particular case of derivation.

See [provenance concepts](provenance_concepts#quotation)

Quotation can be recorded bin chronicle using the `wasQuotedFrom` mutation,
which takes two entities - the generatedEntity having quoted from the
usedEntity.

```graphql
mutation {
  revision {
    usedEntity: "chronicle:entity:evidence-2321231",
    generatedEntity: "chronicle:entity:anaphylaxis-guidance-revision-2",
  }
}
```

### Chronicle-specific cryptographic operations

#### Has identity

#### Had evidence

Evidence is a Chronicle-specific provenance feature that simplifies the
association of a cryptographic signature with an Entity. You will need a GraphQL
client with multipart support for the attachment to sign.
