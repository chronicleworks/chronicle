use async_sawtooth_sdk::{ledger::SawtoothLedger, zmq_client::ZmqRequestResponseSawtoothChannel};
use messages::ChronicleSubmitTransaction;
use protocol::ChronicleOperationEvent;

pub mod address;
pub mod messages;
pub mod protocol;
pub mod settings;

pub use async_sawtooth_sdk;

static PROTOCOL_VERSION: &str = "1";

pub type ChronicleLedger = SawtoothLedger<
    ZmqRequestResponseSawtoothChannel,
    ChronicleOperationEvent,
    ChronicleSubmitTransaction,
>;

pub mod sawtooth {
    #![allow(clippy::derive_partial_eq_without_eq)]

    include!(concat!(env!("OUT_DIR"), "/_.rs"));
}
