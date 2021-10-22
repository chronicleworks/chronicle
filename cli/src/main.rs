#[macro_use]
extern crate serde_derive;

use api::{AgentCommand, Api, ApiCommand, ApiError, ApiResponse, NamespaceCommand};
use clap::{App, Arg, ArgMatches};
use custom_error::custom_error;
use k256::{elliptic_curve::sec1::ToEncodedPoint, SecretKey};
use pkcs8::{ToPrivateKey, ToPublicKey};
use question::{Answer, Question};
use rand::prelude::StdRng;
use rand_core::SeedableRng;
use std::path::{Path, PathBuf};
use url::Url;
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
                .value_name("config")
                .default_value("~/.chronicle/config.toml")
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
                    )
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
                    ),
            ),
        )
}

fn api_exec(config: Config, options: ArgMatches) -> Result<ApiResponse, ApiError> {
    let api = Api::new(
        &Path::join(&config.store.path, &PathBuf::from("db.sqlite")).to_string_lossy(),
        &config.validator.address,
        &config.secrets.path,
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
    FileSystem{source: std::io::Error}       = "Cannot locate configuration file",
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

fn handle_config_and_init(cli: &App) -> Result<Config, CliError> {
    let path = cli
        .clone()
        .get_matches()
        .value_of("config")
        .unwrap()
        .to_owned();
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
    let cli = cli();

    handle_config_and_init(&cli)
        .and_then(|config| Ok(api_exec(config, cli.get_matches())?))
        .map(|response| {
            match response {
                ApiResponse::Iri(iri) => {
                    println!("{}", iri);
                }
                ApiResponse::Document(doc) => {
                    println!("{}", doc.pretty(2));
                }
                ApiResponse::Unit => {}
            }
            std::process::exit(0);
        })
        .map_err(|e| {
            e.into_ufe().print();
            std::process::exit(1);
        })
        .ok();
}
