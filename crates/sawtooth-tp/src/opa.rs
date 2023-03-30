use chronicle_protocol::protocol::messages::OpaPolicy;
use common::opa::{CliPolicyLoader, ExecutorContext};
use sawtooth_sdk::processor::handler::{ApplyError, TransactionContext};
use std::{
    cell::RefCell,
    collections::HashMap,
    sync::{Arc, Mutex},
};

#[derive(Debug)]
pub struct TpOpa {
    pub embedded: ExecutorContext,
    pub on_chain: Arc<Mutex<HashMap<String, (u64, ExecutorContext)>>>,
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

    pub fn executor_context_from_submission(
        &self,
        policy: Option<OpaPolicy>,
        ctx: &mut dyn TransactionContext,
    ) -> Result<ExecutorContext, ApplyError> {
        if let Some(policy) = policy {
            // Load on chain opa policy metadata if its id has been supplied in the submission
            // failure to do so is a hard, but resumable error
            let policy_meta: Vec<u8> = ctx
                .get_state_entry(&opa_tp_protocol::state::policy_meta_address(&policy.id))?
                .ok_or_else(|| {
                    ApplyError::InternalError(format!(
                        "Failed to load policy metadata for policy {}",
                        policy.id
                    ))
                })?;

            let policy_meta: opa_tp_protocol::state::PolicyMeta =
                serde_json::from_slice(&policy_meta).map_err(|e| {
                    ApplyError::InternalError(format!("Cannot parse policy meta for {}", policy.id))
                })?;

            // Check if we have the policy loaded as an executor context and the
            // loaded policy version against the current policy
            // version. If either the policy is not loaded or the policy version is nor
            // current, load the policy from the chain and cache it
            if let Some((version, executor_context)) = self.on_chain.lock().unwrap().get(&policy.id)
            {
                if *version == policy_meta.version {
                    return Ok(executor_context.clone());
                }
            }

            // Load the policy from the chain
            let policy_bytes = ctx
                .get_state_entry(&opa_tp_protocol::state::policy_address(&policy.id))?
                .ok_or_else(|| {
                    ApplyError::InternalError(format!(
                        "Failed to load policy for policy {}",
                        policy.id
                    ))
                })?;

            let ctx = ExecutorContext::from_loader(&CliPolicyLoader::from_policy_bytes(
                &policy.entrypoint,
                &policy_bytes,
            ))
            .map_err(|e| ApplyError::InternalError(e.to_string()))?;

            self.on_chain
                .lock()
                .unwrap()
                .insert(policy.id, (policy_meta.version, ctx.clone()));

            Ok(ctx)
        } else {
            Ok(self.embedded.clone())
        }
    }
}
