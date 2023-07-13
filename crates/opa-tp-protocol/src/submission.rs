use std::{
    cell::RefCell,
    sync::{Arc, Mutex},
};

use crate::{messages, PROTOCOL_VERSION};
use chronicle_signing::{
    ChronicleSigning, OpaKnownKeyNamesSigner, SecretError, WithSecret, OPA_NAMESPACE,
};
use k256::{
    ecdsa::{Signature, SigningKey, VerifyingKey},
    pkcs8::{EncodePublicKey, LineEnding},
    schnorr::signature::Signer,
    PublicKey,
};
use prost::Message;

fn bootstrap_root(public_key: VerifyingKey) -> messages::BootstrapRoot {
    let public_key: PublicKey = public_key.into();
    messages::BootstrapRoot {
        public_key: public_key.to_public_key_pem(LineEnding::CRLF).unwrap(),
    }
}

fn register_key(
    id: impl AsRef<str>,
    public_key: &VerifyingKey,
    overwrite_existing: bool,
) -> messages::RegisterKey {
    let public_key: PublicKey = public_key.into();
    messages::RegisterKey {
        id: id.as_ref().to_string(),
        public_key: public_key.to_public_key_pem(LineEnding::CRLF).unwrap(),
        overwrite_existing,
    }
}

fn rotate_key(
    id: impl AsRef<str>,
    old_key: &SigningKey,
    new_key: &SigningKey,
) -> messages::RotateKey {
    let new_key_message = messages::rotate_key::NewPublicKey {
        id: id.as_ref().to_string(),
        public_key: new_key.to_bytes().to_vec(),
    };

    let new_key_bytes = new_key_message.encode_to_vec();

    let old_signature: Signature = old_key.sign(&new_key_bytes);
    let old_verifying_key = old_key.verifying_key();
    let old_verifying_public_key: PublicKey = old_verifying_key.into();

    let new_signature: Signature = new_key.sign(&new_key_bytes);
    let new_verifying_key = new_key.verifying_key();
    let new_verifying_public_key: PublicKey = new_verifying_key.into();

    messages::RotateKey {
        payload: Some(new_key_message),
        previous_signature: old_signature.to_vec(),
        previous_signing_key: old_verifying_public_key
            .to_public_key_pem(LineEnding::CRLF)
            .unwrap(),
        new_signature: new_signature.to_vec(),
        new_signing_key: new_verifying_public_key
            .to_public_key_pem(LineEnding::CRLF)
            .unwrap(),
    }
}

fn set_policy(id: impl AsRef<str>, policy: Vec<u8>) -> messages::SetPolicy {
    messages::SetPolicy {
        id: id.as_ref().to_owned(),
        policy,
    }
}

enum BuildingMessage {
    BootstrapRoot(messages::BootstrapRoot),
    RegisterKey(messages::SignedOperation),
    RotateKey(messages::SignedOperation),
    SetPolicy(messages::SignedOperation),
}

pub struct SubmissionBuilder {
    message: Option<BuildingMessage>,
}

impl SubmissionBuilder {
    pub fn bootstrap_root(public_key: VerifyingKey) -> Self {
        Self {
            message: Some(BuildingMessage::BootstrapRoot(bootstrap_root(public_key))),
        }
    }

    pub async fn register_key(
        id: impl AsRef<str>,
        new_key: &str,
        signer: &ChronicleSigning,
        overwrite_existing: bool,
    ) -> Result<Self, SecretError> {
        let operation = messages::signed_operation::Payload {
            operation: Some(messages::signed_operation::payload::Operation::RegisterKey(
                register_key(
                    id,
                    &signer.verifying_key(OPA_NAMESPACE, new_key).await?,
                    overwrite_existing,
                ),
            )),
        };

        let signature = signer.opa_sign(&operation.encode_to_vec()).await?;
        let key: PublicKey = signer.opa_verifying().await?.into();
        let signed_operation = messages::SignedOperation {
            payload: Some(operation),
            signature: signature.to_vec(),
            verifying_key: key.to_public_key_pem(LineEnding::CRLF).unwrap(),
        };
        Ok(Self {
            message: Some(BuildingMessage::RegisterKey(signed_operation)),
        })
    }

    pub async fn rotate_key(
        id: &str,
        signer: &ChronicleSigning,
        old_key: &str,
        new_key: &str,
    ) -> Result<Self, SecretError> {
        let extract_key: Arc<Mutex<RefCell<Option<SigningKey>>>> =
            Arc::new(Mutex::new(None.into()));

        signer
            .with_signing_key(OPA_NAMESPACE, old_key, |old_key| {
                extract_key.lock().unwrap().replace(Some(old_key.clone()));
            })
            .await?;

        let old_key = extract_key.lock().unwrap().borrow().clone().unwrap();

        signer
            .with_signing_key(OPA_NAMESPACE, new_key, |new_key| {
                extract_key.lock().unwrap().replace(Some(new_key.clone()));
            })
            .await?;

        let new_key = extract_key.lock().unwrap().borrow().clone().unwrap();

        let operation = messages::signed_operation::Payload {
            operation: Some(messages::signed_operation::payload::Operation::RotateKey(
                rotate_key(id, &old_key, &new_key),
            )),
        };

        let signature = signer.opa_sign(&operation.encode_to_vec()).await?;
        let key: PublicKey = signer.opa_verifying().await?.into();

        let signed_operation = messages::SignedOperation {
            payload: Some(operation),
            signature,
            verifying_key: key.to_public_key_pem(LineEnding::CRLF).unwrap(),
        };
        Ok(Self {
            message: Some(BuildingMessage::RotateKey(signed_operation)),
        })
    }

    pub async fn set_policy(
        id: &str,
        policy: Vec<u8>,
        signer: &ChronicleSigning,
    ) -> Result<Self, SecretError> {
        let operation = messages::signed_operation::Payload {
            operation: Some(messages::signed_operation::payload::Operation::SetPolicy(
                set_policy(id, policy),
            )),
        };
        let signature = signer.opa_sign(&operation.encode_to_vec()).await?;
        let key: PublicKey = signer.opa_verifying().await?.into();

        let signed_operation = messages::SignedOperation {
            payload: Some(operation),
            signature,
            verifying_key: key.to_public_key_pem(LineEnding::CRLF).unwrap(),
        };

        Ok(Self {
            message: Some(BuildingMessage::SetPolicy(signed_operation)),
        })
    }

    pub fn build(mut self, span_id: u64) -> messages::Submission {
        let mut submission = messages::Submission::default();
        match self.message.take().unwrap() {
            BuildingMessage::BootstrapRoot(message) => {
                submission.payload = Some(messages::submission::Payload::BootstrapRoot(message));
            }
            BuildingMessage::RotateKey(message) => {
                submission.payload = Some(messages::submission::Payload::SignedOperation(message));
            }
            BuildingMessage::SetPolicy(message) => {
                submission.payload = Some(messages::submission::Payload::SignedOperation(message));
            }
            BuildingMessage::RegisterKey(message) => {
                submission.payload = Some(messages::submission::Payload::SignedOperation(message));
            }
        };
        submission.span_id = span_id;
        submission.version = PROTOCOL_VERSION.to_string();

        submission
    }
}
