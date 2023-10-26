use crate::{identity::SignedIdentity, prov::operations::ChronicleOperation};
#[cfg(not(feature = "std"))]
use parity_scale_codec::alloc::vec::Vec;

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct ChronicleTransaction {
	pub tx: Vec<ChronicleOperation>,
	pub identity: SignedIdentity,
}

impl ChronicleTransaction {
	pub fn new(tx: Vec<ChronicleOperation>, identity: SignedIdentity) -> Self {
		Self { tx, identity }
	}
}
