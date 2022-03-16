mod address;
mod messages;
mod messaging;
mod tp;

mod sawtooth {
    include!(concat!(env!("OUT_DIR"), "/_.rs"));
}

use clap::{Arg, Command, ValueHint};
use clap_generate::Shell;
use sawtooth_sdk::processor::TransactionProcessor;

use tracing::subscriber::set_global_default;
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_log::LogTracer;
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, EnvFilter, Registry};

pub fn tracing() {
    LogTracer::init().expect("Failed to set logger");
    // Fall back to printing all spans at info-level or above
    // if the RUST_LOG environment variable has not been set.
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let formatting_layer =
        BunyanFormattingLayer::new("chronicle-sawtooth-tp".into(), std::io::stdout);
    let subscriber = Registry::default()
        .with(env_filter)
        .with(JsonStorageLayer)
        .with(formatting_layer);
    set_global_default(subscriber).expect("Failed to set subscriber");
}

#[tokio::main]
async fn main() {
    let matches = Command::new("chronicle-sawtooth-tp")
        .version("1.0")
        .author("Blockchain technology partners")
        .about("Write and query provenance data to distributed ledgers")
        .arg(
            Arg::new("connect")
                .short('C')
                .long("connect")
                .value_hint(ValueHint::Url)
                .help("Sets sawtooth validator address")
                .takes_value(true),
        )
        .arg(
            Arg::new("completions")
                .long("completions")
                .value_name("completions")
                .possible_values(Shell::possible_values())
                .help("Generate shell completions and exit"),
        )
        .arg(
            Arg::new("instrument")
                .short('i')
                .long("instrument")
                .help("Instrument using RUST_LOG environment"),
        )
        .get_matches();

    if matches.is_present("instrument") {
        tracing();
    }

    let endpoint = matches.value_of("connect").unwrap_or("localhost:4004");

    let handler = crate::tp::ChronicleTransactionHandler::new();
    let mut processor = TransactionProcessor::new(endpoint);

    processor.add_handler(&handler);
    processor.start();
}
