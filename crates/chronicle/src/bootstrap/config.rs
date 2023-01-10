use super::{CliError, CliModel, SubCommand};
use common::signing::DirectoryStoredKeys;
use question::{Answer, Question};
use serde_derive::{Deserialize, Serialize};
use uuid::Uuid;

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use url::Url;

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct SecretConfig {
    pub path: PathBuf,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct ValidatorConfig {
    pub address: Url,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    pub(crate) secrets: SecretConfig,
    pub(crate) validator: ValidatorConfig,
    pub(crate) namespace_bindings: HashMap<String, Uuid>,
}

pub(crate) fn handle_config_and_init(model: &CliModel) -> Result<Config, CliError> {
    let path = model
        .as_cmd()
        .get_matches()
        .get_one::<String>("config")
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

    let secretpath = Question::new("Where should Chronicle store secrets?")
        .default(Answer::RESPONSE(
            Path::join(
                path.parent().ok_or(CliError::InvalidPath {
                    path: path.to_string_lossy().to_string(),
                })?,
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

    match (secretpath, validatorurl) {
        (Answer::RESPONSE(secretpath), Answer::RESPONSE(validatorurl)) => {
            let secretpath = Path::new(&secretpath);

            println!("Creating config dir {} if needed", path.to_string_lossy());
            std::fs::create_dir_all(path.parent().unwrap())?;
            println!(
                "Creating secret dir {} if needed",
                &secretpath.to_string_lossy()
            );
            std::fs::create_dir_all(secretpath)?;

            let config = format!(
                r#"[secrets]
path = "{}"
[validator]
address = "{}"
[namespace_bindings]
"#,
                &*secretpath.to_string_lossy(),
                validatorurl
            );

            println!("Writing config to {}", &path.to_string_lossy());
            println!("{}", &config);

            std::fs::write(path, config)?;

            if generatesecret == Answer::YES {
                DirectoryStoredKeys::new(secretpath)?.generate_chronicle()?;
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
