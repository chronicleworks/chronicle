use std::path::PathBuf;

use chronicle_signing::{
	opa_secret_names, ChronicleSecretsOptions, ChronicleSigning, SecretError, BATCHER_NAMESPACE,
	OPA_NAMESPACE,
};
use clap::{
	builder::{NonEmptyStringValueParser, StringValueParser},
	Arg, ArgAction, ArgMatches, Command, ValueHint,
};

use tracing::info;
use url::Url;

// Generate an ephemeral key if no key is provided
fn batcher_key() -> Arg {
	Arg::new( "batcher-key-from-store")
      .long("batcher-key-from-store")
      .num_args(0)
      .help("If specified the key 'batcher-pk' will be used to sign sawtooth transactions, otherwise an ephemeral key will be generated")
}

fn wait_args(command: Command) -> Command {
	command.arg(
		Arg::new("wait")
			.long("wait")
			.num_args(0..=1)
			.value_parser(clap::value_parser!(u64).range(0..))
			.default_value("5")
			.default_missing_value("5")
			.help("Wait for the specified number of blocks to be committed before exiting"),
	)
}

fn bootstrap() -> Command {
	wait_args(
		Command::new("bootstrap")
			.about("Initialize the OPA transaction processor with a root key from the keystore")
			.arg(batcher_key()),
	)
}

fn generate() -> Command {
	Command::new("generate")
		.arg(Arg::new("output").short('o').long("output").num_args(0..=1).help(
			"The name to write the key to, if not specified then the key is written to stdout",
		))
		.about("Generate a new private key and write it to the keystore")
}

fn rotate_root() -> Command {
	wait_args(
		Command::new("rotate-root")
			.about("Rotate the root key for the OPA transaction processor")
			.arg(
				Arg::new("new-root-key")
					.short('n')
					.long("new-root-key")
					.env("NEW_ROOT_KEY")
					.required(true)
					.num_args(1)
					.value_hint(ValueHint::FilePath)
					.help("The name of the new key in the keystore to register as the root key"),
			)
			.arg(batcher_key()),
	)
}

fn register_key() -> Command {
	wait_args(
		Command::new("register-key")
			.about("Register a new non root key with the OPA transaction processor")
			.arg(
				Arg::new("new-key")
					.long("new-key")
					.required(true)
					.num_args(1)
					.value_hint(ValueHint::FilePath)
					.help("The keystore name of a PEM-encoded key to register"),
			)
			.arg(
				Arg::new("id")
					.short('i')
					.long("id")
					.required(true)
					.num_args(1)
					.value_hint(ValueHint::Unknown)
					.value_parser(NonEmptyStringValueParser::new())
					.help("The id of the key"),
			)
			.arg(
				Arg::new("overwrite")
					.short('o')
					.long("overwrite")
					.action(ArgAction::SetTrue)
					.help("Replace any existing non-root key"),
			)
			.arg(batcher_key()),
	)
}

fn rotate_key() -> Command {
	wait_args(
		Command::new("rotate-key")
			.about("Rotate the key with the specified id for the OPA transaction processor")
			.arg(
				Arg::new("current-key")
					.long("current-key")
					.env("CURRENT_KEY")
					.required(true)
					.num_args(1)
					.value_hint(ValueHint::FilePath)
					.help("The keystore name of the current registered key"),
			)
			.arg(
				Arg::new("new-key")
					.long("new-key")
					.env("NEW_KEY")
					.required(true)
					.num_args(1)
					.value_hint(ValueHint::FilePath)
					.help("The keystore name of the new key to register"),
			)
			.arg(
				Arg::new("id")
					.short('i')
					.long("id")
					.required(true)
					.num_args(1)
					.value_hint(ValueHint::Unknown)
					.value_parser(NonEmptyStringValueParser::new())
					.help("The id of the key"),
			)
			.arg(batcher_key()),
	)
}

fn set_policy() -> Command {
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
					.required(true)
					.value_hint(ValueHint::Url)
					.value_parser(StringValueParser::new())
					.help("A path or url to a policy bundle"),
			)
			.arg(batcher_key()),
	)
}

fn get_key() -> Command {
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

fn get_policy() -> Command {
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
				.required(true)
				.value_hint(ValueHint::FilePath)
				.value_parser(NonEmptyStringValueParser::new())
				.help("The path to write the policy to"),
		)
}

pub const LONG_VERSION: &str = const_format::formatcp!(
	"{}:{}",
	env!("CARGO_PKG_VERSION"),
	include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.VERSION"))
);

