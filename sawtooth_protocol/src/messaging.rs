use crate::address::SawtoothAddress;
use crate::messages::MessageBuilder;
use crate::sawtooth::ClientBatchSubmitRequest;

use common::ledger::{LedgerWriter, SubmissionError};
use common::prov::ChronicleTransaction;
use custom_error::*;
use derivative::Derivative;
use k256::ecdsa::SigningKey;
use prost::Message as ProstMessage;

use sawtooth_sdk::messages::validator::Message_MessageType;
use sawtooth_sdk::messaging::stream::{MessageSender, ReceiveError, SendError};
use sawtooth_sdk::messaging::{
    stream::{MessageConnection, MessageReceiver},
    zmq_stream::{ZmqMessageConnection, ZmqMessageSender},
};
use tokio::task::JoinError;
use tracing::instrument;
use tracing::{debug, trace};

#[derive(Derivative)]
#[derivative(Debug)]
pub struct SawtoothSubmitter {
    #[derivative(Debug = "ignore")]
    tx: ZmqMessageSender,
    rx: MessageReceiver,
    builder: MessageBuilder,
}

pub enum SubmissionResult {
    Accepted,
}

custom_error! {pub SawtoothSubmissionError
    Send{source: SendError}                                 = "Submission failed to send to validator",
    Recv{source: ReceiveError}                              = "Submission failed to send to validator",
    UnexpectedReply{}                                       = "Validator reply unexpected",
    Join{source: JoinError}                                 = "Submission blocking thread pool",
}

impl Into<SubmissionError> for SawtoothSubmissionError {
    fn into(self) -> SubmissionError {
        SubmissionError::Implementation {
            source: Box::new(self),
        }
    }
}

/// The sawtooth futures and their sockets are not controlled by a compatible reactor
impl SawtoothSubmitter {
    pub fn new(address: &url::Url, signer: &SigningKey) -> Self {
        let builder = MessageBuilder::new(signer.to_owned(), "chronicle", "1.0");
        let (tx, rx) = ZmqMessageConnection::new(address.as_str()).create();
        SawtoothSubmitter { tx, rx, builder }
    }

    #[instrument]
    fn submit(
        &mut self,
        transactions: Vec<&ChronicleTransaction>,
    ) -> Result<(), SawtoothSubmissionError> {
        let transactions = transactions
            .iter()
            .map(|payload| {
                // Symetric input / output addresses for now, this can be optimised if needed
                let addresses = payload
                    .dependencies()
                    .iter()
                    .map(SawtoothAddress::from)
                    .map(|addr| addr.to_string())
                    .collect::<Vec<_>>();

                self.builder.make_sawtooth_transaction(
                    addresses.clone(),
                    addresses,
                    vec![],
                    payload,
                )
            })
            .collect();

        debug!(?transactions, "Create batch");

        let batch = self.builder.make_sawtooth_batch(transactions);

        trace!(?batch, "Validator request");

        let mut request = ClientBatchSubmitRequest::default();

        request.batches = vec![batch];

        let mut future = self.tx.send(
            Message_MessageType::CLIENT_BATCH_SUBMIT_REQUEST,
            &uuid::Uuid::new_v4().to_string(),
            &*request.encode_to_vec(),
        )?;

        let result = future.get_timeout(std::time::Duration::from_secs(10))?;

        debug!(?result, "Validator response");

        if result.message_type == Message_MessageType::CLIENT_BATCH_SUBMIT_RESPONSE {
            Ok(())
        } else {
            Err(SawtoothSubmissionError::UnexpectedReply {})
        }
    }
}

#[async_trait::async_trait(?Send)]
impl LedgerWriter for SawtoothSubmitter {
    /// TODO: This blocks on a bunch of non tokio / futures 'futures' in the sawtooth rust SDK,
    /// which also exposes a buch of non clonable types so we probably need another dispatch / join mpsc here
    async fn submit(&mut self, tx: Vec<&ChronicleTransaction>) -> Result<(), SubmissionError> {
        self.submit(tx).map_err(SawtoothSubmissionError::into)
    }
}
