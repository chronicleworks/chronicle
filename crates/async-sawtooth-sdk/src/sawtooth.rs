use std::sync::{Arc, Mutex};

use k256::{
    ecdsa::{Signature, SigningKey},
    schnorr::signature::Signer,
    sha2::{Digest, Sha256},
};
use openssl::sha::Sha512;
use prost::Message;
use protobuf::Message as ProtobufMessage;
use rand::{prelude::StdRng, Rng, SeedableRng};
use sawtooth_sdk::messages::{
    batch::{Batch, BatchHeader},
    client_block::ClientBlockListRequest,
    client_event::ClientEventsSubscribeRequest,
    client_list_control::{ClientPagingControls, ClientSortControls},
    client_state::ClientStateGetRequest,
    events::{EventFilter, EventFilter_FilterType, EventSubscription},
    transaction::{Transaction, TransactionHeader},
};
use tracing::{debug, instrument};

use crate::ledger::{BlockId, TransactionId};

#[derive(Debug, Clone)]
pub struct MessageBuilder {
    family_name: String,
    family_version: String,
    prefix: String,
    rng: Arc<Mutex<StdRng>>,
}

impl MessageBuilder {
    pub fn new(family_name: &str, family_version: &str) -> Self {
        let mut sha = Sha256::new();
        sha.update(family_name.as_bytes());
        let prefix = hex::encode(sha.finalize())[..6].to_string();

        let rng = StdRng::from_entropy();
        Self {
            family_name: family_name.to_owned(),
            family_version: family_version.to_owned(),
            prefix,
            rng: Mutex::new(rng).into(),
        }
    }
    pub fn new_deterministic(family_name: &str, family_version: &str) -> Self {
        let mut sha = Sha256::new();
        sha.update(family_name.as_bytes());
        let prefix = hex::encode(sha.finalize())[..6].to_string();
        let rng = StdRng::from_seed([0; 32]);
        Self {
            family_name: family_name.to_owned(),
            family_version: family_version.to_owned(),
            prefix,
            rng: Mutex::new(rng).into(),
        }
    }
    fn generate_nonce(&self) -> String {
        let bytes = self.rng.lock().unwrap().gen::<[u8; 20]>();
        hex::encode(bytes)
    }

    // Issue a block list request with no ids specified, a reverse order, and a
    // limit of 1.
    pub fn make_block_height_request(&self) -> ClientBlockListRequest {
        ClientBlockListRequest {
            block_ids: vec![].into(),
            paging: Some(ClientPagingControls {
                limit: 1,
                ..Default::default()
            })
            .into(),
            sorting: vec![ClientSortControls {
                keys: vec!["block_num".to_owned()].into(),
                reverse: true,
                ..Default::default()
            }]
            .into(),
            ..Default::default()
        }
    }

    pub fn make_state_request(&self, address: &str) -> ClientStateGetRequest {
        ClientStateGetRequest {
            address: address.to_owned(),
            ..Default::default()
        }
    }

    pub fn make_subscription_request(
        &self,
        block_id: &Option<BlockId>,
        event_types: Vec<String>,
    ) -> ClientEventsSubscribeRequest {
        let mut request = ClientEventsSubscribeRequest::default();

        let filter_address = EventFilter {
            key: "address".to_string(),
            match_string: self.prefix.clone(),
            filter_type: EventFilter_FilterType::REGEX_ALL as _,
            ..Default::default()
        };

        let mut operation_subscriptions = event_types
            .into_iter()
            .map(|event_type| EventSubscription {
                filters: vec![filter_address.clone()].into(),
                event_type,
                ..Default::default()
            })
            .collect::<Vec<_>>();

        let block_subscription = EventSubscription {
            event_type: "sawtooth/block-commit".to_owned(),
            filters: vec![].into(),
            ..Default::default()
        };

        if let Some(block_id) = block_id.as_ref() {
            request.last_known_block_ids = vec![block_id.to_string()].into();
        }

        operation_subscriptions.push(block_subscription);

        request.subscriptions = operation_subscriptions.into();

        request
    }

    #[instrument(skip(self))]
    pub fn wrap_tx_as_sawtooth_batch(&self, tx: Transaction, signer: &SigningKey) -> Batch {
        let mut batch = Batch::default();

        let mut header = BatchHeader::default();

        let pubkey = hex::encode(signer.verifying_key().to_bytes());
        header.transaction_ids = vec![tx.header_signature.clone()].into();
        header.signer_public_key = pubkey;

        let mut encoded_header = vec![];
        ProtobufMessage::write_to_vec(&header, &mut encoded_header).unwrap();
        let s: Signature = signer.sign(&encoded_header);
        let s = s.normalize_s().unwrap_or(s);
        let s = hex::encode(s.as_ref());

        debug!(batch_header=?header, batch_header_signature=?s, transactions = ?tx);

        batch.transactions = vec![tx].into();
        batch.header = encoded_header;
        batch.header_signature = s;
        batch.trace = true;

        batch
    }

    #[instrument]
    pub async fn make_sawtooth_transaction<M: Message>(
        &self,
        input_addresses: Vec<String>,
        output_addresses: Vec<String>,
        dependencies: Vec<String>,
        payload: &M,
        signer: &SigningKey,
    ) -> (Transaction, TransactionId) {
        let bytes = payload.encode_to_vec();

        let mut hasher = Sha512::new();
        hasher.update(&bytes);

        let pubkey = hex::encode(signer.verifying_key().to_bytes());

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
        let s: Signature = signer.sign(&encoded_header);
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
            TransactionId::new(s),
        )
    }
}
