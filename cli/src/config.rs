use super::CliError;
use clap::ArgMatches;
use common::signing::DirectoryStoredKeys;


use question::{Answer, Question};


use std::path::{Path, PathBuf};
use url::Url;

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

pub fn handle_config_and_init(matches: &ArgMatches) -> Result<Config, CliError> {
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
                DirectoryStoredKeys::new(&secretpath)?.generate_chronicle()?;
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
