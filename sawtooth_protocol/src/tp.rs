use crate::address::{SawtoothAddress, PREFIX};
use common::{ledger::StateInput, prov::ChronicleTransaction};

use k256::ecdsa::VerifyingKey;

use sawtooth_sdk::{
    messages::processor::TpProcessRequest,
    processor::handler::{ApplyError, TransactionContext, TransactionHandler},
};
use tokio::runtime::Handle;
use tracing::{debug, instrument};

#[derive(Debug)]
pub struct ChronicleTransactionHandler {
    family_name: String,
    family_versions: Vec<String>,
    namespaces: Vec<String>,
}

impl ChronicleTransactionHandler {
    pub fn new() -> ChronicleTransactionHandler {
        ChronicleTransactionHandler {
            family_name: "chronicle".into(),
            family_versions: vec!["1.0".into()],
            namespaces: vec![PREFIX.to_string()],
        }
    }
}

impl TransactionHandler for ChronicleTransactionHandler {
    fn family_name(&self) -> String {
        self.family_name.clone()
    }

    fn family_versions(&self) -> Vec<String> {
        self.family_versions.clone()
    }

    fn namespaces(&self) -> Vec<String> {
        self.namespaces.clone()
    }

    #[instrument(skip(request, context))]
    fn apply(
        &self,
        request: &TpProcessRequest,
        context: &mut dyn TransactionContext,
    ) -> Result<(), ApplyError> {
        request
            .header
            .clone()
            .map(|h| {
                VerifyingKey::from_sec1_bytes(
                    &hex::decode(h.signer_public_key)
                        .map_err(|e| ApplyError::InvalidTransaction(e.to_string()))?,
                )
                .map_err(|e| ApplyError::InvalidTransaction(e.to_string()))
            })
            .into_option()
            .ok_or(ApplyError::InvalidTransaction(String::from(
                "Invalid header, missing signer public key",
            )))?
            .ok();

        let tx: ChronicleTransaction = serde_cbor::from_slice(request.get_payload())
            .map_err(|e| ApplyError::InternalError(e.to_string()))?;

        let deps = tx.dependencies();

        let input = context
            .get_state_entries(
                &deps
                    .iter()
                    .map(|x| SawtoothAddress::from(x).into())
                    .collect::<Vec<_>>(),
            )?
            .into_iter()
            .map(|(_, data)| StateInput::new(data))
            .collect();

        debug!(?input, "Processing input state");

        let (send, recv) = crossbeam::channel::bounded(1);
        Handle::current().spawn(async move {
            send.send(
                tx.process(input)
                    .await
                    .map_err(|e| ApplyError::InternalError(e.to_string())),
            )
        });

        let output = recv
            .recv()
            .map_err(|e| ApplyError::InternalError(e.to_string()))??;

        debug!(?output, "Storing output state");

        context.set_state_entries(
            output
                .into_iter()
                .map(|output| (SawtoothAddress::from(&output.address).into(), output.data))
                .collect(),
        )?;

        Ok(())
    }
}
