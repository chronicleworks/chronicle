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

## Health Metrics

Chronicle can enable liveness depth charge checks to ensure system availability.

The following options in the Chronicle Helm Chart `values.yaml` file can be used
to enable and configure the health check:

```yaml
healthMetrics:
  enabled: true
  # Interval in seconds
  interval: 1800
```

If enabled, these metrics will be available by default at `0.0.0.0:9982/metrics`.

For more on how the [Health Metrics](#health-metrics) work, see
[Health Checks and Testing](./health-checks-and-testing#health-metrics).
