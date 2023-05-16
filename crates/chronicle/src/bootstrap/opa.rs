use chronicle_protocol::{
    address::{FAMILY, VERSION},
    async_sawtooth_sdk::{ledger::SawtoothLedger, zmq_client::ZmqRequestResponseSawtoothChannel},
    settings::{read_opa_settings, OpaSettings, SettingsReader},
};
use clap::ArgMatches;
use common::opa::{CliPolicyLoader, ExecutorContext, PolicyLoader, SawtoothPolicyLoader};
use tracing::{debug, instrument};
use url::Url;

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
    validator_address: &Url,
) -> Result<(ExecutorContext, OpaSettings), CliError> {
    let settings = SettingsReader::new(SawtoothLedger::new(
        ZmqRequestResponseSawtoothChannel::new(validator_address).retrying(),
        FAMILY,
        VERSION,
    ));
    let opa_settings = read_opa_settings(&settings).await?;
    debug!(on_chain_opa_policy = ?opa_settings);
    let mut loader = SawtoothPolicyLoader::new(
        validator_address,
        &opa_settings.policy_name,
        &opa_settings.entrypoint,
    );
    loader.load_policy().await?;
    Ok((ExecutorContext::from_loader(&loader)?, opa_settings))
}
