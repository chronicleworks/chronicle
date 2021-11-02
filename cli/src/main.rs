#[macro_use]
extern crate serde_derive;

mod cli;

use api::{
    ActivityCommand, AgentCommand, Api, ApiCommand, ApiError, ApiResponse, NamespaceCommand,
};
use clap::{App, Arg, ArgMatches};
use clap_generate::{generate, Generator, Shell};
use cli::cli;
use colored_json::prelude::*;
use common::{
    ledger::{InMemLedger, LedgerWriter},
    signing::DirectoryStoredKeys,
};
use custom_error::custom_error;
use k256::{elliptic_curve::sec1::ToEncodedPoint, SecretKey};
use pkcs8::{ToPrivateKey, ToPublicKey};
use proto::messaging::SawtoothValidator;
use question::{Answer, Question};
use rand::prelude::StdRng;
use rand_core::SeedableRng;
use std::{
    io,
    path::{Path, PathBuf},
};
use tracing::{error, instrument, Level};
use url::Url;
use user_error::UFE;

#[cfg(not(feature = "inmem"))]
fn ledger(config: &Config) -> Box<dyn LedgerWriter> {
    Box::new(SawtoothValidator::new(
        &config.validator.address,
        DirectoryStoredKeys::new(&config.secrets.path)
            .unwrap()
            .default(),
    ))
}

#[cfg(feature = "inmem")]
fn ledger(_config: &Config) -> Box<dyn LedgerWriter> {
    Box::new(InMemLedger::default())
}

#[instrument]
fn api_exec(config: Config, options: &ArgMatches) -> Result<ApiResponse, ApiError> {
    let api = Api::new(
        &Path::join(&config.store.path, &PathBuf::from("db.sqlite")).to_string_lossy(),
        ledger(&config),
        &config.secrets.path,
        || uuid::Uuid::new_v4(),
    )?;

    vec![
        options.subcommand_matches("namespace").and_then(|m| {
            m.subcommand_matches("create").map(|m| {
                api.dispatch(ApiCommand::NameSpace(NamespaceCommand::Create {
                    name: m.value_of("namespace").unwrap().to_owned(),
                }))
            })
        }),
        options.subcommand_matches("agent").and_then(|m| {
            vec![
                m.subcommand_matches("create").map(|m| {
                    api.dispatch(ApiCommand::Agent(AgentCommand::Create {
                        name: m.value_of("agent_name").unwrap().to_owned(),
                        namespace: m.value_of("namespace").unwrap().to_owned(),
                    }))
                }),
                m.subcommand_matches("register-key").map(|m| {
                    api.dispatch(ApiCommand::Agent(AgentCommand::RegisterKey {
                        name: m.value_of("agent_name").unwrap().to_owned(),
                        namespace: m.value_of("namespace").unwrap().to_owned(),
                        public: m.value_of("publickey").unwrap().to_owned(),
                        private: m.value_of("privatekey").map(|x| x.to_owned()),
                    }))
                }),
                m.subcommand_matches("use").map(|m| {
                    api.dispatch(ApiCommand::Agent(AgentCommand::Use {
                        name: m.value_of("agent_name").unwrap().to_owned(),
                        namespace: m.value_of("namespace").unwrap().to_owned(),
                    }))
                }),
            ]
            .into_iter()
            .flatten()
            .next()
        }),
        options.subcommand_matches("activity").and_then(|m| {
            vec![
                m.subcommand_matches("create").map(|m| {
                    api.dispatch(ApiCommand::Activity(ActivityCommand::Create {
                        name: m.value_of("activity_name").unwrap().to_owned(),
                        namespace: m.value_of("namespace").unwrap().to_owned(),
                    }))
                }),
                m.subcommand_matches("start").map(|m| {
                    api.dispatch(ApiCommand::Activity(ActivityCommand::Start {
                        name: m.value_of("activity_name").unwrap().to_owned(),
                        namespace: m.value_of("namespace").unwrap().to_owned(),
                        time: None,
                    }))
                }),
                m.subcommand_matches("end").map(|m| {
                    api.dispatch(ApiCommand::Activity(ActivityCommand::End {
                        name: m.value_of("activity_name").map(|x| x.to_owned()),
                        namespace: m.value_of("namespace").map(|x| x.to_owned()),
                        time: None,
                    }))
                }),
                m.subcommand_matches("use").map(|m| {
                    api.dispatch(ApiCommand::Activity(ActivityCommand::Use {
                        name: m.value_of("activity_name").map(|x| x.to_owned()),
                        namespace: m.value_of("namespace").map(|x| x.to_owned()),
                        entity: m.value_of("entity_name").unwrap().to_owned(),
                    }))
                }),
                m.subcommand_matches("generate").map(|m| {
                    api.dispatch(ApiCommand::Activity(ActivityCommand::Generate {
                        name: m.value_of("activity_name").map(|x| x.to_owned()),
                        namespace: m.value_of("namespace").map(|x| x.to_owned()),
                        entity: m.value_of("entity_name").unwrap().to_owned(),
                    }))
                }),
            ]
            .into_iter()
            .flatten()
            .next()
        }),
    ]
    .into_iter()
    .flatten()
    .next()
    .unwrap_or(Ok(ApiResponse::Unit))
}

custom_error! {pub CliError
    Api{source: api::ApiError}                  = "Api error",
    Pkcs8{source: pkcs8::Error}                 = "Key encoding",
    FileSystem{source: std::io::Error}          = "Cannot locate configuration file",
    ConfigInvalid{source: toml::de::Error}      = "Invalid configuration file",
    InvalidPath                                 = "Invalid path",
}

