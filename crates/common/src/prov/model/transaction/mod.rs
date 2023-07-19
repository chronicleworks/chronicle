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
    fn get_version(self) -> u16;
}

impl HasVersion for v1::ChronicleTransaction {
    fn get_version(self) -> u16 {
        1
    }
}

impl HasVersion for v2::ChronicleTransaction {
    fn get_version(self) -> u16 {
        2
    }
}

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
