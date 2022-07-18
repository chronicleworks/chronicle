use std::fmt::Display;

use common::{ledger::LedgerAddress, prov::AsCompact};
use lazy_static::lazy_static;
use openssl::sha::Sha256;

lazy_static! {
    pub static ref PREFIX: String = {
        let mut sha = Sha256::new();
        sha.update("chronicle".as_bytes());
        hex::encode(sha.finish())[..6].to_string()
    };
}

pub static VERSION: &str = "1.0";
pub static FAMILY: &str = "chronicle";

pub struct SawtoothAddress(String);

/// Our sawtooth addresses use hash(chronicle)[..6] as the prefix,
/// followed by a 256 bit hash of the resource Iri and namespace Iri.
impl From<&LedgerAddress> for SawtoothAddress {
    fn from(addr: &LedgerAddress) -> Self {
        let mut sha = Sha256::new();
        if let Some(ns) = addr.namespace_part().as_ref() {
            sha.update(ns.compact().as_bytes())
        }
        sha.update(addr.resource_part().compact().as_bytes());
        SawtoothAddress(format!("{}{}", &*PREFIX, hex::encode(sha.finish())))
    }
}

impl Display for SawtoothAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
