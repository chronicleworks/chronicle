use clap::{
    builder::{NonEmptyStringValueParser, PathBufValueParser},
    parser::ValueSource,
    Arg, ArgMatches, Command, ValueHint,
};
use k256::{pkcs8::DecodePrivateKey, SecretKey};
use rand::rngs::StdRng;
use rand_core::SeedableRng;
use url::Url;

// Generate an ephemeral key if no key is provided
fn transactor_key() -> Arg {
    Arg::new( "transactor-key")
     .short('t')
     .long("transactor-key")
     .env("TRANSACTOR_KEY")
     .num_args(0..=1)
     .value_hint(ValueHint::FilePath)
     .help("A PEM-encoded private key, used to sign the sawtooth transaction. If not specified an ephemeral key will be generated")
}

fn wait_args(command: Command) -> Command {
    command.arg(
      Arg::new("wait")
        .long("wait")
        .num_args(0..=1)
        .value_parser(clap::value_parser!(u64).range(1..))
        .default_value("5")
        .default_missing_value("5")
        .help( "Wait for the specified number of blocks transaction to be committed before returning"))
}

pub fn bootstrap() -> Command {
    wait_args(
        Command::new("bootstrap")
            .about("Initialize the OPA transaction processor with a root key")
            .arg(
                Arg::new("root-key")
                    .short('r')
                    .long("root-key")
                    .env("ROOT_KEY")
                    .num_args(1)
                    .value_hint(ValueHint::FilePath)
                    .help("A PEM-encoded private key"),
            )
            .arg(transactor_key()),
    )
}

pub fn generate() -> Command {
    Command::new("generate")
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .num_args(0..=1)
                .value_hint(ValueHint::FilePath)
                .value_parser(PathBufValueParser::new())
                .help("The path to write the policy to, if not specified then the key is written to stdout"),
        )
        .about("Generate a new private key and write it to stdout")
}

pub fn rotate_root() -> Command {
    wait_args(
        Command::new("rotate-root")
            .about("Rotate the root key for the OPA transaction processor")
            .arg(
                Arg::new("current-root-key")
                    .short('c')
                    .long("current-root-key")
                    .env("CURRENT_ROOT_KEY")
                    .num_args(1)
                    .value_hint(ValueHint::FilePath)
                    .help("The current registered root private key"),
            )
            .arg(
                Arg::new("new-root-key")
                    .short('n')
                    .long("new-root-key")
                    .env("NEW_ROOT_KEY")
                    .num_args(1)
                    .value_hint(ValueHint::FilePath)
                    .help("The new key to register as the root key"),
            )
            .arg(transactor_key()),
    )
}

pub fn register_key() -> Command {
    wait_args(
        Command::new("register-key")
            .about("Register a new non root key with the OPA transaction processor")
            .arg(
                Arg::new("new-key")
                    .short('k')
                    .long("new-key")
                    .env("NEW_KEY")
                    .num_args(1)
                    .value_hint(ValueHint::FilePath)
                    .help("A PEM encoded key to register"),
            )
            .arg(
                Arg::new("root-key")
                    .short('r')
                    .long("root-key")
                    .env("ROOT_KEY")
                    .num_args(1)
                    .value_hint(ValueHint::FilePath)
                    .help("A PEM-encoded private key"),
            )
            .arg(
                Arg::new("id")
                    .short('i')
                    .long("id")
                    .num_args(1)
                    .value_hint(ValueHint::Unknown)
                    .value_parser(NonEmptyStringValueParser::new())
                    .help("The new key to register as the root key"),
            )
            .arg(transactor_key()),
    )
}

pub fn rotate_key() -> Command {
    wait_args(
        Command::new("rotate-key")
            .about("Rotate the key with the specified id for the OPA transaction processor")
            .arg(
                Arg::new("current-key")
                    .short('c')
                    .long("current-key")
                    .env("CURRENT_KEY")
                    .num_args(1)
                    .value_hint(ValueHint::FilePath)
                    .help("The current registered root key"),
            )
            .arg(
                Arg::new("root-key")
                    .short('r')
                    .long("root-key")
                    .env("ROOT_KEY")
                    .num_args(1)
                    .value_hint(ValueHint::FilePath)
                    .help("A PEM-encoded private key"),
            )
            .arg(
                Arg::new("new-key")
                    .short('n')
                    .long("new-key")
                    .env("NEW_KEY")
                    .num_args(1)
                    .value_hint(ValueHint::FilePath)
                    .help("The new key to register for the given name"),
            )
            .arg(
                Arg::new("id")
                    .short('i')
                    .long("id")
                    .num_args(1)
                    .value_hint(ValueHint::Unknown)
                    .value_parser(NonEmptyStringValueParser::new())
                    .help("The id of the key"),
            )
            .arg(transactor_key()),
    )
}

