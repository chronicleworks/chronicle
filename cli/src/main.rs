#![cfg_attr(feature = "strict", deny(warnings))]
mod lib;

use api;
use lib::*;

//! The default graphql api - only abstract resources



#[tokio::main]
async fn main() {
    let matches = cli().get_matches();

    if let Ok(generator) = matches.value_of_t::<Shell>("completions") {
        let mut app = cli();
        eprintln!("Generating completion file for {}...", generator);
        print_completions(generator, &mut app);
        std::process::exit(0);
    }

    if matches.is_present("export-schema") {
        print!("{}", api::exportable_schema());
        std::process::exit(0);
    }

    if matches.is_present("console-logging") {
        telemetry::console_logging();
    }

    if matches.is_present("instrument") {
        telemetry::telemetry(
            Url::parse(&*matches.value_of_t::<String>("instrument").unwrap()).unwrap(),
        );
    }

    config_and_exec(&matches)
        .await
        .map_err(|e| {
            error!(?e, "Api error");
            e.into_ufe().print();
            std::process::exit(1);
        })
        .ok();

    std::process::exit(0);
}
