use common::{
    ledger::Offset,
    prov::{ChronicleOperation, ChronicleTransactionId},
};
use custom_error::custom_error;
use k256::ecdsa::{signature::Signer, Signature, SigningKey};
use openssl::sha::Sha512;
use prost::Message;
use rand::{prelude::StdRng, Rng, SeedableRng};
use tracing::{debug, instrument};

use crate::{address::PREFIX, sawtooth::event_filter::FilterType};

use super::sawtooth::*;

custom_error! {pub MessageBuilderError
    Serialize{source: serde_cbor::Error}                              = "Could not serialize as CBOR",
}

#[derive(Debug, Clone)]
pub struct MessageBuilder {
    signer: SigningKey,
    family_name: String,
    family_version: String,
    rng: StdRng,
}

impl MessageBuilder {
    #[allow(dead_code)]
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

    #[allow(dead_code)]
    pub fn make_subcription_request(&self, offset: &Offset) -> ClientEventsSubscribeRequest {
        let mut request = ClientEventsSubscribeRequest::default();

        let mut delta_subscription = EventSubscription::default();
        let filter_address = EventFilter {
            key: "address".to_string(),
            match_string: (*PREFIX).to_string(),
            filter_type: FilterType::RegexAll as _,
        };

        delta_subscription.filters = vec![filter_address];
        delta_subscription.event_type = "chronicle/prov-update".to_owned();

        let block_subscription = EventSubscription {
            event_type: "sawtooth/block-commit".to_owned(),
            filters: vec![],
        };

        offset.map(|offset| {
            request.last_known_block_ids = vec![offset.to_string()];
        });

        request.subscriptions = vec![delta_subscription, block_subscription];

        request
    }

    pub fn wrap_tx_as_sawtooth_batch(&self, tx: Transaction) -> Batch {
        let mut batch = Batch::default();

        let mut header = BatchHeader::default();

        let pubkey = hex::encode_upper(self.signer.verifying_key().to_bytes());
        header.transaction_ids = vec![tx.header_signature.clone()];
        header.signer_public_key = pubkey;

        let encoded_header = header.encode_to_vec();
        let s: Signature = self.signer.sign(&*encoded_header);

        batch.transactions = vec![tx];
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
        payload: &[ChronicleOperation],
    ) -> (Transaction, ChronicleTransactionId) {
        let bytes = serde_cbor::to_vec(&payload).unwrap();

        let mut hasher = Sha512::new();
        hasher.update(&*bytes);

        let pubkey = hex::encode_upper(self.signer.verifying_key().to_bytes());

        let header = TransactionHeader {
            payload_sha512: hex::encode_upper(hasher.finish()),
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

        (
            Transaction {
                header: encoded_header,
                header_signature: hex::encode_upper(s.as_ref()),
                payload: bytes,
            },
            ChronicleTransactionId::from(s),
        )
    }
}

#[cfg(test)]
mod test {
    use common::prov::{vocab::Chronicle, ChronicleOperation, CreateNamespace};
    use k256::{ecdsa::SigningKey, SecretKey};
    use prost::Message;
    use rand::prelude::StdRng;
    use rand_core::SeedableRng;
    use sawtooth_sdk::messages::batch::Batch;
    use uuid::Uuid;

    use super::MessageBuilder;

    #[test]
    fn sawtooth_batch_roundtrip() {
        let secret = SecretKey::random(StdRng::from_entropy());
        let mut builder = MessageBuilder::new(SigningKey::from(secret), "name", "version");

        let uuid = Uuid::new_v4();

        let batch = vec![ChronicleOperation::CreateNamespace(CreateNamespace {
            id: Chronicle::namespace("t", &uuid).into(),
            name: "t".to_owned(),
            uuid,
        })];

        let input_addresses = vec!["inone".to_owned(), "intwo".to_owned()];
        let output_addresses = vec!["outtwo".to_owned(), "outtwo".to_owned()];
        let dependencies = vec!["dependency".to_owned()];

        let (proto_tx, _id) = builder.make_sawtooth_transaction(
            input_addresses,
            output_addresses,
            dependencies,
            batch.as_slice(),
        );

        let batch = builder.wrap_tx_as_sawtooth_batch(proto_tx);

        let _batch_sdk_parsed: Batch =
            protobuf::Message::parse_from_bytes(&*batch.encode_to_vec()).unwrap();
    }
}
