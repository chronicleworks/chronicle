use clap::*;
use clap_complete::Shell;

pub fn cli() -> Command<'static> {
    let app = Command::new("chronicle")
        .version("1.0")
        .author("Blockchain technology partners")
        .about("Write and query provenance data to distributed ledgers")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("config")
                .value_hint(ValueHint::FilePath)
                .default_value("~/.chronicle/config.toml")
                .help("Sets a custom config file")
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
                .value_name("jea")
                .takes_value(true)
                .value_hint(ValueHint::Url)
                .help("Insutrment using RUST_LOG environment"),
        )
        .arg(
            Arg::new("ui")
                .long("ui")
                .required(false)
                .takes_value(false)
                .help("Start a web user interface"),
        )
        .arg(
            Arg::new("open")
                .long("open")
                .required(false)
                .takes_value(false)
                .help("Open the default browser for the user interface"),
        )
        .arg(
            Arg::new("ui-interface")
                .long("ui-interface")
                .required(false)
                .takes_value(true)
                .default_value("127.0.0.1:9982")
                .help("The user interface address"),
        )
        .subcommand(
            Command::new("namespace")
                .about("controls namespace features")
                .subcommand(
                    Command::new("create")
                        .about("Create a new namespace")
                        .arg(Arg::new("namespace").required(true).takes_value(true)),
                ),
        )
        .subcommand(
            Command::new("agent")
                .about("controls agents")
                .subcommand(
                    Command::new("create")
                        .about("Create a new agent, if required")
                        .arg(Arg::new("agent_name").required(true).takes_value(true))
                        .arg(
                            Arg::new("namespace")
                                .short('n')
                                .long("namespace")
                                .default_value("default")
                                .required(false)
                                .takes_value(true),
                        ).arg(
                            Arg::new("domaintype")
                                .short('t')
                                .long("domaintype")
                                .default_value("default")
                                .takes_value(true),
                        ).arg(
                            Arg::new("untyped")
                                .long("untyped")
                                .takes_value(false),
                        ).group(ArgGroup::new("type")
                                    .args(&["domaintype","untyped"])
                                    .required(true)),
                )
                .subcommand(
                    Command::new("register-key")
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
                        .arg(Arg::new("generate")
                            .help("Automatically generate a signing key for this agent and store it in the configured key store")
                            .required_unless_present_any(vec!["publickey", "privatekey"])
                            .short('g')
                            .long("generate")
                            .takes_value(false),
                        )
                        .arg(
                            Arg::new("publickey")
                                .help("Import the public key at this location to the configured key store")
                                .short('p')
                                .long("publickey")
                                .value_hint(ValueHint::FilePath)
                                .required_unless_present_any(vec!["generate","privatekey"])
                                .takes_value(true),
                        )
                        .arg(
                            Arg::new("privatekey")
                                .help("Import the private key at the specifed path to the configured key store, ensure you have configured the key store to be in an appropriate location")
                                .short('k')
                                .long("privatekey")
                                .required_unless_present_any(vec!["generate","publickey"])
                                .value_hint(ValueHint::FilePath)
                                .required(false)
                                .takes_value(true),
                        ),
                )
                .subcommand(
                    Command::new("use")
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
            Command::new("activity")
                .subcommand(
                    Command::new("create")
                        .about("Create a new activity, if required")
                        .arg(Arg::new("activity_name").required(true).takes_value(true))
                        .arg(
                            Arg::new("namespace")
                                .short('n')
                                .long("namespace")
                                .default_value("default")
                                .required(false)
                                .takes_value(true),
                        ).arg(
                            Arg::new("domaintype")
                                .short('t')
                                .long("domaintype")
                                .default_value("default")
                                .takes_value(true),
                        ).arg(
                            Arg::new("untyped")
                                .long("untyped")
                                .takes_value(false),
                        ).group(ArgGroup::new("type")
                                    .args(&["domaintype","untyped"])
                                    .required(true)),
                )
                .subcommand(
                    Command::new("start")
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
                    Command::new("end")
                        .about("Record this activity as ended at the current time")
                        .arg(Arg::new("activity_name").required(false).takes_value(true))
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
                    Command::new("use")
                        .about("Record this activity as having used the specified entity, creating it if required")
                        .arg(Arg::new("entity_name").required(true).takes_value(true))
                        .arg(
                            Arg::new("namespace")
                                .short('n')
                                .long("namespace")
                                .default_value("default")
                                .required(false)
                                .takes_value(true),
                        )
                        .arg(
                            Arg::new("activity_name")
                                .required(false)
                                .short('a')
                                .long("activity")
                                .takes_value(true),
                        ).arg(
                            Arg::new("domaintype")
                                .short('t')
                                .long("domaintype")
                                .default_value("default")
                                .takes_value(true),
                        ).arg(
                            Arg::new("untyped")
                                .long("untyped")
                                .takes_value(false),
                        ).group(ArgGroup::new("type")
                                    .args(&["domaintype","untyped"])
                                    .required(true)),
                )
                .subcommand(
                    Command::new("generate")
                        .about("Records this activity as having generated the specified entity, creating it if required")
                        .arg(Arg::new("entity_name").required(true).takes_value(true))
                        .arg(
                            Arg::new("namespace")
                                .short('n')
                                .long("namespace")
                                .default_value("default")
                                .required(false)
                                .takes_value(true),
                        )
                        .arg(
                            Arg::new("activity_name")
                                .required(false)
                                .short('a')
                                .long("activity")
                                .takes_value(true),
                        ).arg(
                            Arg::new("domaintype")
                                .short('t')
                                .long("domaintype")
                                .default_value("default")
                                .takes_value(true),
                        ).arg(
                            Arg::new("untyped")
                                .long("untyped")
                                .takes_value(false),
                        ).group(ArgGroup::new("type")
                                    .args(&["domaintype","untyped"])
                                    .required(true)),
                ),
        )
        .subcommand(
            Command::new("entity")
                .about("Operations on entities")
                .subcommand(
                    Command::new("attach")
                        .about("Sign the input file and record it against the entity")
                        .arg(Arg::new("entity_name").required(true).takes_value(true))
                        .arg(
                            Arg::new("namespace")
                                .short('n')
                                .long("namespace")
                                .default_value("default")
                                .required(false)
                                .takes_value(true),
                        )
                        .arg(
                            Arg::new("file")
                                .short('f')
                                .help("A path to the file to be signed and attached")
                                .long("file")
                                .value_hint(ValueHint::FilePath)
                                .required(true)
                                .takes_value(true),
                        )
                        .arg(
                            Arg::new("locator")
                                .short('l')
                                .long("locator")
                                .help("A url or other way of identifying the attachment")
                                .value_hint(ValueHint::Url)
                                .required(false)
                                .takes_value(true),
                        )
                        .arg(
                            Arg::new("agent")
                                .long("agent")
                                .help("The agent id attaching the entity")
                                .required(false)
                                .takes_value(true),
                        )
                        .arg(
                            Arg::new("activity_name")
                                .required(false)
                                .short('a')
                                .long("activity")
                                .takes_value(true),
                        ),
                ),
        )
        .subcommand(
            Command::new("export")
                .about("Query prov data")
                .arg(
                        Arg::new("namespace")
                            .short('n')
                            .long("namespace")
                            .default_value("default")
                            .required(false)
                            .takes_value(true),
                    )
            );

    #[cfg(not(feature = "inmem"))]
    {
        app.arg(
            Arg::new("sawtooth")
                .long("sawtooth")
                .value_name("sawtooth")
                .value_hint(ValueHint::Url)
                .default_value("tcp://localhost:4004")
                .help("Sets sawtooth validator address")
                .takes_value(true),
        )
    }
    #[cfg(feature = "inmem")]
    {
        app
    }
}
