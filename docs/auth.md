# Establishing Identity with OIDC and JWT

## API Endpoints Support Bearer Tokens

Chronicle's API listens on HTTP endpoints that accept requests. These may
include an `Authorization:` header that provides a bearer token. Chronicle
will attempt to verify the token as a basis for establishing the user's
Chronicle identity. The API is configured by various options of the
[serve-api](./cli.md#serve-api) subcommand.

The requestor's identity is provided to the OPA evaluator for deciding whether
to process the request or to refuse because it violates the policy rules.
[OPA policies](./opa.md) should be set up accordingly.

## Constructing Chronicle Identity from Bearer Token Claims

The signature of JSON web tokens must be verifiable against a JSON Web Key
Set available from the
[configured JWKS address URI](./cli.md#jwks-address-uri). If a
[userinfo address](./cli.md#userinfo-address) is configured then provided
tokens must be accepted by that endpoint. If either test fails then the
request is refused. Any JWKS or userinfo endpoint provided must be
[OIDC-compliant](https://openid.net/specs/openid-connect-core-1_0.html).

If a bearer token is provided, it may be an access token or an ID token.
Access tokens may be opaque but, if so, a userinfo endpoint must be configured
because Chronicle must be able to extract claims for providing to the OPA
evaluator. These claims may be from,

- a JSON Web Token directly provided as `Authorization:` and/or
- a JSON Web Token or a JSON object provided by the userinfo endpoint in
  response to a token provided as `Authorization:`.

In the case of a JSON Web Token being provided by the user, and the userinfo
endpoint providing further information, the claims from both sources are
combined. This can be a means of including both OAuth scope (from an access
token) and user identity (from the user info endpoint) among the resulting
claims.

For constructing the Chronicle identity from the claims,
[configured field names](./cli.md#id-claims-jwt-field-names) identify the
claims whose values should be used in deriving the external ID of the
Chronicle user who submitted the request.

Omitting the `Authorization:` header means that the user is regarded as
being anonymous and has no accompanying claims. Such requests may be
wholly disallowed by [requiring authentication](./cli.md#require-auth).
Anonymous users may also be disallowed by
[policy rules](./opa.md#authorization-rules-in-opa) set with OPA.

## Illustrative Example

For example, with the specifics depending on the behavior and configuration
settings of your authorization server,

```shell
chronicle serve-api \
  --jwks-address https://auth.example.com/.well-known/jwks.json \
  --userinfo-address https://auth.example.com/userinfo \
  --id-claims nickname \
  --require-auth \
  --jwt-must-claim iss https://auth.example.com/ \
  --jwt-must-claim aud https://auth.example.com/chronicle-api/ \
  --jwt-must-claim azp chronicle-application-id
```

This,

- requires bearer tokens to be presented with API requests
- uses the fetched `jwks.json` to verify JWT signatures
- uses the `userinfo` endpoint to obtain claims associated with the token
- uses value of the `"nickname"` field among the claims to derive the external
  ID of the authenticated Chronicle user
- rejects any requests that do not include an `Authorization:` header with a
  bearer token
- rejects any tokens that lead to JWT claims that do not include those
  specified.

That last constraint relating to the `iss`, `aud`, `azp` claims ensures that
the presented token's claims were asserted by the expected authorization
server and are intended for Chronicle to use. Those claims may also be checked
by OPA [policy rules](./opa.md#authorization-rules-in-opa).

Note that the various URIs, etc. in this example can all be set in environment
variables instead. This can make deployments easier to manage. See the
`--help` text for details. For instance, the first option can be set with
`JWKS_URI=https://auth.example.com/.well-known/jwks.json` and the last line
with `JWT_MUST_CLAIM_AZP=chronicle-application-id`. Note that the latter's
value would be chosen in the settings by which the Chronicle application is
registered in the authorization server's configuration.
