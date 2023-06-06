# Authentication, Authorization, and Helm

## Choices of Operating Mode

### Overview

#### OIDC

Chronicle can require that all HTTP requests to its API are accompanied
by an `Authorization:` header with a bearer token that can be verified
with an OIDC-compliant identity provider. The token authenticates the
requesting user and may be a JWT that includes information about the user's
roles or permissions.

Alternatively, if authorization is not enabled then incoming requests are
regarded as being anonymous. Any user- or role-based access control must be
enforced by other means of securing access to Chronicle's API because
Chronicle itself would have no basis for enforcing such.

Chronicle's use of OIDC's JWKS and userinfo endpoints, and the
identity claims fields, are described more fully in
[establishing user identity](./auth.md).

Chronicle records onto the blockchain the identity of the user who performed
each transaction, whether *anonymous* or based on their verified JWT.

#### OPA

Chronicle can evaluate an access control policy before executing requests.
The rules for such policies may depend on information borne in the token
presented from the user's OIDC-based authorization: their identity, roles,
etc. By evaluating the rules, the Open Policy Agent (OPA) may permit users to
perform operations, or forbid them from doing so.

Alternatively, OPA can be disabled, in which case Chronicle allows all
requests, and any access control must be enforced by other means.

Defining and configuring access policies with OPA is described more fully in
[controlling access](./opa.md), including how to create your own policy
bundle.

### OIDC and OPA: authentication and access control

When OIDC and OPA are both enabled, Chronicle is able to usefully control
access to its API:

- OIDC establishes a user's identity and provides Chronicle with information
  relating to the user's roles or authorization, perhaps via OAuth scopes.
- Chronicle uses OPA to consider if the user's OIDC-verified identity and
  roles allow them to perform the requested operation.

Not all installations benefit from integrating Chronicle with both user
authentication and access control. Such other configurations are discussed
later in this document.

A simplified yet illustrative outline of the data flow with OIDC and OPA is:

![file](diagrams/out/chronicle-access-control-seq.svg)

1. The `opactl` command is used to set an OPA policy.
1. The OPA transaction processor writes the policy to the blockchain.
1. When Chronicle starts up, it reads the OPA policy from the blockchain.
1. A user's client software has them log in via an OIDC server which provides
   a bearer token to that software via a callback.
1. The client software makes a request to Chronicle's API on behalf of the
   user, including the provided token in the HTTP `Authorization:` header.
1. Chronicle uses JWKS to verify the token issued by the OIDC server, and may
   further query the server for more information on the user.
1. The information about the user from the OIDC server is used as context
   for checking if the OPA policy permits the request that was submitted to
   Chronicle's API.

![file](diagrams/out/chronicle-access-control-arch.svg)

Chronicle thus allows API requests only if the user, as securely verified with
the OIDC server, is permitted that request according to the access control
policy securely recorded on the blockchain.

#### OIDC configuration

##### Identity provider endpoints

Define with Helm values, substituting for your identity provider's endpoints,
typically available from its configuration interface:

```yaml
auth:
  jwks:
    enabled: true
    url: https://id.example.com:80/.well-known/jwks.json
  userinfo:
    url: https://id.example.com:80/userinfo

```

Setting the JWKS URL is required if Chronicle may be provided a JWT with the
request's `Authorization:` or by the OIDC's userinfo endpoint, because it
requires JWKS to verify the JWT. The URL, and its `jwks:` section, may be
omitted only if the user provides an opaque access token from which the
userinfo endpoint then provides profile information as a plain, unsigned JSON
object, so neither is a JWT.

Setting the userinfo URL is required if Chronicle is to pass the bearer token
from the request's `Authorization:` to an OIDC userinfo endpoint. This
provides user profile information for OPA's policy engine to use in applying
the access rules. The URL, and its `userinfo:` section, may be omitted if the
provided token is a JWT that already includes all required claims.

The applicability of the above depends on the configuration of your OIDC
server, and the rules in your OPA access policy. At least one of the above
URLs must be set for Chronicle to accept a user's authorization.

##### Claims fields

Define with a Helm value:

```yaml
auth:
  id:
    claims: "iss sub"
```

The claims listed in this value name the fields that determine the user's
Chronicle identity. This setting can typically be omitted because the default
of `iss sub` often suffices but any fields are permitted that, in combination,
identify the user.

##### Further reading

See [establishing user identity](./auth.md) for more information.

#### OPA configuration

The rules for the OPA engine must target WASM and they must be bundled for
fetching when the blockchain is initialized with the access control policy.

The Helm values depend on how you have defined your access control policy.

```yaml
opa:
  policy:
    id: my_policy
    entrypoint: my_policy.is_permitted
    url: https://example.com/my-opa-bundle.tar.gz
```

With OIDC also enabled, it can be prudent to have one's Rego file require the
expected values for the `iss`, `aud`, `azp` claims. This ensures that the token
was issued by the appropriate identity provider for use by Chronicle. All
provided claims are made available to the policy engine for rule evaluation.

See [controlling access](./opa.md) for more information on creating and
bundling your own access control policy. Note the [use of `opactl set-policy`
and `sawset`](./opa.md#configuring-chronicle-to-use-opa) to set the policy on
the blockchain for Chronicle to read. It is anticipated that, with an
appropriate policy, typical access control changes can then be effected by
managing users and roles in the OIDC server, without needing to change the
policy further.

### OIDC but not OPA: allow everything, recording identity

Chronicle can record who performed transactions while permitting them all.
For this, configure OIDC's `chronicle` and `auth` Helm values
[as above](#oidc-configuration) but disable OPA:

```yaml
opa:
  enabled: false
```

### OPA but not OIDC: universally restrict kinds of requests

Chronicle can enforce access control policies based on what the request is
regardless of who the requesting user is. Without OIDC, all requests to
Chronicle's API are regarded as being from the *anonymous* user. Therefore,
any access control can do no more than universally restrict the kinds of
request that are permitted.

For this, configure OIDC's `opa` Helm value [as above](#opa-configuration) but
disable OIDC:

```yaml
auth:
  required: false
  jwks:
    enabled: false
  userinfo:
    enabled: false
```

### Neither OIDC nor OPA: any controls are wholly external

If access to Chronicle's API does not need to be controlled by Chronicle
itself, nor does the identity of requesting users need to be recorded with
transactions, both OIDC and OPA may be disabled for the Helm installation:

```yaml
auth:
  required: false
  jwks:
    enabled: false
  userinfo:
    enabled: false

opa:
  enabled: false
```
