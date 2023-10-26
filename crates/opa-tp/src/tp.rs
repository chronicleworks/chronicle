use k256::{
	ecdsa::signature::Verifier,
	pkcs8::DecodePublicKey,
	sha2::{Digest, Sha256},
};
use opa_tp_protocol::{
	address::{HasSawtoothAddress, FAMILY, PREFIX, VERSION},
	events::opa_event,
	messages::Submission,
	state::{
		key_address, policy_address, policy_meta_address, KeyRegistration, Keys, OpaOperationEvent,
		PolicyMeta,
	},
};
use std::str::from_utf8;

use k256::ecdsa::{Signature, VerifyingKey};
use prost::Message;
use sawtooth_sdk::{
	messages::processor::TpProcessRequest,
	processor::handler::{ApplyError, TransactionContext, TransactionHandler},
};
use tracing::{debug, error, instrument};

use crate::abstract_tp::TP;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum OpaTpError {
	#[error("Operation signature verification failed")]
	OperationSignatureVerification,
	#[error("Submission message not well formed")]
	MalformedMessage,
	#[error("Invalid signing key")]
	InvalidSigningKey,
	#[error("Json serialization error {0}")]
	JsonSerialize(#[from] serde_json::Error),
	#[error("Invalid operation")]
	InvalidOperation,
	#[error("Sawtooth context {0}")]
	SawtoothContext(#[from] sawtooth_sdk::processor::handler::ContextError),
}

#[derive(Debug)]
pub struct OpaTransactionHandler {
	family_name: String,
	family_versions: Vec<String>,
	namespaces: Vec<String>,
}

impl OpaTransactionHandler {
	pub fn new() -> OpaTransactionHandler {
		OpaTransactionHandler {
			family_name: FAMILY.to_owned(),
			family_versions: vec![VERSION.to_owned()],
			namespaces: vec![PREFIX.to_string()],
		}
	}
}

impl Default for OpaTransactionHandler {
	fn default() -> Self {
		Self::new()
	}
}

// Verifies the submission.
// Keys == None indicates that the opa tp is not bootstrapped, so the bootstrap
// operation can be performed, otherwise this will be an error
// If the system has been bootstrapped, then the current key must match the signing
// key of the operation
#[instrument(skip(submission, root_keys), ret(Debug))]
fn verify_signed_operation(
	submission: &Submission,
	root_keys: &Option<Keys>,
) -> Result<(), OpaTpError> {
	match &submission.payload {
		Some(opa_tp_protocol::messages::submission::Payload::BootstrapRoot(_)) => Ok(()),
		Some(opa_tp_protocol::messages::submission::Payload::SignedOperation(
			opa_tp_protocol::messages::SignedOperation {
				payload: Some(ref payload),
				verifying_key,
				signature,
			},
		)) => {
			if root_keys.is_none() {
				error!("No registered root keys for signature verification");
				return Err(OpaTpError::OperationSignatureVerification)
			}
			let payload_bytes = payload.encode_to_vec();
			let signature: Signature = k256::ecdsa::signature::Signature::from_bytes(signature)
				.map_err(|e| {
					error!(signature = ?signature, signature_load_error = ?e);
					OpaTpError::OperationSignatureVerification
				})?;
			let signing_key = VerifyingKey::from_public_key_pem(verifying_key).map_err(|e| {
				error!(verifying_key = ?verifying_key, key_load_error = ?e);
				OpaTpError::OperationSignatureVerification
			})?;
			signing_key.verify(&payload_bytes, &signature).map_err(|e| {
				error!(signature = ?signature, verify_error = ?e);
				OpaTpError::OperationSignatureVerification
			})?;

			if *verifying_key == root_keys.as_ref().unwrap().current.key {
				Ok(())
			} else {
				error!(verifying_key = ?verifying_key, current_key = ?root_keys.as_ref().unwrap().current.key, "Invalid signing key");
				Err(OpaTpError::InvalidSigningKey)
			}
		},
		_ => {
			error!(malformed_message = ?submission);
			Err(OpaTpError::MalformedMessage)
		},
	}
}

// Either apply our bootstrap operation or our signed operation
#[instrument(skip(context, request, payload), ret(Debug))]
fn apply_signed_operation(
	payload: opa_tp_protocol::messages::submission::Payload,
	request: &TpProcessRequest,
	context: &mut dyn TransactionContext,
) -> Result<(), OpaTpError> {
	match payload {
		opa_tp_protocol::messages::submission::Payload::BootstrapRoot(
			opa_tp_protocol::messages::BootstrapRoot { public_key },
		) => {
			let existing_key = context.get_state_entry(&key_address("root"))?;

			if existing_key.is_some() {
				error!("OPA TP has already been bootstrapped");
				return Err(OpaTpError::InvalidOperation)
			}

			let keys = Keys {
				id: "root".to_string(),
				current: KeyRegistration { key: public_key, version: 0 },
				expired: None,
			};

			context
				.set_state_entry(keys.get_address(), serde_json::to_string(&keys)?.into_bytes())?;

			context.add_event(
				"opa/operation".to_string(),
				vec![("transaction_id".to_string(), request.signature.clone())],
				&opa_event(1, keys.into())?,
			)?;

			Ok(())
		},
		opa_tp_protocol::messages::submission::Payload::SignedOperation(
			opa_tp_protocol::messages::SignedOperation {
				payload:
					Some(opa_tp_protocol::messages::signed_operation::Payload {
						operation: Some(operation),
					}),
				verifying_key: _,
				signature: _,
			},
		) => apply_signed_operation_payload(request, operation, context),
		_ => {
			error!(malformed_message = ?payload);
			Err(OpaTpError::MalformedMessage)
		},
	}
}

#[instrument(skip(request, context, payload), ret(Debug))]
fn apply_signed_operation_payload(
	request: &TpProcessRequest,
	payload: opa_tp_protocol::messages::signed_operation::payload::Operation,
	context: &mut dyn TransactionContext,
) -> Result<(), OpaTpError> {
	match payload {
		opa_tp_protocol::messages::signed_operation::payload::Operation::RegisterKey(
			opa_tp_protocol::messages::RegisterKey { public_key, id, overwrite_existing },
		) => {
			if id == "root" {
				error!("Cannot register a key with the id 'root'");
				return Err(OpaTpError::InvalidOperation)
			}

			let existing_key = context.get_state_entry(&key_address(id.clone()))?;
			if existing_key.is_some() {
				if overwrite_existing {
					debug!("Registration replaces existing key");
				} else {
					error!("Key already registered");
					return Err(OpaTpError::InvalidOperation)
				}
			}

			let keys = Keys {
				id,
				current: KeyRegistration { key: public_key, version: 0 },
				expired: None,
			};

			context
				.set_state_entry(keys.get_address(), serde_json::to_string(&keys)?.into_bytes())?;

			context.add_event(
				"opa/operation".to_string(),
				vec![("transaction_id".to_string(), request.signature.clone())],
				&opa_event(1, keys.into())?,
			)?;

			Ok(())
		},
		opa_tp_protocol::messages::signed_operation::payload::Operation::RotateKey(
			opa_tp_protocol::messages::RotateKey {
				payload: Some(payload),
				previous_signing_key,
				previous_signature,
				new_signing_key,
				new_signature,
			},
		) => {
			// Get current key registration from state
			let existing_key = context.get_state_entry(&key_address(payload.id.clone()))?;

			if existing_key.is_none() {
				error!("No key to rotate");
				return Err(OpaTpError::InvalidOperation)
			}

			let existing_key: Keys = serde_json::from_str(
				from_utf8(&existing_key.unwrap()).map_err(|_| OpaTpError::MalformedMessage)?,
			)?;

			if previous_signing_key != existing_key.current.key {
				error!("Key does not match current key");
				return Err(OpaTpError::InvalidOperation)
			}

			// Verify the previous key and signature
			let payload_bytes = payload.encode_to_vec();
			let previous_signature = Signature::try_from(&*previous_signature)
				.map_err(|_| OpaTpError::OperationSignatureVerification)?;
			let previous_key = VerifyingKey::from_public_key_pem(&previous_signing_key)
				.map_err(|_| OpaTpError::OperationSignatureVerification)?;

			previous_key
				.verify(&payload_bytes, &previous_signature)
				.map_err(|_| OpaTpError::OperationSignatureVerification)?;

			//Verify the new key and signature
			let new_signature = Signature::try_from(&*new_signature)
				.map_err(|_| OpaTpError::OperationSignatureVerification)?;
			let new_key = VerifyingKey::from_public_key_pem(&new_signing_key)
				.map_err(|_| OpaTpError::OperationSignatureVerification)?;

			new_key
				.verify(&payload_bytes, &new_signature)
				.map_err(|_| OpaTpError::OperationSignatureVerification)?;

			//Store new keys
			let keys = Keys {
				id: payload.id,
				current: KeyRegistration {
					key: new_signing_key,
					version: existing_key.current.version + 1,
				},
				expired: Some(KeyRegistration {
					key: previous_signing_key,
					version: existing_key.current.version,
				}),
			};

			context
				.set_state_entry(keys.get_address(), serde_json::to_string(&keys)?.into_bytes())?;

			context.add_event(
				"opa/operation".to_string(),
				vec![("transaction_id".to_string(), request.signature.clone())],
				&opa_event(1, keys.into())?,
			)?;

			Ok(())
		},
		opa_tp_protocol::messages::signed_operation::payload::Operation::SetPolicy(
			opa_tp_protocol::messages::SetPolicy { policy, id },
		) => {
			let _existing_policy_meta = context.get_state_entry(&policy_meta_address(&*id))?;
			let hash = Sha256::digest(&policy);
			let hash = hex::encode(hash);

			let policy_meta =
				PolicyMeta { id: id.clone(), hash, policy_address: policy_address(&*id) };

			context.set_state_entry(
				policy_meta_address(&id),
				serde_json::to_string(&policy_meta)?.into_bytes(),
			)?;

			context.set_state_entry(policy_address(&id), policy)?;

			context.add_event(
				"opa/operation".to_string(),
				vec![("transaction_id".to_string(), request.signature.clone())],
				&opa_event(1, policy_meta.into())?,
			)?;

			Ok(())
		},
		_ => Err(OpaTpError::MalformedMessage),
	}
}

fn root_keys_from_state(
	_request: &TpProcessRequest,
	context: &dyn TransactionContext,
) -> Result<Option<Keys>, OpaTpError> {
	let existing_key = context.get_state_entry(&key_address("root"))?;

	if let Some(existing_key) = existing_key {
		let existing_key: Keys = serde_json::from_str(
			from_utf8(&existing_key).map_err(|_| OpaTpError::MalformedMessage)?,
		)?;

		Ok(Some(existing_key))
	} else {
		Ok(None)
	}
}

impl TP for OpaTransactionHandler {
	#[instrument(skip(request, context))]
	fn apply(
		&self,
		request: &TpProcessRequest,
		context: &mut dyn TransactionContext,
	) -> Result<(), ApplyError> {
		let payload = request.get_payload();
		let submission = Submission::decode(payload).map_err(|err| {
			ApplyError::InvalidTransaction(format!("Failed to parse payload: {err}"))
		})?;

		debug!(signed_operation = ?submission);

		let process: Result<_, OpaTpError> = (|| {
			verify_signed_operation(&submission, &root_keys_from_state(request, context)?)?;

			apply_signed_operation(submission.payload.unwrap(), request, context)?;
			Ok(())
		})();

		// Protocol errors are non resumable, but operation state / signing
		// errors may possibly be resumable.
		match process {
			Ok(_) => Ok(()),
			Err(e @ OpaTpError::MalformedMessage) => Err(ApplyError::InternalError(e.to_string())),
			Err(e @ OpaTpError::JsonSerialize(_)) => Err(ApplyError::InternalError(e.to_string())),
			Err(e) => Ok(context.add_event(
				"opa/operation".to_string(),
				vec![("transaction_id".to_string(), request.signature.clone())],
				&opa_event(1, OpaOperationEvent::Error(e.to_string()))
					.map_err(|e| ApplyError::InternalError(e.to_string()))?,
			)?),
		}
	}
}

impl TransactionHandler for OpaTransactionHandler {
	fn family_name(&self) -> String {
		self.family_name.clone()
	}

	fn family_versions(&self) -> Vec<String> {
		self.family_versions.clone()
	}

	fn namespaces(&self) -> Vec<String> {
		self.namespaces.clone()
	}

	#[instrument(
        skip(request,context),
        fields(
            transaction_id = %request.signature,
            inputs = ?request.header.as_ref().map(|x| &x.inputs),
            outputs = ?request.header.as_ref().map(|x| &x.outputs),
            dependencies = ?request.header.as_ref().map(|x| &x.dependencies)
        )
    )]
	fn apply(
		&self,
		request: &TpProcessRequest,
		context: &mut dyn TransactionContext,
	) -> Result<(), ApplyError> {
		TP::apply(self, request, context)
	}
}

