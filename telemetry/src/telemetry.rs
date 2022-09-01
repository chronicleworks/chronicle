use tracing::{subscriber::set_global_default};
use tracing_elastic_apm::config::Config;
use tracing_log::{log::LevelFilter, LogTracer};
use tracing_subscriber::{
    prelude::*,
    EnvFilter, Registry,
};
use url::Url;

#[derive(Debug, Clone, Copy)]
pub enum ConsoleLogging {
    Off,
    Pretty,
    Json,
}

macro_rules! console_layer {
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

pub fn telemetry(collector_endpoint: Option<Url>, console_logging: ConsoleLogging) {
    LogTracer::init_with_filter(LevelFilter::Trace).ok();

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    match (collector_endpoint, console_logging) {
        (Some(otel), ConsoleLogging::Json) => {
            set_global_default(
                Registry::default()
                    .with(env_filter)
                    .with(apm_layer!(otel))
                    .with(console_layer!().json()),
            )
            .ok();
        }
        (Some(otel), ConsoleLogging::Pretty) => {
            set_global_default(
                Registry::default()
                    .with(env_filter)
                    .with(apm_layer!(otel.as_str()))
                    .with(console_layer!().pretty()),
            )
            .ok();
        }
        (Some(otel), ConsoleLogging::Off) => {
            set_global_default(
                Registry::default()
                    .with(env_filter)
                    .with(apm_layer!(otel.as_str())),
            )
            .ok();
        }
        (None, ConsoleLogging::Json) => {
            set_global_default(
                Registry::default()
                    .with(env_filter)
                    .with(console_layer!().json()),
            )
            .ok();
        }
        (None, ConsoleLogging::Pretty) => {
            set_global_default(
                Registry::default()
                    .with(env_filter)
                    .with(console_layer!().pretty()),
            )
            .ok();
        }
        _ => (),
    }
}
