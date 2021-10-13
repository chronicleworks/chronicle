use api::AgentCommand;
use api::Api;
use api::ApiCommand;
use api::ApiError;
use api::ApiResponse;
use api::NamespaceCommand;
use clap::ArgMatches;
use clap::{App, Arg};
use user_error::UFE;

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
                        .arg(Arg::new("namespace").required(true).takes_value(true)),
                ),
        )
        .subcommand(
            App::new("agent").about("controls agents").subcommand(
                App::new("create")
                    .about("Create a new agent, if required")
                    .arg(Arg::new("agent_name").required(true).takes_value(true))
                    .arg(
                        Arg::new("namespace")
                            .short('n')
                            .long("namespace")
                            .default_value("default")
                            .required(false)
                            .takes_value(true),
                    ),
            ),
        )
}

fn api_exec(options: ArgMatches) -> Result<ApiResponse, ApiError> {
    let api = Api::new("./.sqlite")?;

    vec![
        options.subcommand_matches("namespace").and_then(|m| {
            m.subcommand_matches("create").map(|m| {
                api.dispatch(ApiCommand::NameSpace(NamespaceCommand::Create {
                    name: m.value_of("namespace").unwrap().to_owned(),
                }))
            })
        }),
        options.subcommand_matches("agent").and_then(|m| {
            m.subcommand_matches("create").map(|m| {
                api.dispatch(ApiCommand::Agent(AgentCommand::Create {
                    name: m.value_of("agent_name").unwrap().to_owned(),
                    namespace: m.value_of("namespace").unwrap().to_owned(),
                }))
            })
        }),
    ]
    .into_iter()
    .flatten()
    .next()
    .unwrap_or(Ok(ApiResponse::Unit))
}

fn main() {
    std::process::exit(match api_exec(cli().get_matches()) {
        Ok(response) => {
            match response {
                ApiResponse::Iri(iri) => {
                    println!("{}", iri);
                }
                ApiResponse::Unit => {}
            }
            0
        }
        Err(e) => {
            e.into_ufe().print();
            1
        }
    });
}
