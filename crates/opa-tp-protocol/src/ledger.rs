use std::time::Duration;

use derivative::Derivative;
use futures::{
    future,
    stream::{self, BoxStream},
    StreamExt,
};

use k256::ecdsa::SigningKey;
use protobuf::Message;
use sawtooth_sdk::messages::{
    batch::Batch,
    block::BlockHeader,
    client_batch_submit::{
        ClientBatchSubmitRequest, ClientBatchSubmitResponse, ClientBatchSubmitResponse_Status,
    },
    client_block::{ClientBlockListResponse, ClientBlockListResponse_Status},
    client_event::{ClientEventsSubscribeResponse, ClientEventsSubscribeResponse_Status},
    client_state::{ClientStateGetResponse, ClientStateGetResponse_Status},
    events::EventList,
    transaction::Transaction,
    validator::Message_MessageType,
};
use serde::{Deserialize, Serialize};

use tracing::{debug, error, instrument};

use crate::{
    address::{FAMILY, VERSION},
    events::deserialize_opa_event,
    messages::Submission,
    sawtooth::MessageBuilder,
    state::{key_address, OpaOperationEvent},
    submission::OpaTransactionId,
    zmq_client::{RequestResponseSawtoothChannel, SawtoothCommunicationError},
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Offset {
    Genesis,
    Identity(u64),
}

impl PartialOrd for Offset {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (Offset::Genesis, Offset::Genesis) => Some(std::cmp::Ordering::Equal),
            (Offset::Genesis, Offset::Identity(_)) => Some(std::cmp::Ordering::Less),
            (Offset::Identity(_), Offset::Genesis) => Some(std::cmp::Ordering::Greater),
            (Offset::Identity(x), Offset::Identity(y)) => x.partial_cmp(y),
        }
    }
}

impl Offset {
    pub fn map<T, F>(&self, f: F) -> Option<T>
    where
        F: FnOnce(&u64) -> T,
    {
        if let Offset::Identity(x) = self {
            Some(f(x))
        } else {
            None
        }
    }

    pub fn distance(&self, other: &Self) -> u64 {
        match (self, other) {
            (Offset::Genesis, Offset::Genesis) => 0,
            (Offset::Genesis, Offset::Identity(_)) => 0,
            (Offset::Identity(_), Offset::Genesis) => 0,
            (Offset::Identity(x), Offset::Identity(y)) => x.saturating_sub(*y),
        }
    }
}

impl From<u64> for Offset {
    fn from(offset: u64) -> Self {
        let x = offset;
        Offset::Identity(x)
    }
}

#[async_trait::async_trait]
pub trait LedgerTransaction {
    type Id;

    fn signer(&self) -> &SigningKey;
    fn addresses(&self) -> Vec<String>;
    async fn as_sawtooth_tx(&self, message_builder: &MessageBuilder) -> (Transaction, Self::Id);
}

#[async_trait::async_trait(?Send)]
pub trait LedgerWriter<TX: LedgerTransaction> {
    type Error: std::error::Error;

    // Pre-submit is used to get the transaction id before submitting the transaction
    async fn pre_submit(&self, tx: &TX) -> Result<(TX::Id, Transaction), Self::Error>;

    // Submit is used to submit a transaction to the ledger
    async fn submit(&self, tx: Transaction, signer: &SigningKey) -> Result<(), Self::Error>;

    fn message_builder(&self) -> &MessageBuilder;
}

#[async_trait::async_trait]
pub trait LedgerReader<EV> {
    type Error: std::error::Error;

    async fn get_state_entry(&self, address: &str) -> Result<Vec<u8>, Self::Error>;

    async fn block_height(&self) -> Result<Offset, Self::Error>;
    /// Subscribe to state updates from this ledger, starting at `offset`, and
    /// ending the stream after `number_of_blocks` blocks have been processed.
    async fn state_updates(
        &self,
        // The offset to start from, or `None` to start from the current block
        from_offset: Option<Offset>,
        // The number of blocks to process before ending the stream
        number_of_blocks: Option<u64>,
    ) -> Result<BoxStream<EV>, Self::Error>;
}

#[derive(Derivative)]
#[derivative(Debug, Clone)]
pub struct OpaLedger<T: RequestResponseSawtoothChannel + Clone + Send + Sync> {
    builder: MessageBuilder,
    channel: T,
}

pub type OpaEvent = (OpaOperationEvent, OpaTransactionId, Offset, u64);

impl<T: RequestResponseSawtoothChannel + Clone + Send + Sync> OpaLedger<T> {
    pub fn new(channel: T) -> Self {
        let builder = MessageBuilder::new(FAMILY, VERSION);
        OpaLedger { builder, channel }
    }

