use std::string::FromUtf8Error;

use thiserror::Error;

use crate::{ledger::BlockIdError, messages::message::MessageType};

#[derive(Error, Debug)]
pub enum SawtoothCommunicationError {
    #[error("ZMQ error: {0}")]
    ZMQ(#[from] zmq::Error),

    #[error("Send error: {0}")]
    SendMpsc(#[from] futures::channel::mpsc::SendError),

    #[error("Timeout error: {0}")]
    Elapsed(#[from] tokio::time::error::Elapsed),

    #[error("Oneshot receive error: {0}")]
    ReceiveOneshot(#[from] tokio::sync::oneshot::error::RecvError),
    #[error("Send socket command")]
    SendSocketCommand,

    #[error("Protobuf decode error: {0}")]
    ProtobufDecode(#[from] prost::DecodeError),
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

    #[error("Resource not found")]
    ResourceNotFound,

    #[error("Invalid message type: expected {expected:?}, got {got:?}")]
    InvalidMessageType {
        expected: MessageType,
        got: MessageType,
    },

    #[error("No connected validators")]
    NoConnectedValidators,

    #[error("Sending to disconnected validator")]
    SendingToDisconnectedValidator,

    #[error("Tmq {0}")]
    TmqError(#[from] tmq::TmqError),
}
