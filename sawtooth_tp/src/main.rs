mod tp;

use clap::{builder::PossibleValuesParser, Arg, Command, ValueHint};
use sawtooth_sdk::processor::TransactionProcessor;

use tp::ChronicleTransactionHandler;
use tracing::subscriber::set_global_default;
use tracing_log::LogTracer;
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, EnvFilter, Registry};
use url::Url;

pub fn telemetry(collector_endpoint: Url) {
    LogTracer::init().expect("Failed to set logger");

    let tracer = opentelemetry_jaeger::new_pipeline()
        .with_service_name("chronicle_tp")
        .with_collector_endpoint(collector_endpoint.as_str())
        .install_batch(opentelemetry::runtime::Tokio)
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
                .value_parser(PossibleValuesParser::new(&["bash", "zsh", "fish"]))
                .help("Generate shell completions and exit"),
        )
        .arg(
            Arg::new("instrument")
                .short('i')
                .long("instrument")
                .value_name("instrument")
                .takes_value(true)
                .value_hint(ValueHint::Url)
                .help("Instrument using RUST_LOG environment"),
        )
        .get_matches();

    if matches.contains_id("instrument") {
        telemetry(Url::parse(&*matches.get_one::<String>("instrument").unwrap()).unwrap());
    }

    let handler = ChronicleTransactionHandler::new();
    let mut processor = TransactionProcessor::new({
        if let Some(connect) = matches.get_one::<String>("connect") {
            connect
        } else {
            "tcp://127.0.0.1:4004"
        }
    });

    processor.add_handler(&handler);
    processor.start();
}