pub fn cli() -> Command {
	info!(opa_version = LONG_VERSION);
	Command::new("opactl")
		.version(LONG_VERSION)
		.author("Blockchain Technology Partners")
		.about("A command line tool for interacting with the OPA transaction processor")
		.arg(
			Arg::new("keystore-path")
				.long("keystore-path")
				.help("The path to a directory containing keys")
				.value_parser(clap::value_parser!(PathBuf))
				.value_hint(ValueHint::DirPath)
				.env("KEYSTORE_PATH")
				.default_value("."),
		)
		.arg(
			Arg::new("batcher-key-from-path")
				.long("batcher-key-from-path")
				.action(ArgAction::SetTrue)
				.help("Load batcher key from keystore path")
				.conflicts_with("batcher-key-from-vault")
				.conflicts_with("batcher-key-generated"),
		)
		.arg(
			Arg::new("batcher-key-from-vault")
				.long("batcher-key-from-vault")
				.action(ArgAction::SetTrue)
				.help("Use Hashicorp Vault to store the batcher key")
				.conflicts_with("batcher-key-from-path")
				.conflicts_with("batcher-key-generated"),
		)
		.arg(
			Arg::new("batcher-key-generated")
				.long("batcher-key-generated")
				.action(ArgAction::SetTrue)
				.help("Generate the batcher key in memory")
				.conflicts_with("batcher-key-from-path")
				.conflicts_with("batcher-key-from-vault"),
		)
		.arg(
			Arg::new("opa-key-from-path")
				.long("opa-key-from-path")
				.action(ArgAction::SetTrue)
				.help("Use keystore path for the opa key located in 'opa-pk'")
				.conflicts_with("opa-key-from-vault"),
		)
		.arg(
			Arg::new("opa-key-from-vault")
				.long("opa-key-from-vault")
				.action(ArgAction::SetTrue)
				.help("Use Hashicorp Vault to store the Opa key")
				.conflicts_with("opa-key-from-path"),
		)
		.arg(
			Arg::new("vault-address")
				.long("vault-address")
				.num_args(0..=1)
				.value_parser(clap::value_parser!(Url))
				.value_hint(ValueHint::Url)
				.help("URL for connecting to Hashicorp Vault")
				.env("VAULT_ADDRESS"),
		)
		.arg(
			Arg::new("vault-token")
				.long("vault-token")
				.num_args(0..=1)
				.help("Token for connecting to Hashicorp Vault")
				.env("VAULT_TOKEN"),
		)
		.arg(
			Arg::new("vault-mount-path")
				.long("vault-mount-path")
				.num_args(0..=1)
				.value_hint(ValueHint::DirPath)
				.help("Mount path for vault secrets")
				.default_value("/")
				.env("VAULT_MOUNT_PATH"),
		)
		.arg(
			Arg::new("sawtooth-address")
				.short('a')
				.long("sawtooth-address")
				.num_args(0..=1)
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

// Chronicle secret store needs to know what secret names are used in advance,
// so extract from potential cli args
fn additional_secret_names(expected: Vec<&str>, matches: &ArgMatches) -> Vec<String> {
	expected.iter().filter_map(|x| matches.get_one::<String>(x).cloned()).collect()
}

// Batcher keys may be ephemeral if batcher-key-from-path is not set, also we need to know secret
// names in advance, so must inspect the supplied CLI arguments
pub(crate) async fn configure_signing(
	expected: Vec<&str>,
	root_matches: &ArgMatches,
	matches: &ArgMatches,
) -> Result<ChronicleSigning, SecretError> {
	let mut secret_names = opa_secret_names();
	secret_names.append(
		&mut additional_secret_names(expected, matches)
			.into_iter()
			.map(|name| (OPA_NAMESPACE.to_string(), name.to_string()))
			.collect(),
	);
	let keystore_path = root_matches.get_one::<PathBuf>("keystore-path").unwrap();

	let opa_key_from_vault = root_matches.get_one("opa-key-from-vault").is_some_and(|x| *x);
	let opa_secret_options = if opa_key_from_vault {
		ChronicleSecretsOptions::stored_in_vault(
			matches.get_one("vault-url").unwrap(),
			matches.get_one("vault-token").cloned().unwrap(),
			matches.get_one("vault-mount-path").cloned().unwrap(),
		)
	} else {
		ChronicleSecretsOptions::stored_at_path(keystore_path)
	};
	let opa_secret = (OPA_NAMESPACE.to_string(), opa_secret_options);

	let batcher_key_from_path = root_matches.get_one("batcher-key-from-path").is_some_and(|x| *x);
	let batcher_key_from_vault = root_matches.get_one("batcher-key-from-vault").is_some_and(|x| *x);
	let batcher_secret_options = if batcher_key_from_path {
		ChronicleSecretsOptions::stored_at_path(keystore_path)
	} else if batcher_key_from_vault {
		ChronicleSecretsOptions::stored_in_vault(
			matches.get_one("vault-url").unwrap(),
			matches.get_one("vault-token").cloned().unwrap(),
			matches.get_one("vault-mount-path").cloned().unwrap(),
		)
	} else {
		ChronicleSecretsOptions::generate_in_memory()
	};
	let batcher_secret = (BATCHER_NAMESPACE.to_string(), batcher_secret_options);

	let secrets = vec![opa_secret, batcher_secret];
	ChronicleSigning::new(secret_names, secrets).await
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Wait {
	NoWait,
	NumberOfBlocks(u64),
}

impl Wait {
	pub(crate) fn from_matches(matches: &ArgMatches) -> Self {
		match matches.get_one::<u64>("wait") {
			Some(blocks) if *blocks > 0 => Wait::NumberOfBlocks(*blocks),
			_ => Wait::NoWait,
		}
	}
}
