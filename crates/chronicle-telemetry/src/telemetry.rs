
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
	() => {{
		let tracer = opentelemetry_otlp::new_pipeline()
			.tracing()
			.with_exporter(
				opentelemetry_otlp::new_exporter().tonic().with_env(),
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
	otel_enable: bool,
	console_logging: ConsoleLogging,
) -> impl Drop {
	full_telemetry(otel_enable, None, console_logging)
}

pub fn full_telemetry(
	otel_export: bool,
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
	match (otel_export, flame_layer, console_logging) {
		(true, Some(flame_layer), ConsoleLogging::Json) => set_global_default(
			Registry::default()
				.with(env_filter)
				.with(flame_layer)
				.with(stdio_layer!().json())
				.with(oltp_exporter_layer!()),
		),

		(true, Some(flame_layer), ConsoleLogging::Pretty) => set_global_default(
			Registry::default()
				.with(env_filter)
				.with(flame_layer)
				.with(stdio_layer!().pretty())
				.with(oltp_exporter_layer!()),
		),
		(true, Some(flame_layer), ConsoleLogging::Off) => set_global_default(
			Registry::default()
				.with(env_filter)
				.with(flame_layer)
				.with(oltp_exporter_layer!()),
		),
		(false, Some(flame_layer), ConsoleLogging::Json) => set_global_default(
			Registry::default()
				.with(env_filter)
				.with(flame_layer)
				.with(stdio_layer!().json()),
		),
		(false, Some(flame_layer), ConsoleLogging::Pretty) => {
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
		(true, None, ConsoleLogging::Json) => set_global_default(
			Registry::default()
				.with(env_filter)
				.with(stdio_layer!().json())
				.with(oltp_exporter_layer!()),
		),
		(true, None, ConsoleLogging::Pretty) => set_global_default(
			Registry::default()
				.with(env_filter)
				.with(stdio_layer!().pretty())
				.with(oltp_exporter_layer!()),
		),
		(true, None, ConsoleLogging::Off) => {
			let otel_layer = oltp_exporter_layer!();
			set_global_default(Registry::default().with(env_filter).with(otel_layer))
		},
		(false, None, ConsoleLogging::Json) =>
			set_global_default(Registry::default().with(env_filter).with(stdio_layer!().json())),
		(false, None, ConsoleLogging::Pretty) => {
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
