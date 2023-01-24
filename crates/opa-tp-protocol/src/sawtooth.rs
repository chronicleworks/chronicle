use k256::{
    ecdsa::{Signature, SigningKey},
    schnorr::signature::Signer,
};
use openssl::sha::Sha512;
use prost::Message;
use protobuf::Message as ProtobufMessage;
use rand::{prelude::StdRng, Rng, SeedableRng};
use sawtooth_sdk::messages::{
    batch::{Batch, BatchHeader},
    transaction::{Transaction, TransactionHeader},
};
use tracing::{debug, instrument};

use crate::{messages::Submission, submission::OpaTransactionId};

#[derive(Debug, Clone)]
pub struct MessageBuilder {
    signer: SigningKey,
    family_name: String,
    family_version: String,
    rng: StdRng,
}

impl MessageBuilder {
    pub fn new(signer: SigningKey, family_name: &str, family_version: &str) -> Self {
        let rng = StdRng::from_entropy();
        Self {
            signer,
            family_name: family_name.to_owned(),
            family_version: family_version.to_owned(),
            rng,
        }
    }

    fn generate_nonce(&mut self) -> String {
        let bytes = self.rng.gen::<[u8; 20]>();
        hex::encode(bytes)
    }

    pub fn wrap_tx_as_sawtooth_batch(&self, tx: Transaction) -> Batch {
        let mut batch = Batch::default();

        let mut header = BatchHeader::default();

        let pubkey = hex::encode(self.signer.verifying_key().to_bytes());
        header.transaction_ids = vec![tx.header_signature.clone()].into();
        header.signer_public_key = pubkey;

        let mut encoded_header = vec![];
        ProtobufMessage::write_to_vec(&header, &mut encoded_header).unwrap();
        let s: Signature = self.signer.sign(&encoded_header);
        let s = s.normalize_s().unwrap_or(s);
        let s = hex::encode(s.as_ref());

        debug!(batch_header=?header, batch_header_signature=?s);

        batch.transactions = vec![tx].into();
        batch.header = encoded_header;
        batch.header_signature = s;
        batch.trace = true;

        batch
    }

    #[instrument]
    pub fn make_sawtooth_transaction(
        &mut self,
        input_addresses: Vec<String>,
        output_addresses: Vec<String>,
        dependencies: Vec<String>,
        payload: Submission,
    ) -> (Transaction, OpaTransactionId) {
        let bytes = payload.encode_to_vec();

        let mut hasher = Sha512::new();
        hasher.update(&bytes);

        let pubkey = hex::encode(self.signer.verifying_key().to_bytes());

        let header = TransactionHeader {
            batcher_public_key: pubkey.clone(),
            dependencies: dependencies.into(),
            family_name: self.family_name.clone(),
            family_version: self.family_version.clone(),
            inputs: input_addresses.into(),
            nonce: self.generate_nonce(),
            outputs: output_addresses.into(),
            payload_sha512: hex::encode(hasher.finish()),
            signer_public_key: pubkey,
            ..Default::default()
        };

        let mut encoded_header = vec![];
        ProtobufMessage::write_to_vec(&header, &mut encoded_header).unwrap();
        let s: Signature = self.signer.sign(&encoded_header);
        let s = s.normalize_s().unwrap_or(s);

        let s = hex::encode(s.to_vec());

        debug!(transaction_header=?header, transaction_header_signature=?s);

        (
            Transaction {
                header: encoded_header,
                header_signature: s.clone(),
                payload: bytes,
                ..Default::default()
            },
            OpaTransactionId::new(s),
        )
    }
}

#[cfg(test)]
mod test {
    use crate::submission::SubmissionBuilder;

    use super::MessageBuilder;
    use k256::{ecdsa::SigningKey, SecretKey};
    use openssl::sha::Sha512;
    use protobuf::Message as ProtoMessage;
    use rand::rngs::StdRng;
    use rand_core::SeedableRng;
    use sawtooth_sdk::messages::{batch::Batch, transaction::TransactionHeader};

    fn key_from_seed(seed: u8) -> SigningKey {
        let secret: SigningKey = SecretKey::random(StdRng::from_seed([seed; 32])).into();

        secret
    }

    #[test]
    fn sawtooth_batch_roundtrip() {
        let signing_key = key_from_seed(0);

        let submission = SubmissionBuilder::bootstrap_root(signing_key.verifying_key()).build(1);
        let mut builder = MessageBuilder::new(signing_key, "external_id", "version");

        let input_addresses = vec!["inone".to_owned(), "intwo".to_owned()];
        let output_addresses = vec!["outone".to_owned(), "outtwo".to_owned()];
        let dependencies = vec!["dependency".to_owned()];

        let (proto_tx, _id) = builder.make_sawtooth_transaction(
            input_addresses,
            output_addresses,
            dependencies,
            submission,
        );

        let batch = builder.wrap_tx_as_sawtooth_batch(proto_tx);

        let mut bytes = vec![];
        // Serialize, then deserialize the batch
        protobuf::Message::write_to_vec(&batch, &mut bytes).unwrap();
        let batch_sdk_parsed: Batch = protobuf::Message::parse_from_bytes(&bytes).unwrap();

        for tx in batch_sdk_parsed.transactions {
            let header = TransactionHeader::parse_from_bytes(tx.header.as_slice()).unwrap();

            let mut hasher = Sha512::new();
            hasher.update(&tx.payload);
            let computed_hash = hasher.finish();

            assert_eq!(header.payload_sha512, hex::encode(computed_hash));
        }
    }
}
