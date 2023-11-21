# Authentication and Authorization Scenarios Enumerated for Helm

## Introduction

The discussion of [JWKS and OPA in Helm](./helm-jwks-opa.md) outlines various
possibilities and invites the user to draw elements from different sections in
composing their `values.yaml` file. With less contextual information, here we
enumerate various useful kinds of Helm chart in full:

### OIDC and OPA: authentication and access control

- OIDC establishes a user's identity and provides Chronicle with information
  relating to the user's roles or authorization, perhaps via OAuth scopes.
- Chronicle uses OPA to consider if the user's OIDC-verified identity and
  roles allow them to perform the requested operation.

[See discussion.](./helm-jwks-opa.md#oidc-and-opa-authentication-and-access-control)

```yaml
devIdProvider:
  enabled: false

auth:
  required: true
  jwks:
    url: https://id.example.com/.well-known/jwks.json
  userinfo:
    url: https://id.example.com/userinfo
  id:
    claims: iss sub

opa:
  policy:
    id: my_policy
    entrypoint: my_policy.is_permitted
    url: https://example.com/my-opa-bundle.tar.gz
```

One variant of the above is when a JWT provided by the user already contains
all the claims that are needed. In this case, the `userinfo:` section may be
omitted. Similarly, if the claims determining Chronicle identity are `iss sub`
then `id:` can also be omitted as those are the default. To also allow
anonymous requests, without an `Authorization:` header, also omit `required:`.

**Note**: By default, if `auth.userinfo.url` is provided, `test.auth.token` is
required. To learn more about testing with Helm and default settings, see
this [Note on Default Settings](./helm-testing.md#note-on-default-settings).

### OIDC but not OPA: allow everything, recording identity

Chronicle can record who performed transactions while permitting them all:

```yaml
devIdProvider:
  enabled: false

auth:
  required: true
  jwks:
    url: https://id.example.com/.well-known/jwks.json
  userinfo:
    url: https://id.example.com/userinfo
  id:
    claims: iss sub

opa:
  enabled: false
```

As [above](#oidc-and-opa-authentication-and-access-control), one may commonly
omit `userinfo:` and/or `id:`, perhaps `required:`.
[See discussion.](./helm-jwks-opa.md#oidc-but-not-opa-allow-everything-recording-identity)

### OPA but not OIDC: universally restrict kinds of requests

Chronicle can enforce access control policies based on what the request is
regardless of who the requesting user is:

```yaml
devIdProvider:
  enabled: false

auth:
  required: false

opa:
  policy:
    id: my_policy
    entrypoint: my_policy.is_permitted
    url: https://example.com/my-opa-bundle.tar.gz
```

[See discussion.](./helm-jwks-opa.md#opa-but-not-oidc-universally-restrict-kinds-of-requests)

### Neither OIDC nor OPA: any controls are wholly external

If access to Chronicle's API does not need to be controlled by Chronicle
itself, nor does the identity of requesting users need to be recorded with
transactions, both OIDC and OPA may be disabled for the Helm installation:

```yaml
devIdProvider:
  enabled: false

auth:
  required: false

opa:
  enabled: false
```

[See discussion.](./helm-jwks-opa.md#neither-oidc-nor-opa-any-controls-are-wholly-external)

### Mock OIDC Server

Chronicle provides a mock OIDC server that can be used for simple testing.

```yaml
devIdProvider:
  enabled: true

auth:
  enabled: true

opa:
  policy:
    id: my_policy
    entrypoint: my_policy.is_permitted
    url: https://example.com/my-opa-bundle.tar.gz
```

In this configuration, Chronicle uses a mock OIDC server.
[See discussion.](./helm-jwks-opa.md#oidc-testing)

You may instead choose to disable OPA with,

```yaml
opa:
  enabled: false
```

For more on Chronicle Helm testing scenarios, see our documentation on
[Helm Testing Scenarios](./helm-testing.md#testing-scenarios).
