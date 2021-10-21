#[macro_use]
extern crate serde_derive;

pub mod messaging;

pub mod sawtooth {
    include!(concat!(env!("OUT_DIR"), "/_.rs"));
}

pub mod messages {

    use std::{cell::RefCell, rc::Rc};

    use common::models::ChronicleTransaction;
    use crypto::{digest::Digest, sha2::Sha512};
    use custom_error::custom_error;
    use k256::{
        ecdsa::signature::Signer,
        ecdsa::{Signature, SigningKey},
        PublicKey, Secp256k1,
    };
    use prost::Message;
    use rand::{prelude::StdRng, Rng, SeedableRng};

    use super::sawtooth::*;

    custom_error! {pub MessageBuilderError
        Serialize{source: serde_cbor::Error}                              = "Could not serialize as CBOR",
    }

    pub struct MessageBuilder {
        signer: SigningKey,
        family_name: String,
        family_version: String,
        rng: Rc<RefCell<StdRng>>,
    }

    impl MessageBuilder {
        pub fn new(signer: SigningKey, family_name: String, family_version: String) -> Self {
            let rng = StdRng::from_entropy();
            Self {
                signer,
                family_name,
                family_version,
                rng: Rc::new(rng.into()),
            }
        }

        fn generate_nonce(&self) -> String {
            let bytes = self.rng.borrow_mut().gen::<[u8; 20]>();
            hex::encode_upper(bytes)
        }

        pub fn make_sawtooth_transaction(
            &self,
            _input_addresses: Vec<String>,
            _output_addresses: Vec<String>,
            _dependencies: Vec<String>,
            payload: &ChronicleTransaction,
        ) -> Result<Transaction, MessageBuilderError> {
            let bytes = serde_cbor::to_vec(payload)?;

            let mut hasher = Sha512::new();
            hasher.input(&*bytes);

            let mut header = TransactionHeader::default();

            header.payload_sha512 = hasher.result_str();
            header.family_name = self.family_name.clone();
            header.family_version = self.family_version.clone();
            header.nonce = self.generate_nonce();

            let pubkey = hex::encode_upper(self.signer.verifying_key().to_bytes());

            header.batcher_public_key = pubkey.clone();
            header.signer_public_key = pubkey.clone();

            let encoded_header = header.encode_to_vec();
            let s: Signature = self.signer.sign(&*encoded_header);

            let mut tx = Transaction::default();
            tx.header = encoded_header;
            tx.header_signature = hex::encode_upper(s.as_ref());
            tx.payload = bytes;

            Ok(tx)
        }
    }
}
