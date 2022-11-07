#![cfg_attr(feature = "strict", deny(warnings))]
pub mod address;
pub mod client;
pub mod events;
pub mod sawtooth;
pub mod state;
pub mod submission;

static PROTOCOL_VERSION: &str = "1";

// generated from ./protos/
pub mod messages {
    #![allow(clippy::derive_partial_eq_without_eq)]

    include!(concat!(env!("OUT_DIR"), "/_.rs"));
}
