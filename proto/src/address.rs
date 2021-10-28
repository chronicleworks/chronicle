use common::models::NamespaceId;

///
/// A namespace corresponds to a sawtooth address
pub struct SawtoothAddress(NamespaceId);

impl From<NamespaceId> for SawtoothAddress {
    fn from(ns: NamespaceId) -> Self {
        Self(ns)
    }
}

impl SawtoothAddress {}
