use tracing::subscriber::set_global_default;
use tracing_log::{log::LevelFilter, LogTracer};
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{prelude::*, EnvFilter, Registry};
use url::Url;

pub fn telemetry(collector_endpoint: Option<Url>, console_logging: bool) {
    LogTracer::init_with_filter(LevelFilter::Trace).ok();

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    match (collector_endpoint, console_logging) {
        (Some(otel), true) => {
            let console = tracing_subscriber::fmt::layer()
                .with_level(true) // don't include levels in formatted output
                .with_target(true) // don't include targets
                .with_thread_ids(true) // include the thread ID of the current thread
                .pretty();
            let otel = OpenTelemetryLayer::new(
                opentelemetry_jaeger::new_pipeline()
                    .with_service_name("chronicle_api")
                    .with_collector_endpoint(otel.as_str())
                    .install_batch(opentelemetry::runtime::Tokio)
                    .unwrap(),
            );

            set_global_default(
                Registry::default()
                    .with(env_filter)
                    .with(otel)
                    .with(console),
            )
            .ok();
        }
        (None, true) => {
            let console = tracing_subscriber::fmt::layer()
                .with_level(true) // don't include levels in formatted output
                .with_target(true) // don't include targets
                .with_thread_ids(true) // include the thread ID of the current thread
                .pretty();
            set_global_default(Registry::default().with(env_filter).with(console)).ok();
        }
        (Some(otel), false) => {
            let otel = OpenTelemetryLayer::new(
                opentelemetry_jaeger::new_pipeline()
                    .with_service_name("chronicle_api")
                    .with_collector_endpoint(otel.as_str())
                    .install_batch(opentelemetry::runtime::Tokio)
                    .unwrap(),
            );
            set_global_default(Registry::default().with(env_filter).with(otel)).ok();
        }
        _ => (),
    }
}
