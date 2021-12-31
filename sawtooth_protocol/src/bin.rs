use clap::clap_app;
use sawtooth_protocol::tp::ChronicleTransactionHandler;
use sawtooth_sdk::processor::TransactionProcessor;
use tracing::{Level};

#[tokio::main]
async fn main() {
    let handler = ChronicleTransactionHandler::new();

    let matches = clap_app!(intkey =>
        (about: "Intkey Transaction Processor (Rust)")
        (@arg connect: -C --connect +takes_value
         "connection endpoint for validator")
        (@arg verbose: -v --verbose +multiple
         "increase output verbosity"))
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
