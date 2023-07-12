use std::iter::repeat;

use async_sawtooth_sdk::{
    error::SawtoothCommunicationError, ledger::LedgerReader, messages::Setting,
};
use k256::sha2::{Digest, Sha256};
use prost::Message;
use tracing::error;

use crate::ChronicleLedger;

fn setting_key_to_address(key: &str) -> String {
    let mut address = String::new();
    address.push_str("000000");
    address.push_str(
        &key.splitn(4, '.')
            .chain(repeat(""))
            .map(short_hash)
            .take(4)
            .collect::<Vec<_>>()
            .join(""),
    );

    address
}

fn short_hash(s: &str) -> String {
    hex::encode(Sha256::digest(s.as_bytes()))[..16].to_string()
}

/// Generates a Sawtooth address for a given setting key.
///
/// The address is a hex string that is computed based on the input key,
/// according to the Sawtooth settings addressing algorithm. The key is
/// split into four parts based on the dots in the string. If there are
/// less than four parts, the remaining parts are filled with empty strings.
/// Each of these parts has a short hash computed (the first 16 characters
/// of its SHA256 hash in hex) and is joined into a single address, with the
/// settings namespace (`000000`) added at the beginning.
///
/// Does not account for settings keys with more than 4 components
///
/// # Arguments
///
/// * `s` - A string representing the setting key to generate an address for.
///
/// # Example
///
/// ```
/// use chronicle_protocol::settings::sawtooth_settings_address;
///
/// let address = sawtooth_settings_address("sawtooth.config.vote.proposals");
/// assert_eq!(address, "000000a87cb5eafdcca6a8b79606fb3afea5bdab274474a6aa82c1c0cbf0fbcaf64c0b");
/// ```
pub fn sawtooth_settings_address(s: &str) -> String {
    setting_key_to_address(s)
}

/// This `SettingsReader` struct is used for extracting particular configuration
/// settings from the Sawtooth settings TP given the key.
pub struct SettingsReader(ChronicleLedger);

impl SettingsReader {
    pub fn new(reader: ChronicleLedger) -> Self {
        Self(reader)
    }

    /// Async function that returns the value of a specific configuration setting, given its key.
    ///
    /// # Arguments
    /// * `key` - a reference to a string that contains the key for the setting to retrieve.
    ///
    /// # Errors
    /// If the value is not found, returns a `SawtoothCommunicationError`, which indicates that there was an error in communicating with the Sawtooth network.
    ///
    /// Settings values are not uniform, so we return a `Vec<u8>` for further processing
    pub async fn read_settings(&self, key: &str) -> Result<Setting, SawtoothCommunicationError> {
        let address = sawtooth_settings_address(key);
        loop {
            let res = self.0.get_state_entry(&address).await;

            if let Err(e) = res {
                error!("Error reading settings: {}", e);
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                continue;
            }

            return Ok(Setting::decode(&*res.unwrap())?);
        }
    }
}

#[derive(Debug, Clone)]
pub struct OpaSettings {
    pub policy_name: String,
    pub entrypoint: String,
}

pub async fn read_opa_settings(
    settings: &SettingsReader,
) -> Result<OpaSettings, SawtoothCommunicationError> {
    let policy_id = settings.read_settings("chronicle.opa.policy_name").await?;
    let entrypoint = settings.read_settings("chronicle.opa.entrypoint").await?;
    let policy_id = policy_id
        .entries
        .first()
        .ok_or_else(|| SawtoothCommunicationError::MalformedMessage)?;
    let entrypoint = entrypoint
        .entries
        .first()
        .ok_or_else(|| SawtoothCommunicationError::MalformedMessage)?;
    Ok(OpaSettings {
        policy_name: policy_id.value.clone(),
        entrypoint: entrypoint.value.clone(),
    })
}
