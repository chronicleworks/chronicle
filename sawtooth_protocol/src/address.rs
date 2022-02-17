use std::fmt::Display;

use common::ledger::LedgerAddress;
use crypto::digest::Digest;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref PREFIX: String = {
        let mut sha = crypto::sha2::Sha256::new();
        sha.input_str("chronicle");
        sha.result_str()[..6].to_string()
    };
}

pub struct SawtoothAddress(String);

/// Our sawtooth addresses use hash(chronicle)[..6] as the prefix,
/// followed by a 256 bit hash of the resource Iri and namespace Iri.
impl From<&LedgerAddress> for SawtoothAddress {
    fn from(addr: &LedgerAddress) -> Self {
        let mut sha = crypto::sha2::Sha256::new();
        addr.namespace.as_ref().map(|ns| sha.input_str(ns));
        sha.input_str(&addr.resource);
        SawtoothAddress(format!("{}{}", &*PREFIX, sha.result_str()))
    }
}

impl Display for SawtoothAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
