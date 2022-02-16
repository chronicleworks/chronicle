use common::{ledger::Offset, prov::ChronicleTransaction};
use crypto::{digest::Digest, sha2::Sha512};
use custom_error::custom_error;
use k256::{
    ecdsa::signature::Signer,
    ecdsa::{Signature, SigningKey},
};
use prost::Message;
use rand::{prelude::StdRng, Rng, SeedableRng};
use tracing::{debug, instrument};

use crate::{address::PREFIX, sawtooth::event_filter::FilterType};

use super::sawtooth::*;

custom_error! {pub MessageBuilderError
    Serialize{source: serde_cbor::Error}                              = "Could not serialize as CBOR",
}

#[derive(Debug)]
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
        hex::encode_upper(bytes)
    }

    pub fn make_subcription_request(&self, offset: Offset) -> ClientEventsSubscribeRequest {
        let mut request = ClientEventsSubscribeRequest::default();

        request.last_known_block_ids = vec![offset.to_string()];
        let mut subscription = EventSubscription::default();
        let mut filter_address = EventFilter::default();

        filter_address.key = "address".to_string();
        filter_address.match_string = format!("{}", *PREFIX);
        filter_address.filter_type = FilterType::RegexAll as _;

        let mut filter_type = EventFilter::default();
        filter_type.match_string = "sawtooth/state-delta".to_owned();
        filter_type.filter_type = FilterType::RegexAll as _;

        subscription.filters = vec![filter_address, filter_type];
        request.subscriptions = vec![subscription];

        request
    }

    pub fn make_sawtooth_batch(&self, tx: Vec<Transaction>) -> Batch {
        let mut batch = Batch::default();

        let mut header = BatchHeader::default();

        let pubkey = hex::encode_upper(self.signer.verifying_key().to_bytes());
        header.transaction_ids = tx.iter().map(|tx| tx.header_signature.to_owned()).collect();
        header.signer_public_key = pubkey;

        let encoded_header = header.encode_to_vec();
        let s: Signature = self.signer.sign(&*encoded_header);

        batch.transactions = tx;
        batch.header = encoded_header;
        batch.header_signature = hex::encode_upper(s.as_ref());

        batch
    }

    #[instrument]
    pub fn make_sawtooth_transaction(
        &mut self,
        input_addresses: Vec<String>,
        output_addresses: Vec<String>,
        dependencies: Vec<String>,
        payload: &ChronicleTransaction,
    ) -> Transaction {
        let bytes = serde_cbor::to_vec(payload).unwrap();

        let mut hasher = Sha512::new();
        hasher.input(&*bytes);

        let pubkey = hex::encode_upper(self.signer.verifying_key().to_bytes());

        let header = TransactionHeader {
            payload_sha512: hasher.result_str(),
            family_name: self.family_name.clone(),
            family_version: self.family_version.clone(),
            nonce: self.generate_nonce(),
            batcher_public_key: pubkey.clone(),
            signer_public_key: pubkey,
            dependencies,
            inputs: input_addresses,
            outputs: output_addresses,
        };

        debug!(?header);

        let encoded_header = header.encode_to_vec();
        let s: Signature = self.signer.sign(&*encoded_header);

        Transaction {
            header: encoded_header,
            header_signature: hex::encode_upper(s.as_ref()),
            payload: bytes,
        }
    }
}

#[cfg(test)]
mod test {
    use common::prov::{vocab::Chronicle, ChronicleTransaction, CreateNamespace};
    use k256::{ecdsa::SigningKey, SecretKey};
    use prost::Message;
    use rand::prelude::StdRng;
    use rand_core::SeedableRng;
    use uuid::Uuid;

    use super::MessageBuilder;

    #[test]
    fn sawtooth_batch_roundtrip() {
        let secret = SecretKey::random(StdRng::from_entropy());
        let mut builder = MessageBuilder::new(SigningKey::from(secret), "name", "version");

        let uuid = Uuid::new_v4();

        let batch = vec![ChronicleTransaction::CreateNamespace(CreateNamespace {
            id: Chronicle::namespace("t", &uuid).into(),
            name: "t".to_owned(),
            uuid,
        })];

        let input_addresses = vec!["inone".to_owned(), "intwo".to_owned()];
        let output_addresses = vec!["outtwo".to_owned(), "outtwo".to_owned()];
        let dependencies = vec!["dependency".to_owned()];

        let proto_tx = batch
            .iter()
            .map(|tx| {
                builder.make_sawtooth_transaction(
                    input_addresses.clone(),
                    output_addresses.clone(),
                    dependencies.clone(),
                    tx,
                )
            })
            .collect();

        let batch = builder.make_sawtooth_batch(proto_tx);

        let _batch_sdk_parsed = protobuf::parse_from_bytes::<sawtooth_sdk::messages::batch::Batch>(
            &*batch.encode_to_vec(),
        )
        .unwrap();
    }
}