    #[instrument(skip(self))]
    async fn submit_opa_operation(&self, batch: Batch) -> Result<(), SawtoothCommunicationError> {
        let request = ClientBatchSubmitRequest {
            batches: vec![batch].into(),
            ..Default::default()
        };
        let batch_response: ClientBatchSubmitResponse = self
            .channel
            .send_and_recv_one(
                request,
                sawtooth_sdk::messages::validator::Message_MessageType::CLIENT_BATCH_SUBMIT_REQUEST,
                std::time::Duration::from_secs(10),
            )
            .await?;
        if batch_response.status == ClientBatchSubmitResponse_Status::OK {
            Ok(())
        } else {
            Err(SawtoothCommunicationError::UnexpectedStatus {
                status: batch_response.status as i32,
            })
        }
    }

    #[instrument(skip(self))]
    async fn get_block_height(&self) -> Result<u64, SawtoothCommunicationError> {
        let request = self.builder.make_block_height_request();
        let response: ClientBlockListResponse = self
            .channel
            .send_and_recv_one(
                request,
                sawtooth_sdk::messages::validator::Message_MessageType::CLIENT_BLOCK_LIST_REQUEST,
                std::time::Duration::from_secs(10),
            )
            .await?;
        if response.status == ClientBlockListResponse_Status::OK {
            let block = response
                .get_blocks()
                .first()
                .ok_or(SawtoothCommunicationError::NoBlocksReturned)?;

            let header = BlockHeader::parse_from_bytes(&block.header)?;
            Ok(header.block_num)
        } else {
            Err(SawtoothCommunicationError::UnexpectedStatus {
                status: response.status as i32,
            })
        }
    }

    /// Subscribe to state delta events and then set up the event stream
    #[instrument(skip(self))]
    async fn get_state_from(
        &self,
        offset: &Offset,
    ) -> Result<BoxStream<OpaEvent>, SawtoothCommunicationError> {
        let request = self.builder.make_subscription_request(offset);

        let sub = self
            .channel
            .send_and_recv_one::<ClientEventsSubscribeResponse, _>(
                request,
                Message_MessageType::CLIENT_EVENTS_SUBSCRIBE_REQUEST,
                Duration::from_secs(10),
            )
            .await?;

        if sub.status != ClientEventsSubscribeResponse_Status::OK {
            return Err(SawtoothCommunicationError::SubscribeError {
                code: sub.status as i32,
            });
        }

        self.event_stream(*offset).await
    }

    async fn event_stream(
        &self,
        block: Offset,
    ) -> Result<BoxStream<OpaEvent>, SawtoothCommunicationError> {
        #[derive(Debug)]
        enum ParsedEvent {
            Block(u64),
            Operation(OpaOperationEvent, OpaTransactionId, u64),
        }

        let channel = self.channel.clone();

        let event_stream = channel.recv_stream::<EventList>().await?;

        let event_stream = event_stream.map(move |events| {
            debug!(?events, "Received events");
            let mut updates = vec![];
            for event in events.events {
                updates.push(match &*event.event_type {
                    "sawtooth/block-commit" => event
                        .attributes
                        .iter()
                        .find(|attr| attr.key == "block_num")
                        .ok_or(SawtoothCommunicationError::MissingBlockNum)
                        .and_then(|attr| attr.value.parse().map_err(Into::into))
                        .map(|attr| Some(ParsedEvent::Block(attr))),
                    "opa/operation" => {
                        let transaction_id = event
                            .attributes
                            .iter()
                            .find(|attr| attr.key == "transaction_id")
                            .ok_or(SawtoothCommunicationError::MissingTransactionId)
                            .map(|attr| OpaTransactionId::from(&*attr.value));

                        let event = deserialize_opa_event(&event.data)
                            .map_err(SawtoothCommunicationError::from);

                        transaction_id
                            .map_err(SawtoothCommunicationError::from)
                            .and_then(|transaction_id| {
                                event.map(|event| {
                                    let (operation, span) = event;
                                    Some(ParsedEvent::Operation(operation, transaction_id, span))
                                })
                            })
                    }
                    _ => Ok(None),
                });
            }

            debug!(?updates, "Parsed events");

            // Fold the updates into a vector of operations and their block num

            updates
                .into_iter()
                .fold((vec![], block), |(mut operations, block), event| {
                    match event {
                        // Next block num
                        Ok(Some(ParsedEvent::Block(next))) => (operations, Offset::from(next)),
                        Ok(Some(ParsedEvent::Operation(next_operation, transaction_id, span))) => {
                            operations.push((next_operation, transaction_id, span));
                            (operations, block)
                        }
                        Err(e) => {
                            error!(?e, "Parsing state update");
                            (operations, block)
                        }
                        _ => (operations, block),
                    }
                })
        });

        let events_with_block = event_stream
            .flat_map(|events_for_block| {
                stream::iter(events_for_block.0)
                    .map(move |event| (event.0, event.1, events_for_block.1, event.2))
            })
            .boxed();

        Ok(events_with_block)
    }
}

