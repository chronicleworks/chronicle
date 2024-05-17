use std::{convert::Infallible, fs::File, io::Write, path::PathBuf, time::Duration};

use clap::ArgMatches;
use futures::{channel::oneshot, Future, FutureExt, StreamExt};
use k256::{
	pkcs8::{EncodePrivateKey, LineEnding},
	SecretKey,
};
use rand::rngs::StdRng;
use rand_core::SeedableRng;
use serde::Serialize;
use thiserror::Error;
use tokio::runtime::Handle;
use tracing::{debug, error, info, instrument, Instrument, Level, span};
use url::Url;
use user_error::UFE;
use uuid::Uuid;

use chronicle_signing::{OPA_PK, OpaKnownKeyNamesSigner, SecretError};
use cli::{configure_signing, Wait};
use common::{
	opa::{
		codec::{KeysV1, PolicyV1},
		Keys,
		Policy, std::{FromUrlError, key_address, load_bytes_from_url},
	},
	prov::ChronicleTransactionId,
};
use protocol_abstract::{FromBlock, LedgerReader, LedgerWriter};
use protocol_substrate::{PolkadotConfig, SubstrateStateReader, SubxtClientError};
use protocol_substrate_opa::{
	OpaEvent,
	OpaSubstrateClient,
	submission_builder::SubmissionBuilder, transaction::{OpaTransaction, TransactionError},
};

mod cli;

#[cfg(test)]
mod test;

#[derive(Error, Debug)]
pub enum OpaCtlError {
    #[error("Operation cancelled {0}")]
    Cancelled(oneshot::Canceled),

    #[error("Communication error: {0}")]
    Communication(
        #[from]
        #[source]
        SubxtClientError,
    ),

    #[error("IO error: {0}")]
    IO(
        #[from]
        #[source]
        std::io::Error,
    ),

    #[error("Json error: {0}")]
    Json(
        #[from]
        #[source]
        serde_json::Error,
    ),

    #[error("Pkcs8 error")]
    Pkcs8,

    #[error("Transaction failed: {0}")]
    TransactionFailed(String),

    #[error("Transaction not found after wait: {0}")]
    TransactionNotFound(ChronicleTransactionId),

    #[error("Error loading from URL: {0}")]
    Url(
        #[from]
        #[source]
        FromUrlError,
    ),

    #[error("Utf8 error: {0}")]
    Utf8(
        #[from]
        #[source]
        std::str::Utf8Error,
    ),

    #[error("Signing: {0}")]
    Signing(
        #[from]
        #[source]
        SecretError,
    ),

    #[error("Missing Argument")]
    MissingArgument(String),

    #[error("Not found")]
    NotFound,

    #[error("Could not build transaction {0}")]
    InvalidTransaction(
        #[from]
        #[source]
        TransactionError,
    ),
}

impl From<Infallible> for OpaCtlError {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}

impl UFE for OpaCtlError {}

#[derive(Debug, Serialize)]
pub enum Waited {
    NoWait,
    WaitedAndFound(OpaEvent),
    WaitedAndDidNotFind,
}

// Collect incoming transaction ids before running submission, as there is the
/// potential to miss transactions if we do not collect them 'before' submission
async fn ambient_transactions<
    R: LedgerReader<Event=OpaEvent, Error=SubxtClientError> + Send + Sync + Clone + 'static,
