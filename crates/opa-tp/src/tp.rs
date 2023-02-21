use std::str::from_utf8;

use chrono::Utc;
use k256::{ecdsa::signature::Verifier, pkcs8::DecodePublicKey};
use opa_tp_protocol::{
    address::{HasSawtoothAddress, FAMILY, PREFIX, VERSION},
    events::opa_event,
    messages::Submission,
    state::{key_address, policy_address, KeyRegistration, Keys, OpaOperationEvent, PolicyMeta},
};

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
#[instrument(ret(Debug))]
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
                return Err(OpaTpError::OperationSignatureVerification);
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
            signing_key
                .verify(&payload_bytes, &signature)
                .map_err(|e| {
                    error!(signature = ?signature, verify_error = ?e);
                    OpaTpError::OperationSignatureVerification
                })?;

            if *verifying_key == root_keys.as_ref().unwrap().current.key {
                Ok(())
            } else {
                error!(verifying_key = ?verifying_key, current_key = ?root_keys.as_ref().unwrap().current.key, "Invalid signing key");
                Err(OpaTpError::InvalidSigningKey)
            }
        }
        _ => {
            error!(malformed_message = ?submission);
            Err(OpaTpError::MalformedMessage)
        }
    }
}

// Either apply our bootstrap operation or our signed operation
#[instrument(skip(context, request), ret(Debug))]
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
                return Err(OpaTpError::InvalidOperation);
            }

            let keys = Keys {
                id: "root".to_string(),
                current: KeyRegistration {
                    key: public_key,
                    date: Utc::now(),
                },
                expired: None,
            };

            context.set_state_entry(
                keys.get_address(),
                serde_json::to_string(&keys)?.into_bytes(),
            )?;

            context.add_event(
                "opa/operation".to_string(),
                vec![("transaction_id".to_string(), request.signature.clone())],
                &opa_event(1, keys.into())?,
            )?;

            Ok(())
        }
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
        }
    }
}

#[instrument(skip(context), ret(Debug))]
fn apply_signed_operation_payload(
    request: &TpProcessRequest,
    payload: opa_tp_protocol::messages::signed_operation::payload::Operation,
    context: &mut dyn TransactionContext,
) -> Result<(), OpaTpError> {
    match payload {
        opa_tp_protocol::messages::signed_operation::payload::Operation::RegisterKey(
            opa_tp_protocol::messages::RegisterKey { public_key, id },
        ) => {
            if id == "root" {
                error!("Cannot register a key with the id 'root'");
                return Err(OpaTpError::InvalidOperation);
            }

            let existing_key = context.get_state_entry(&key_address(id.clone()))?;

            if existing_key.is_some() {
                error!("Key already registered");
                return Err(OpaTpError::InvalidOperation);
            }

            let keys = Keys {
                id,
                current: KeyRegistration {
                    key: public_key,
                    date: Utc::now(),
                },
                expired: None,
            };

            context.set_state_entry(
                keys.get_address(),
                serde_json::to_string(&keys)?.into_bytes(),
            )?;

            context.add_event(
                "opa/operation".to_string(),
                vec![("transaction_id".to_string(), request.signature.clone())],
                &opa_event(1, keys.into())?,
            )?;

            Ok(())
        }
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
                return Err(OpaTpError::InvalidOperation);
            }

            let existing_key: Keys = serde_json::from_str(
                from_utf8(&existing_key.unwrap()).map_err(|_| OpaTpError::MalformedMessage)?,
            )?;

            if previous_signing_key != existing_key.current.key {
                error!("Key does not match current key");
                return Err(OpaTpError::InvalidOperation);
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
                    date: Utc::now(),
                },
                expired: Some(KeyRegistration {
                    key: previous_signing_key,
                    date: Utc::now(),
                }),
            };

            context.set_state_entry(
                keys.get_address(),
                serde_json::to_string(&keys)?.into_bytes(),
            )?;

            context.add_event(
                "opa/operation".to_string(),
                vec![("transaction_id".to_string(), request.signature.clone())],
                &opa_event(1, keys.into())?,
            )?;

            Ok(())
        }
        opa_tp_protocol::messages::signed_operation::payload::Operation::SetPolicy(
            opa_tp_protocol::messages::SetPolicy { policy, id },
        ) => {
            let policy_meta = PolicyMeta {
                id: id.clone(),
                date: Utc::now(),
                policy_address: policy_address(&*id),
            };

            context.set_state_entry(
                policy_meta.policy_address.clone(),
                serde_json::to_string(&policy)?.into_bytes(),
            )?;

            context.set_state_entry(policy_address(id), policy)?;

            context.add_event(
                "opa/operation".to_string(),
                vec![("transaction_id".to_string(), request.signature.clone())],
                &opa_event(1, policy_meta.into())?,
            )?;

            Ok(())
        }
        _ => Err(OpaTpError::MalformedMessage),
    }
}

