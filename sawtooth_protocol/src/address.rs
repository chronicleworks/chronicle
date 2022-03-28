use std::fmt::Display;

use common::ledger::LedgerAddress;
use lazy_static::lazy_static;
use openssl::sha::Sha256;

lazy_static! {
    pub static ref PREFIX: String = {
        let mut sha = Sha256::new();
        sha.update("chronicle".as_bytes());
        hex::encode_upper(sha.finish())[..6].to_string()
    };
}

pub struct SawtoothAddress(String);

/// Our sawtooth addresses use hash(chronicle)[..6] as the prefix,
/// followed by a 256 bit hash of the resource Iri and namespace Iri.
impl From<&LedgerAddress> for SawtoothAddress {
    fn from(addr: &LedgerAddress) -> Self {
        let mut sha = Sha256::new();
        if let Some(ns) = addr.namespace.as_ref() {
            sha.update(ns.as_bytes())
        }
        sha.update(&addr.resource.as_bytes());
        SawtoothAddress(format!("{}{}", &*PREFIX, hex::encode_upper(sha.finish())))
    }
}

impl Display for SawtoothAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
