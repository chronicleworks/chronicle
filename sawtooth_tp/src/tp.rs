use std::collections::BTreeMap;

use common::{
    ledger::StateInput,
    prov::{operations::ChronicleOperation, ProvModel},
};
use sawtooth_protocol::address::{SawtoothAddress, FAMILY, PREFIX, VERSION};

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
            family_name: FAMILY.to_owned(),
            family_versions: vec![VERSION.to_owned()],
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

    #[instrument(
        name = "Process transaction",
        skip(request,context),
        fields(
            correlation_id = %request.context_id,
            inputs = ?request.header.as_ref().map(|x| &x.inputs),
            outputs = ?request.header.as_ref().map(|x| &x.outputs),
            dependencies = ?request.header.as_ref().map(|x| &x.dependencies)
        )
    )]
    fn apply(
        &self,
        request: &TpProcessRequest,
        context: &mut dyn TransactionContext,
    ) -> Result<(), ApplyError> {
        let tx: Vec<ChronicleOperation> = serde_cbor::from_slice(request.get_payload())
            .map_err(|e| ApplyError::InternalError(e.to_string()))?;

        //Prepare all state inputs for the transactions
        let mut state = {
            context
                .get_state_entries(
                    &tx.iter()
                        .flat_map(|tx| tx.dependencies())
                        .map(|x| SawtoothAddress::from(&x).to_string())
                        .collect::<Vec<_>>(),
                )?
                .into_iter()
                .collect::<BTreeMap<_, _>>()
        };

        debug!(state_inputs = ?state.keys());

        let mut model = ProvModel::default();

        for tx in tx {
            debug!(operation = ?tx);
            let input = {
                tx.dependencies()
                    .iter()
                    .flat_map(|x| {
                        state
                            .get(&SawtoothAddress::from(&*x).to_string())
                            .map(|input| StateInput::new(input.clone()))
                    })
                    .collect()
            };

            let (send, recv) = crossbeam::channel::bounded(1);
            Handle::current().spawn(async move {
                send.send(
                    tx.process(model, input)
                        .await
                        .map_err(|e| ApplyError::InternalError(e.to_string())),
                )
            });

            let (tx_output, updated_model) = recv
                .recv()
                .map_err(|e| ApplyError::InternalError(e.to_string()))??;

            for output in tx_output {
                let address = SawtoothAddress::from(&output.address).to_string();
                debug!(state_output = ?output.address,
                       state_output_sawtooth = ?&address);
                *state.entry(address).or_insert_with(|| output.data.clone()) = output.data.clone();
            }

            model = updated_model;
        }

        context.add_event(
            "chronicle/prov-update".to_string(),
            vec![],
            &*serde_cbor::to_vec(&model)
                .map_err(|e| ApplyError::InvalidTransaction(e.to_string()))?,
        )?;

        context.set_state_entries(state.into_iter().collect())?;

        Ok(())
    }
}