#[cfg(test)]
mod test {
	use async_stl_client::sawtooth::MessageBuilder;
	use chronicle_signing::{
		chronicle_secret_names, BatcherKnownKeyNamesSigner, ChronicleSigning,
		OpaKnownKeyNamesSigner, BATCHER_NAMESPACE, CHRONICLE_NAMESPACE, OPA_NAMESPACE, OPA_PK,
	};
	use k256::{ecdsa::SigningKey, SecretKey};
	use opa_tp_protocol::{
		address,
		messages::{OpaEvent, Submission},
		state::key_address,
		submission::SubmissionBuilder,
	};
	use prost::Message;
	use rand::rngs::StdRng;
	use rand_core::SeedableRng;
	use sawtooth_sdk::{
		messages::{processor::TpProcessRequest, transaction::TransactionHeader},
		processor::handler::{ContextError, TransactionContext},
	};
	use serde_json::Value;
	use std::{cell::RefCell, collections::BTreeMap};

	use crate::abstract_tp::TP;

	use super::OpaTransactionHandler;

	type TestTxEvents = Vec<(String, Vec<(String, String)>, Vec<u8>)>;

	#[derive(Clone)]
	pub struct TestTransactionContext {
		pub state: RefCell<BTreeMap<String, Vec<u8>>>,
		pub events: RefCell<TestTxEvents>,
	}

