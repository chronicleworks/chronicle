use std::net::SocketAddr;

use opentelemetry_otlp::WithExportConfig;
use tracing::subscriber::set_global_default;
use tracing_flame::FlameLayer;
use tracing_log::{log::LevelFilter, LogTracer};

use tracing_subscriber::{prelude::*, EnvFilter, Registry};

#[derive(Debug, Clone, Copy)]
pub enum ConsoleLogging {
	Off,
	Pretty,
	Json,
}

#[cfg(feature = "tokio-tracing")]
macro_rules! console_layer {
	() => {
		console_subscriber::ConsoleLayer::builder().with_default_env().spawn()
	};
}

macro_rules! stdio_layer {
	() => {
		tracing_subscriber::fmt::layer()
			.with_level(true)
			.with_target(true)
			.with_thread_ids(true)
	};
}

macro_rules! oltp_exporter_layer {
	( $address: expr ) => {{
		let tracer = opentelemetry_otlp::new_pipeline()
			.tracing()
			.with_exporter(
				opentelemetry_otlp::new_exporter().tonic().with_endpoint($address.to_string()),
			)
			.install_simple()
			.expect("Failed to install OpenTelemetry tracer");

		tracing_opentelemetry::OpenTelemetryLayer::new(tracer)
	}};
}

pub struct OptionalDrop<T> {
	inner: Option<T>,
}

impl<T> OptionalDrop<T> {
	pub fn new(inner: T) -> Self {
		Self { inner: Some(inner) }
	}
}

impl<T> Drop for OptionalDrop<T> {
	fn drop(&mut self) {
		self.inner.take();
	}
}

pub fn telemetry(
	collector_endpoint: Option<SocketAddr>,
	console_logging: ConsoleLogging,
) -> impl Drop {
	full_telemetry(collector_endpoint, None, console_logging)
}

pub fn full_telemetry(
	exporter_port: Option<SocketAddr>,
	flame_file: Option<&str>,
	console_logging: ConsoleLogging,
) -> impl Drop {
	let (flame_layer, guard) = flame_file
		.map(|path| {
			let (flame_layer, guard) = FlameLayer::with_file(path).unwrap();
			(Some(flame_layer), Some(guard))
		})
		.unwrap_or((None, None));

	LogTracer::init_with_filter(LevelFilter::Trace).ok();

	let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("error"));
	match (exporter_port, flame_layer, console_logging) {
		(Some(otel), Some(flame_layer), ConsoleLogging::Json) => set_global_default(
			Registry::default()
				.with(env_filter)
				.with(flame_layer)
				.with(stdio_layer!().json())
				.with(oltp_exporter_layer!(otel)),
		),

		(Some(otel), Some(flame_layer), ConsoleLogging::Pretty) => set_global_default(
			Registry::default()
				.with(env_filter)
				.with(flame_layer)
				.with(stdio_layer!().pretty())
				.with(oltp_exporter_layer!(otel)),
		),
		(Some(otel), Some(flame_layer), ConsoleLogging::Off) => set_global_default(
			Registry::default()
				.with(env_filter)
				.with(flame_layer)
				.with(oltp_exporter_layer!(otel)),
		),
		(None, Some(flame_layer), ConsoleLogging::Json) => set_global_default(
			Registry::default()
				.with(env_filter)
				.with(flame_layer)
				.with(stdio_layer!().json()),
		),
		(None, Some(flame_layer), ConsoleLogging::Pretty) => {
			cfg_if::cfg_if! {
			  if #[cfg(feature = "tokio-tracing")] {
				set_global_default(Registry::default()
				  .with(env_filter)
				  .with(flame_layer)
				  .with(stdio_layer!().pretty())
				  .with(console_layer!()))
			  } else {
				set_global_default(Registry::default()
				  .with(env_filter)
				  .with(flame_layer)
				  .with(stdio_layer!().pretty())
				)
			  }
			}
		},
		(Some(otel), None, ConsoleLogging::Json) => set_global_default(
			Registry::default()
				.with(env_filter)
				.with(stdio_layer!().json())
				.with(oltp_exporter_layer!(otel)),
		),
		(Some(otel), None, ConsoleLogging::Pretty) => set_global_default(
			Registry::default()
				.with(env_filter)
				.with(stdio_layer!().pretty())
				.with(oltp_exporter_layer!(otel)),
		),
		(Some(otel), None, ConsoleLogging::Off) => {
			let otel_layer = oltp_exporter_layer!(otel);
			set_global_default(Registry::default().with(env_filter).with(otel_layer))
		},
		(None, None, ConsoleLogging::Json) =>
			set_global_default(Registry::default().with(env_filter).with(stdio_layer!().json())),
		(None, None, ConsoleLogging::Pretty) => {
			cfg_if::cfg_if! {
			  if #[cfg(feature = "tokio-tracing")] {
				set_global_default(Registry::default()
				  .with(env_filter)
				  .with(stdio_layer!().pretty())
				  .with(console_layer!()))
			  } else {
				set_global_default(Registry::default()
				  .with(env_filter)
				  .with(stdio_layer!().pretty())
				)
			  }
			}
		},
		_ => set_global_default(Registry::default().with(env_filter)),
	}
	.map_err(|e| eprintln!("Failed to set global default subscriber: {:?}", e))
	.ok();

	OptionalDrop::new(guard)
}
