use crate::{identity::SignedIdentity, prov::operations::ChronicleOperation};

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct ChronicleTransaction {
    pub tx: Vec<ChronicleOperation>,
    pub identity: SignedIdentity,
}

impl ChronicleTransaction {
    pub fn new(tx: Vec<ChronicleOperation>, identity: SignedIdentity) -> Self {
        Self { tx, identity }
    }
}
