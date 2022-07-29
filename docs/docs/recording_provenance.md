# Recording Provenance using Chronicle

## Immutability

Chronicle provenance is immutable. Once entered you cannot contradict it, only add additional provenance.

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

``` graphql title="Not a contradiction - "
    mutation {
        endActivity(id:"")
    }
```

