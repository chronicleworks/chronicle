# `agentById`

## Examples

An agent could be defined like so:

```graphql
mutation {
  defineContractorAgent(
      externalId: "externalId",
      attributes: { locationAttribute: "location" }
  ) {
      context
  }
}
```

A user could query that agent in the following way:

```graphql
query {
    agentById(id: {externalId: "externalid" }) {
    ... on ContractorAgent {
      id
      externalId
      locationAttribute
    }
  }
}
```
