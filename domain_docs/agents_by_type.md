# `agentsByType`

## Examples

An agent could be defined like so:

```graphql
mutation {
  defineContractorAgent(
      externalId: "contractor1",
      attributes: { locationAttribute: "Shenzhen" }
  ) {
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
