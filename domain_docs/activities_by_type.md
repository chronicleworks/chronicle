# `activitiesByType`

## Examples

An activity could be defined like so:

```graphql
mutation {
  defineItemCertifiedActivity(
      externalId: "certification1",
      attributes: { certIdAttribute: "123" }
  ) {
      context
  }
}
```

A user could query all activities of that type as in the following example:

```graphql
query {
  activitiesByType(activityType: ItemCertifiedActivity) {
    nodes {
      ...on ItemCertifiedActivity {
        id
        certIdAttribute
      }
    }
  }
}
```
