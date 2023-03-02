#![cfg_attr(feature = "strict", deny(warnings))]

use async_sawtooth_sdk::{ledger::SawtoothLedger, zmq_client::ZmqRequestResponseSawtoothChannel};
use state::OpaOperationEvent;
use transaction::OpaSubmitTransaction;
pub mod address;
pub mod events;
pub mod state;
pub mod submission;
pub mod transaction;

pub use async_sawtooth_sdk;

static PROTOCOL_VERSION: &str = "1";

pub type OpaLedger =
    SawtoothLedger<ZmqRequestResponseSawtoothChannel, OpaOperationEvent, OpaSubmitTransaction>;

// generated from ./protos/
pub mod messages {
    #![allow(clippy::derive_partial_eq_without_eq)]

    include!(concat!(env!("OUT_DIR"), "/_.rs"));
}