	type PrintableEvent = Vec<(String, Vec<(String, String)>, Value)>;

	impl TestTransactionContext {
		pub fn new() -> Self {
			chronicle_telemetry::telemetry(None, chronicle_telemetry::ConsoleLogging::Pretty);
			Self { state: RefCell::new(BTreeMap::new()), events: RefCell::new(vec![]) }
		}

		/// Returns a list of tuples representing the readable state of this
		/// `TestTransactionContext`.
		///
		/// The method first converts raw byte strings into JSON objects for meta data and keys.
		pub fn readable_state(&self) -> Vec<(String, Value)> {
			// Deal with the fact that policies are raw bytes, but meta data and
			// keys are json
			self.state
				.borrow()
				.iter()
				.map(|(k, v)| {
					let as_string = String::from_utf8(v.clone()).unwrap();
					match serde_json::from_str(&as_string) {
						Ok(json) => (k.clone(), json),
						Err(_) => (k.clone(), serde_json::to_value(v.clone()).unwrap()),
					}
				})
				.collect()
		}

		/// Returns the events as a vector of PrintableEvent structs.
		pub fn readable_events(&self) -> PrintableEvent {
			self.events
				.borrow()
				.iter()
				.map(|(k, attr, data)| {
					(
						k.clone(),
						attr.clone(),
						match &OpaEvent::decode(&**data).unwrap().payload.unwrap() {
							opa_tp_protocol::messages::opa_event::Payload::Operation(operation) =>
								serde_json::from_str(operation).unwrap(),
							opa_tp_protocol::messages::opa_event::Payload::Error(error) =>
								serde_json::from_str(error).unwrap(),
						},
					)
				})
				.collect()
		}
	}

	impl TransactionContext for TestTransactionContext {
		fn add_receipt_data(
			self: &TestTransactionContext,
			_data: &[u8],
		) -> Result<(), ContextError> {
			unimplemented!()
		}

		fn add_event(
			self: &TestTransactionContext,
			event_type: String,
			attributes: Vec<(String, String)>,
			data: &[u8],
		) -> Result<(), ContextError> {
			self.events.borrow_mut().push((event_type, attributes, data.to_vec()));
			Ok(())
		}

		fn delete_state_entries(
			self: &TestTransactionContext,
			_addresses: &[std::string::String],
		) -> Result<Vec<String>, ContextError> {
			unimplemented!()
		}

		fn get_state_entries(
			&self,
			addresses: &[String],
		) -> Result<Vec<(String, Vec<u8>)>, ContextError> {
			Ok(self
				.state
				.borrow()
				.iter()
				.filter(|(k, _)| addresses.contains(k))
				.map(|(k, v)| (k.clone(), v.clone()))
				.collect())
		}

		fn set_state_entries(
			self: &TestTransactionContext,
			entries: Vec<(String, Vec<u8>)>,
		) -> std::result::Result<(), sawtooth_sdk::processor::handler::ContextError> {
			for entry in entries {
				self.state.borrow_mut().insert(entry.0, entry.1);
			}

			Ok(())
		}
	}