fn root_keys_from_state(
    _request: &TpProcessRequest,
    context: &mut dyn TransactionContext,
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
    use k256::{ecdsa::SigningKey, SecretKey};
    use opa_tp_protocol::{
        address,
        messages::{OpaEvent, Submission},
        sawtooth::MessageBuilder,
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
    pub struct TestTransactionContext {
        pub state: RefCell<BTreeMap<String, Vec<u8>>>,
        pub events: RefCell<TestTxEvents>,
    }

    type PrintableEvent = Vec<(String, Vec<(String, String)>, Value)>;

    impl TestTransactionContext {
        pub fn new() -> Self {
            chronicle_telemetry::telemetry(None, chronicle_telemetry::ConsoleLogging::Pretty);
            Self {
                state: RefCell::new(BTreeMap::new()),
                events: RefCell::new(vec![]),
            }
        }

        pub fn readable_state(&self) -> Vec<(String, Value)> {
            // Deal with the fact that policies are raw bytes, but meta data and
            // keys are json

            self.state
                .borrow()
                .iter()
                .map(|(k, v)| {
                    let as_string = String::from_utf8(v.clone()).unwrap();
                    if serde_json::from_str::<Value>(&as_string).is_ok() {
                        (k.clone(), serde_json::from_str(&as_string).unwrap())
                    } else {
                        (k.clone(), serde_json::to_value(v.clone()).unwrap())
                    }
                })
                .collect()
        }

        pub fn readable_events(&self) -> PrintableEvent {
            self.events
                .borrow()
                .iter()
                .map(|(k, attr, data)| {
                    (
                        k.clone(),
                        attr.clone(),
                        match &OpaEvent::decode(&**data).unwrap().payload.unwrap() {
                            opa_tp_protocol::messages::opa_event::Payload::Operation(operation) => {
                                serde_json::from_str(operation).unwrap()
                            }
                            opa_tp_protocol::messages::opa_event::Payload::Error(error) => {
                                serde_json::from_str(error).unwrap()
                            }
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
            self.events
                .borrow_mut()
                .push((event_type, attributes, data.to_vec()));
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

    fn submission_to_state(
        mut context: TestTransactionContext,
        transactor_key: SigningKey,
        addresses: Vec<String>,
        submission: Submission,
    ) -> TestTransactionContext {
        let message_builder = MessageBuilder::new_deterministic(address::FAMILY, address::VERSION);

        let input_addresses = addresses.clone();
        let output_addresses = addresses;
        let dependencies = vec![];

        let (tx, id) = message_builder.make_sawtooth_transaction(
            input_addresses,
            output_addresses,
            dependencies,
            &submission,
            &transactor_key,
        );

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

    #[test]
    fn bootstrap_from_initial_state() {
        let root_key = key_from_seed(0);
        let context = TestTransactionContext::new();
        let builder = SubmissionBuilder::bootstrap_root(root_key.verifying_key());
        let submission = builder.build(0xffff);

        let context = submission_to_state(context, root_key, vec![key_address("root")], submission);

        insta::assert_yaml_snapshot!(context.readable_state(), {
            ".**.date" => "[date]",
            ".**.transaction_id" => "[hash]",
            ".**.key" => "[pem]",
        }, @r###"
        ---
        - - 7ed19313e8ece6c4f5551b9bd1090797ad25c6d85f7b523b2214d4fe448372279aa95c
          - current:
              date: "[date]"
              key: "[pem]"
            expired: ~
            id: root
        "###);
        insta::assert_yaml_snapshot!(context.readable_events(), {
            ".**.date" => "[date]",
        }, @r###"
        ---
        - - opa/operation
          - - - transaction_id
              - f9ad4db8361e558014fb7ef69d906ab805e84bf4df6cc32f4517bb11de513f37009ee785de3d8103696aa05fdec582a223b1de3775a2d3721ef0dc13adcf067e
          - KeyUpdate:
              current:
                date: "[date]"
                key: "-----BEGIN PUBLIC KEY-----\r\nMFYwEAYHKoZIzj0CAQYFK4EEAAoDQgAEm++NVW2A5Drn4L7LOn5oOLld7+RYlu1g\r\ndbuQNdBsmWQFjSRFb/vyW2dcdou7IhKn73bgfza7E9H49xQEG7eMJA==\r\n-----END PUBLIC KEY-----\r\n"
              expired: ~
              id: root
        "###);
    }

    #[test]
    fn bootstrap_from_initial_state_does_not_require_transactor_key() {
        let root_key = key_from_seed(0);
        let context = TestTransactionContext::new();
        let another_key = key_from_seed(1);
        let builder = SubmissionBuilder::bootstrap_root(root_key.verifying_key());
        let submission = builder.build(0xffff);

        let context =
            submission_to_state(context, another_key, vec![key_address("root")], submission);

        insta::assert_yaml_snapshot!(context.readable_state(),{
            ".**.date" => "[date]",
            ".**.transaction_id" => "[hash]",
            ".**.key" => "[pem]",
        }, @r###"
        ---
        - - 7ed19313e8ece6c4f5551b9bd1090797ad25c6d85f7b523b2214d4fe448372279aa95c
          - current:
              date: "[date]"
              key: "[pem]"
            expired: ~
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
              - 3b8241d24ff6c90035ed8f64f3b6ffc64999670d9b8406a7fe717976ea650a164581fec952450e1284b36079748c794ddb8b5776b1db2384a38cd872d3f75737
          - KeyUpdate:
              current:
                date: "[date]"
                key: "[pem]"
              expired: ~
              id: root
        "###);
    }

    /// Needed for further tests
    fn bootstrap_root() -> (TestTransactionContext, SigningKey) {
        let root_key = key_from_seed(0);

        let context = TestTransactionContext::new();
        let builder = SubmissionBuilder::bootstrap_root(root_key.verifying_key());
        let submission = builder.build(0xffff);

        (
            submission_to_state(
                context,
                root_key.clone(),
                vec![key_address("root")],
                submission,
            ),
            root_key,
        )
    }

    #[test]
    fn rotate_root() {
        let old_key = key_from_seed(0);

        let (context, root_key) = bootstrap_root();
        let new_root = key_from_seed(1);
        let builder = SubmissionBuilder::rotate_key("root", &old_key, &new_root, &root_key);
        let submission = builder.build(0xffff);

        let context = submission_to_state(context, old_key, vec![key_address("root")], submission);

        insta::assert_yaml_snapshot!(context.readable_state(),{
            ".**.date" => "[date]",
            ".**.transaction_id" => "[hash]",
            ".**.key" => "[pem]",

        },  @r###"
        ---
        - - 7ed19313e8ece6c4f5551b9bd1090797ad25c6d85f7b523b2214d4fe448372279aa95c
          - current:
              date: "[date]"
              key: "[pem]"
            expired:
              date: "[date]"
              key: "[pem]"
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
              - f9ad4db8361e558014fb7ef69d906ab805e84bf4df6cc32f4517bb11de513f37009ee785de3d8103696aa05fdec582a223b1de3775a2d3721ef0dc13adcf067e
          - KeyUpdate:
              current:
                date: "[date]"
                key: "[pem]"
              expired: ~
              id: root
        - - opa/operation
          - - - transaction_id
              - c9f05ca8c5f70d577707b2c547cc7ab75fdc7034f1484dab2658c1c2e2ef25461b1fe8df23d3949811cd78f9927aa4655a984e344299011e2d009e8638fb7eb8
          - KeyUpdate:
              current:
                date: "[date]"
                key: "[pem]"
              expired:
                date: "[date]"
                key: "[pem]"
              id: root
        "### );
    }

    #[test]
    fn register_valid_key() {
        let (context, root_key) = bootstrap_root();
        let non_root_key = key_from_seed(1);

        let builder =
            SubmissionBuilder::register_key("nonroot", &non_root_key.verifying_key(), &root_key);
        let submission = builder.build(0xffff);

        let context =
            submission_to_state(context, root_key, vec![key_address("nonroot")], submission);

        insta::assert_yaml_snapshot!(context.readable_state(),{
            ".**.date" => "[date]",
            ".**.transaction_id" => "[hash]",
            ".**.key" => "[pem]",
        }, @r###"
        ---
        - - 7ed19313e8ece6c4f5551b9bd1090797ad25c6d85f7b523b2214d4fe448372279aa95c
          - current:
              date: "[date]"
              key: "[pem]"
            expired: ~
            id: root
        - - 7ed1931c7f165dbd2e5e0dd1c360aa665b4366010392df934edbea19b9bd29f0a228af
          - current:
              date: "[date]"
              key: "[pem]"
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
              - f9ad4db8361e558014fb7ef69d906ab805e84bf4df6cc32f4517bb11de513f37009ee785de3d8103696aa05fdec582a223b1de3775a2d3721ef0dc13adcf067e
          - KeyUpdate:
              current:
                date: "[date]"
                key: "[pem]"
              expired: ~
              id: root
        - - opa/operation
          - - - transaction_id
              - f02e95d91b00e441e12ae03d84855c0cec6a107f802cfe73e45daacebb83eebd49857616982936a2ca688d9c68735df01e5ebe7843dce3632ea1f1fab59e7ec5
          - KeyUpdate:
              current:
                date: "[date]"
                key: "[pem]"
              expired: ~
              id: nonroot
        "###);
    }

    #[test]
    fn rotate_valid_key() {
        let (context, root_key) = bootstrap_root();
        let non_root_key = key_from_seed(1);

        let builder =
            SubmissionBuilder::register_key("nonroot", &non_root_key.verifying_key(), &root_key);
        let submission = builder.build(0xffff);

        let context = submission_to_state(
            context,
            root_key.clone(),
            vec![key_address("nonroot")],
            submission,
        );

        let new_non_root = key_from_seed(2);
        let builder =
            SubmissionBuilder::rotate_key("nonroot", &non_root_key, &new_non_root, &root_key);
        let submission = builder.build(0xffff);

        let context =
            submission_to_state(context, root_key, vec![key_address("nonroot")], submission);

        insta::assert_yaml_snapshot!(context.readable_state(),{
            ".**.date" => "[date]",
            ".**.transaction_id" => "[hash]",
            ".**.key" => "[pem]",
        }, @r###"
        ---
        - - 7ed19313e8ece6c4f5551b9bd1090797ad25c6d85f7b523b2214d4fe448372279aa95c
          - current:
              date: "[date]"
              key: "[pem]"
            expired: ~
            id: root
        - - 7ed1931c7f165dbd2e5e0dd1c360aa665b4366010392df934edbea19b9bd29f0a228af
          - current:
              date: "[date]"
              key: "[pem]"
            expired:
              date: "[date]"
              key: "[pem]"
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
              - f9ad4db8361e558014fb7ef69d906ab805e84bf4df6cc32f4517bb11de513f37009ee785de3d8103696aa05fdec582a223b1de3775a2d3721ef0dc13adcf067e
          - KeyUpdate:
              current:
                date: "[date]"
                key: "[pem]"
              expired: ~
              id: root
        - - opa/operation
          - - - transaction_id
              - f02e95d91b00e441e12ae03d84855c0cec6a107f802cfe73e45daacebb83eebd49857616982936a2ca688d9c68735df01e5ebe7843dce3632ea1f1fab59e7ec5
          - KeyUpdate:
              current:
                date: "[date]"
                key: "[pem]"
              expired: ~
              id: nonroot
        - - opa/operation
          - - - transaction_id
              - df0e2ce772a73cb8aab7591a05fdae10bdd169279925b94221091e662ff336a971d18175f98b67608e4fb08ac0bef3c0c5fece2ec1ab6d91765abcb4174f294b
          - KeyUpdate:
              current:
                date: "[date]"
                key: "[pem]"
              expired:
                date: "[date]"
                key: "[pem]"
              id: nonroot
        "###);
    }

    #[test]
    fn cannot_register_key_as_root() {
        let (context, root_key) = bootstrap_root();
        let non_root_key = key_from_seed(1);

        let builder =
            SubmissionBuilder::register_key("root", &non_root_key.verifying_key(), &root_key);
        let submission = builder.build(0xffff);

        let context =
            submission_to_state(context, root_key, vec![key_address("nonroot")], submission);

        insta::assert_yaml_snapshot!(context.readable_state(),{
            ".**.date" => "[date]",
            ".**.transaction_id" => "[hash]",
            ".**.key" => "[pem]",
        }, @r###"
        ---
        - - 7ed19313e8ece6c4f5551b9bd1090797ad25c6d85f7b523b2214d4fe448372279aa95c
          - current:
              date: "[date]"
              key: "[pem]"
            expired: ~
            id: root
        "### );
        insta::assert_yaml_snapshot!(context.readable_events(), {
            ".**.date" => "[date]",
        }, @r###"
        ---
        - - opa/operation
          - - - transaction_id
              - f9ad4db8361e558014fb7ef69d906ab805e84bf4df6cc32f4517bb11de513f37009ee785de3d8103696aa05fdec582a223b1de3775a2d3721ef0dc13adcf067e
          - KeyUpdate:
              current:
                date: "[date]"
                key: "-----BEGIN PUBLIC KEY-----\r\nMFYwEAYHKoZIzj0CAQYFK4EEAAoDQgAEm++NVW2A5Drn4L7LOn5oOLld7+RYlu1g\r\ndbuQNdBsmWQFjSRFb/vyW2dcdou7IhKn73bgfza7E9H49xQEG7eMJA==\r\n-----END PUBLIC KEY-----\r\n"
              expired: ~
              id: root
        - - opa/operation
          - - - transaction_id
              - b11356fef5b170aa20d80d6f16c2da5dd284e4bf18a08f1a75a16e5d2736e02e562901349b0c173de62b54e7103d6b6c3061100329c7f6ae1444418f7aea964c
          - error: Invalid operation
        "###);
    }
    #[test]
    fn cannot_register_existing_key() {
        let (context, root_key) = bootstrap_root();
        let non_root_key = key_from_seed(1);

        let builder =
            SubmissionBuilder::register_key("nonroot", &non_root_key.verifying_key(), &root_key);
        let submission = builder.build(0xffff);

        let context = submission_to_state(
            context,
            root_key.clone(),
            vec![key_address("nonroot")],
            submission,
        );

        let builder =
            SubmissionBuilder::register_key("nonroot", &non_root_key.verifying_key(), &root_key);
        let submission = builder.build(0xffff);

        let context =
            submission_to_state(context, root_key, vec![key_address("nonroot")], submission);

        insta::assert_yaml_snapshot!(context.readable_state(),{
            ".**.date" => "[date]",
            ".**.transaction_id" => "[hash]",
            ".**.key" => "[pem]",
        }, @r###"
        ---
        - - 7ed19313e8ece6c4f5551b9bd1090797ad25c6d85f7b523b2214d4fe448372279aa95c
          - current:
              date: "[date]"
              key: "[pem]"
            expired: ~
            id: root
        - - 7ed1931c7f165dbd2e5e0dd1c360aa665b4366010392df934edbea19b9bd29f0a228af
          - current:
              date: "[date]"
              key: "[pem]"
            expired: ~
            id: nonroot
        "### );
        insta::assert_yaml_snapshot!(context.readable_events(), {
            ".**.date" => "[date]",
        }, @r###"
        ---
        - - opa/operation
          - - - transaction_id
              - f9ad4db8361e558014fb7ef69d906ab805e84bf4df6cc32f4517bb11de513f37009ee785de3d8103696aa05fdec582a223b1de3775a2d3721ef0dc13adcf067e
          - KeyUpdate:
              current:
                date: "[date]"
                key: "-----BEGIN PUBLIC KEY-----\r\nMFYwEAYHKoZIzj0CAQYFK4EEAAoDQgAEm++NVW2A5Drn4L7LOn5oOLld7+RYlu1g\r\ndbuQNdBsmWQFjSRFb/vyW2dcdou7IhKn73bgfza7E9H49xQEG7eMJA==\r\n-----END PUBLIC KEY-----\r\n"
              expired: ~
              id: root
        - - opa/operation
          - - - transaction_id
              - f02e95d91b00e441e12ae03d84855c0cec6a107f802cfe73e45daacebb83eebd49857616982936a2ca688d9c68735df01e5ebe7843dce3632ea1f1fab59e7ec5
          - KeyUpdate:
              current:
                date: "[date]"
                key: "-----BEGIN PUBLIC KEY-----\r\nMFYwEAYHKoZIzj0CAQYFK4EEAAoDQgAEPpmlQdtpvTIEDf5QN/v1IQ2vqBUaceIc\r\nUgSwXZXOCmL7g7qGlvAO9WIdhMhAGBqtVoeU+dmwlpmdP5vsUhVHnQ==\r\n-----END PUBLIC KEY-----\r\n"
              expired: ~
              id: nonroot
        - - opa/operation
          - - - transaction_id
              - f02e95d91b00e441e12ae03d84855c0cec6a107f802cfe73e45daacebb83eebd49857616982936a2ca688d9c68735df01e5ebe7843dce3632ea1f1fab59e7ec5
          - error: Invalid operation
        "###);
    }

    #[test]
    fn set_a_policy() {
        let (context, root_key) = bootstrap_root();

        // Policies can only be set by the root key owner
        let builder = SubmissionBuilder::set_policy("test", vec![0, 1, 2, 3], root_key.clone());

        let submission = builder.build(0xffff);

        let context =
            submission_to_state(context, root_key, vec![key_address("nonroot")], submission);

        insta::assert_yaml_snapshot!(context.readable_state(),{
            ".**.date" => "[date]",
            ".**.transaction_id" => "[hash]",
            ".**.key" => "[pem]",
        }, @r###"
        ---
        - - 7ed19313e8ece6c4f5551b9bd1090797ad25c6d85f7b523b2214d4fe448372279aa95c
          - current:
              date: "[date]"
              key: "[pem]"
            expired: ~
            id: root
        - - 7ed1931c262a4be700b69974438a35ae56a07ce96778b276c8a061dc254d9862c7ecff
          - - 0
            - 1
            - 2
            - 3
        "###);

        insta::assert_yaml_snapshot!(context.readable_events(),{
            ".**.date" => "[date]",
        }, @r###"
        ---
        - - opa/operation
          - - - transaction_id
              - f9ad4db8361e558014fb7ef69d906ab805e84bf4df6cc32f4517bb11de513f37009ee785de3d8103696aa05fdec582a223b1de3775a2d3721ef0dc13adcf067e
          - KeyUpdate:
              current:
                date: "[date]"
                key: "-----BEGIN PUBLIC KEY-----\r\nMFYwEAYHKoZIzj0CAQYFK4EEAAoDQgAEm++NVW2A5Drn4L7LOn5oOLld7+RYlu1g\r\ndbuQNdBsmWQFjSRFb/vyW2dcdou7IhKn73bgfza7E9H49xQEG7eMJA==\r\n-----END PUBLIC KEY-----\r\n"
              expired: ~
              id: root
        - - opa/operation
          - - - transaction_id
              - d97436915a49d3a37eb5b0615c9fd415fc7f79cd5cb1c8f45fcd2d948e9bff7029cb8351cb59a5d25aee9315158af8bed3be5416dd28975803d83465dfd189e1
          - PolicyUpdate:
              date: "[date]"
              id: test
              policy_address: 7ed1931c262a4be700b69974438a35ae56a07ce96778b276c8a061dc254d9862c7ecff
        "###);
    }
}
