use tracing::subscriber::set_global_default;
use tracing_log::LogTracer;
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, EnvFilter, Registry};
use url::Url;

pub fn telemetry(collector_endpoint: Url) {
    LogTracer::init().expect("Failed to set logger");

    let tracer = opentelemetry_jaeger::new_pipeline()
        .with_service_name("chronicle_api")
        .with_collector_endpoint(collector_endpoint.as_str())
        .install_batch(opentelemetry::runtime::Tokio)
        .unwrap();

    let opentelemetry = tracing_opentelemetry::layer().with_tracer(tracer);
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let collector = Registry::default().with(env_filter).with(opentelemetry);

    set_global_default(collector).expect("Failed to set collector");
}
