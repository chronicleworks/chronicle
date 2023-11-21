# Health Checks and Testing

## Helm Testing

See [Helm Testing](./helm-testing.md).

## Liveness Health Check

When the `--liveness-check` option is used, Chronicle enables liveness
depth charge checks to ensure system availability. It starts a background
task that periodically executes a transaction to Chronicle's
[System Namespace](./namespaces.md#chronicle-system). At each interval,
the depth charge operation is executed and timed. If the transaction is
successfully committed, the elapsed time of the round trip is calculated
and made available as a histogram metric.

The liveness check task installs a [Prometheus](https://prometheus.io/)
exporter to record metrics. If the transaction is successfully committed,
the elapsed time of the round trip is calculated and recorded as a
histogram metric in the Prometheus exporter. The collected data can be
scraped from the exposed endpoint for analysis and monitoring purposes at
`127.0.0.1:9000/metrics` or simply `127.0.0.1:9000`.

### CLI

See the section in our CLI docs on the
[Liveness Health Check](./cli.md#liveness-health-check).

### Helm

See our Helm Options documentation on the
[Liveness Health Check](./helm-options.md#liveness-health-check).
