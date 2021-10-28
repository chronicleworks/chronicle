use custom_error::custom_error;

use crate::models::ChronicleTransaction;

custom_error! {pub SubmissionError
    Implementation{source: Box<dyn std::error::Error>} = "Ledger error",
}

pub trait LedgerWriter {
    fn submit(&self, tx: Vec<&ChronicleTransaction>) -> Result<(), SubmissionError>;
}

/// An in memory ledger implementation for development and testing purposes
#[derive(Debug, Default)]
pub struct InMemLedger {}

impl LedgerWriter for InMemLedger {
    fn submit(&self, _tx: Vec<&ChronicleTransaction>) -> Result<(), SubmissionError> {
        Ok(())
    }
}
