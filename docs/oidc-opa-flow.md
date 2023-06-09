# How OIDC and OPA Affect Request Processing

The authorization checks for a request that comes in to Chronicle's API follow
this flow:

![file](diagrams/out/oidc-process.svg)

Sub-flows referenced in the above are for verifying JSON Web Tokens,

![file](diagrams/out/oidc-process-jwks.svg)

and for obtaining information on the requesting user,

![file](diagrams/out/oidc-process-userinfo.svg)
