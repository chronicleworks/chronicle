use std::net::SocketAddr;

use async_sawtooth_sdk::zmq_client::{
    HighestBlockValidatorSelector, ZmqRequestResponseSawtoothChannel,
};
use chronicle_protocol::{
    address::{FAMILY, VERSION},
    async_sawtooth_sdk::ledger::SawtoothLedger,
    settings::{read_opa_settings, OpaSettings, SettingsReader},
};
use clap::ArgMatches;
use common::opa::{
    CliPolicyLoader, ExecutorContext, PolicyLoader, SawtoothPolicyLoader, UrlPolicyLoader,
};
use tracing::{debug, instrument};

use super::CliError;

trait SetRuleOptions {
    fn rule_addr(&mut self, options: &ArgMatches) -> Result<(), CliError>;
    fn rule_entrypoint(&mut self, options: &ArgMatches) -> Result<(), CliError>;
    fn set_addr_and_entrypoint(&mut self, options: &ArgMatches) -> Result<(), CliError> {
        self.rule_addr(options)?;
        self.rule_entrypoint(options)?;
        Ok(())
    }
}

impl SetRuleOptions for CliPolicyLoader {
    fn rule_addr(&mut self, options: &ArgMatches) -> Result<(), CliError> {
        if let Some(val) = options.get_one::<String>("opa-rule") {
            self.set_address(val);
            Ok(())
        } else {
            Err(CliError::MissingArgument {
                arg: "opa-rule".to_string(),
            })
        }
    }

    fn rule_entrypoint(&mut self, options: &ArgMatches) -> Result<(), CliError> {
        if let Some(val) = options.get_one::<String>("opa-entrypoint") {
            self.set_entrypoint(val);
            Ok(())
        } else {
            Err(CliError::MissingArgument {
                arg: "opa-entrypoint".to_string(),
            })
        }
    }
}

#[instrument()]
pub async fn opa_executor_from_embedded_policy(
    policy_name: &str,
    entrypoint: &str,
) -> Result<ExecutorContext, CliError> {
    let loader = CliPolicyLoader::from_embedded_policy(policy_name, entrypoint)?;
    Ok(ExecutorContext::from_loader(&loader)?)
}

#[instrument()]
pub async fn opa_executor_from_sawtooth_settings(
    validator_address: &Vec<SocketAddr>,
) -> Result<(ExecutorContext, OpaSettings), CliError> {
    let settings = SettingsReader::new(SawtoothLedger::new(
        ZmqRequestResponseSawtoothChannel::new(
            "opa_executor",
            validator_address,
            HighestBlockValidatorSelector,
        )?
        .retrying(),
        FAMILY,
        VERSION,
    ));
    let opa_settings = read_opa_settings(&settings).await?;
    debug!(on_chain_opa_policy = ?opa_settings);
    let mut loader = SawtoothPolicyLoader::new(
        validator_address.get(0).unwrap(),
        &opa_settings.policy_name,
        &opa_settings.entrypoint,
    )?;
    loader.load_policy().await?;
    Ok((ExecutorContext::from_loader(&loader)?, opa_settings))
}

#[instrument()]
pub async fn opa_executor_from_url(
    url: &str,
    policy_name: &str,
    entrypoint: &str,
) -> Result<ExecutorContext, CliError> {
    let mut loader = UrlPolicyLoader::new(url, policy_name, entrypoint);
    loader.load_policy().await?;
    Ok(ExecutorContext::from_loader(&loader)?)
}