impl UFE for CliError {}

#[derive(Serialize, Deserialize, Debug)]
pub struct SecretConfig {
    pub path: PathBuf,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct StoreConfig {
    pub path: PathBuf,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ValidatorConfig {
    pub address: Url,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub secrets: SecretConfig,
    pub store: StoreConfig,
    pub validator: ValidatorConfig,
}

fn handle_config_and_init(matches: &ArgMatches) -> Result<Config, CliError> {
    let path = matches.value_of("config").unwrap().to_owned();
    let path = shellexpand::tilde(&path);
    let path = PathBuf::from(&*path);

    let meta = std::fs::metadata(&*path);
    if meta.is_err() {
        init_chronicle_at(&path)?
    }

    let toml = std::fs::read_to_string(&path)?;
    Ok(toml::from_str(&toml)?)
}

/// Interrogate the user for required configuration and initialise chronicle to a working state, including key generation if needed
fn init_chronicle_at(path: &Path) -> Result<(), CliError> {
    let init = Question::new(&format!(
        "No configuration found at {}, create?",
        path.to_string_lossy()
    ))
    .default(question::Answer::YES)
    .show_defaults()
    .confirm();

    if init != Answer::YES {
        std::process::exit(0);
    }

    let dbpath = Question::new("Where should chronicle store state?")
        .default(Answer::RESPONSE(
            Path::join(
                path.parent().ok_or(CliError::InvalidPath)?,
                PathBuf::from("store"),
            )
            .to_string_lossy()
            .to_string(),
        ))
        .show_defaults()
        .confirm();

    let secretpath = Question::new("Where should chronicle store secrets?")
        .default(Answer::RESPONSE(
            Path::join(
                path.parent().ok_or(CliError::InvalidPath)?,
                PathBuf::from("secrets"),
            )
            .to_string_lossy()
            .to_string(),
        ))
        .show_defaults()
        .confirm();

    let validatorurl =
        Question::new("What is the address of the sawtooth validator zeromq service?")
            .default(Answer::RESPONSE("tcp://localhost:4004".to_owned()))
            .show_defaults()
            .confirm();

    let generatesecret = Question::new("Generate a new default key in the secret store?")
        .default(Answer::YES)
        .show_defaults()
        .confirm();

    match (dbpath, secretpath, validatorurl) {
        (
            Answer::RESPONSE(dbpath),
            Answer::RESPONSE(secretpath),
            Answer::RESPONSE(validatorurl),
        ) => {
            let dbpath = Path::new(&dbpath);
            let secretpath = Path::new(&secretpath);

            println!("Creating config dir {} if needed", path.to_string_lossy());
            std::fs::create_dir_all(path.parent().unwrap())?;
            println!("Creating db dir {} if needed", &dbpath.to_string_lossy());
            std::fs::create_dir_all(&dbpath)?;
            println!(
                "Creating secret dir {} if needed",
                &secretpath.to_string_lossy()
            );
            std::fs::create_dir_all(&secretpath)?;

            let config = format!(
                r#"
            [secrets]
            path = "{}"
            [store]
            path = "{}"
            [validator]
            address = "{}"
            "#,
                &*secretpath.to_string_lossy(),
                &*dbpath.to_string_lossy(),
                validatorurl
            );

            println!("Writing config to {}", &path.to_string_lossy());
            println!("{}", &config);

            std::fs::write(path, config)?;

            if generatesecret == Answer::YES {
                println!("Generating Secp256k1 secret key");
                let secret = SecretKey::random(StdRng::from_entropy());

                let privpem = secret.to_pkcs8_pem()?;

                let pubpem = secret.public_key().to_public_key_pem()?;

                println!(
                    "Writing new key {} to store",
                    hex::encode_upper(secret.public_key().to_encoded_point(true).as_bytes())
                );

                std::fs::write(
                    Path::join(Path::new(&secretpath), Path::new("default.priv.pem")),
                    privpem.as_bytes(),
                )?;

                std::fs::write(
                    Path::join(Path::new(&secretpath), Path::new("default.pub.pem")),
                    pubpem.as_bytes(),
                )?;
            } else {
                println!(
                    "Please install your keys in .pem format in the configured secret location"
                );
                std::process::exit(0);
            }
        }
        _ => unreachable!(),
    }

    Ok(())
}

fn main() {
    let matches = cli().get_matches();

    if let Ok(generator) = matches.value_of_t::<Shell>("completions") {
        let mut app = cli();
        eprintln!("Generating completion file for {}...", generator);
        print_completions(generator, &mut app);
        std::process::exit(0);
    }

    let _tracer = {
        if matches.is_present("debug") {
            Some(
                tracing_subscriber::fmt()
                    .pretty()
                    .with_max_level(Level::TRACE)
                    .init(),
            )
        } else {
            None
        }
    };

    handle_config_and_init(&matches)
        .and_then(|config| Ok(api_exec(config, &matches)?))
        .map(|response| {
            match response {
                ApiResponse::Prov(doc) => {
                    println!(
                        "{}",
                        doc.to_json()
                            .compact()
                            .unwrap()
                            .0
                            .pretty(4)
                            .to_colored_json_auto()
                            .unwrap()
                    );
                }
                ApiResponse::Unit => {}
            }
            std::process::exit(0);
        })
        .map_err(|e| {
            error!(?e, "Api error");
            e.into_ufe().print();
            std::process::exit(1);
        })
        .ok();
}

fn print_completions<G: Generator>(gen: G, app: &mut App) {
    generate(gen, app, app.get_name().to_string(), &mut io::stdout());
}
