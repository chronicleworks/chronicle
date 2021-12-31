use crate::messages::MessageBuilder;

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
use tracing::instrument;
use tracing::{debug, trace};

#[derive(Derivative)]
#[derivative(Debug)]
pub struct SawtoothValidator {
    #[derivative(Debug = "ignore")]
    tx: ZmqMessageSender,
    rx: MessageReceiver,
    builder: MessageBuilder,
}

pub enum SubmissionResult {
    Accepted,
}

custom_error! {pub SawtoothSubmissionError
    Send{source: SendError}                              = "Submission failed to send to validator",
    Recv{source: ReceiveError}                           = "Submission failed to send to validator",
    UnexpectedReply{}                                    = "Validator reply unexpected",
}

impl Into<SubmissionError> for SawtoothSubmissionError {
    fn into(self) -> SubmissionError {
        SubmissionError::Implementation {
            source: Box::new(self),
        }
    }
}

/// The sawtooth futures and their soickets are not controlled by a compatible reactor
impl SawtoothValidator {
    pub fn new(address: &url::Url, signer: &SigningKey) -> Self {
        let builder = MessageBuilder::new(signer.to_owned(), "chronicle", "1.0");
        let (tx, rx) = ZmqMessageConnection::new(address.as_str()).create();
        SawtoothValidator { tx, rx, builder }
    }

    #[instrument]
    fn submit(
        &mut self,
        transactions: Vec<&ChronicleTransaction>,
    ) -> Result<(), SawtoothSubmissionError> {
        let transactions = transactions
            .iter()
            .map(|payload| {
                self.builder
                    .make_sawtooth_transaction(vec![], vec![], vec![], payload)
            })
            .collect();

        debug!(?transactions, "Create batch");

        let batch = self.builder.make_sawtooth_batch(transactions);

        trace!(?batch, "Validator request");

        let mut future = self.tx.send(
            Message_MessageType::CLIENT_BATCH_SUBMIT_REQUEST,
            &uuid::Uuid::new_v4().to_string(),
            &*batch.encode_to_vec(),
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
impl LedgerWriter for SawtoothValidator {
    async fn submit(&mut self, tx: Vec<&ChronicleTransaction>) -> Result<(), SubmissionError> {
        self.submit(tx).map_err(SawtoothSubmissionError::into)
    }
}
