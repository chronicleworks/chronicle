use async_stl_client::{
	ledger::SawtoothLedger,
	zmq_client::{RetryingRequestResponseChannel, ZmqRequestResponseSawtoothChannel},
};
use messages::ChronicleSubmitTransaction;

pub mod address;
pub mod messages;
pub mod protocol;
pub mod settings;

pub use async_stl_client;
use protocol::ChronicleOperationEvent;

static PROTOCOL_VERSION: &str = "2";
const SUBMISSION_BODY_VERSION: u16 = 1;

pub type ChronicleLedger = SawtoothLedger<
	RetryingRequestResponseChannel<ZmqRequestResponseSawtoothChannel>,
	ChronicleOperationEvent,
	ChronicleSubmitTransaction,
>;

pub mod sawtooth {
	#![allow(clippy::derive_partial_eq_without_eq)]

	include!(concat!(env!("OUT_DIR"), "/_.rs"));
}
