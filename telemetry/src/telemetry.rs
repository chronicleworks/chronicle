use tracing::subscriber::set_global_default;
use tracing::Level;
use tracing_log::{log::LevelFilter, LogTracer};
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, EnvFilter, Registry};
use url::Url;

pub fn telemetry(collector_endpoint: Url) {
    LogTracer::init_with_filter(LevelFilter::Trace).ok();

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

pub fn console_logging_info() {
    LogTracer::init_with_filter(LevelFilter::Info).ok();
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .pretty()
        .try_init()
        .ok();
}

pub fn console_logging_trace() {
    LogTracer::init_with_filter(LevelFilter::Trace).ok();
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .pretty()
        .with_max_level(Level::TRACE)
        .try_init()
        .ok();
}
