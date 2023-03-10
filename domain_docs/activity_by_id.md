# `activityById`

## Examples

An activity could be defined like so:

```graphql
  mutation {
    defineItemCertifiedActivity(
        externalId: "externalid",
        attributes: { certIdAttribute: "234" }
    ) {
        context
    }
  }
```

A user could query that activity in the following way:

```graphql
query {
  activityById(id: {externalId: "externalid" }) {
    ... on ItemCertifiedActivity {
      id
      externalId
      certIdAttribute
    }
  }
}
```
