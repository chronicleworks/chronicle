use std::collections::BTreeMap;

use common::{
    ledger::StateInput,
    prov::{ChronicleOperation, ProvModel},
};
use sawtooth_protocol::address::{SawtoothAddress, PREFIX};

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

    #[instrument(
        name = "Process transaction",
        skip(request,context),
        fields(
            correlation_id = %request.context_id,
        )
    )]
    fn apply(
        &self,
        request: &TpProcessRequest,
        context: &mut dyn TransactionContext,
    ) -> Result<(), ApplyError> {
        let tx: Vec<ChronicleOperation> = serde_cbor::from_slice(request.get_payload())
            .map_err(|e| ApplyError::InternalError(e.to_string()))?;

        let mut model = ProvModel::default();
        let mut output = vec![];

        for tx in tx {
            debug!(?tx, "Processing");

            let deps = tx.dependencies();

            debug!(?deps, "Input addresses");

            let input = context
                .get_state_entries(
                    &deps
                        .iter()
                        .map(|x| SawtoothAddress::from(x).to_string())
                        .collect::<Vec<_>>(),
                )?
                .into_iter()
                .map(|(_, data)| StateInput::new(data))
                .collect();

            debug!(?input, "Processing input state");

            let (send, recv) = crossbeam::channel::bounded(1);
            Handle::current().spawn(async move {
                send.send(
                    tx.process(model, input)
                        .await
                        .map_err(|e| ApplyError::InternalError(e.to_string())),
                )
            });

            let (mut tx_output, updated_model) = recv
                .recv()
                .map_err(|e| ApplyError::InternalError(e.to_string()))??;

            output.append(&mut tx_output);
            model = updated_model;
        }

        //Merge state output (last update wins) and sort by address, so push into a btree then iterate back to a vector
        let output = output
            .into_iter()
            .map(|state| (state.address.clone(), state))
            .collect::<BTreeMap<_, _>>()
            .into_iter()
            .map(|x| x.1)
            .collect::<Vec<_>>();

        debug!(?output, "Storing output state");

        context.add_event(
            "chronicle/prov-update".to_string(),
            vec![],
            &*serde_cbor::to_vec(&model)
                .map_err(|e| ApplyError::InvalidTransaction(e.to_string()))?,
        )?;

        context.set_state_entries(
            output
                .into_iter()
                .map(|output| {
                    (
                        SawtoothAddress::from(&output.address).to_string(),
                        output.data,
                    )
                })
                .collect(),
        )?;

        Ok(())
    }
}