	fn key_from_seed(seed: u8) -> SigningKey {
		let secret: SigningKey = SecretKey::random(StdRng::from_seed([seed; 32])).into();

		secret
	}

	/// Apply a transaction to a transaction context
	async fn apply_tx(
		mut context: TestTransactionContext,
		addresses: &[String],
		submission: &Submission,
		signer: &ChronicleSigning,
	) -> TestTransactionContext {
		let message_builder = MessageBuilder::new_deterministic(address::FAMILY, address::VERSION);
		let (tx, id) = message_builder
			.make_sawtooth_transaction(
				addresses.to_vec(),
				addresses.to_vec(),
				vec![],
				submission,
				signer.batcher_verifying().await.unwrap(),
				|bytes| {
					let signer = signer.clone();
					let bytes = bytes.to_vec();
					async move { signer.batcher_sign(&bytes).await }
				},
			)
			.await
			.unwrap();
		let processor = OpaTransactionHandler::new();
		let header =
			<TransactionHeader as protobuf::Message>::parse_from_bytes(&tx.header).unwrap();
		let mut request = TpProcessRequest::default();
		request.set_header(header);
		request.set_payload(tx.payload);
		request.set_signature(id.as_str().to_owned());
		processor.apply(&request, &mut context).unwrap();
		context
	}

	/// Assert that all contexts in the given slice are equal
	fn assert_contexts_are_equal(contexts: &[TestTransactionContext]) {
		// get the first context in the slice
		let first_context = &contexts[0];

		// check that all contexts have the same readable state and events
		assert!(
			contexts.iter().all(|context| {
				(first_context.readable_state(), first_context.readable_events()) ==
					(context.readable_state(), context.readable_events())
			}),
			"All contexts must be the same"
		);
	}

	/// Applies a transaction `submission` to `context` using `batcher_key`,
	/// `number_of_determinism_checking_cycles` times, checking for determinism between
	/// each cycle.
	async fn submission_to_state(
		context: TestTransactionContext,
		signer: ChronicleSigning,
		addresses: &[String],
		submission: Submission,
	) -> TestTransactionContext {
		// Set the number of determinism checking cycles
		let number_of_determinism_checking_cycles = 5;

		// Get the current state and events before applying the transactions.
		let preprocessing_state_and_events =
			{ (context.readable_state(), context.readable_events()) };

		// Create a vector of `number_of_determinism_checking_cycles` contexts
		let contexts = vec![context; number_of_determinism_checking_cycles];

		let mut results = Vec::with_capacity(number_of_determinism_checking_cycles);

		for context in contexts {
			let result = apply_tx(context, addresses, &submission, &signer).await;
			results.push(result);
		}

		// Check that the context has been updated after running `apply_tx`
		let updated_readable_state_and_events = {
			let context = results.last().unwrap();
			(context.readable_state(), context.readable_events())
		};
		assert_ne!(
			preprocessing_state_and_events, updated_readable_state_and_events,
			"Context must be updated after running apply"
		);

		// Check if all contexts are the same after running `apply_tx`
		assert_contexts_are_equal(&results);

		// Return the last context from the vector of contexts
		results.pop().unwrap()
	}

