use std::collections::BTreeMap;

use common::{
    ledger::OperationState,
    protocol::{chronicle_operations_from_submission, deserialize_submission},
    prov::ProvModel,
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
        let submission = deserialize_submission(request.get_payload())
            .map_err(|e| ApplyError::InternalError(e.to_string()))?;

        let _protocol_version = submission.version;

        let _span_id = submission.span_id;

        let submission_body = submission.body;

        let (send, recv) = crossbeam::channel::bounded(1);

        Handle::current().spawn(async move {
            send.send(
                chronicle_operations_from_submission(submission_body)
                    .await
                    .map_err(|e| ApplyError::InternalError(e.to_string())),
            )
        });

        let transactions = recv
            .recv()
            .map_err(|e| ApplyError::InternalError(e.to_string()))??;

        let mut model = ProvModel::default();

        let mut state = OperationState::new();

        for tx in transactions {
            debug!(operation = ?tx);

            let deps = tx.dependencies();

            // order of `get_state_entries` should be in order in which sent
            let sawtooth_entries = context
                .get_state_entries(
                    &deps
                        .iter()
                        .map(|x| SawtoothAddress::from(x).to_string())
                        .collect::<Vec<_>>(),
                )?
                .into_iter()
                .collect::<Vec<_>>()
                .into_iter()
                .collect::<BTreeMap<_, _>>();

            let entries = sawtooth_entries
                .into_iter()
                .enumerate()
                .map(|(i, (_k, v))| (deps[i].clone(), v))
                .collect::<BTreeMap<_, _>>()
                .into_iter();

            state.append_input(entries);

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

            state.append_output(
                tx_output
                    .into_iter()
                    .map(|output| output.into())
                    .collect::<BTreeMap<_, _>>()
                    .into_iter(),
            );

            model = updated_model;
        }

        context.set_state_entries(
            state
                .dirty()
                .map(|output| {
                    let address = output.address;
                    (SawtoothAddress::from(&address).to_string(), output.data)
                })
                .collect(),
        )?;

        // Events should be after state updates, generally
        context.add_event(
            "chronicle/prov-update".to_string(),
            vec![("transaction_id".to_owned(), request.signature.clone())],
            &*serde_cbor::to_vec(&model)
                .map_err(|e| ApplyError::InvalidTransaction(e.to_string()))?,
        )?;

        Ok(())
    }
}
