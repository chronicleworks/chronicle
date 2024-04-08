use tracing::subscriber::set_global_default;
use tracing_elastic_apm::config::Config;
use tracing_flame::FlameLayer;
use tracing_log::{log::LevelFilter, LogTracer};
use tracing_subscriber::{prelude::*, EnvFilter, Registry};
use url::Url;

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

macro_rules! apm_layer {
	( $address: expr ) => {
		tracing_elastic_apm::new_layer(
			"chronicle".to_string(),
			// remember to use desired protocol below, e.g. http://
			Config::new($address.to_string()),
		)
		.unwrap()
	};
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

pub fn telemetry(collector_endpoint: Option<Url>, console_logging: ConsoleLogging) -> impl Drop {
	full_telemetry(collector_endpoint, None, console_logging)
}

pub fn full_telemetry(
	collector_endpoint: Option<Url>,
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
	match (collector_endpoint, flame_layer, console_logging) {
		(Some(otel), Some(flame_layer), ConsoleLogging::Json) => set_global_default(
			Registry::default()
				.with(env_filter)
				.with(flame_layer)
				.with(apm_layer!(otel))
				.with(stdio_layer!().json()),
		),
		(Some(otel), Some(flame_layer), ConsoleLogging::Pretty) => set_global_default(
			Registry::default()
				.with(env_filter)
				.with(flame_layer)
				.with(stdio_layer!().pretty())
				.with(apm_layer!(otel.as_str())),
		),
		(Some(otel), Some(flame_layer), ConsoleLogging::Off) => set_global_default(
			Registry::default()
				.with(env_filter)
				.with(flame_layer)
				.with(apm_layer!(otel.as_str())),
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
				.with(apm_layer!(otel))
				.with(stdio_layer!().json()),
		),
		(Some(otel), None, ConsoleLogging::Pretty) => set_global_default(
			Registry::default()
				.with(env_filter)
				.with(stdio_layer!().pretty())
				.with(apm_layer!(otel.as_str())),
		),
		(Some(otel), None, ConsoleLogging::Off) =>
			set_global_default(Registry::default().with(env_filter).with(apm_layer!(otel.as_str()))),
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