>(
    client: &R,
    goal_tx_id: ChronicleTransactionId,
    max_steps: u32,
) -> impl Future<Output=Result<Waited, oneshot::Canceled>> {
    let span = span!(Level::DEBUG, "wait_for_opa_transaction");
    let client = client.clone();
    // Set up a oneshot channel to notify the returned task
    let (notify_tx, notify_rx) = oneshot::channel::<Waited>();

    // And a oneshot channel to ensure we are receiving events from the chain
    // before we return
    let (receiving_events_tx, receiving_events_rx) = oneshot::channel::<()>();

    Handle::current().spawn(async move {
        // We can immediately return if we are not waiting
        debug!(waiting_for=?goal_tx_id, max_steps=?max_steps);
        let goal_clone = goal_tx_id;

        let mut stream = loop {
            let stream = client
                .state_updates(
                    FromBlock::Head,
                    Some(max_steps),
                )
                .await;

            if let Ok(stream) = stream {
                break stream;
            }
            if let Err(e) = stream {
                error!(subscribe_to_events=?e);
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        };

        receiving_events_tx.send(()).ok();

        loop {
            futures::select! {
              next_block = stream.next().fuse() => {
                if let Some((op,tx, block_id, position,_)) = next_block {
                info!(goal_tx_found=tx==goal_clone,tx=?tx, goal=%goal_clone, op=?op, block_id=%block_id, position=?position);
                if tx == goal_clone {
                    notify_tx
                        .send(Waited::WaitedAndFound(op))
                        .map_err(|e| error!(e=?e))
                        .ok();
                    return;
                  }
                }
              },
              complete => {
                debug!("Streams completed");
                break;
              }
            }
        }
    }.instrument(span));

    // Wait for the task to start receiving events
    let _ = receiving_events_rx.await;

    notify_rx
}

#[instrument(skip(client, matches, submission))]
async fn handle_wait<
    CLIENT: LedgerReader<Event=OpaEvent, Error=SubxtClientError>
    + LedgerWriter<Transaction=OpaTransaction, Error=SubxtClientError>
    + Clone
    + Send
    + Sync
    + 'static,
>(
    matches: &ArgMatches,
    client: &CLIENT,
    submission: OpaTransaction,
) -> Result<Waited, OpaCtlError> {
    let wait = Wait::from_matches(matches);
    match wait {
        Wait::NoWait => {
            let (ext, _) = client.pre_submit(submission).await?;
            client
                .do_submit(protocol_abstract::WriteConsistency::Weak, ext)
                .await
                .map_err(|(e, _id)| e)?;

            Ok(Waited::NoWait)
        }
        Wait::NumberOfBlocks(blocks) => {
            let (ext, tx_id) = client.pre_submit(submission).await?;
            let waiter = ambient_transactions(client, tx_id, blocks).await;
            client
                .do_submit(protocol_abstract::WriteConsistency::Strong, ext)
                .await
                .map_err(|(e, _id)| e)?;
            debug!(awaiting_tx=%tx_id, waiting_blocks=%blocks);
            match waiter.await {
                Ok(Waited::WaitedAndDidNotFind) => Err(OpaCtlError::TransactionNotFound(tx_id)),
                Ok(x) => Ok(x),
                Err(e) => Err(OpaCtlError::Cancelled(e)),
            }
        }
    }
}

async fn dispatch_args<
    CLIENT: LedgerWriter<Transaction=OpaTransaction, Error=SubxtClientError>
    + Send
    + Sync
    + LedgerReader<Event=OpaEvent, Error=SubxtClientError>
    + SubstrateStateReader<Error=SubxtClientError>
    + Clone
    + 'static,
>(
    matches: ArgMatches,
    client: &CLIENT,
) -> Result<Waited, OpaCtlError> {
    let span = span!(Level::TRACE, "dispatch_args");
    let _entered = span.enter();
    let span_id = span.id().map(|x| x.into_u64()).unwrap_or(u64::MAX);
    match matches.subcommand() {
        Some(("bootstrap", command_matches)) => {
            let signing = configure_signing(vec![], &matches, command_matches).await?;
            let bootstrap = SubmissionBuilder::bootstrap_root(signing.opa_verifying().await?)
                .build(span_id, Uuid::new_v4());
            Ok(handle_wait(
                command_matches,
                client,
                OpaTransaction::bootstrap_root(bootstrap, &signing).await?,
            )
                .await?)
        }
        Some(("generate", matches)) => {
            let key = SecretKey::random(StdRng::from_entropy());
            let key = key.to_pkcs8_pem(LineEnding::CRLF).map_err(|_| OpaCtlError::Pkcs8)?;

            if let Some(path) = matches.get_one::<PathBuf>("output") {
                let mut file = File::create(path)?;
                file.write_all(key.as_bytes())?;
            } else {
                print!("{}", *key);
            }

            Ok(Waited::NoWait)
        }
        Some(("rotate-root", command_matches)) => {
            let signing =
                configure_signing(vec!["new-root-key"], &matches, command_matches).await?;
            let rotate_key = SubmissionBuilder::rotate_key(
                "root",
                &signing,
                OPA_PK,
                command_matches
                    .get_one::<String>("new-root-key")
                    .ok_or_else(|| OpaCtlError::MissingArgument("new-root-key".to_owned()))?,
            )
                .await?
                .build(span_id, Uuid::new_v4());
            Ok(handle_wait(
                command_matches,
                client,
                OpaTransaction::rotate_root(rotate_key, &signing).await?,
            )
                .await?)
        }
        Some(("register-key", command_matches)) => {
            let signing = configure_signing(vec!["new-key"], &matches, command_matches).await?;
            let new_key = &command_matches
                .get_one::<String>("new-key")
                .ok_or_else(|| OpaCtlError::MissingArgument("new-key".to_owned()))?;
            let id = command_matches.get_one::<String>("id").unwrap();
            let overwrite_existing = command_matches.get_flag("overwrite");
            let register_key =
                SubmissionBuilder::register_key(id, new_key, &signing, overwrite_existing)
                    .await?
                    .build(span_id, Uuid::new_v4());
            Ok(handle_wait(
                command_matches,
                client,
                OpaTransaction::register_key(id, register_key, &signing, overwrite_existing)
                    .await?,
            )
                .await?)
        }
        Some(("rotate-key", command_matches)) => {
            let signing =
                configure_signing(vec!["current-key", "new-key"], &matches, command_matches)
                    .await?;

            let current_key = &command_matches
                .get_one::<String>("current-key")
                .ok_or_else(|| OpaCtlError::MissingArgument("new-key".to_owned()))?;
            let new_key = &command_matches
                .get_one::<String>("new-key")
                .ok_or_else(|| OpaCtlError::MissingArgument("new-key".to_owned()))?;
            let id = command_matches.get_one::<String>("id").unwrap();
            let rotate_key = SubmissionBuilder::rotate_key(id, &signing, new_key, current_key)
                .await?
                .build(span_id, Uuid::new_v4());
            Ok(handle_wait(
                command_matches,
                client,
                OpaTransaction::rotate_key(id, rotate_key, &signing).await?,
            )
                .await?)
        }
        Some(("set-policy", command_matches)) => {
            let signing = configure_signing(vec![], &matches, command_matches).await?;
            let policy: &String = command_matches.get_one("policy").unwrap();

            let policy = load_bytes_from_url(policy).await?;

            let id = command_matches.get_one::<String>("id").unwrap();

            let bootstrap = SubmissionBuilder::set_policy(id, policy, &signing)
                .await?
                .build(span_id, Uuid::new_v4());
            Ok(handle_wait(
                command_matches,
                client,
                OpaTransaction::set_policy(id, bootstrap, &signing).await?,
            )
                .await?)
        }
        Some(("get-key", matches)) => {
            let key: Result<Option<KeysV1>, _> = client
                .get_state_entry(
                    "Opa",
                    "KeyStore",
                    key_address(matches.get_one::<String>("id").unwrap()),
                )
                .await;

            let key: KeysV1 = key.map_err(OpaCtlError::from)?.ok_or(OpaCtlError::NotFound)?;
            let key = Keys::try_from(key)?;

            debug!(loaded_key = ?key);

            let key = key.current.key;

            if let Some(path) = matches.get_one::<String>("output") {
                let mut file = File::create(path)?;
                file.write_all(key.as_bytes())?;
            } else {
                print!("{}", key.as_str());
            }

            Ok(Waited::NoWait)
        }
        Some(("get-policy", matches)) => {
            let policy: Option<PolicyV1> = client
                .get_state_entry(
                    "Opa",
                    "PolicyStore",
                    key_address(matches.get_one::<String>("id").unwrap()),
                )
                .await?;

            let policy = policy.ok_or(OpaCtlError::NotFound)?;

            if let Some(path) = matches.get_one::<String>("output") {
                let mut file = File::create(path)?;
                file.write_all(Policy::try_from(policy)?.as_bytes())?;
            }

            Ok(Waited::NoWait)
        }
        _ => Ok(Waited::NoWait),
    }
}

#[tokio::main]
async fn main() {
    chronicle_telemetry::telemetry(false, chronicle_telemetry::ConsoleLogging::Pretty);
    let args = cli::cli().get_matches();
    let address: &Url = args.get_one("sawtooth-address").unwrap();
    let client = match OpaSubstrateClient::<PolkadotConfig>::connect(address).await {
        Ok(client) => client,
        Err(e) => {
            error!("Failed to connect to the OPA Substrate Client: {:?}", e);
            std::process::exit(-1);
        }
    };
    dispatch_args(args, &client)
        .await
        .map_err(|opactl| {
            error!(?opactl);
            opactl.into_ufe().print();
            std::process::exit(1);
        })
        .map(|waited| {
            if let Waited::WaitedAndFound(op) = waited {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::to_value(op).unwrap()).unwrap()
                );
            }
        })
        .ok();
}