#[async_trait::async_trait]
impl<T: RequestResponseSawtoothChannel + Clone + Send + Sync> LedgerReader<OpaEvent>
    for OpaLedger<T>
{
    type Error = SawtoothCommunicationError;

    async fn get_state_entry(&self, address: &str) -> Result<Vec<u8>, Self::Error> {
        let request = self.builder.make_state_request(address);

        let response = self
            .channel
            .send_and_recv_one::<ClientStateGetResponse, _>(
                request,
                Message_MessageType::CLIENT_STATE_GET_REQUEST,
                Duration::from_secs(10),
            )
            .await?;

        if response.status == ClientStateGetResponse_Status::OK {
            Ok(response.value)
        } else {
            Err(SawtoothCommunicationError::UnexpectedStatus {
                status: response.status as i32,
            })
        }
    }

    async fn block_height(&self) -> Result<Offset, Self::Error> {
        let block = self.get_block_height().await?;
        Ok(Offset::from(block))
    }

    #[instrument(skip(self))]
    async fn state_updates(
        &self,
        from_offset: Option<Offset>,
        number_of_blocks: Option<u64>,
    ) -> Result<BoxStream<OpaEvent>, Self::Error> {
        let self_clone = self.clone();
        let from_offset = if let Some(offset) = from_offset {
            offset
        } else {
            self_clone.block_height().await?
        };

        let subscribe = self.get_state_from(&from_offset).await?;

        Ok(subscribe
            .take_while(move |(_, _, offset, _)| {
                future::ready(if let Some(number_of_blocks) = number_of_blocks {
                    offset.distance(&from_offset) <= number_of_blocks
                } else {
                    true
                })
            })
            .boxed())
    }
}

#[derive(Debug, Clone)]
pub enum OpaSubmitTransaction {
    BootstrapRoot(Submission, SigningKey),
    RotateRoot(Submission, SigningKey),
    RegisterKey(Submission, SigningKey, String),
    RotateKey(Submission, SigningKey, String),
}

impl OpaSubmitTransaction {
    pub fn bootstrap_root(submission: Submission, sawtooth_signer: &SigningKey) -> Self {
        Self::BootstrapRoot(submission, sawtooth_signer.to_owned())
    }

    pub fn rotate_root(submission: Submission, sawtooth_signer: &SigningKey) -> Self {
        Self::RotateRoot(submission, sawtooth_signer.to_owned())
    }

    pub fn register_key(
        name: impl AsRef<str>,
        submission: Submission,
        sawtooth_signer: &SigningKey,
    ) -> Self {
        Self::RegisterKey(
            submission,
            sawtooth_signer.to_owned(),
            name.as_ref().to_owned(),
        )
    }

    pub fn rotate_key(
        name: impl AsRef<str>,
        submission: Submission,
        sawtooth_signer: &SigningKey,
    ) -> Self {
        Self::RegisterKey(
            submission,
            sawtooth_signer.to_owned(),
            name.as_ref().to_owned(),
        )
    }
}

#[async_trait::async_trait]
impl LedgerTransaction for OpaSubmitTransaction {
    type Id = OpaTransactionId;

    fn signer(&self) -> &SigningKey {
        match self {
            Self::BootstrapRoot(_, signer) => signer,
            Self::RotateRoot(_, signer) => signer,
            Self::RegisterKey(_, signer, _) => signer,
            Self::RotateKey(_, signer, _) => signer,
        }
    }

    fn addresses(&self) -> Vec<String> {
        match self {
            Self::BootstrapRoot(_, _) => {
                vec![key_address("root")]
            }
            Self::RotateRoot(_, _) => {
                vec![key_address("root")]
            }
            Self::RegisterKey(_, _, name) => {
                vec![key_address(name.clone())]
            }
            Self::RotateKey(_, _, name) => {
                vec![key_address(name.clone())]
            }
        }
    }

    async fn as_sawtooth_tx(&self, message_builder: &MessageBuilder) -> (Transaction, Self::Id) {
        message_builder.make_sawtooth_transaction(
            self.addresses(),
            self.addresses(),
            vec![],
            match self {
                Self::BootstrapRoot(submission, _) => submission,
                Self::RotateRoot(submission, _) => submission,
                Self::RegisterKey(submission, _, _) => submission,
                Self::RotateKey(submission, _, _) => submission,
            },
            self.signer(),
        )
    }
}

#[async_trait::async_trait(?Send)]
impl<T: RequestResponseSawtoothChannel + Clone + Send + Sync> LedgerWriter<OpaSubmitTransaction>
    for OpaLedger<T>
{
    type Error = SawtoothCommunicationError;

    fn message_builder(&self) -> &MessageBuilder {
        &self.builder
    }

    async fn pre_submit(
        &self,
        tx: &OpaSubmitTransaction,
    ) -> Result<(OpaTransactionId, Transaction), Self::Error> {
        let (sawtooth_tx, id) = tx.as_sawtooth_tx(self.message_builder()).await;
        Ok((id, sawtooth_tx))
    }

    #[instrument(skip(self) ret(Debug))]
    async fn submit(&self, tx: Transaction, signer: &SigningKey) -> Result<(), Self::Error> {
        let batch = self.message_builder().wrap_tx_as_sawtooth_batch(tx, signer);
        self.submit_opa_operation(batch).await?;
        Ok(())
    }
}
