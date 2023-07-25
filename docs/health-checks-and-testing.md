# Health Checks and Testing

## Helm Testing

See [Helm Testing](./helm_testing).

## Health Metrics

When the `--enable-health-metrics` option is used, Chronicle enables liveness
depth charge checks to ensure system availability. It starts a background
task that periodically executes a transaction to Chronicle's
[System Namespace](./namespaces#chronicle-system). At each interval,
the depth charge operation is executed and timed. If the transaction is
successfully committed, the elapsed time of the round trip is calculated
and made available as a histogram metric.

The health metrics task installs a [Prometheus](https://prometheus.io/)
exporter to record metrics. If the transaction is successfully committed,
the elapsed time of the round trip is calculated and recorded as a
histogram metric in the Prometheus exporter. The collected data can be
scraped from the exposed endpoint for analysis and monitoring purposes, by
default at `127.0.0.1:9982/metrics`.

### CLI

See the section in our CLI docs on
[Health Metrics](./cli#health-metrics).

### Helm

See our Helm Options documentation on
[Health Metrics](./helm-options#health-metrics).
