# Other Chronicle Helm Options

## Endpoints

Chronicle can offer endpoints for `data` (at `/context` and `/data`) and
`graphql` (at `/` and `/ws`).

These are served by default and can be configured in the Chronicle Helm Chart
`values.yaml` with the following options:

```yaml
endpoints:
  data:
    enabled: true
  graphql:
    enabled: true
```

See [command line options](cli#offer-endpoints-name-name) for more information.

## Liveness Health Check

Chronicle can enable liveness depth charge checks to ensure system availability.

The following options in the Chronicle Helm Chart `values.yaml` file can be used
to enable and configure the health check:

```yaml
healthCheck:
  enabled: true
  # Interval in seconds
  interval: 1800
```

For more on how the [Liveness Health Check](#liveness-health-check) works, see
[Health Checks and Testing](./health-checks-and-testing.md#liveness-health-check).
