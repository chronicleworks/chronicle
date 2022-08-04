# Recording Provenance using Chronicle

## Immutability

Chronicle provenance is immutable. Once recorded you cannot contradict it - only record additional provenance.

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

## Contradiction

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

For this section we will use a our simplified [domain model for recording the provenance of medical guidance](./domain_modelling.md). We have a number of people who may be any combination of authors, editors or researchers collaborating to produce documents from evidence produced by a search process. An external CMS is being used that has identifiers for documents and users.

### The `name` parameter

The definition mutations for prov terms -  Entity, Activity and Agent are all supplied with a `name` parameter. This should be something meaningful to the domain you are recording provenance for - a unique identifier from an external system or a natural key. This will form part of the identity of the term.

### Graphql mutation result - `Submission`

All Chronicle mutations return a `Submission` type defined by:

``` graphql
type Submission {
	context: String!
	correlationId: String!
}
```

The `context` property here will be the identity of the Chronicle term you have changed in the mutation. i.e calling `agent(..)` will return the identity of the agent, calling `startActivity(..)` the identity of the started activity.

The `correlationId` property corresponds to the transaction id when running chronicle with a backend ledger, or a randomly generated uuid when used in [in memory](./chronicle_architecture.md/#development) mode.

### Commit notification subscriptions

Chronicle provides a [graphql subscription](https://graphql.org/blog/subscriptions-in-graphql-and-relay/) to notify clients when a chronicle operation has completed.

``` graphql

subscription {
    commitNotifications {
        correlationId
    }
}


```

The `correlationId` on this subscription will match the correlationId from the [Submission](#graphql-mutation-result---submission). Clients that wish to know the result of an operation should await both the [Submission](#graphql-mutation-result---submission) and the corresponding correlation id from a commit notification.


### Define an Entity

> In PROV, things we want to describe the provenance of are called entities and have some fixed aspects. The term "things" encompasses a broad diversity of notions, including digital objects such as a file or web page, physical things such as a mountain, a building, a printed book, or a car as well as abstract concepts and ideas.
> An entity is a physical, digital, conceptual, or other kind of thing with some fixed aspects; entities may be real or imaginary.

See [provenance concepts](./provenance_concepts.md#entity)


Chronicle will have generated two entity subtypes for us, `Document` and `Evidence` as a graphql union called `Entity`. The definition mutations `document` and `evidence` will also have been created. See [domain modelling](./domain_modelling.md/#graphql_generation) for details on the generated graphql SDL.

The following example mutation `question` will define an `Entity` of subtype `Question`, along with its attributes.

``` graphql title="Define a Question entity with graphql"
mutation {
    document(name: "", attributes: {
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
