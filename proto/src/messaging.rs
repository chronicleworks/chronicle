use std::future::Future;

use crate::messages::MessageBuilder;
use common::models::ChronicleTransaction;
use crypto::digest::Digest;
use custom_error::*;
use derivative::Derivative;
use k256::ecdsa::SigningKey;
use prost::Message as ProstMessage;
use sawtooth_sdk::messages::processor::TpProcessRequest;
use sawtooth_sdk::messages::validator::Message;
use sawtooth_sdk::messages::validator::Message_MessageType;
use sawtooth_sdk::messaging::stream::{
    MessageFuture, MessageResult, MessageSender, ReceiveError, SendError,
};
use sawtooth_sdk::{
    messaging::{
        stream::{MessageConnection, MessageReceiver},
        zmq_stream::{ZmqMessageConnection, ZmqMessageSender},
    },
    processor::handler::{ApplyError, TransactionContext, TransactionHandler},
};
use tracing::instrument;

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

custom_error! {pub SubmissionError
    Send{source: SendError}                              = "Submission failed to send to validator",
    Recv{source: ReceiveError}                           = "Submission failed to send to validator",
    UnexpectedReply{}                                    = "Validator reply unexpected",
}

impl SawtoothValidator {
    pub fn new(address: &url::Url, signer: &SigningKey) -> Self {
        let builder = MessageBuilder::new(signer.to_owned(), "chronicle", "1.0");
        let (tx, rx) = ZmqMessageConnection::new(address.as_str()).create();
        SawtoothValidator { tx, rx, builder }
    }

    #[instrument]
    pub fn submit(
        &self,
        transactions: Vec<ChronicleTransaction>,
    ) -> Result<SubmissionResult, SubmissionError> {
        let transactions = transactions
            .iter()
            .map(|payload| {
                self.builder
                    .make_sawtooth_transaction(vec![], vec![], vec![], &payload)
            })
            .collect();

        let batch = self.builder.make_sawtooth_batch(transactions);

        let mut future = self.tx.send(
            Message_MessageType::CLIENT_BATCH_SUBMIT_REQUEST,
            &uuid::Uuid::new_v4().to_string(),
            &*batch.encode_to_vec(),
        )?;

        let result = future.get_timeout(std::time::Duration::from_secs(10))?;

        if result.message_type == Message_MessageType::CLIENT_BATCH_SUBMIT_RESPONSE {
            Ok(SubmissionResult::Accepted)
        } else {
            Err(SubmissionError::UnexpectedReply {})
        }
    }
}

pub fn get_prefix() -> String {
    let mut sha = crypto::sha2::Sha512::new();
    sha.input_str("chronicle");
    sha.result_str()[..6].to_string()
}

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
            namespaces: vec![get_prefix()],
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

    fn apply(
        &self,
        _request: &TpProcessRequest,
        _context: &mut dyn TransactionContext,
    ) -> Result<(), ApplyError> {
        Ok(())
    }
}
