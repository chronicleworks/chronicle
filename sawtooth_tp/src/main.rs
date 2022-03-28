mod tp;

use clap::{Arg, Command, ValueHint};
use clap_generate::Shell;
use sawtooth_sdk::processor::TransactionProcessor;

use tp::ChronicleTransactionHandler;
use tracing::subscriber::set_global_default;
use tracing_log::LogTracer;
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, EnvFilter, Registry};

pub fn tracing() {
    LogTracer::init().expect("Failed to set logger");

    let tracer = opentelemetry_jaeger::new_pipeline()
        .with_service_name("chronicle_tp")
        .install_simple()
        .unwrap();

    let opentelemetry = tracing_opentelemetry::layer().with_tracer(tracer);
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let collector = Registry::default().with(env_filter).with(opentelemetry);

    set_global_default(collector).expect("Failed to set collector");
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

    let endpoint = matches
        .value_of("connect")
        .unwrap_or("tcp://localhost:4004");

    let handler = ChronicleTransactionHandler::new();
    let mut processor = TransactionProcessor::new(endpoint);

    processor.add_handler(&handler);
    processor.start();
}
