use std::string::FromUtf8Error;

use futures::channel::mpsc::SendError;
use protobuf::ProtobufError;
use sawtooth_sdk::messaging::stream::ReceiveError;
use thiserror::Error;

use crate::ledger::BlockIdError;

#[derive(Error, Debug)]
pub enum SawtoothCommunicationError {
    #[error("ZMQ error: {0}")]
    ZMQ(#[from] zmq::Error),

    #[error("Send error: {0}")]
    SendChannel(#[from] SendError),

    #[error("Sawtooth receive error: {0}")]
    ReceiveSawtooth(#[from] ReceiveError),

    #[error("Sawtooth send error: {0}")]
    SendSawtooth(#[from] sawtooth_sdk::messaging::stream::SendError),

    #[error("Protobuf error: {0}")]
    Protobuf(#[from] ProtobufError),
    #[error("Protobuf decode error: {0}")]
    ProtobufProst(#[from] prost::DecodeError),
    #[error("Unexpected Status: {status:?}")]
    UnexpectedStatus { status: i32 },
    #[error("No transaction id for event")]
    MissingTransactionId,
    #[error("Cannot determine block number for event")]
    MissingBlockNum,
    #[error("Cannot determine block id for event")]
    MissingBlockId,
    #[error("Unexpected message structure")]
    MalformedMessage,
    #[error("Json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Subscribe error: {code}")]
    SubscribeError { code: i32 },
    #[error("Block number is not number: {source}")]
    BlockNumNotNumber {
        #[from]
        source: std::num::ParseIntError,
    },
    #[error("No blocks returned when searching for current block")]
    NoBlocksReturned,
    #[error("Ledger event parse error: {source}")]
    LedgerEventParse {
        #[from]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("Utf8 {source}")]
    Utf8 {
        #[from]
        source: FromUtf8Error,
    },

    #[error("BlockId {source}")]
    BlockId {
        #[from]
        source: BlockIdError,
    },

    #[error("Infallible {0}")]
    Infallible(#[from] std::convert::Infallible),
}
