mod abstract_tp;
mod tp;

use clap::{builder::PossibleValuesParser, Arg, Command, ValueHint};
use sawtooth_sdk::processor::TransactionProcessor;

use ::telemetry::ConsoleLogging;
use telemetry::telemetry;
use tp::OpaTransactionHandler;
use url::Url;

#[tokio::main]
async fn main() {
    let matches = Command::new("opa-tp")
        .version("1.0")
        .author("BTPWorks")
        .about("PKI and OPA rule storage")
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
                .value_parser(PossibleValuesParser::new(["bash", "zsh", "fish"]))
                .help("Generate shell completions and exit"),
        )
        .arg(
            Arg::new("instrument")
                .long("instrument")
                .value_name("instrument")
                .takes_value(true)
                .value_hint(ValueHint::Url)
                .help("Instrument using RUST_LOG environment"),
        )
        .arg(
            Arg::new("console-logging")
                .long("console-logging")
                .value_name("console-logging")
                .takes_value(true)
                .help("Log to console using RUST_LOG environment"),
        )
        .get_matches();

    telemetry(
        matches
            .get_one::<String>("instrument")
            .and_then(|s| Url::parse(s).ok()),
        match matches.get_one::<String>("console-logging") {
            Some(level) => match level.as_str() {
                "pretty" => ConsoleLogging::Pretty,
                "json" => ConsoleLogging::Json,
                _ => ConsoleLogging::Off,
            },
            _ => ConsoleLogging::Off,
        },
    );

    let handler = OpaTransactionHandler::new();
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