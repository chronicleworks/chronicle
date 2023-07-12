use chronicle_protocol::settings::sawtooth_settings_address;
use common::opa::{CliPolicyLoader, ExecutorContext};
use protobuf::Message;
use sawtooth_sdk::{
    messages::setting::Setting,
    processor::handler::{ApplyError, TransactionContext},
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tracing::{debug, info, warn};

#[derive(Debug)]
pub struct TpOpa {
    pub embedded: ExecutorContext,
    pub on_chain: Arc<Mutex<HashMap<String, (String, ExecutorContext)>>>,
}

impl TpOpa {
    pub fn new(policy: &str, entrypoint: &str) -> Result<Self, ApplyError> {
        Ok(Self {
            on_chain: Arc::new(HashMap::new().into()),
            embedded: {
                ExecutorContext::from_loader(
                    &CliPolicyLoader::from_embedded_policy(policy, entrypoint)
                        .map_err(|e| ApplyError::InternalError(e.to_string()))?,
                )
                .map_err(|e| ApplyError::InternalError(e.to_string()))?
            },
        })
    }

    pub fn executor_context(
        &self,
        ctx: &mut dyn TransactionContext,
    ) -> Result<ExecutorContext, ApplyError> {
        let policy_name_settings_entry =
            ctx.get_state_entry(&sawtooth_settings_address("chronicle.opa.policy_name"))?;

        let entrypoint_settings_entry =
            ctx.get_state_entry(&sawtooth_settings_address("chronicle.opa.entrypoint"))?;

        match (policy_name_settings_entry, entrypoint_settings_entry) {
            (None, None) => {
                warn!("Insecure operating mode - no on-chain policy name or entrypoint settings found");
                Ok(self.embedded.clone())
            }
            (Some(policy_name_settings_entry), Some(entrypoint_settings_entry)) => {
                info!("Chronicle operating in secure mode - on-chain policy name and entrypoint settings found");
                let policy_name_settings_entry: Setting =
                    Message::parse_from_bytes(&policy_name_settings_entry).map_err(|_e| {
                        ApplyError::InternalError("Invalid setting entry".to_string())
                    })?;
                let entrypoint_settings_entry: Setting =
                    Message::parse_from_bytes(&entrypoint_settings_entry).map_err(|_e| {
                        ApplyError::InternalError("Invalid setting entry".to_string())
                    })?;

                let policy_name = policy_name_settings_entry
                    .get_entries()
                    .iter()
                    .next()
                    .ok_or_else(|| {
                        ApplyError::InternalError("Invalid setting entry".to_string())
                    })?;

                let policy_entrypoint = entrypoint_settings_entry
                    .get_entries()
                    .iter()
                    .next()
                    .ok_or_else(|| {
                        ApplyError::InternalError("Invalid setting entry".to_string())
                    })?;

                let policy_meta_address =
                    opa_tp_protocol::state::policy_meta_address(&policy_name.value);

                let policy_meta: Vec<u8> =
                    ctx.get_state_entry(&policy_meta_address)?.ok_or_else(|| {
                        ApplyError::InternalError(format!(
                            "Failed to load policy metadata for policy '{}' from '{}'",
                            policy_name.value, policy_meta_address
                        ))
                    })?;

                let policy_meta: opa_tp_protocol::state::PolicyMeta =
                    serde_json::from_slice(&policy_meta).map_err(|_e| {
                        ApplyError::InternalError(format!(
                            "Cannot parse policy meta for {}",
                            policy_name.value
                        ))
                    })?;

                debug!(policy_from_submission_meta = ?policy_meta);

                // Check if we have the policy loaded as an executor context and the
                // loaded policy version against the current policy
                // version. If either the policy is not loaded or the policy version is nor
                // current, load the policy from the chain and cache it
                if let Some((hash, executor_context)) =
                    self.on_chain.lock().unwrap().get(&policy_name.value)
                {
                    if *hash == policy_meta.hash {
                        return Ok(executor_context.clone());
                    }
                }

                // Load the policy from the chain
                let policy_bytes = ctx
                    .get_state_entry(&opa_tp_protocol::state::policy_address(&policy_name.value))?
                    .ok_or_else(|| {
                        ApplyError::InternalError(format!(
                            "Failed to load policy for policy {}",
                            policy_name.value
                        ))
                    })?;

                let loader = CliPolicyLoader::from_policy_bytes(
                    &policy_name.value,
                    &policy_entrypoint.value,
                    &policy_bytes,
                )
                .map_err(|e| ApplyError::InternalError(e.to_string()))?;

                let ctx = ExecutorContext::from_loader(&loader)
                    .map_err(|e| ApplyError::InternalError(e.to_string()))?;

                self.on_chain
                    .lock()
                    .unwrap()
                    .insert(policy_name.value.clone(), (policy_meta.hash, ctx.clone()));

                Ok(ctx)
            }
            _ => Err(ApplyError::InternalError(
                "Opa policy settings are invalid".to_string(),
            )),
        }
    }
}
