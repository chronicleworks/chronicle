# `entitiesByType`

## Examples

An entity could be defined like so:

```graphql
mutation {
  defineCertificateEntity(
      externalId: "testentity1",
      attributes: { certIdAttribute: "something" }
  ) {
      context
  }
}
```

A user could query all entities of that type as in the following example:

```graphql
query {
  entitiesByType(entityType: CertificateEntity) {
    nodes {
      ...on CertificateEntity {
        id
      }
    }
  }
}
```