	#[tokio::test]
	async fn bootstrap_from_initial_state() {
		let secrets = chronicle_signing().await;
		let context = TestTransactionContext::new();
		let builder = SubmissionBuilder::bootstrap_root(secrets.opa_verifying().await.unwrap());
		let submission = builder.build(0xffff);

		let context =
			submission_to_state(context, secrets, &[key_address("root")], submission).await;

		insta::assert_yaml_snapshot!(context.readable_state(), {
            ".**.date" => "[date]",
            ".**.transaction_id" => "[hash]",
            ".**.key" => "[pem]",
        }, @r###"
        ---
        - - 7ed19313e8ece6c4f5551b9bd1090797ad25c6d85f7b523b2214d4fe448372279aa95c
          - current:
              key: "[pem]"
              version: 0
            expired: ~
            id: root
        "###);
		insta::assert_yaml_snapshot!(context.readable_events(), {
            ".**.date" => "[date]",
        }, @r###"
        ---
        - - opa/operation
          - - - transaction_id
              - 706557445d7bcc80ca1ce3a9efc56e57d3a7b8ab425691ebb7d63774934603ec0cb4bafc1b4da0890a0e9c86c8c67d9f848bf3c5b718d39dbe20eb14977e7738
          - KeyUpdate:
              current:
                key: "-----BEGIN PUBLIC KEY-----\r\nMFYwEAYHKoZIzj0CAQYFK4EEAAoDQgAEHr4pXCvHq3s3mcbduwpcEwRsE0GA2mJ1\r\nmelruzkYSf/BcAeqzHkjv2BvoA6mC/coJsVazfRiKTzB2pPqRLTQUQ==\r\n-----END PUBLIC KEY-----\r\n"
                version: 0
              expired: ~
              id: root
        "###);
	}

	/// Needed for further tests
	async fn bootstrap_root() -> (TestTransactionContext, ChronicleSigning) {
		let secrets = chronicle_signing().await;

		let context = TestTransactionContext::new();
		let builder = SubmissionBuilder::bootstrap_root(secrets.opa_verifying().await.unwrap());
		let submission = builder.build(0xffff);

		(
			submission_to_state(context, secrets.clone(), &[key_address("root")], submission).await,
			secrets,
		)
	}

	async fn chronicle_signing() -> ChronicleSigning {
		let mut names = chronicle_secret_names();
		names.append(&mut vec![
			(CHRONICLE_NAMESPACE.to_string(), "rotate_root".to_string()),
			(CHRONICLE_NAMESPACE.to_string(), "non_root_1".to_string()),
			(CHRONICLE_NAMESPACE.to_string(), "non_root_2".to_string()),
			(CHRONICLE_NAMESPACE.to_string(), "key_1".to_string()),
			(CHRONICLE_NAMESPACE.to_string(), "key_2".to_string()),
			(CHRONICLE_NAMESPACE.to_string(), "opa-pk".to_string()),
		]);

		names.append(&mut vec![
			(OPA_NAMESPACE.to_string(), "rotate_root".to_string()),
			(OPA_NAMESPACE.to_string(), "non_root_1".to_string()),
			(OPA_NAMESPACE.to_string(), "non_root_2".to_string()),
			(OPA_NAMESPACE.to_string(), "key_1".to_string()),
			(OPA_NAMESPACE.to_string(), "key_2".to_string()),
			(OPA_NAMESPACE.to_string(), "opa-pk".to_string()),
			(OPA_NAMESPACE.to_string(), "new_root_1".to_string()),
		]);

		ChronicleSigning::new(
			names,
			vec![
				(
					CHRONICLE_NAMESPACE.to_string(),
					chronicle_signing::ChronicleSecretsOptions::test_keys(),
				),
				(
					OPA_NAMESPACE.to_string(),
					chronicle_signing::ChronicleSecretsOptions::test_keys(),
				),
				(
					BATCHER_NAMESPACE.to_string(),
					chronicle_signing::ChronicleSecretsOptions::test_keys(),
				),
			],
		)
		.await
		.unwrap()
	}

	#[tokio::test]
	async fn rotate_root() {
		let (context, signing) = bootstrap_root().await;
		let builder = SubmissionBuilder::rotate_key("root", &signing, "opa-pk", "new_root_1")
			.await
			.unwrap();
		let submission = builder.build(0xffff);

		let context =
			submission_to_state(context, signing, &[key_address("root")], submission).await;

		insta::assert_yaml_snapshot!(context.readable_state(),{
            ".**.date" => "[date]",
            ".**.transaction_id" => "[hash]",
            ".**.key" => "[pem]",

        },  @r###"
        ---
        - - 7ed19313e8ece6c4f5551b9bd1090797ad25c6d85f7b523b2214d4fe448372279aa95c
          - current:
              key: "[pem]"
              version: 1
            expired:
              key: "[pem]"
              version: 0
            id: root
        "### );

		insta::assert_yaml_snapshot!(context.readable_events(), {
           ".**.date" => "[date]",
            ".**.transaction_id" => "[hash]",
            ".**.key" => "[pem]",
        }, @r###"
        ---
        - - opa/operation
          - - - transaction_id
              - 706557445d7bcc80ca1ce3a9efc56e57d3a7b8ab425691ebb7d63774934603ec0cb4bafc1b4da0890a0e9c86c8c67d9f848bf3c5b718d39dbe20eb14977e7738
          - KeyUpdate:
              current:
                key: "[pem]"
                version: 0
              expired: ~
              id: root
        - - opa/operation
          - - - transaction_id
              - f2c7909f1c92206715e1d3f1299b47b5bcdae10b6c055909670c8b4bc5cbc5e25a8ef020e280fc20b2d93052cae4f0e91ae5deda4c2a2e1ac636f710e8838834
          - KeyUpdate:
              current:
                key: "[pem]"
                version: 1
              expired:
                key: "[pem]"
                version: 0
              id: root
        "### );
	}

	#[tokio::test]
	async fn register_valid_key() {
		let (context, signing) = bootstrap_root().await;

		let builder = SubmissionBuilder::register_key("nonroot", "opa-pk", &signing, false)
			.await
			.unwrap();
		let submission = builder.build(0xffff);

		let context =
			submission_to_state(context, signing, &[key_address("nonroot")], submission).await;

		insta::assert_yaml_snapshot!(context.readable_state(),{
            ".**.date" => "[date]",
            ".**.transaction_id" => "[hash]",
            ".**.key" => "[pem]",
        }, @r###"
        ---
        - - 7ed19313e8ece6c4f5551b9bd1090797ad25c6d85f7b523b2214d4fe448372279aa95c
          - current:
              key: "[pem]"
              version: 0
            expired: ~
            id: root
        - - 7ed1931c7f165dbd2e5e0dd1c360aa665b4366010392df934edbea19b9bd29f0a228af
          - current:
              key: "[pem]"
              version: 0
            expired: ~
            id: nonroot
        "### );
		insta::assert_yaml_snapshot!(context.readable_events(), {
            ".**.date" => "[date]",
            ".**.transaction_id" => "[hash]",
            ".**.key" => "[pem]",
        }, @r###"
        ---
        - - opa/operation
          - - - transaction_id
              - 706557445d7bcc80ca1ce3a9efc56e57d3a7b8ab425691ebb7d63774934603ec0cb4bafc1b4da0890a0e9c86c8c67d9f848bf3c5b718d39dbe20eb14977e7738
          - KeyUpdate:
              current:
                key: "[pem]"
                version: 0
              expired: ~
              id: root
        - - opa/operation
          - - - transaction_id
              - ac590f5e7d83c7dbfb14d80002238e29741a2dfacb86fe860d5807c7d86d81245ca904fb47842ed192b50e176b97bb8df0042cc803f18b391e72a104a8bdf7a5
          - KeyUpdate:
              current:
                key: "[pem]"
                version: 0
              expired: ~
              id: nonroot
        "###);
	}

	#[tokio::test]
	async fn rotate_valid_key() {
		let (context, signing) = bootstrap_root().await;
		let _non_root_key = key_from_seed(1);

		let builder = SubmissionBuilder::register_key("nonroot", "key_1", &signing, false)
			.await
			.unwrap();
		let submission = builder.build(0xffff);

		let context =
			submission_to_state(context, signing.clone(), &[key_address("nonroot")], submission)
				.await;

		let _new_non_root = key_from_seed(2);
		let builder = SubmissionBuilder::rotate_key("nonroot", &signing, "key_1", "key_2")
			.await
			.unwrap();
		let submission = builder.build(0xffff);

		let context =
			submission_to_state(context, signing, &[key_address("nonroot")], submission).await;

		insta::assert_yaml_snapshot!(context.readable_state(),{
            ".**.date" => "[date]",
            ".**.transaction_id" => "[hash]",
            ".**.key" => "[pem]",
        }, @r###"
        ---
        - - 7ed19313e8ece6c4f5551b9bd1090797ad25c6d85f7b523b2214d4fe448372279aa95c
          - current:
              key: "[pem]"
              version: 0
            expired: ~
            id: root
        - - 7ed1931c7f165dbd2e5e0dd1c360aa665b4366010392df934edbea19b9bd29f0a228af
          - current:
              key: "[pem]"
              version: 1
            expired:
              key: "[pem]"
              version: 0
            id: nonroot
        "### );
		insta::assert_yaml_snapshot!(context.readable_events(), {
            ".**.date" => "[date]",
            ".**.transaction_id" => "[hash]",
            ".**.key" => "[pem]",
        }, @r###"
        ---
        - - opa/operation
          - - - transaction_id
              - 706557445d7bcc80ca1ce3a9efc56e57d3a7b8ab425691ebb7d63774934603ec0cb4bafc1b4da0890a0e9c86c8c67d9f848bf3c5b718d39dbe20eb14977e7738
          - KeyUpdate:
              current:
                key: "[pem]"
                version: 0
              expired: ~
              id: root
        - - opa/operation
          - - - transaction_id
              - d88103ab8f4aecbf9d3dd399b2fd5d1ab531d8c312937a2a13e6ca87dffaeaf07d7b5a0a54f902587abdf7c2180fbe525cd7ad28f86718ae8c87c9df7fc2582f
          - KeyUpdate:
              current:
                key: "[pem]"
                version: 0
              expired: ~
              id: nonroot
        - - opa/operation
          - - - transaction_id
              - ae1138856987116728fb420b67e50281ee39500cc09f3d3d75cb1f46bdf4758033041e593bb11639247d6afc4c907551e5cf6d07fadddb23b63d3be98ad6e6e3
          - KeyUpdate:
              current:
                key: "[pem]"
                version: 1
              expired:
                key: "[pem]"
                version: 0
              id: nonroot
        "###);
	}

	#[tokio::test]
	async fn cannot_register_nonroot_key_as_root() {
		let (context, root_key) = bootstrap_root().await;
		let _non_root_key = key_from_seed(1);

		let builder = SubmissionBuilder::register_key("root", "key_1", &root_key, false)
			.await
			.unwrap();
		let submission = builder.build(0xffff);

		let context =
			submission_to_state(context, root_key, &[key_address("nonroot")], submission).await;

		insta::assert_yaml_snapshot!(context.readable_state(),{
            ".**.date" => "[date]",
            ".**.transaction_id" => "[hash]",
            ".**.key" => "[pem]",
        }, @r###"
        ---
        - - 7ed19313e8ece6c4f5551b9bd1090797ad25c6d85f7b523b2214d4fe448372279aa95c
          - current:
              key: "[pem]"
              version: 0
            expired: ~
            id: root
        "### );
		insta::assert_yaml_snapshot!(context.readable_events(), {
            ".**.date" => "[date]",
        }, @r###"
        ---
        - - opa/operation
          - - - transaction_id
              - 706557445d7bcc80ca1ce3a9efc56e57d3a7b8ab425691ebb7d63774934603ec0cb4bafc1b4da0890a0e9c86c8c67d9f848bf3c5b718d39dbe20eb14977e7738
          - KeyUpdate:
              current:
                key: "-----BEGIN PUBLIC KEY-----\r\nMFYwEAYHKoZIzj0CAQYFK4EEAAoDQgAEHr4pXCvHq3s3mcbduwpcEwRsE0GA2mJ1\r\nmelruzkYSf/BcAeqzHkjv2BvoA6mC/coJsVazfRiKTzB2pPqRLTQUQ==\r\n-----END PUBLIC KEY-----\r\n"
                version: 0
              expired: ~
              id: root
        - - opa/operation
          - - - transaction_id
              - 20da702eb32cb0af245752bea18b8878759282c3abd8e8334a08caae1711a896087056042c7f9d6e5d818ceef0767d0099383f82758003601513ff7356f9c24d
          - error: Invalid operation
        "###);
	}

	#[tokio::test]
	async fn cannot_register_key_as_nonroot_with_overwrite() {
		let (context, root_key) = bootstrap_root().await;

		let builder =
			SubmissionBuilder::register_key("root", "key_1", &root_key, true).await.unwrap();
		let submission = builder.build(0xffff);

		let context =
			submission_to_state(context, root_key, &[key_address("nonroot")], submission).await;

		insta::assert_yaml_snapshot!(context.readable_state(),{
            ".**.date" => "[date]",
            ".**.transaction_id" => "[hash]",
            ".**.key" => "[pem]",
        }, @r###"
        ---
        - - 7ed19313e8ece6c4f5551b9bd1090797ad25c6d85f7b523b2214d4fe448372279aa95c
          - current:
              key: "[pem]"
              version: 0
            expired: ~
            id: root
        "### );
		insta::assert_yaml_snapshot!(context.readable_events(), {
            ".**.date" => "[date]",
        }, @r###"
        ---
        - - opa/operation
          - - - transaction_id
              - 706557445d7bcc80ca1ce3a9efc56e57d3a7b8ab425691ebb7d63774934603ec0cb4bafc1b4da0890a0e9c86c8c67d9f848bf3c5b718d39dbe20eb14977e7738
          - KeyUpdate:
              current:
                key: "-----BEGIN PUBLIC KEY-----\r\nMFYwEAYHKoZIzj0CAQYFK4EEAAoDQgAEHr4pXCvHq3s3mcbduwpcEwRsE0GA2mJ1\r\nmelruzkYSf/BcAeqzHkjv2BvoA6mC/coJsVazfRiKTzB2pPqRLTQUQ==\r\n-----END PUBLIC KEY-----\r\n"
                version: 0
              expired: ~
              id: root
        - - opa/operation
          - - - transaction_id
              - a31aafdbde8b242beace8c62a8d13f6de4c853b6257426e962598a53dcab35a81c3b424b0da8cef52092a8bc0d036c54617f793a0ab29f7b4f235176525ffc03
          - error: Invalid operation
        "###);
	}

	#[tokio::test]
	async fn cannot_register_existing_key() {
		let (context, signing) = bootstrap_root().await;

		let builder = SubmissionBuilder::register_key("nonroot", "key_1", &signing, false)
			.await
			.unwrap();
		let submission = builder.build(0xffff);

		let context =
			submission_to_state(context, signing.clone(), &[key_address("nonroot")], submission)
				.await;

		let builder = SubmissionBuilder::register_key("nonroot", "key_1", &signing, false)
			.await
			.unwrap();
		let submission = builder.build(0xffff);

		let context =
			submission_to_state(context, signing, &[key_address("nonroot")], submission).await;

		insta::assert_yaml_snapshot!(context.readable_state(),{
            ".**.date" => "[date]",
            ".**.transaction_id" => "[hash]",
            ".**.key" => "[pem]",
        }, @r###"
        ---
        - - 7ed19313e8ece6c4f5551b9bd1090797ad25c6d85f7b523b2214d4fe448372279aa95c
          - current:
              key: "[pem]"
              version: 0
            expired: ~
            id: root
        - - 7ed1931c7f165dbd2e5e0dd1c360aa665b4366010392df934edbea19b9bd29f0a228af
          - current:
              key: "[pem]"
              version: 0
            expired: ~
            id: nonroot
        "### );
		insta::assert_yaml_snapshot!(context.readable_events(), {
            ".**.date" => "[date]",
        }, @r###"
        ---
        - - opa/operation
          - - - transaction_id
              - 706557445d7bcc80ca1ce3a9efc56e57d3a7b8ab425691ebb7d63774934603ec0cb4bafc1b4da0890a0e9c86c8c67d9f848bf3c5b718d39dbe20eb14977e7738
          - KeyUpdate:
              current:
                key: "-----BEGIN PUBLIC KEY-----\r\nMFYwEAYHKoZIzj0CAQYFK4EEAAoDQgAEHr4pXCvHq3s3mcbduwpcEwRsE0GA2mJ1\r\nmelruzkYSf/BcAeqzHkjv2BvoA6mC/coJsVazfRiKTzB2pPqRLTQUQ==\r\n-----END PUBLIC KEY-----\r\n"
                version: 0
              expired: ~
              id: root
        - - opa/operation
          - - - transaction_id
              - d88103ab8f4aecbf9d3dd399b2fd5d1ab531d8c312937a2a13e6ca87dffaeaf07d7b5a0a54f902587abdf7c2180fbe525cd7ad28f86718ae8c87c9df7fc2582f
          - KeyUpdate:
              current:
                key: "-----BEGIN PUBLIC KEY-----\r\nMFYwEAYHKoZIzj0CAQYFK4EEAAoDQgAEUVDPfl4iZJS68q5oNIPKmyDUgxOZ2Mz9\r\nNoo4G7SEpVFwpyYA1FI+NYMUaIDtX/MRTcxEGWMELAKOysSCwKSKvw==\r\n-----END PUBLIC KEY-----\r\n"
                version: 0
              expired: ~
              id: nonroot
        - - opa/operation
          - - - transaction_id
              - d88103ab8f4aecbf9d3dd399b2fd5d1ab531d8c312937a2a13e6ca87dffaeaf07d7b5a0a54f902587abdf7c2180fbe525cd7ad28f86718ae8c87c9df7fc2582f
          - error: Invalid operation
        "###);
	}

	#[tokio::test]
	async fn can_register_existing_key_with_overwrite() {
		let (context, root_key) = bootstrap_root().await;
		let _non_root_key = key_from_seed(1);

		let builder = SubmissionBuilder::register_key("nonroot", "key_1", &root_key, false)
			.await
			.unwrap();
		let submission = builder.build(0xffff);

		let context =
			submission_to_state(context, root_key.clone(), &[key_address("nonroot")], submission)
				.await;

		let builder = SubmissionBuilder::register_key("nonroot", "key_1", &root_key, true)
			.await
			.unwrap();
		let submission = builder.build(0xffff);

		let context =
			submission_to_state(context, root_key, &[key_address("nonroot")], submission).await;

		insta::assert_yaml_snapshot!(context.readable_state(),{
            ".**.date" => "[date]",
            ".**.transaction_id" => "[hash]",
            ".**.key" => "[pem]",
        }, @r###"
        ---
        - - 7ed19313e8ece6c4f5551b9bd1090797ad25c6d85f7b523b2214d4fe448372279aa95c
          - current:
              key: "[pem]"
              version: 0
            expired: ~
            id: root
        - - 7ed1931c7f165dbd2e5e0dd1c360aa665b4366010392df934edbea19b9bd29f0a228af
          - current:
              key: "[pem]"
              version: 0
            expired: ~
            id: nonroot
        "### );
		insta::assert_yaml_snapshot!(context.readable_events(), {
            ".**.date" => "[date]",
            ".**.transaction_id" => "[hash]",
            ".**.key" => "[pem]",
        }, @r###"
        ---
        - - opa/operation
          - - - transaction_id
              - 706557445d7bcc80ca1ce3a9efc56e57d3a7b8ab425691ebb7d63774934603ec0cb4bafc1b4da0890a0e9c86c8c67d9f848bf3c5b718d39dbe20eb14977e7738
          - KeyUpdate:
              current:
                key: "[pem]"
                version: 0
              expired: ~
              id: root
        - - opa/operation
          - - - transaction_id
              - d88103ab8f4aecbf9d3dd399b2fd5d1ab531d8c312937a2a13e6ca87dffaeaf07d7b5a0a54f902587abdf7c2180fbe525cd7ad28f86718ae8c87c9df7fc2582f
          - KeyUpdate:
              current:
                key: "[pem]"
                version: 0
              expired: ~
              id: nonroot
        - - opa/operation
          - - - transaction_id
              - f1feebd35ec516ee1b7546d0c63bd7efe03351d9ea7c016682a54a11acf61ea66d01bcf19cf5dc2b31ae07c56478eaa0d5f156208cbd8a0ce37c4712a35b8a6e
          - KeyUpdate:
              current:
                key: "[pem]"
                version: 0
              expired: ~
              id: nonroot
        "###);
	}

	#[tokio::test]
	async fn cannot_register_existing_root_key_with_overwrite() {
		let (context, signing) = bootstrap_root().await;

		let builder =
			SubmissionBuilder::register_key("root", OPA_PK, &signing, true).await.unwrap();
		let submission = builder.build(0xffff);

		let context =
			submission_to_state(context, signing, &[key_address("root")], submission).await;

		insta::assert_yaml_snapshot!(context.readable_state(),{
            ".**.date" => "[date]",
            ".**.transaction_id" => "[hash]",
            ".**.key" => "[pem]",
        }, @r###"
        ---
        - - 7ed19313e8ece6c4f5551b9bd1090797ad25c6d85f7b523b2214d4fe448372279aa95c
          - current:
              key: "[pem]"
              version: 0
            expired: ~
            id: root
        "### );
		insta::assert_yaml_snapshot!(context.readable_events(), {
            ".**.date" => "[date]",
        }, @r###"
        ---
        - - opa/operation
          - - - transaction_id
              - 706557445d7bcc80ca1ce3a9efc56e57d3a7b8ab425691ebb7d63774934603ec0cb4bafc1b4da0890a0e9c86c8c67d9f848bf3c5b718d39dbe20eb14977e7738
          - KeyUpdate:
              current:
                key: "-----BEGIN PUBLIC KEY-----\r\nMFYwEAYHKoZIzj0CAQYFK4EEAAoDQgAEHr4pXCvHq3s3mcbduwpcEwRsE0GA2mJ1\r\nmelruzkYSf/BcAeqzHkjv2BvoA6mC/coJsVazfRiKTzB2pPqRLTQUQ==\r\n-----END PUBLIC KEY-----\r\n"
                version: 0
              expired: ~
              id: root
        - - opa/operation
          - - - transaction_id
              - bd3395015bfe80530016a9ec49e4c47037da397e9992bff9d9e450361e4d5ec871d2cd23fd26bdc3687c81ddca05220608c159962f9cd57192c6d33cd906fc4c
          - error: Invalid operation
        "###);
	}

	#[tokio::test]
	async fn set_a_policy() {
		let (context, signing) = bootstrap_root().await;

		// Policies can only be set by the root key owner
		let builder =
			SubmissionBuilder::set_policy("test", vec![0, 1, 2, 3], &signing).await.unwrap();

		let submission = builder.build(0xffff);

		let context =
			submission_to_state(context, signing, &[key_address("nonroot")], submission).await;

		insta::assert_yaml_snapshot!(context.readable_state(),{
            ".**.date" => "[date]",
            ".**.transaction_id" => "[hash]",
            ".**.key" => "[pem]",
        }, @r###"
        ---
        - - 7ed19313e8ece6c4f5551b9bd1090797ad25c6d85f7b523b2214d4fe448372279aa95c
          - current:
              key: "[pem]"
              version: 0
            expired: ~
            id: root
        - - 7ed1931c262a4be700b69974438a35ae56a07ce96778b276c8a061dc254d9862c7ecff
          - - 0
            - 1
            - 2
            - 3
        - - 7ed1932b35db049f40833c5c2eaa47e070ce2648c478469a4cdf44ff7a37dd5468208e
          - hash: 054edec1d0211f624fed0cbca9d4f9400b0e491c43742af2c5b0abebf0c990d8
            id: test
            policy_address: 7ed1931c262a4be700b69974438a35ae56a07ce96778b276c8a061dc254d9862c7ecff
        "###);

		insta::assert_yaml_snapshot!(context.readable_events(),{
            ".**.date" => "[date]",
        }, @r###"
        ---
        - - opa/operation
          - - - transaction_id
              - 706557445d7bcc80ca1ce3a9efc56e57d3a7b8ab425691ebb7d63774934603ec0cb4bafc1b4da0890a0e9c86c8c67d9f848bf3c5b718d39dbe20eb14977e7738
          - KeyUpdate:
              current:
                key: "-----BEGIN PUBLIC KEY-----\r\nMFYwEAYHKoZIzj0CAQYFK4EEAAoDQgAEHr4pXCvHq3s3mcbduwpcEwRsE0GA2mJ1\r\nmelruzkYSf/BcAeqzHkjv2BvoA6mC/coJsVazfRiKTzB2pPqRLTQUQ==\r\n-----END PUBLIC KEY-----\r\n"
                version: 0
              expired: ~
              id: root
        - - opa/operation
          - - - transaction_id
              - 8f8804e51058a10db8895c7822af7f4162e2f2d5766752e9d2a4f19586924c0312ea886b50ea810f5f7a24cc2e5da09a16529f26e3d803af5bc82ce75d9cdbe8
          - PolicyUpdate:
              hash: 054edec1d0211f624fed0cbca9d4f9400b0e491c43742af2c5b0abebf0c990d8
              id: test
              policy_address: 7ed1931c262a4be700b69974438a35ae56a07ce96778b276c8a061dc254d9862c7ecff
        "###);
	}
}
