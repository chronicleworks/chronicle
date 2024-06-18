use std::{convert::Infallible, net::AddrParseError};

use thiserror::Error;
use tokio::{sync::mpsc::error::SendError, task::JoinError};
use user_error::UFE;

use chronicle_signing::SecretError;
use common::{
	identity::IdentityError,
	ledger::SubmissionError,
	prov::{Contradiction, ProcessorError},
};
use protocol_substrate::SubxtClientError;

use crate::{chronicle_graphql, dispatch::ApiSendWithReply};

#[derive(Error, Debug)]
pub enum ApiError {
	#[error("Storage: {0:?}")]
	Store(
		#[from]
		#[source]
		chronicle_persistence::StoreError,
	),

	#[error("Storage: {0:?}")]
	ArrowService(#[source] anyhow::Error),

	#[error("Transaction failed: {0}")]
	Transaction(
		#[from]
		#[source]
		diesel::result::Error,
	),

	#[error("Invalid IRI: {0}")]
	Iri(
		#[from]
		#[source]
		iref::Error,
	),

	#[error("JSON-LD processing: {0}")]
	JsonLD(String),

	#[error("Signing: {0}")]
	Signing(
		#[from]
		#[source]
		SecretError,
	),

	#[error("No agent is currently in use, please call agent use or supply an agent in your call")]
	NoCurrentAgent,

	#[error("Api shut down before reply")]
	ApiShutdownRx,

	#[error("Api shut down before send: {0}")]
	ApiShutdownTx(
		#[from]
		#[source]
		SendError<ApiSendWithReply>,
	),

	#[error("Invalid socket address: {0}")]
	AddressParse(
		#[from]
		#[source]
		AddrParseError,
	),

	#[error("Connection pool: {0}")]
	ConnectionPool(
		#[from]
		#[source]
		r2d2::Error,
	),

	#[error("IO error: {0}")]
	InputOutput(
		#[from]
		#[source]
		std::io::Error,
	),

	#[error("Blocking thread pool: {0}")]
	Join(
		#[from]
		#[source]
		JoinError,
	),

	#[error("No appropriate activity to end")]
	NotCurrentActivity,

	#[error("Processor: {0}")]
	ProcessorError(
		#[from]
		#[source]
		ProcessorError,
	),

	#[error("Identity: {0}")]
	IdentityError(
		#[from]
		#[source]
		IdentityError,
	),

	#[error("Authentication endpoint error: {0}")]
	AuthenticationEndpoint(
		#[from]
		#[source]
		chronicle_graphql::AuthorizationError,
	),

	#[error("Substrate : {0}")]
	ClientError(
		#[from]
		#[source]
		SubxtClientError,
	),

	#[error("Submission : {0}")]
	Submission(
		#[from]
		#[source]
		SubmissionError,
	),

	#[error("Contradiction: {0}")]
	Contradiction(Contradiction),

	#[error("Embedded substrate: {0}")]
	EmbeddedSubstrate(anyhow::Error),
}

/// Ugly but we need this until ! is stable, see <https://github.com/rust-lang/rust/issues/64715>
impl From<Infallible> for ApiError {
	fn from(_: Infallible) -> Self {
		unreachable!()
	}
}

impl From<Contradiction> for ApiError {
	fn from(x: Contradiction) -> Self {
		Self::Contradiction(x)
	}
}

impl UFE for ApiError {}
