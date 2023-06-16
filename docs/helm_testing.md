# Helm Testing

This section provides an overview and instructions for testing the Chronicle application
using Helm. The Chronicle Helm Chart includes relevant components and configurations
related to testing the application.

## Introduction

Helm is a package manager for Kubernetes that enables streamlined deployment, management,
and testing of applications. The testing functionality in Helm allows you to verify
the correctness and stability of your application after deployment.

To learn `helm test`, see the [Chart Tests](https://helm.sh/docs/topics/chart_tests/)
and [Helm Test](https://helm.sh/docs/helm/helm_test/) sections of the Helm documentation.

The testing components and configurations specific to testing with the Chronicle
Helm Chart are described below.

## Testing Components

### API Test Job

The API Test Job is responsible for executing a test against the Chronicle API. It
ensures the availability GraphQL API endpoint. The Job is defined in the Helm Chart
under the condition:

```yaml
test:
  api:
    enabled: true
```

The API Test Job includes the following test steps:

- Authenticate and obtain a token (if required).
- Wait for the API to be ready.
- Execute the tests using the `subscribe-submit-test` script.

### Auth Endpoints Test Job

The Auth Endpoints Test Job verifies the availability and correctness of the
authentication endpoints used in the Chronicle application. The Job checks the
JWKS endpoint and userinfo endpoint, either using the provided URLs or the
`devIdProvider` if enabled. The `Job` is defined in the Helm Chart under the
condition:

```yaml
test:
  auth:
    enabled: true
```

The Auth Endpoints Test Job includes the following test steps:

- Checks the JWKS endpoint for a valid JSON response.
- Checks the userinfo endpoint for a valid JSON response, using the provided
  `test.auth.token` or the token obtained from the `devIdProvider`.

### `devIdProvider` (optional)

The `devIdProvider` is an optional component used for authentication during
testing and development. It simulates an identity provider to provide tokens for
API and auth endpoint tests. The `devIdProvider` is defined in the Helm Chart
under the condition:

```yaml
devIdProvider:
  enabled: true
```

The following resources are created for the `devIdProvider`:

- `Service`: Provides network access to the `devIdProvider`.
- `StatefulSet`: Manages the stateful container that makes up the `devIdProvider`.

### RBAC Configuration

The RBAC (Role-Based Access Control) configuration allows the necessary permissions
for testing. It grants the required access rights to the Service Account used during
testing.

## Configuration

The relevant values for testing are located under the `test` section in the `values.yaml`
file. Specifically:

- `auth.required`: Specifies whether Chronicle's API will require authentication.
  If set to `true`, either `devIdProvider` must be enabled, or the user must provide
  `test.auth.token`.
- `test.api.enabled`: Specifies whether the API test functionality is enabled (`true`)
  or not (`false`).
- `test.auth.enabled`: Specifies whether the Auth Endpoints test functionality is
  enabled (`true`) or not (`false`).
- `test.auth.token`: Provides a token that can be used for authentication-related
  testing. This value can be set to a specific token for testing authentication scenarios.
- `devIdProvider.enabled`: Specifies whether the `devIdProvider` is
  enabled (`true`) or not (`false`).
- `auth.jwks.url`: Specifies the URL of the JWKS endpoint for third-party authentication.
- `auth.userinfo.url`: Specifies the URL of the userinfo endpoint for third-party
  authentication.

```yaml
test:
  api:
    enabled: true
  auth:
    enabled: true

auth:
  required: true
  jwks:
    url:
  userinfo:
    url:

devIdProvider:
  enabled: true
```

## Testing Scenarios

### Without Auth

```yaml
auth:
  required: false

test:
  api:
    enabled: true
  auth:
    enabled: true
```

These are Chronicle's default `values.yaml` settings. Running `helm test <installation>`
will run the API test and Auth Endpoints test without using an authorization token.

### Auth Required, Using `devIdProvider`

```yaml
auth:
  jwks:
    url:
  required: true
  userinfo:
    url:

devIdProvider:
  enabled: true

test:
  api:
    enabled: true
  auth:
    enabled: true
  auth:
    token:
```

The test uses the `devIdProvider` to acquire a token, which it passes in the
authorization header to both the API test and Auth Endpoints test. Chronicle has
been initialized with the default `devIdProvider` auth endpoints. If `test.auth.token`
is not provided and `auth.required: true`, then `devIdProvider` must be enabled.

### Auth Required, Third Party Auth Service

```yaml
auth:
  jwks:
    url: https://dev-xyz-9abc.us.auth0.com/.well-known/jwks.json
  required: true
  userinfo:
    url: https://dev-xyz-9abc.us.auth0.com/userinfo

devIdProvider:
  enabled: false

test:
  api:
    enabled: true
  auth:
    enabled: true
  auth:
    token: eyJhbGciOiJkaXIiLCJlbmMiOiJBMjU2R0NNIiwiaXNzIjoiaHR0cHM6Ly9kZXYtY2hhLTlhZXQudXMuYXV0aDAuY29tLyJ9..wSM9N-paE7lA_YaL.T8Mla-PJ5VcFWdBX6SaxCkzq5LVFnEGg2eiMNc-rCXgCd6CUTFQ9Ra_JbuFZrfVZA0JxaaeY5XHJYVBJ6Gwjq25qU5XxXrXk64ZdHNIBgUYhkHoKOvEIjqYpvv8pl1A4MndAbE8NqFpyYgkaWVhSk0X9zSMWTZ6D_Y4lwMr4ihCNqJ4nd8KuyswwDYrHCnQbmBDE6u0yGmLQEIoLm1ZaCnhgDTzdnX2RgcluOrZR5a-yW8Vw6VogsGHwh6-2gsDHxgdmjpZlfR0jGHkceeCw9xl-ccVaLmTH2DS49nrhiYBfrx8oZ5dTKdj9d0ZWJ91c4CI.beiznku1urlYppbo8WHoCg
```

The user provides a token and auth endpoints. Note that in this scenario, it does
not matter whether the devIdProvider is enabled or not, but testing requires that
the user provides a token that will work with their third-party auth service.
