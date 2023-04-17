# Modeling a Provenance Domain with Chronicle

Here we will present a reference domain that uses all the provenance features
Chronicle provides and work through the process of representing it using
Chronicle's domain modeling syntax. This should help you both to understand
Chronicle's capabilities and to translate your own problem domain's provenance.

Chronicle uses the [W3C Provenance Ontology](https://www.w3.org/TR/prov-o/) as
the basis for provenance modeling.

## Reference Domain - Medical Evidence

This is a toy model of some aspects of evidence-based medicine, from an initial
`Question` - the area and scope that the organization wishes to research and
make guidance on - to revisions of a published `Guidance` document. The system
is currently handled by a content management system that has identities for
documents and users, and we will use Chronicle to add provenance capabilities.

### Question Creation

The question for medical evidence can vary significantly, but for the purposes
of this example imagine it as something along the lines of, "How best to assess
and refer patients who have required emergency treatment for Anaphylaxis".

Various actors and processes are involved in the production of the question, but
for our purposes we van view it like this:

![file](diagrams/out/question.svg)

The `Question` is then used to inform the `Research` for the production of
`Guidance`.

To model and record this process you will need the Chronicle domain model
definition explained here, along with the following operations:

- [defineQuestionEntity](./recording_provenance.md#define-an-entity) defines an Entity
  of subtype Question
- [defineQuestionAskedActivity](./recording_provenance.md#define-an-activity) defines
  an Activity of subtype QuestionAsked
- [definePersonAgent or defineOrganizationAgent](./recording_provenance.md#define-an-agent)
  defines an Agent of subtype Person or Organization to act as Stakeholders
- [definePersonAgent](./recording_provenance.md#define-an-agent) defines an Agent
  of subtype Person to act as Authors
- [wasGeneratedBy](./recording_provenance.md#generation) specifies that the QuestionAsked
  Activity produced the Question
- [wasAssociatedWith](./recording_provenance.md#association) specifies the Person
  who authored and the Organizations that asked
- [startedAtTime](./recording_provenance.md#started-at-time) and
  [endedAtTime](./recording_provenance.md#ended-at-time) specify that the
  question was asked at a point in time

This process represented as provenance will look like:

![file](diagrams/out/question_as_prov.svg)

### Research

The `Question` is used to inform one or more searches to a search engine by a
researcher, the parameters to the search engine are recorded, and the results
are used to create references to some `Evidence`.

![file](diagrams/out/research.svg)

To model and record this process you will need the Chronicle domain model
definition explained here, along with the following operations:

- [defineQuestionEntity](./recording_provenance.md#define-an-entity) defines an Entity
  of subtype Question
- [defineEvidenceEntity](./recording_provenance.md#define-an-entity) defines an
  Entity of subtype Evidence
- [defineResearchedActivity](./recording_provenance.md#define-an-activity) defines
  an Activity of subtype Researched
- [definePersonAgent](./recording_provenance.md#define-an-agent) defines an Agent
  of subtype Person
- [used](./recording_provenance.md#usage) specifies that the Research Activity
  used the Question
- [wasGeneratedBy](./recording_provenance.md#generation) specifies that the
  Research Activity produced the Evidence
- [wasAssociatedWith](./recording_provenance.md#association) specifies that the
  research was done by a Person acting as a researcher
- [startedAtTime](./recording_provenance.md#started-at-time) specifies the research
  began at a point in time
- [endedAtTime](./recording_provenance.md#ended-at-time) specifies the research
  ended at a point in time

This process represented as provenance will look like:

![file](diagrams/out/research_as_prov.svg)

### Revision

Guidance, like authorship, is triggered by research - in this case for changes
or additions to the evidence base. Evidence is used to inform a new
revision of the Guidance document.

![file](diagrams/out/revision.svg)

To model and record this process you will need the Chronicle domain model
definition explained here, along with the following operations:

- [defineQuestionEntity](./recording_provenance.md#define-an-entity) defines an Entity
  of subtype Question
- [defineGuidanceEntity](./recording_provenance.md#define-an-entity) defines an Entity
  of subtype Guidance
- [defineEvidenceEntity](./recording_provenance.md#define-an-entity) defines an Entity
  of subtype Evidence
- [defineRevisedActivity](./recording_provenance.md#define-an-activity) defines an
  Activity of subtype Revised
- [used](./recording_provenance.md#usage) specifies that the Revised Activity
  used the Question
- [used](./recording_provenance.md#usage) specifies that the Revised Activity
  used the Evidence
- [wasGeneratedBy](./recording_provenance.md#generation) specifies that the
  Revision Activity produced the Guidance
- [wasRevisionOf](./recording_provenance.md#revision) specifies that the
  Guidance is possibly a Revision of previous Guidance
- [hadPrimarySource](./recording_provenance.md#primary-source) specifies that
  the Guidance possibly has a primary source of the Question (for the first
  version)
- [startedAtTime](./recording_provenance.md#started-at-time) specifies the
  Guidance process began at a point in time
- [endedAtTime](./recording_provenance.md#ended-at-time) specifies the Guidance
  process ended at a point in time

This process represented as provenance will look like:

![file](diagrams/out/revision_as_prov.svg)

### Publication

A version of Guidance can be approved for Publication by one or more Editors or
Stakeholders. Publication produces a digital artifact that can be signed.

![file](diagrams/out/publication.svg)

- [defineGuidanceEntity](./recording_provenance.md#define-an-entity) defines an Entity
  of subtype Guidance
- [definePublishedGuidanceEntity](./recording_provenance.md#define-an-entity) defines
  an Entity of subtype PublishedGuidance
- [defineEvidenceEntity](./recording_provenance.md#define-an-entity) defines an
  Entity of subtype Evidence
- [definePublishedActivity](./recording_provenance.md#define-an-activity) defines
  an Activity of subtype Published
- [used](./recording_provenance.md#usage) specifies that the Published activity
  used the Guidance
- [wasGeneratedBy](./recording_provenance.md#generation) specifies that the
  Published Activity produced the PublishedGuidance
- [wasAssociatedWith](./recording_provenance.md#association) specifies that the
  Publication was done by a Person acting as an Editor
- [actedOnBehalfOf](./recording_provenance.md#delegation) specifies that the
  Publication was done by on behalf of on or more Stakeholders
- [hadPrimarySource](./recording_provenance.md#primarySource) specifies that
  the PublishedGuidance has a primary source of the Guidance
- [endedAtTime](./recording_provenance.md#ended-at-time) specifies the
  Published process happened at a point in time
- [hadEvidence](./recording_provenance.md#had-evidence) attaches a signature of
  the published PDF document to the PublishedGuidance activity

This process represented as provenance will look like:

![file](diagrams/out/publication_as_prov.svg)

## Conceptual Design

Provenance is *immutable*. Once you have recorded it there is no way to
contradict the provenance you have recorded. When translating your domain to
provenance, your activities should be things that have either already take
place, or in progress - so choose the past tense. From the process descriptions
above we can create the following provenance domain:

### Required Attributes

#### Content

Plaintext content of an external resource.

#### CMSId

An opaque identifier from the CMS being used to author and publish documents.

#### Title

A plaintext title.

#### SearchParameter

The input to a search engine.

#### Reference

A [BibTex](http://www.bibtex.org/) reference to evidence.

#### Version

A simple incrementing integer representing a version number.

### Entities

See [provenance concepts](./provenance_concepts.md#entity)

When determining entities, a useful approach from [process
mapping](https://www.bpmleader.com/2012/07/23/introductory-guide-to-process-mapping-modelling/)
is to look for nouns in your analysis. Provenance modeling is no different. We
can identify the following Entities:

#### Question

The initial question that forms the basis of all research, informing guidance
via research.

Has attributes:

- CMSId
- Content

#### Evidence

A reference to evidence gathered from a search engine.

Has attributes:

- SearchParameter
- Reference

#### Guidance

The source text of a document, either in the process of authoring or potentially
published.

Has attributes:

- Title
- Guidance

#### PublishedGuidance

A published guidance document, containing a digital signature of the released
PDF.

Has no attributes.

### Activities

See [provenance concepts](./provenance_concepts.md#activity)

When determining activities, a useful approach from [process
mapping](https://www.bpmleader.com/2012/07/23/introductory-guide-to-process-mapping-modelling/)
is to look for verbs in your analysis. Provenance modeling is similar, except
we are modeling things that have taken place or are in progress. It is useful
to use past tense for this reason. We can identify:

#### QuestionAsked

The first Activity we need to record, it will Generate a QuestionAskedActivity.

#### Researched

This activity will model the use of a search engine by a Researcher to produce
an EvidenceEntity.

#### Revised

This activity will model authorship and refinement by an Editor of a single
revision of guidance, informed by the Question and Evidence from research.

#### Published

This activity models the publication of a particular revision of Guidance,
approved by an editor under the advice of stakeholders.

### Agents

See [provenance concepts](./provenance_concepts.md#agent)

For our example domain, actors are best modeled as Roles rather than Agents -
People and Organizations can participate in multiple ways. So we will specify
the following agents:

#### Person

An individual person

#### Organization

A named organization consisting of one or more persons, the details of the
organizational model are not required to be recorded in provenance.

### Roles

When participating in activities, when either directly responsible or via
delegation, Agents can have a Role. Agents form the who, whereas Roles are the
'what'. Agents may have multiple roles in the same Activity. From our example
domain we can identify the following roles:

#### Stakeholder

A stakeholder is an Organization or Person involved in the formulation of a
Question and the approval of Publication.

#### Author

An Author is a Person who creates a Guidance of Guidance supervised by an
Editor.

#### Researcher

A researcher is a Person who submits SearchParameter to a search engine and then
creates Evidence.

#### Editor

An editor is a Person who approves Publication after consulting one or more
Stakeholders and supervises Authors creating Guidances of Guidance.

## Domain Model Format

We will now translate this conceptual design into Chronicle's domain-modeling
syntax. Chronicle domain models are specified in YAML, a complete model for the
conceptual design can be written like this:

```yaml
name: 'evidence'
attributes:
  Content:
    type: String
  CMSId:
    type: String
  Title:
    type: String
  SearchParameter:
    type: String
  Reference:
    type: String
  Version:
    type: Int
entities:
  Question:
    attributes:
      - CMSId
      - Content
  Evidence:
    attributes:
      - SearchParameter
      - Reference
  Guidance:
    attributes:
      - Title
      - Version
  PublishedGuidance:
    attributes: []
activities:
  QuestionAsked:
    attributes:
      - Content
  Researched:
    attributes: []
  Published:
    attributes:
      - Version
  Revised:
    attributes:
      - CMSId
      - Version
agents:
  Person:
    attributes:
      - CMSId
  Organization:
    attributes:
      - Title
roles:
  - STAKEHOLDER
  - AUTHOR
  - RESEARCHER
  - EDITOR
```

### ExternalId

A string that names your domain, used to coordinate deployments that require
multiple namespaces.

```yaml
name: "evidence"
```

### Attributes

Attributes are used to assign additional data to the prov terms - `Agent`,
`Activity`, and `Entity`. They are defined by their externalId and Primitive type,
one of:

- String
- Int
- Bool
- JSON

Attribute names should be meaningful to your domain - choose things like 'Title'
or 'Description', they can be reused between any of prov terms - Entity,
Activity and Agent.

```yaml
attributes:
  Content:
    type: String
  CMSId:
    type: String
  Title:
    type: String
  SearchParameter:
    type: String
  Reference:
    type: String
  Version:
    type: Int
```

#### Inputting a JSON Attribute

To input a JSON attribute, make sure to add an attribute to your domain of type
`JSON`, for example,

```yaml
attributes:
  Manifest:
    type: JSON
```

To add a JSON attribute in a GraphQL mutation, create an input query variable,
such as:

```json
{
  "input": {
        "Id": "d577e4f14441b94a71fdfc6415b574370101236a40a82107d0305ddcafbdba16",
        "Created": "2022-11-07T15:04:10.123735797Z",
        "Path": "docker-entrypoint.sh",
        "Args": [
            "postgres"
        ],
        "State": {
            "Status": "running",
            "Running": true,
            "Paused": false,
            "Restarting": false,
            "OOMKilled": false,
            "Dead": false,
            "Pid": 15733,
            "ExitCode": 0,
            "Error": "",
            "StartedAt": "2022-11-07T15:04:10.576174958Z",
            "FinishedAt": "0001-01-01T00:00:00Z"
        },
    }
}
```

That data can then be used as input in a mutation like the one below, defining
an Agent called Test with a JSON attribute named Manifest:

```graphql
mutation defineTestAgent($input: JSON!) {
  defineTestAgent(externalId: "testagent", attributes: { manifestAttribute: $input }) {
    context
  }
}
```

The Agent's id and JSON attribute data can then be queried like so:

```graphql
query agentQuery {
  agentById(id: { id: "chronicle:agent:testagent" }) {
    __typename
    ... on TestAgent {
      id
      manifestAttribute
    }
  }
}
```

### Agent

Using Chronicle's domain model definitions an Agent can be subtyped and
associated with attributes like other provenance terms. In the following example
we define the two Agent subtypes, Person has an id from the CMS, Organization a
text title.

```yaml
agents:
  Person:
    attributes:
      - CMSId
  Organization:
    attributes:
      - Title
```

### Entity

Using Chronicle's domain model definitions an Entity can be subtyped and
associated with attributes like other provenance terms. In the following example
we define the four Entity subtypes. Question has an id from the CMS and its
content, Evidence has at least one search parameter and a reference,
Guidance has a title and version, and PublishedGuidance needs no attributes.

```yaml
entities:
  Question:
    attributes:
      - CMSId
      - Content
  Evidence:
    attributes:
      - SearchParameter
      - Reference
  Guidance:
    attributes:
      - Title
      - Version
  PublishedGuidance:
    attributes: []

```

### Activity

See [provenance concepts](./provenance_concepts.md#activity)

Using Chronicle's domain model definitions an Activity can be subtyped and
associated with attributes like other provenance terms. In the following example
we define the four Activity subtypes, QuestionAsked has content, Researched has
no attributes, Published has a version, and Revised has an id from the CMS and a
version.

```yaml
activities:
  QuestionAsked:
    attributes:
      - Content
  Researched:
    attributes: []
  Published:
    attributes:
      - Version
  Revised:
    attributes:
      - CMSId
      - Version
```

### Role

Corresponding to actors in the example domain we specify the following roles:

```yaml
roles:
  - STAKEHOLDER
  - AUTHOR
  - RESEARCHER
  - EDITOR
```

Supplying this as a YAML file to the Chronicle build image as documented in
[building chronicle](./building.md) will produce a well-typed API for your
domain. The next step is then [recording provenance](./recording_provenance.md).

## Chronicle Domain Definition Documentation Capabilities

Chronicle's documentation capabilities provide users with a powerful tool for
generating Rust code documentation and GraphQL schema documentation for their
domains.

Chronicle provides users with the ability to define a domain by using a
domain.yaml file and adding documentation comments to it. Upon loading a
valid domain.yaml file, Chronicle generates the domain based on its contents.
Any `doc` and `roles_doc` field comments included in the domain.yaml file are
used to document both the Rust code and the GraphQL schema associated with
the Chronicle domain. The `doc` and `roles_doc` field comments are optional
and can be left out.

Below, in the snippet of the `attributes` of a domain entiteld `Artworld`, the
`Title` attribute of the `Artwork` entity is documented with a Markdown
comment that describes what the `Title` attribute represents and the entities
and activities it is associated with.

```yaml
name: Artworld
attributes:
  Title:
    doc: |
      # `Title`

      `Title` can be the title attributed to

      * `Artwork`
      * `ArtworkDetails`
      * `Created`

    type: String
  Location:
    type: String
  PurchaseValue:
    type: String
  PurchaseValueCurrency:
    type: String
  Description:
    type: String
  Name:
    type: String
```

Similarly, as you see in the snippet below, the `Collector` and `Artist`
agents are documented with `doc` field comments that describe what they
represent and their relationship to other entities and activities in the
domain.

```yaml
agents:
  Collector:
    doc: |
      # `Collector`

      Collectors purchase and amass collections of art.

      Collectors might well be involved in exhibiting (`Exhibited`) and selling (`Sold`) works of art.
    attributes:
      - Name
  Artist:
    doc: |
      # `Artist`

      Artists create new works of art.

      Artists might well be involved in exhibiting (`Exhibited`) and selling (`Sold`) works of art.
    attributes:
      - Name
```

The `Artwork` and `ArtworkDetails` entities are also documented with
`doc` field comments that provide more information about what they
represent and how they can be defined using GraphQL.

```yaml
entities:
  Artwork:
    doc: |
      # `Artwork`

      Refers to the actual physical art piece.

      ## Examples

      This entity can be defined in Chronicle using GraphQL like so:

      ```graphql

      mutation {
        defineArtworkEntity(
          externalId: "salvatormundi"
          attributes: { titleAttribute: "Salvator Mundi" }
        ) {
          context
          txId
        }
      }

      ```

    attributes:
      - Title
  ArtworkDetails:
    doc: |
      # `ArtworkDetails`

      Provides more information about the piece, such as its title and description

      ## Examples

      This entity can be defined in Chronicle using GraphQL like so:

      ```graphql

      mutation {
        defineArtworkDetailsEntity(
          externalId: "salvatormundidetails"
          attributes: {
            titleAttribute: "Salvator Mundi"
            descriptionAttribute: "Depiction of Christ holding a crystal orb and making the sign of the blessing."
          }
        ) {
          context
          txId
        }
      }

      ```

    attributes:
      - Title
      - Description
```

The `roles_doc` field provides additional documentation for the
roles associated with the entities and activities in the domain.
It includes examples of how these roles can be used in the context
of selling or creating an artwork, and provides an overview of the
roles associated with buying, selling, and creating art.

```yaml
roles_doc: |
  # Buyer, Seller, and Creator Roles

  ## Examples

  In the context of association with selling (`Sold`) of an `Artwork`,
  an `Artist`'s function could be `SELLER`, and `CREATOR` in the context
  of creation (`Created`).

  A `Collector`'s function in the context of the sale (`Sold`) of an
  `Artwork` could be `BUYER` or `SELLER`.
roles:
  - BUYER
  - SELLER
  - CREATOR
```

For more information on writing documentation comments for Rust, see
[the rustdoc book](https://doc.rust-lang.org/rustdoc/what-is-rustdoc.html).
See the [Markdown Guide](https://www.markdownguide.org/) for more about
how to use Markdown.

## Evolution

Redefinition of a Chronicle domain with existing data is possible, with some
caveats:

### Type Removal

You can remove a prov term (Entity, Agent or Activity), but as Chronicle data is
immutable it will still exist on the back end. Terms can still be returned via
queries, but will be as their Untyped variant - ProvEntity, ProvAgent and
ProvActivity and their attributes will no longer be available via GraphQL.

### Attribute Removal

You can remove an attribute, but again it will still exist in provenance you
have already recorded.

### Attribute Addition

You can add new attributes and add their values to both existing and new data.

This conforms to most reasonable models of interface and protocol evolution,
where you should design for extension rather than modification.

### Formatting Domain Terms

In order to keep the GraphQL description of data readable and consistent,
devise domain terms with the following in mind:

Domain terms (agents, activities, entities, attributes) MUST be alphanumeric,
not starting with a digit, and MUST start with at least one capital letter.

Roles MUST be in SCREAMING_SNAKE_CASE.

When describing domains in GraphQL, the following transformations take place:

Definition methods will be prefixed with "define." The method for creating a
generic agent will be `defineAgent`. Defining an agent named Person will
require the `definePersonAgent` method, while an entity named Item will be
defined with `defineItemEntity`, and so on.

GraphQL objects are preserved in pascal case (`ItemCheckActivity`), except when
acronyms are preserved in object names (`HSBCAgent`), while non-acronym attributes
will be transformed into camel case (`itemAttribute`).

An activity named ItemChecked with an associated attribute named Item will be
represented in GraphQL as `ItemCheckedActivity { itemAttribute }`.

Acronyms in domain terms are preserved.

An agent named NYU will be described as `NYUAgent`, while NPRListener will be
described in GraphQL as `NPRListenerAgent`, including in operations:

`defineNYUAgent(id:)`

```graphql
... on NPRListenerAgent { NPRListenerAttribute }
```
