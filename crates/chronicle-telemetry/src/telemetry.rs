use opentelemetry::{
	global,
	logs::LogError,
	metrics::MetricsError,
	trace::{TraceContextExt, TraceError, Tracer, TracerProvider},
	Key, KeyValue,
};
use opentelemetry_appender_log::OpenTelemetryLogBridge;
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::{ExportConfig, WithExportConfig};
use opentelemetry_sdk::{runtime, trace as sdktrace, trace::Config, Resource};
use tracing::Level;
use tracing_subscriber::{fmt::format::FmtSpan, prelude::*, EnvFilter};

fn init_tracer_provider() -> Result<sdktrace::TracerProvider, TraceError> {
	opentelemetry_otlp::new_pipeline()
		.tracing()
		.with_exporter(opentelemetry_otlp::new_exporter().tonic())
		.with_trace_config(Config::default())
		.install_batch(runtime::Tokio)
}

fn init_metrics() -> Result<opentelemetry_sdk::metrics::SdkMeterProvider, MetricsError> {

	opentelemetry_otlp::new_pipeline()
		.metrics(runtime::Tokio)
		.with_exporter(opentelemetry_otlp::new_exporter().tonic())
		.build()
}

fn init_logs() -> Result<opentelemetry_sdk::logs::LoggerProvider, LogError> {
	opentelemetry_otlp::new_pipeline()
		.logging()
		.with_exporter(opentelemetry_otlp::new_exporter().tonic())
		.install_batch(runtime::Tokio)
}

#[derive(Debug, Clone, Copy)]
pub enum ConsoleLogging {
	Off,
	Pretty,
	Json,
}

pub fn telemetry(console_logging: ConsoleLogging) {
	let result = init_tracer_provider();
	assert!(result.is_ok(), "Init tracer failed with error: {:?}", result.err());
	let tracer_provider = result.unwrap();
	global::set_tracer_provider(tracer_provider.clone());

	let result = init_metrics();
	assert!(result.is_ok(), "Init metrics failed with error: {:?}", result.err());
	let meter_provider = result.unwrap();
	global::set_meter_provider(meter_provider.clone());

	// Initialize logs and save the logger_provider.
	let logger_provider = init_logs().unwrap();

   // Create a new OpenTelemetryTracingBridge using the above LoggerProvider.
	let trace_bridge = OpenTelemetryTracingBridge::new(&logger_provider);

	let filter = EnvFilter::from_default_env();

	let fmt_layer: Option<Box<dyn tracing_subscriber::Layer<_> + Send + Sync>> = match console_logging {
		ConsoleLogging::Json => Some(Box::new(tracing_subscriber::fmt::layer()
			.with_span_events(FmtSpan::ACTIVE)
			.compact()
			.json())),
		ConsoleLogging::Pretty => Some(Box::new(tracing_subscriber::fmt::layer()
			.with_span_events(FmtSpan::ACTIVE)
			.compact()
			.pretty())),
		ConsoleLogging::Off => None,
	};

	let registry = tracing_subscriber::registry()
		.with(filter)
		.with(trace_bridge);

	if let Some(layer) = fmt_layer {
		registry.with(layer).init();
	} else {
		registry.init();
	}
    log::info!("Log bridge to telemetry initialized");
    tracing::info!("Trace bridge to telemetry initialized");
    let _ = tracing::span!(Level::INFO, "Span telemetry initialized").enter();

}
