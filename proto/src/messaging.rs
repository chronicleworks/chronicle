use common::models::ChronicleTransaction;
use crypto::digest::Digest;
use custom_error::*;
use sawtooth_sdk::{
    messages::processor::TpProcessRequest,
    messaging::{
        stream::{MessageConnection, MessageReceiver},
        zmq_stream::{ZmqMessageConnection, ZmqMessageSender},
    },
    processor::handler::{ApplyError, TransactionContext, TransactionHandler},
};

pub struct SawtoothValidator {
    tx: ZmqMessageSender,
    rx: MessageReceiver,
}

pub enum SubmissionResult {
    Accepted,
}

custom_error! {pub SubmissionError
    Unknown{}                           = "Submission failed",
}

impl SawtoothValidator {
    pub fn new(address: &url::Url) -> Self {
        let (tx, rx) = ZmqMessageConnection::new(address.as_str()).create();
        SawtoothValidator { tx, rx }
    }

    pub fn submit(
        &self,
        _transactions: Vec<ChronicleTransaction>,
    ) -> Result<SubmissionResult, SubmissionError> {
        Ok(SubmissionResult::Accepted)
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
            family_name: "xo".into(),
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