pub fn set_policy() -> Command {
    wait_args(
        Command::new("set-policy")
            .about("Set policy with id, requires access to root private key")
            .arg(
                Arg::new("id")
                    .short('i')
                    .long("id")
                    .num_args(1)
                    .value_hint(ValueHint::Unknown)
                    .value_parser(NonEmptyStringValueParser::new())
                    .default_value("default")
                    .help("The id of the new policy"),
            )
            .arg(
                Arg::new("policy")
                    .short('p')
                    .long("policy")
                    .num_args(1)
                    .value_hint(ValueHint::FilePath)
                    .value_parser(PathBufValueParser::new())
                    .help("A path to the policy wasm to register"),
            )
            .arg(
                Arg::new("root-key")
                    .short('k')
                    .long("root-key")
                    .env("ROOT_KEY")
                    .num_args(1)
                    .value_hint(ValueHint::FilePath)
                    .help("A PEM-encoded private key"),
            )
            .arg(transactor_key()),
    )
}

pub fn get_key() -> Command {
    Command::new("get-key")
        .about("Get the currently registered public key")
        .arg(
            Arg::new("id")
                .short('i')
                .long("id")
                .num_args(1)
                .value_hint(ValueHint::Unknown)
                .value_parser(NonEmptyStringValueParser::new())
                .default_value("root")
                .help("The id of the key, if not specified then the root key is returned"),
        )
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .num_args(0..=1)
                .value_hint(ValueHint::FilePath)
                .value_parser(NonEmptyStringValueParser::new())
                .help("The path to write the policy to, if not specified then the key is written to stdout"),
        )
}

pub fn get_policy() -> Command {
    Command::new("get-policy")
        .about("Get the currently registered policy")
        .arg(
            Arg::new("id")
                .short('i')
                .long("id")
                .num_args(1)
                .value_hint(ValueHint::Unknown)
                .value_parser(NonEmptyStringValueParser::new())
                .default_value("default")
                .help("The id of the policy, if not specified then the default policy is returned"),
        )
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .num_args(1)
                .value_hint(ValueHint::FilePath)
                .value_parser(NonEmptyStringValueParser::new())
                .help("The path to write the policy to"),
        )
}

pub fn cli() -> Command {
    Command::new("opactl")
        .version(env!("CARGO_PKG_VERSION"))
        .author("BTP.works TODO WHAT IS THIS NOW")
        .about("A command line tool for interacting with the OPA transaction processor")
        .arg(
            Arg::new("sawtooth-address")
                .short('a')
                .help("The address of the Sawtooth ZMQ api, as zmq://host:port")
                .value_parser(clap::value_parser!(Url))
                .env("SAWTOOTH_ADDRESS")
                .default_value("tcp://localhost:4004"),
        )
        .subcommand(bootstrap())
        .subcommand(generate())
        .subcommand(rotate_root())
        .subcommand(register_key())
        .subcommand(rotate_key())
        .subcommand(set_policy())
        .subcommand(get_key())
        .subcommand(get_policy())
}

// Keys are either file paths to a PEM encoded key or a PEM encoded key supplied
// as an environment variable, so we need to load them based on the input type
pub fn load_key_from_match(name: &str, matches: &ArgMatches) -> SecretKey {
    if name == "transactor-key" && matches.value_source(name).is_none() {
        return SecretKey::random(StdRng::from_entropy());
    }

    let key = matches.value_source(name).unwrap();
    match key {
        ValueSource::CommandLine | ValueSource::DefaultValue => {
            let path: &String = matches.get_one(name).unwrap();
            let key = std::fs::read_to_string(path)
                .unwrap_or_else(|_| panic!("Unable to read file {path}"));
            println!("key: {}", key);
            SecretKey::from_pkcs8_pem(&key).unwrap()
        }
        ValueSource::EnvVariable => {
            let key: &String = matches.get_one(name).unwrap();
            SecretKey::from_pkcs8_pem(key).unwrap()
        }
        _ => unreachable!(),
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Wait {
    NoWait,
    NumberOfBlocks(u64),
}

impl Wait {
    pub fn from_matches(matches: &ArgMatches) -> Self {
        if matches.get_one::<u64>("wait").is_some() {
            let blocks = matches.get_one::<u64>("wait").unwrap();
            Wait::NumberOfBlocks(*blocks)
        } else {
            Wait::NoWait
        }
    }
}
