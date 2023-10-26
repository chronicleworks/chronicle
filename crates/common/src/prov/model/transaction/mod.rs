pub mod v1;
pub mod v2;

// This is the current version.
pub use v2::ChronicleTransaction;

impl From<v1::ChronicleTransaction> for v2::ChronicleTransaction {
	fn from(transaction: v1::ChronicleTransaction) -> Self {
		v2::ChronicleTransaction::new(transaction.tx, transaction.identity)
	}
}

pub trait HasVersion {
	fn get_version(&self) -> u16;
}

impl HasVersion for v1::ChronicleTransaction {
	fn get_version(&self) -> u16 {
		1
	}
}

impl HasVersion for v2::ChronicleTransaction {
	fn get_version(&self) -> u16 {
		2
	}
}

pub const CURRENT_VERSION: u16 = 2;

pub trait ToChronicleTransaction {
	fn to_current(self) -> ChronicleTransaction;
}

impl ToChronicleTransaction for v1::ChronicleTransaction {
	fn to_current(self) -> ChronicleTransaction {
		self.into()
	}
}

impl ToChronicleTransaction for v2::ChronicleTransaction {
	fn to_current(self) -> ChronicleTransaction {
		self
	}
}

#[cfg(test)]
mod test {
	use super::{HasVersion, ToChronicleTransaction};
	use crate::identity::SignedIdentity;

	#[test]
	fn transaction_versions() {
		let transaction_v1 =
			super::v1::ChronicleTransaction::new(Vec::default(), SignedIdentity::new_no_identity());
		let transaction_v2: super::v2::ChronicleTransaction = transaction_v1.clone().into();
		let transaction_current = transaction_v2.clone();

		// check that the above sequence ends at the current version
		assert_eq!(super::CURRENT_VERSION, transaction_current.get_version());

		// check the reported version numbers
		assert_eq!(1, transaction_v1.get_version());
		assert_eq!(2, transaction_v2.get_version());

		// check the conversions to current
		assert_eq!(transaction_current, transaction_v1.to_current());
		assert_eq!(transaction_current, transaction_v2.to_current());
	}
}
