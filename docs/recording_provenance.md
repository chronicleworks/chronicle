# Recording Provenance using Chronicle

## Immutability

Chronicle provenance is immutable. Once recorded you cannot contradict it - only
record additional provenance.

## Open-World Model

Chronicle does not use relational constraints in the way you may expect a
relational database to, or allow free-form data in the manner of a document
store. It is useful to think of Chronicle as recording facts; it will make
simple inferences to keep your provenance data consistent.

For example:

```graphql title="'homer' in the activity 'writing-the-iliad' as a writer"
mutation {
    wasAssociatedWith(
        activity: { id: "chronicle:activity:writing-the-iliad" },
        responsible: { id: "chronicle:agent:homer" },
        role: writer
    )
}
```

```graphql title="An activity 'writing-the-iliad' that took place in Ionia"
mutation {
    defineWritingActivity(
        externalId: "writing-the-iliad",
        attributes: {
            locationAttribute: "ionia"
        }
    )
}
```

```graphql title="An agent 'homer' exists and was a person"
mutation {
    definePersonAgent(
        externalId: "homer",
        attributes: {
            emailAttribute: "homer@ionia.gr"
        }
    )
}
```

These three Chronicle mutations can be executed in *any* order - Chronicle will
assume the existence of the Agent and Activities referred to even if they have
not yet been recorded by their `definePersonAgent` and `defineWritingActivity` mutation.

## Contradiction

Chronicle will not allow you to record provenance that contradicts previously
recorded provenance. For example, you cannot change the start / end dates of
activities once set, or alter the value of an attribute. You can however record
identical provenance any number of times, as this does not cause contradiction,
or change the state of Chronicle meaningfully.

### Contradiction of Attribute Time

Here we attempt to change the end date of an activity after it has been
recorded - one of these mutations will fail.

```graphql
mutation {
    endActivity(id: {id: "chronicle:activity:birthday" },
        time: "2022-07-29T12:41:52.433Z"
    )

    endActivity(id: {id: "chronicle:activity:birthday" },
        time: "2020-07-29T12:41:52.433Z"
    )
}
```

### Contradiction of Attribute Value

Here we attempt to change the value of an attribute - one of these operations
will fail.

```graphql
mutation {
    defineWriterAgent(id: "chronicle:agent:poet", attributes: {
        externalIdAttribute: "Tennyson"
    })
}

mutation {
    defineWriterAgent(id: "chronicle:agent:poet", attributes: {
        externalIdAttribute: "Kipling"
    })
}
```

### Extending Attributes is Not a Contradiction

Where a provenance term has been recorded with attributes, and the domain has
been extended to include new attributes, it is valid to append new attributes,
as long as the values of the already recorded ones are unchanged.

```graphql
mutation {
    defineWriterAgent(id: "chronicle:agent:poet", attributes: {
        externalIdAttribute: "Tennyson"
    })
}

mutation {
    defineWriterAgent(id: "chronicle:agent:poet", attributes: {
        externalIdAttribute: "Tennyson",
        heightInCentimetersAttribute: "185"
    })
}
```

### Representing Mutability

