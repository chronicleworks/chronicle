use clap::*;
use clap_generate::Shell;

pub fn cli() -> App<'static> {
    App::new("chronicle")
        .version("1.0")
        .author("Blockchain technology partners")
        .about("Does awesome things")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("config")
                .value_hint(ValueHint::FilePath)
                .default_value("~/.chronicle/config.toml")
                .about("Sets a custom config file")
                .takes_value(true),
        )
        .arg(
            Arg::new("completions")
                .long("completions")
                .value_name("completions")
                .possible_values(Shell::arg_values())
                .about("Generate shell completions and exit"),
        )
        .arg(
            Arg::new("debug")
                .short('d')
                .long("debug")
                .about("Print debugging information"),
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
            App::new("agent")
                .about("controls agents")
                .subcommand(
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
                )
                .subcommand(
                    App::new("register-key")
                        .about("Register a key pair, or a public key with an agent")
                        .arg(Arg::new("agent_name").required(true).takes_value(true))
                        .arg(
                            Arg::new("namespace")
                                .short('n')
                                .long("namespace")
                                .default_value("default")
                                .required(false)
                                .takes_value(true),
                        )
                        .arg(
                            Arg::new("publickey")
                                .short('p')
                                .long("publickey")
                                .required(true)
                                .takes_value(true),
                        )
                        .arg(
                            Arg::new("privatekey")
                                .short('k')
                                .long("privatekey")
                                .required(false)
                                .takes_value(true),
                        ),
                )
                .subcommand(
                    App::new("use")
                        .about("Make the specified agent the context for activities and entities")
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
        .subcommand(
            App::new("activity")
                .subcommand(
                    App::new("create")
                        .about("Create a new activity, if required")
                        .arg(Arg::new("activity_name").required(true).takes_value(true))
                        .arg(
                            Arg::new("namespace")
                                .short('n')
                                .long("namespace")
                                .default_value("default")
                                .required(false)
                                .takes_value(true),
                        ),
                )
                .subcommand(
                    App::new("start")
                        .about("Record this activity as started at the current time")
                        .arg(Arg::new("activity_name").required(true).takes_value(true))
                        .arg(
                            Arg::new("namespace")
                                .short('n')
                                .long("namespace")
                                .default_value("default")
                                .required(false)
                                .takes_value(true),
                        ),
                )
                .subcommand(
                    App::new("end")
                        .about("Record this activity as ended at the current time")
                        .arg(Arg::new("activity_name").required(true).takes_value(true))
                        .arg(
                            Arg::new("namespace")
                                .short('n')
                                .long("namespace")
                                .default_value("default")
                                .required(false)
                                .takes_value(true),
                        ),
                )
                .subcommand(
                    App::new("use")
                        .about("Record this activity as having used the specified entity")
                        .arg(Arg::new("activity_name").required(true).takes_value(true))
                        .arg(
                            Arg::new("namespace")
                                .short('n')
                                .long("namespace")
                                .default_value("default")
                                .required(false)
                                .takes_value(true),
                        )
                        .arg(Arg::new("entityname").required(true).takes_value(true)),
                )
                .subcommand(
                    App::new("generate")
                        .about("Records this activity as having generated the specified entity")
                        .arg(Arg::new("activity_name").required(true).takes_value(true))
                        .arg(
                            Arg::new("namespace")
                                .short('n')
                                .long("namespace")
                                .default_value("default")
                                .required(false)
                                .takes_value(true),
                        )
                        .arg(Arg::new("entityname").required(true).takes_value(true)),
                ),
        )
}
