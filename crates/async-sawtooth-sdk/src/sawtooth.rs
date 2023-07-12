use std::{
    error::Error,
    sync::{Arc, Mutex},
};

use k256::{
    ecdsa::{Signature, SigningKey},
    schnorr::signature::Signer,
    sha2::{Digest, Sha256, Sha512},
};
use prost::Message;
use rand::{prelude::StdRng, Rng, SeedableRng};
use tracing::{debug, instrument, trace};

use crate::{
    ledger::{BlockId, TransactionId},
    messages::{
        event_filter::FilterType, Batch, BatchHeader, ClientBlockGetByNumRequest,
        ClientBlockListRequest, ClientEventsSubscribeRequest, ClientPagingControls,
        ClientSortControls, ClientStateGetRequest, EventFilter, EventSubscription, Transaction,
        TransactionHeader,
    },
};

#[async_trait::async_trait]
pub trait TransactionPayload {
    type Error: Error;
    async fn to_bytes(&self) -> Result<Vec<u8>, Self::Error>;
}

#[async_trait::async_trait]
impl<T: prost::Message> TransactionPayload for T {
    type Error = prost::EncodeError;

    async fn to_bytes(&self) -> Result<Vec<u8>, Self::Error> {
        Ok(self.encode_to_vec())
    }
}

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
            block_ids: vec![],
            paging: Some(ClientPagingControls {
                limit: 1,
                ..Default::default()
            }),
            sorting: vec![ClientSortControls {
                reverse: false,
                keys: vec!["block_num".to_string()],
            }],
            ..Default::default()
        }
    }

    pub fn get_first_block_id_request(&self) -> ClientBlockGetByNumRequest {
        ClientBlockGetByNumRequest { block_num: 1 }
    }

    pub fn make_state_request(&self, address: &str) -> ClientStateGetRequest {
        ClientStateGetRequest {
            address: address.to_owned(),
            ..Default::default()
        }
    }

    // Create a `ClientEventsSubscribeRequest` for the given offset and event
    // type. sawtooth/block-commit events are always subscribed to.
    pub fn make_subscription_request(
        &self,
        from_block_id: &BlockId,
        event_types: Vec<String>,
    ) -> ClientEventsSubscribeRequest {
        let mut request = ClientEventsSubscribeRequest::default();

        let filter_address = EventFilter {
            key: "address".to_string(),
            match_string: self.prefix.clone(),
            filter_type: FilterType::RegexAll as _,
        };

        let mut operation_subscriptions = event_types
            .into_iter()
            .map(|event_type| EventSubscription {
                filters: vec![filter_address.clone()],
                event_type,
            })
            .collect::<Vec<_>>();

        let block_subscription = EventSubscription {
            event_type: "sawtooth/block-commit".to_owned(),
            filters: vec![],
        };

        if let BlockId::Block(_) = from_block_id {
            request.last_known_block_ids = vec![from_block_id.to_string()];
        }

        operation_subscriptions.push(block_subscription);

        request.subscriptions = operation_subscriptions;

        request
    }

    #[instrument(skip(self))]
    pub fn wrap_tx_as_sawtooth_batch(&self, tx: Transaction, signer: &SigningKey) -> Batch {
        let mut batch = Batch::default();

        let mut header = BatchHeader::default();

        let pubkey = hex::encode(signer.verifying_key().to_bytes());
        header.transaction_ids = vec![tx.header_signature.clone()];
        header.signer_public_key = pubkey;

        let encoded_header = header.encode_to_vec();
        let s: Signature = signer.sign(&encoded_header);
        let s = s.normalize_s().unwrap_or(s);
        let s = hex::encode(s.as_ref());

        trace!(batch_header=?header, batch_header_signature=?s, transactions = ?tx);

        batch.transactions = vec![tx];
        batch.header = encoded_header;
        batch.header_signature = s;

        batch
    }

    #[instrument(skip(payload, signer), level = "trace")]
    pub async fn make_sawtooth_transaction<P: TransactionPayload + std::fmt::Debug>(
        &self,
        input_addresses: Vec<String>,
        output_addresses: Vec<String>,
        dependencies: Vec<String>,
        payload: &P,
        signer: &SigningKey,
    ) -> (Transaction, TransactionId) {
        let bytes = payload.to_bytes().await.unwrap();

        let mut hasher = Sha512::new();
        hasher.update(&bytes);

        let pubkey = hex::encode(signer.verifying_key().to_bytes());

        let header = TransactionHeader {
            batcher_public_key: pubkey.clone(),
            dependencies,
            family_name: self.family_name.clone(),
            family_version: self.family_version.clone(),
            inputs: input_addresses,
            nonce: self.generate_nonce(),
            outputs: output_addresses,
            payload_sha512: hex::encode(hasher.finalize()),
            signer_public_key: pubkey,
        };

        let encoded_header = header.encode_to_vec();
        let s: Signature = signer.sign(&encoded_header);
        let s = s.normalize_s().unwrap_or(s);

        let s = hex::encode(s.to_vec());

        debug!(transaction_header=?header, transaction_header_signature=?s);

        (
            Transaction {
                header: encoded_header,
                header_signature: s.clone(),
                payload: bytes,
            },
            TransactionId::new(s),
        )
    }
}
