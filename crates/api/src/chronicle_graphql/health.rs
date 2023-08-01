use metrics_exporter_prometheus::PrometheusHandle;

#[derive(Clone)]
pub struct Metrics(PrometheusHandle);

impl Metrics {
    pub fn new(handle: PrometheusHandle) -> Self {
        Self(handle)
    }
}

#[poem::async_trait]
impl poem::Endpoint for Metrics {
    type Output = poem::Response;

    async fn call(&self, _req: poem::Request) -> poem::Result<Self::Output> {
        Ok(self.0.render().into())
    }
}
