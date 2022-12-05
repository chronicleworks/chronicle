# Untyped Chronicle

Chronicle can be used without a domain, for scenarios where you do not need
attributes. `chronicle-untyped` is distributed for this purpose.

Except for the lack of attributes and roles, all Chronicle functions are
available - including domain types. Care must be taken with the use of domain
types when using `chronicle-untyped` as they are not checked.

## Creating an Activity, Agent, and Entity in Untyped Chronicle

The untyped creation mutations `defineAgent`, `defineActivity`, and `defineEntity`
can be used to create provenance terms. Mutations that produce provenance
relations work in the same way as typed Chronicle.

```graphql
mutation untypedActivity {
  defineActivity(externalId: "activity", attributes: {type: "attribute"}) {
    context
  }
}

mutation untypedAgent {
  defineAgent(externalId: "agent", attributes: {type: "attribute"}) {
    context
  }
}

mutation untypedEntity {
  defineEntity(externalId: "entity", attributes: {type: "attribute"}) {
    context
  }
}
```

## Querying Untyped Chronicle

The activity timeline can be queried in the same manner as typed chronicle, with
the omission of type filters. Returned prov terms will be of the 'unknown' types
`ProvAgent`, `ProvActivity` or `ProvEntity`. If domain types have been set, then
they will appear on the corresponding type field.

```graphql
query {
    activityTimeline(forEntity: [{id: "chronicle:entity:example"}],
                    from: "1968-01-01T00:00:00Z",
                    to: "2030-01-01T00:00:00Z",
                    activityTypes: [],
                    ) {
        edges {
            node {
                ... on ProvActivity {
                    id
                    externalId
                    type
                    wasAssociatedWith {
                            responsible {
                                agent {
                                    ... on ProvAgent {
                                        id
                                        externalId
                                        type
                                    }
                                }
                                role
                            }
                    }
                    used {
                        ... on ProvEntity {
                            id
                            externalId
                            type
                        }
                    }
                }
           }
        }
    }
}
```
