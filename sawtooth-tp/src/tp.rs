use common::{
    ledger::{LedgerAddress, OperationState, SubmissionError},
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
            transaction_id = %request.signature,
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
        let transactions: Vec<ChronicleOperation> =
            serde_cbor::from_slice(request.get_payload())
                .map_err(|e| ApplyError::InternalError(e.to_string()))?;

        let mut model = ProvModel::default();

        let mut state = OperationState::new();

        let mut state_deps: Vec<LedgerAddress> = vec![];

        for tx in transactions {
            debug!(operation = ?tx);

            let deps = tx.dependencies();

            let entries = context
                .get_state_entries(
                    &deps
                        .iter()
                        .map(|x| SawtoothAddress::from(x).to_string())
                        .collect::<Vec<_>>(),
                )?
                .into_iter();

            state.append_input(
                &deps
                    .iter()
                    .map(|x| SawtoothAddress::from(x).to_string())
                    .collect::<Vec<_>>(),
                entries,
            );

            let input = state.input();

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

            state.append_output(tx_output);

            model = updated_model;

            state_deps.append(
                &mut deps
                    .into_iter()
                    .filter(|d| !state_deps.contains(d))
                    .collect::<Vec<_>>(),
            );
        }

        context.add_event(
            "chronicle/prov-update".to_string(),
            vec![("transaction_id".to_owned(), request.signature.clone())],
            &*serde_cbor::to_vec(&model)
                .map_err(|e| ApplyError::InvalidTransaction(e.to_string()))?,
        )?;

        context.set_state_entries(
            state
                .dirty()
                .map(|output| output.address_is_specified(&state_deps))
                .collect::<Result<Vec<_>, SubmissionError>>()
                .into_iter()
                .flat_map(|v| v.into_iter())
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