Working through our [example domain definitions](https://examples.btp.works/),
one may notice that some define attributes like "location" that may seem as if
they could be mutable. They are *not* mutable: the above rules on
contradiction hold firm. Such examples use entities or agents in fixed
locations, and illustrate other facets of modeling domains in Chronicle.

With attributes like the location of an entity or an agent, in cases where
they may need updating, one approach is to instead model the locations as
entities in their own right. Then, for example, a person could engage in the
*activity* of "entering" or "leaving" a location. By marking the activity with
a [time](./provenance_concepts.md#start-and-end), even a duration, one may
then [query the activity timeline](./querying_provenance.md#activity-timeline)
to retrieve the history of when the person was where.

Similarly, in revising the description of an artifact, each description can
be an entity derived from the artifact itself. An agent can engage in the
activities of "inserting" one of the descriptions and "removing" the other.
As above, the history of an artifact's descriptions is available for query.

Using activities in this way allows changes over time to be represented while
the value for every attribute remains constant. If one chooses to take this
approach then it is only through an activity, happening at a specific time,
that a person is linked to their location, or an artifact to its description.
Through this sequence of related activities, the *provenance* of each state
change and their outcome is *recorded immutably*. They can be assembled into
the story of how things were and how they came to reach their current state.

## Operations

### Example Domain

For this section we will use our simplified
[domain model for recording the provenance of medical guidance](./domain_modeling.md).
We have a number of people who may be any combination of authors, editors, or
researchers collaborating to produce documents from evidence produced by a search
process. An external CMS is being used that has identifiers for documents and users.

### The `externalId` Parameter

The definition mutations for prov terms -  Entity, Activity and Agent - are all
supplied with an `externalId` parameter. This should be something meaningful to the
domain you are recording provenance for - a unique identifier from an external
system or a natural key. This will form part of the identity of the term.

### A Note on Identities

Chronicle identities will contain an encoded form of the externalId parameter, but
should be treated as opaque. Under no circumstances should you attempt to
synthesize an identity from Chronicle in client code as the scheme may be
subject to change.

### GraphQL Mutation Result - `Submission`

All Chronicle mutations return a `Submission` type defined by:

```graphql
type Submission {
  context: String!
  txId: String!
}
```

The `context` property here will be the identity of the Chronicle term you have
changed in the mutation, i.e calling `defineAgent(..)` will return the identity of
the agent, calling `startActivity(..)` the identity of the started activity.

The `txId` property corresponds to the transaction id when running
Chronicle with a backend ledger, or a randomly generated uuid when used in
[in-memory](./building.md#in-memory-version) mode.

### Commit Notification Subscriptions

Chronicle provides a [GraphQL subscription](https://graphql.org/blog/subscriptions-in-graphql-and-relay/)
to notify clients when a Chronicle operation has been sent to a backend ledger and
when that operation has been applied to both the ledger and Chronicle.

```graphql
enum Stage {
 SUBMITTED
 COMMITTED
}

type Submission {
  context: Stage!
  tx_id: String!
}

subscription {
  commitNotifications {
      stage
      tx_id
      error
      delta
  }
}
```

The `txId` on this subscription will match the txId from the
[Submission](#graphql-mutation-result---submission). Clients that wish to know
the result of an operation should await the
[Submission](#graphql-mutation-result---submission) and the corresponding
 `txId` from a commit notification.

Chronicle can detect that submitted operations do not add to or contradict the
state of the provenance objects. If the submission would not have any effect
then a `null` value for `txId` is returned because the operations were not
submitted to the transaction processor. The distributed ledger was not written
to because there was nothing further with which to update it.

Chronicle will notify commit completion in two stages. The first will be a
submit notification like:

```json
{
  "stage": "SUBMIT",
  "error": null,
  "delta": null,
  "txId": "12d3236ae1b227391725d2d9315b7ca53747217c5d..",
}
```

 And the second, when the Chronicle transaction has been committed to the
 ledger:

```json
{
  "stage": "COMMIT",
  "error": null,
  "txId": "12d3236ae1b227391725d2d9315b7ca53747217c5d..",
  "delta":  {
      "@context": "https://btp.works/chr/1.0/c.jsonld",
      "@graph": [
        {
          "@id": "chronicle:activity:publication1",
          "@type": [
            "prov:Activity",
            "chronicle:domaintype:Published"
          ],
          "externalId": "publication1",
          "namespace": "chronicle:ns:default:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
          "value": {
          }
        },
        {
          "@id": "chronicle:ns:default:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
          "@type": "chronicle:Namespace",
          "externalId": "default"
        }
      ]
    },
}
```

Once this message has been received, clients can assume that provenance has been
correctly recorded in the backend ledger, and is also available to be queried
on the Chronicle instance that sent the transaction. Chronicle is now
consistent with the ledger.

The delta field will be set to a JSON-LD object representing the provenance
objects that have been affected by the transaction. Advanced integrations can
use this as an update mechanism for reporting and other features that require
asynchronous updates of Chronicle provenance.

If a submission would not change any provenance objects then, not only would a
`null` value of `txId` be returned to the submitter, but also there would be no
notification of completion: nothing needed recording on the blockchain because,
effectively, there was no `delta`.

If error is set on either SUBMIT or COMMIT notifications, then the
transaction can be assumed to have failed. If the failure is on submission then
it is likely transient and resumable, but a failure on COMMIT should be
assumed to be non-resumable, as it will be a [contradiction](#contradiction).

### Define an Entity

> In PROV, things we want to describe the provenance of are called entities and
> have some fixed aspects. The term "things" encompasses a broad diversity of
> notions, including digital objects such as a file or web page, physical things
> such as a mountain, a building, a printed book, or a car as well as abstract
> concepts and ideas. An entity is a physical, digital, conceptual, or other
> kind of thing with some fixed aspects; entities may be real or imaginary.

See [provenance concepts](./provenance_concepts.md#entity)

Using our example domain, Chronicle will have generated four entity subtypes for
us, `Question`, `Guidance`, `PublishedGuidance`, and `EvidenceReference`, as a GraphQL
union called `Entity`. The union also contains an untyped entity `ProvEntity`.
The untyped entity can be potentially returned where the domain definition has
evolved, see [evolving your domain](./domain_modeling.md#evolution).

The definition mutations `defineQuestion`, `defineGuidance`,
`definePublishedGuidance`, and `defineEvidenceReference` will also have been
created to allow you to define an instance of each subtype and their attributes.
The generated GraphQL mutations and their associated types will look like this:

```graphql

scalar EntityID
scalar DomaintypeID
type EvidenceEntity {
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
  searchParameterAttribute: SearchParameterAttribute
  referenceAttribute: ReferenceAttribute
}
input EvidenceEntityAttributes {
  searchParameterAttribute: String!
  referenceAttribute: String!
}
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
input GuidanceEntityAttributes {
  titleAttribute: String!
  versionAttribute: Int!
}

input PublishedAttributes {
  versionAttribute: Int!
}
type PublishedGuidance {
  id: EntityID!
  namespace: Namespace!
  externalId: String!
  type: DomaintypeID
  evidence: EvidenceReference
  wasGeneratedBy: [Activity!]!
  wasDerivedFrom: [Entity!]!
  hadPrimarySource: [Entity!]!
  wasRevisionOf: [Entity!]!
  wasQuotedFrom: [Entity!]!
}

type Question {
  id: EntityID!
  namespace: Namespace!
  externalId: String!
  type: DomaintypeID
  evidence: EvidenceReference
  wasGeneratedBy: [Activity!]!
  wasDerivedFrom: [Entity!]!
  hadPrimarySource: [Entity!]!
  wasRevisionOf: [Entity!]!
  wasQuotedFrom: [Entity!]!
  CMSIdAttribute: CMSIdAttribute
  contentAttribute: ContentAttribute
}

input QuestionAttributes {
  CMSIdAttribute: String!
  contentAttribute: String!
}

type ProvEntity {
  id: EntityID!
  namespace: Namespace!
  externalId: String!
  type: DomaintypeID
  evidence: EvidenceReference
  wasGeneratedBy: [Activity!]!
  wasDerivedFrom: [Entity!]!
  hadPrimarySource: [Entity!]!
  wasRevisionOf: [Entity!]!
  wasQuotedFrom: [Entity!]!
}

input ProvEntityAttributes {
  type: String
}

union Entity = | ProvEntity | EvidenceReference | Guidance | PublishedGuidance | Question

mutation {
  defineEntity(externalId: String!, namespace: String, attributes: ProvEntityAttributes!): Submission!
  defineEvidenceReference(externalId: String!, namespace: String, attributes: EvidenceReferenceAttributes!): Submission!
  defineGuidance(externalId: String!, namespace: String, attributes: GuidanceAttributes!): Submission!
  definePublishedGuidance(externalId: String!, namespace: String): Submission!
  defineQuestion(externalId: String!, namespace: String, attributes: QuestionAttributes!): Submission!
}
```

Executing the following example mutation `defineQuestionEntity` will define an `Entity`
of subtype `Question`, along with its attributes.

```graphql
mutation {
  defineQuestionEntity(externalId: "anaphylaxis-referral", attributes: {
      CMSIdAttribute: "0c6fa8c5-69da-43d1-95d6-726f5f671b30",
      contentAttribute: "How to assess and refer patients needing emergency treatment for Anaphylaxis",
  })
}
```

And the equivalent operation using the command line interface is:

```bash
chronicle question-entity define anaphylaxis-referral --CMS-id-attribute \
  "How to assess and refer patients needing emergency treatment for Anaphylaxis" --CMS-id-attr "0c6fa8c5-69da-43d1-95d6-726f5f671b30"

```

Either operation will return the ID of the newly defined question.

### Define an Activity

See [provenance concepts](./provenance_concepts.md#activity)

Chronicle will have generated four `Activity` subtypes for us, `QuestionAsked`,
`Researched`, `Revised`, and `Published`, as a GraphQL union called `Activity`.
The union also contains an untyped activity `ProvActivity`. The untyped activity
can be potentially returned where the domain definition has evolved, see
[evolving your domain](./domain_modeling.md#evolution).

The definition mutations `defineQuestionAskedActivity`, `defineResearchedActivity`,
`defineRevisedActivity`, and `definePublished` will also have been created to
allow you to define an instance of each subtype and their attributes. The
generated GraphQL mutations and their associated types will look like this:

```graphql
union Activity = | ProvActivity | Published | QuestionAsked | Researched | Revised

type ProvEntity {
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
}

input ProvEntityAttributes {
  type: String
}

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

input PublishedActivityAttributes {
  versionAttribute: Int!
}

type PublishedGuidanceEntity {
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
}

type ResearchedActivity {
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
}

type RevisedActivity {
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
  CMSIdAttribute: CMSIdAttribute
  versionAttribute: VersionAttribute
}

input RevisedActivityAttributes {
  CMSIdAttribute: String!
  versionAttribute: Int!
}

mutation {
  defineActivity(
    externalId: String!
    namespace: String
    attributes: ProvActivityAttributes!
  ): Submission!
  definePublishedActivity(
    externalId: String!
    namespace: String
    attributes: PublishedActivityAttributes!
  ): Submission!
  defineQuestionAskedActivity(
    externalId: String!
    namespace: String
    attributes: QuestionAskedActivityAttributes!
  ): Submission!
  defineResearchedActivity(externalId: String!, namespace: String): Submission!
  defineRevisedActivity(
    externalId: String!
    namespace: String
    attributes: RevisedActivityAttributes!
  ): Submission!
}
```

The following example mutation `defineRevisedActivity` will define an `Activity`
of subtype `RevisedActivity`:

```graphql title="Define a revised activity with graphql"
mutation {
    defineRevisedActivity(externalId: "september-2018-review", attributes: {
        versionAttribute: 14,
    })
}
```

And the equivalent operation using the command line interface is:

```bash title="Define a document entity with the CLI"
chronicle revised-activity define september-2018-review --version-attr 14
```

### Define an Agent

See [provenance concepts](./provenance_concepts.md#agent)

Chronicle will have generated two `Agent` subtypes for us, `PersonAgent` and
`OrganizationAgent` as a GraphQL union called `Agent`. The union also contains an
untyped activity `ProvAgent`. The untyped agent can be potentially returned
where the domain definition has evolved, see [evolving your domain](./domain_modeling.md#evolution).

The definition mutations `definePersonAgent` and `defineOrganizationAgent` will
also have been created. See [domain modeling](./domain_modeling.md#modeling-a-provenance-domain-with-chronicle)
for details on the generated GraphQL SDL.

```graphql title="Define an organization agent with graphql"
mutation {
  defineOrganizationAgent(externalId: "health-trust", attributes: {})
}
```

And the equivalent operation using the command line interface is:

```bash title="Define an organization entity with the CLI"
chronicle organization-agent define health-trust
```

### Used

See [provenance concepts](./provenance_concepts.md#usage)

Usage operations in Chronicle can applied to all subtypes of Entity and
Activity.

To apply using GraphQL:

```graphql
mutation {
  used(activity: { id: "chronicle:activity:september-2018-review" }, id: {id: "chronicle:entity:anaphylaxis-evidence-12114" })
}
```

And the equivalent operation using the command line interface is:

```bash
chronicle revised-activity use "chronicle:entity:anaphylaxis-evidence-12114" "chronicle:activity:september-2018-review"
```

### Generation

See [provenance concepts](./provenance_concepts.md#generation)

Generation operations in Chronicle can be applied to all subtypes of Entity and
Activity.

To apply using GraphQL:

```graphql
mutation {
  wasGeneratedBy(
    activity: {id: "chronicle:activity:september-2018-review" },
    id: {id: "chronicle:entity:anaphylaxis-guidance-9-2018" }
  )
}
```

And the equivalent operation using the command line interface is:

```bash
chronicle revised-activity generate "chronicle:entity:anaphylaxis-guidance-9-2018" "chronicle:activity:september-2018-review"
```

### Started at Time

See [provenance concepts](./provenance_concepts.md#start)

Chronicle allows you to specify the start time of an activity when you need to
model a time range. If you want an instantaneous activity, simply call [ended at
time](#ended-at-time). Eliding the time parameter will use the current system
time. Time stamps should be in
[RFC3339](https://www.rfc-editor.org/rfc/rfc3339.html) format.

Started at time operations also take an optional `AgentIdOrExternal`, to
associate the activity with the agent - there is no current way to record a role
with this however, so prefer [wasAssociatedWith](#association) if you need
role-based modeling.

```graphql
mutation {
  startActivity(
    id: {id: "chronicle:activity:september-2018-review"},
    time: "2002-10-02T15:00:00Z"
  )
}
```

```bash
chronicle revision-activity start "chronicle:activity:september-2018-review" --time "2002-10-02T15:00:00Z"

```

### Ended at Time

See [provenance concepts](./provenance_concepts.md#end)

Chronicle allows you to specify the end time of an activity when you need to
model a time range. Eliding the time parameter will use the current system time.
Time stamps should be in [RFC3339](https://www.rfc-editor.org/rfc/rfc3339.html)
format.

Ended at time operations also take an optional `AgentIdOrExternal`, to associate
the activity with the agent - there is no current way to record a role with this
however, so prefer [wasAssociatedWith](#association) if you need role-based
modeling.

```graphql
mutation {
  endActivity(
    id: {id: "chronicle:activity:september-2018-review" },
    time: "2002-10-02T15:00:00Z"
  )
}
```

And the equivalent command line operation:

```bash
chronicle revision-activity end "chronicle:activity:september-2018-review" --time "2002-10-02T15:00:00Z"

```

### Instant

Instant is a convenience operation that will set both start and end time in the
same operation.

```graphql
mutation {
  instantActivity(
    id: { id: "chronicle:activity:september-2018-review" },
    time: "2002-10-02T15:00:00Z"
  )
}
```

And the equivalent command line operation:

```bash
chronicle revision-activity instant "chronicle:activity:september-2018-review" --time "2002-10-02T15:00:00Z"

```

### Association

See [provenance concepts](./provenance_concepts.md#association)

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
`QuestionAskedActivity`, using the `Role` `STAKEHOLDER`.

```graphql
mutation {
  wasAssociatedWith(
    responsible: {id: "chronicle:agent:ncsa" },
    activity: {id: "chronicle:activity:anaphylaxis-assessment" },
    role: STAKEHOLDER
  )
}

```

### Delegation

See [provenance concepts](./provenance_concepts.md#delegation)

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
    responsible: {id: "chronicle:agent:john-roberts" },
    delegate: {id: "chronicle:agent:janet-flynn" },
    activity: {id: "chronicle:activity:september-2018-review" },
    role: EDITOR
  )
}

```

### Derivation

#### Derived

See [provenance concepts](./provenance_concepts.md#derivation)

Derived sources can be recorded in Chronicle using the `wasDerivedFrom`
mutation, which takes two entities - the generatedEntity having been derived
from the usedEntity.

```graphql
mutation {
  wasDerivedFrom(
    usedEntity: {id: "chronicle:entity:anaphylaxis-assessment-question" },
    generatedEntity: {id: "chronicle:entity:anaphylaxis-guidance-revision-1" },
  )
}
```

Note that derivation is a general concept into which fit the three below:
primary sources, revision, and quotation, are subtypes of derivation.
Where applicable, use those instead for greater specificity.

#### Primary Source

See [provenance concepts](./provenance_concepts.md#primary-source)

Primary sources can be recorded in Chronicle using the `hadPrimarySource`
mutation, which takes two entities - the generatedEntity having a primary source
of the usedEntity.

```graphql
mutation {
  hadPrimarySource(
    usedEntity: {id: "chronicle:entity:anaphylaxis-assessment-question" },
    generatedEntity: {id: "chronicle:entity:anaphylaxis-guidance-revision-1" },
  )
}
```

#### Revision

See [provenance concepts](./provenance_concepts.md#revision)

Revision can be recorded in Chronicle using the `wasRevisionOf` mutation, which
takes two entities - the generatedEntity being a revision of the usedEntity.

```graphql
mutation {
  wasRevisionOf(
    usedEntity: {id: "chronicle:entity:anaphylaxis-guidance-revision-1" },
    generatedEntity: {id: "chronicle:entity:anaphylaxis-guidance-revision-2" },
  )
}
```

#### Quotation

See [provenance concepts](./provenance_concepts.md#quotation)

Quotation can be recorded bin chronicle using the `wasQuotedFrom` mutation,
which takes two entities - the generatedEntity having quoted from the
usedEntity.

```graphql
mutation {
  wasQuotedFrom(
    usedEntity: {id: "chronicle:entity:evidence-2321231" },
    generatedEntity: {id: "chronicle:entity:anaphylaxis-guidance-revision-2" },
  )
}
```

### Chronicle-Specific Cryptographic Operations

#### Background

The following operations are prototypes. They should not yet be used nor be
expected to be usable. Nonetheless, they are already part of Chronicle so
they are documented, even just as a tentative preview of coming features.

#### Identity

Identity is planned to be a Chronicle-specific provenance feature that allows
a cryptographic key to be associated with an agent. An agent's identity cannot
yet be set via a mutation.

When it can be updated, agents' previous identities will remain recorded,
along with the corresponding public keys.

#### Evidence

Evidence is planned to be a Chronicle-specific provenance feature that
simplifies the association of a cryptographic signature with an entity.
A user would need a GraphQL client with multipart support for submitting the
attachment.

The signature will be recorded. If an entity's evidence is updated
subsequently, details of the previous evidence and their signatures will remain
recorded, just like above wherein agents' past identities remain recorded.

The attachment itself is not preserved, just the details of its location
and signing.

The current `hasAttachment` mutation is intended for recording but is not yet
usable.

## Importing Data into Chronicle

Chronicle provides a CLI command `import` to load data from a JSON-LD file that
contains an array of Chronicle Operations. These operations can be used to
create agents, namespaces, and other resources in the Chronicle database.

To import data into Chronicle, follow these steps:

- Create a JSON-LD file with an array of Chronicle Operations.
- Save the file in the Chronicle root directory with a unique name, for example,
  `import.json`.
- Run the following command:

  ```bash
  chronicle --bin chronicle -- import <namespace-id> <namespace-uuid> <path-to-json-file>
  ```

  where:

  `<namespace-id>`: the name of the namespace to which the data will be imported.
  `<namespace-uuid>`: the UUID of the namespace to which the data will be imported.
  `<path-to-json-file>`: the path to the JSON-LD file containing the Chronicle Operations.

- If the data is successfully imported, the Chronicle Operations will be added
  to the Chronicle database.

For example, suppose you want to create a new agent with the name `testagent`
and a new namespace with the name `testns`. You can add the following
operations to your JSON-LD file:

```json
[
    {
        "@id": "_:n1",
        "@type": [
        "http://btp.works/chronicleoperations/ns#SetAttributes"
        ],
        "http://btp.works/chronicleoperations/ns#agentName": [
        {
            "@value": "testagent"
        }
        ],
        "http://btp.works/chronicleoperations/ns#attributes": [
        {
            "@type": "@json",
            "@value": {}
        }
        ],
        "http://btp.works/chronicleoperations/ns#domaintypeId": [
        {
            "@value": "type"
        }
        ],
        "http://btp.works/chronicleoperations/ns#namespaceName": [
        {
            "@value": "testns"
        }
        ],
        "http://btp.works/chronicleoperations/ns#namespaceUuid": [
        {
            "@value": "6803790d-5891-4dfa-b773-41827d2c630b"
        }
        ]
    },
    {
        "@id": "_:n1",
        "@type": [
        "http://btp.works/chronicleoperations/ns#CreateNamespace"
        ],
        "http://btp.works/chronicleoperations/ns#namespaceName": [
        {
            "@value": "testns"
        }
        ],
        "http://btp.works/chronicleoperations/ns#namespaceUuid": [
        {
            "@value": "6803790d-5891-4dfa-b773-41827d2c630b"
        }
        ]
    },
    {
        "@id": "_:n1",
        "@type": [
        "http://btp.works/chronicleoperations/ns#AgentExists"
        ],
        "http://btp.works/chronicleoperations/ns#agentName": [
        {
            "@value": "testagent"
        }
        ],
        "http://btp.works/chronicleoperations/ns#namespaceName": [
        {
            "@value": "testns"
        }
        ],
        "http://btp.works/chronicleoperations/ns#namespaceUuid": [
        {
            "@value": "6803790d-5891-4dfa-b773-41827d2c630b"
        }
        ]
    }
]
```

To import data into Chronicle using the `import` command, you need to provide
the `namespace-id` and `namespace-uuid` arguments. These arguments are required
to specify the [namespace](./namespaces.md) into which the data will be
imported.

The `namespace-id` argument is a string that specifies the name of the namespace
to which the data will be imported. This value should match the value of the
`http://btp.works/chronicleoperations/ns#namespaceName` property in the JSON-LD
file.

The `namespace-uuid` argument is a string that specifies the UUID of the
namespace to which the data will be imported. This value should match the value
of the `http://btp.works/chronicleoperations/ns#namespaceUuid` property in the
JSON-LD file. Data in the file not matching the specified namespace is ignored.

Once you have provided the `namespace-id` and `namespace-uuid` arguments, you
can run the `import` command using the JSON-LD file containing the Chronicle
operations as the data source. If the data is successfully imported, the
Chronicle Operations will be added to the Chronicle database.

In the example provided, the `import` command imports data from the `import.json`
file into the namespace with the name `testns` and the UUID
`6803790d-5891-4dfa-b773-41827d2c630b`. The JSON-LD file contains three
Chronicle Operations that set the attributes of a new agent with the name
`testagent`, create a new namespace with the name `testns`, and create the new
agent `testagent` in the namespace `testns`.

To run the transaction described above, run the following command:

```bash
chronicle --bin chronicle -- import testns 6803790d-5891-4dfa-b773-41827d2c630b import.json
```

`import` uses standard input as its data source by default. To import data from
standard input run the `import` command like this:

```bash
< import.json cargo run --bin chronicle \
    -- import \
    testns \
    6803790d-5891-4dfa-b773-41827d2c630b
```

If the `import` command is executed successfully, the agent `testagent` and
the namespace `testns` will be created in the Chronicle database.
