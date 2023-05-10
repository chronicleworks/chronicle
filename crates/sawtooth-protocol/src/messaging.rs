use std::collections::HashSet;

use crate::{
    address::SawtoothAddress,
    messages::MessageBuilder,
    sawtooth::{Batch, Transaction},
};

use common::{
    k256::ecdsa::SigningKey,
    protocol::ProtocolError,
    prov::{ChronicleTransaction, ChronicleTransactionId, ProcessorError},
};
use custom_error::*;

use sawtooth_sdk::messaging::stream::{ReceiveError, SendError};
use tokio::task::JoinError;
use tracing::debug;

#[derive(Debug)]
pub struct OperationMessageBuilder {
    builder: MessageBuilder,
}

impl OperationMessageBuilder {
    pub fn new(signer: &SigningKey, family: &str, version: &str) -> Self {
        OperationMessageBuilder {
            builder: MessageBuilder::new(signer.to_owned(), family, version),
        }
    }

    pub async fn make_tx(
        &mut self,
        transactions: &ChronicleTransaction,
    ) -> Result<(Transaction, ChronicleTransactionId), ProtocolError> {
        let addresses = transactions
            .tx
            .iter()
            .flat_map(|tx| tx.dependencies())
            .map(|addr| (SawtoothAddress::from(&addr).to_string(), addr))
            .collect::<HashSet<_>>();

        debug!(address_map = ?addresses);

        self.builder
            .make_sawtooth_transaction(
                addresses.iter().map(|x| x.0.clone()).collect(),
                addresses.into_iter().map(|x| x.0).collect(),
                vec![],
                transactions,
            )
            .await
    }

    pub fn wrap_tx_as_sawtooth_batch(&mut self, tx: Transaction) -> Batch {
        self.builder.wrap_tx_as_sawtooth_batch(tx)
    }

    pub async fn make_sawtooth_transaction(
        &mut self,
        inputs: Vec<String>,
        outputs: Vec<String>,
        dependencies: Vec<String>,
        payload: &ChronicleTransaction,
    ) -> Result<(Transaction, ChronicleTransactionId), ProtocolError> {
        self.builder
            .make_sawtooth_transaction(inputs, outputs, dependencies, payload)
            .await
    }
}

custom_error! {pub SawtoothSubmissionError
    Send{source: SendError}                                 = "Submission failed to send to validator",
    Recv{source: ReceiveError}                              = "Submission failed to send to validator",
    UnexpectedStatus{status: i32}                           = "Validator status unexpected {status}",
    Join{source: JoinError}                                 = "Submission blocking thread pool",
    Ld{source: ProcessorError}                              = "Json LD processing",
    Protocol{source: ProtocolError}                         = "Protocol {source}",
}
