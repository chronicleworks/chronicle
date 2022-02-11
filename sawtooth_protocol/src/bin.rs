mod address;
mod messages;
mod messaging;
mod tp;

mod sawtooth {
    include!(concat!(env!("OUT_DIR"), "/_.rs"));
}

use clap::{App, Arg, ValueHint};
use clap_generate::Shell;
use sawtooth_sdk::processor::TransactionProcessor;
use tracing::Level;

#[tokio::main]
async fn main() {
    let handler = crate::tp::ChronicleTransactionHandler::new();

    let matches = App::new("chronicle")
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
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .takes_value(true)
                .value_hint(ValueHint::Unknown)
                .value_name("verbose")
                .help("Increase outeput verbosity"),
        )
        .get_matches();

    let endpoint = matches
        .value_of("connect")
        .unwrap_or("tcp://localhost:4004");

    let console_log_level = match matches.occurrences_of("verbose") {
        0 => Level::WARN,
        1 => Level::INFO,
        2 => Level::DEBUG,
        _ => Level::TRACE,
    };

    tracing_subscriber::fmt()
        .pretty()
        .with_max_level(console_log_level)
        .init();

    let mut processor = TransactionProcessor::new(endpoint);

    processor.add_handler(&handler);
    processor.start();
}
