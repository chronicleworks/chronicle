# `entityById`

## Examples

An entity could be defined like so:

```graphql
mutation {
  defineItemEntity(
      externalId: "externalid",
      attributes: { partIdAttribute: "432" }
  ) {
      context
  }
}
```

A user could query that activity in the following way:

```graphql
query {
  entityById(id: {externalId: "externalid" }) {
    ... on ItemEntity {
      id
      externalId
      partIdAttribute
    }
  }
}
```
