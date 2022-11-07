use std::path::PathBuf;

use clap::{
    builder::{NonEmptyStringValueParser, PathBufValueParser},
    parser::ValueSource,
    Arg, ArgMatches, Command, ValueHint,
};
use k256::{pkcs8::DecodePrivateKey, SecretKey};
use url::Url;

pub fn bootstrap() -> Command {
    Command::new("bootstrap")
        .about("Initialize the OPA transaction processor with a root key")
        .arg(
            Arg::new("root-key")
                .short('k')
                .long("root-key")
                .env("ROOT_KEY")
                .num_args(1)
                .value_hint(ValueHint::FilePath)
                .help("A PEM-encoded private key"),
        )
}

pub fn generate() -> Command {
    Command::new("generate").about("Generate a new private key and write it to stdout")
}

pub fn rotate_root() -> Command {
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
}

pub fn register_key() -> Command {
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
            Arg::new("id")
                .short('i')
                .long("id")
                .num_args(1)
                .value_hint(ValueHint::Unknown)
                .value_parser(NonEmptyStringValueParser::new())
                .help("The new key to register as the root key"),
        )
}

pub fn rotate_key() -> Command {
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
}

pub fn set_policy() -> Command {
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
}

pub fn get_key() -> Command {
    Command::new("get-key")
        .about("Get the currently registered public key")
        .arg(
            Arg::new("id")
                .short('i')
                .long("id")
                .num_args(0..=1)
                .value_hint(ValueHint::Unknown)
                .value_parser(NonEmptyStringValueParser::new())
                .help("The id of the key, if not specified then the root key is returned"),
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
                .default_value("zmq://localhost:4004"),
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
    let key = matches.value_source(name).unwrap();
    match key {
        ValueSource::CommandLine | ValueSource::DefaultValue => {
            let path: &PathBuf = matches.get_one(name).unwrap();
            let key = std::fs::read_to_string(path)
                .unwrap_or_else(|_| panic!("Unable to read file {}", path.to_string_lossy()));
            SecretKey::from_pkcs8_pem(&key).unwrap()
        }
        ValueSource::EnvVariable => {
            let key: &String = matches.get_one(name).unwrap();
            SecretKey::from_pkcs8_pem(key).unwrap()
        }
        _ => unreachable!(),
    }
}
