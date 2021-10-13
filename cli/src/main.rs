use api::Api;
use api::ApiError;
use api::NameSpace;
use clap::ArgMatches;
use clap::{App, Arg};
use question::{Answer, Question};
use user_error::{UserFacingError, UFE};

fn cli<'a>() -> App<'a> {
    App::new("chronicle")
        .version("1.0")
        .author("Blockchain technology partners")
        .about("Does awesome things")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .default_value("~/.chronicle/.config.toml")
                .about("Sets a custom config file")
                .takes_value(true),
        )
        .subcommand(
            App::new("namespace")
                .about("controls namespace features")
                .subcommand(
                    App::new("create")
                        .about("Create a new namespace")
                        .arg(Arg::new("name").required(true).takes_value(true)),
                ),
        )
}

fn establish_config_file() {}

fn api_exec(options: ArgMatches) -> Result<(), ApiError> {
    let api = Api::new("")?;

    options
        .subcommand_matches("namespace")
        .and_then(|m| {
            m.subcommand_matches("create")
                .map(|m| api.name_space(&NameSpace::new(m.value_of("name").unwrap())))
        })
        .unwrap_or(Ok(()))
}

fn main() {
    std::process::exit(match api_exec(cli().get_matches()) {
        Ok(_) => 0,
        Err(e) => {
            e.into_ufe().print();
            1
        }
    });
}
