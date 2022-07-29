# Recording Provenance using Chronicle

## Immutability

Chronicle provenance is immutable. Once recorded you cannot contradict it - only add additional provenance.

## Open world model

Chronicle does not use relational constraints in the way you may expect a relational database to, or allow free form data in the manner of a document store. It is useful to think of Chronicle as recording facts, and it will make simple inferences to keep your provenance data consistent.

For example:

``` graphql title="The agent 'homer' participated in the activity 'writing-the-iliad' as a writer"
mutation {
    wasAssociatedWith(
        activity: "chronicle:activity:writing-the-iliad",
        agent: "chronicle:agent:homer",
        role: writer
    )
}
```

``` graphql title="There was an activity 'writing-the-iliad' that took place in Ionia"
mutation {
    writing(
        name: "writing-the-iliad",
        attributes: {
            location: "ionia"
        }
    )
}
```


``` graphql title="An agent 'homer' exists and was a person"
mutation {
    person(
        name: "homer",
        attributes: {
            email: "homer@ionia.gr"
        }
    )
}
```

These three Chronicle mutations can be executed in *any* order - Chronicle will assume the existence of the Agent and Activities referred to even if they have not yet been recorded by their `person` and `writing` mutation.

## Contradiction of activity time

Chronicle will not allow you to record provenance that contradicts previously recorded provenance. For example - you cannot change the start / end dates of activities once set, or alter the value of an attributes. You can however record identical provenance any number of times, as this does not cause contradiction, or change the state of Chronicle meaningfully.

### Contradiction of attribute time

Here we attempt to change the end date of an activity after it has been recorded - one of these mutations will fail

``` graphql
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

Here we attempt to change the value of an attribute - one of these operations will fail.

``` graphql
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

Where a provenance term has been recorded with attributes, and the domain has been extended to include new attributes it is valid to append new attributes, as long as the values of the already recorded ones are unchanged.

``` graphql

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

For this section we will use a highly simplified domain model for recording the provenance of evidenced documentation. We have a number of people who may be any combination of authors, editors or researchers collaborating to produce documents from evidence produced by a search process. An external CMS is being used that has identifiers for documents and users.

TODO!: Lift this up into another doc about provenance domain design


``` yaml
name: "documentation"
attributes:
    Name:
        type: String
    CmsId:
        type: String
    DOI:
        type: String
    Email:
        type: String
    Title:
        type: String
    SearchParameters:
        type: String
    Version:
        type: Integer
agents:
    Person:
        attributes:
            - CmsId
entities:
    Document:
        attributes:
            - Title
            - CmsId
            - Version
    Evidence:
        attributes:
            - DOI
            - Reference
activities:
    Search:
        attributes:
            - SearchParameters
    Authoring:
        attributes:
            - Version
    Publishing:
        attributes:
            - Version
roles:
    - Researcher
    - Author
    - Editor
```

### The `name` parameter

The definition mutations for `entities`, `activities` and `agents` are all supplied with a `name` parameter. This should be something meaningful to the domain you are recording provenance for - a unique identifier from an external system or a natural key.

### Graphql mutation result - `Submission`

All Chronicle mutations

### The commit notification subscription

The `correlationId` from the


### Define an Entity

Chronicle will have generated two entity subtypes for us, `Document` and `Evidence` as a graphql union called `Entity`. The definition mutations `document` and `evidence` will also have been created. See [domain modelling](./domain_modelling.md/#graphql_generation) for details on the generated graphql SDL.

The following example mutation `document` will define an `Entity` of subtype `Document`, along with its attributes.

``` graphql title="Define a document entity with graphql"
mutation {
    document(name: "evidence-summary-2313", attributes: {
        titleAttribute: "Evidence Summary",
        cmsIdAttribute: "2312",
        versionAttribute: 0
    })
}
```

And the equivalent operation using the command line interface is

``` bash title="Define a document entity with the CLI"
chronicle document define evidence-summary-2313 --title-attr "Evidence Summary" --cms-id-attr "2313" --version-attr 0

```


### Define an Activity

Chronicle will have generated three `Activity` subtypes for us, `Search`, `Authoring` and `Publishing` as a graphql union called `Activity`. The definition mutations `search` `authoring` and `publishing` will also have been created. See [domain modelling](./domain_modelling.md/#graphql_generation) for details on the generated graphql SDL.

The following example mutation `authoring` will define an `Activity` of subtype `Authoring`

``` graphql title="Define a document entity with graphql"
mutation {
    authoring(name: "september-2018-review", attributes: {
        versionAttribute: 14,
    })
}
```

And the equivalent operation using the command line interface is:

``` bash title="Define a document entity with the CLI"
chronicle authoring define september-2018-review --version-attr 14
```


### Define an Agent

Chronicle will have generated one `Agent` subtype for us, `Person` as a graphql union called `Agent`. The definition mutation `person` will also have been created. See [domain modelling](./domain_modelling.md/#graphql_generation) for details on the generated graphql SDL.

The following example mutation `person` will define an `Agent` of subtype `Person`, along with its attribute.

``` graphql title="Define a document entity with graphql"
mutation {
    document(name: "janet-jones-3212", attributes: {
        cmsIdAttribute: "3212",
    })
}
```

And the equivalent operation using the command line interface is:

``` bash title="Define a document entity with the CLI"
chronicle person define janet-jones-3212 --cms-id-attr "3212"
```
