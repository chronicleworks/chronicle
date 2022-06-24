pub mod telemetry;
mod tp;

use clap::{builder::PossibleValuesParser, Arg, Command, ValueHint};
use sawtooth_sdk::processor::TransactionProcessor;

use telemetry::telemetry;
use tp::ChronicleTransactionHandler;
use url::Url;

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
