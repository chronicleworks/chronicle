use core::panic;
use std::{
    cell::RefCell,
    sync::{Arc, Mutex},
};

use chronicle_signing::{
    ChronicleSigning, OpaKnownKeyNamesSigner, SecretError, WithSecret, OPA_NAMESPACE,
};
use common::{
    k256::{
        ecdsa::{Signature, SigningKey, VerifyingKey},
        pkcs8::{EncodePublicKey, LineEnding},
        schnorr::signature::Signer,
        PublicKey,
    },
    opa::{
        codec::{NewPublicKeyV1, SignedOperationPayloadV1},
        BootstrapRoot, NewPublicKey, OpaSubmission, Operation, Payload, Policy, RegisterKey,
        RotateKey, SetPolicy, SignedOperation, SignedOperationPayload,
    },
};
use subxt::ext::codec::Encode;
use uuid::Uuid;

fn bootstrap_root(public_key: VerifyingKey) -> BootstrapRoot {
    let public_key: PublicKey = public_key.into();
    BootstrapRoot { public_key: public_key.to_public_key_pem(LineEnding::CRLF).unwrap().into() }
}

fn register_key(
    id: impl AsRef<str>,
    public_key: &VerifyingKey,
    overwrite_existing: bool,
) -> RegisterKey {
    let public_key: PublicKey = public_key.into();
    RegisterKey {
        id: id.as_ref().to_string(),
        public_key: public_key.to_public_key_pem(LineEnding::CRLF).unwrap().into(),
        overwrite_existing,
    }
}

fn rotate_key(id: impl AsRef<str>, old_key: &SigningKey, new_key: &SigningKey) -> RotateKey {
    let new_verifying_public_key: PublicKey = new_key.verifying_key().into();
    let new_key_message = NewPublicKey {
        id: id.as_ref().to_string(),
        public_key: new_verifying_public_key.to_public_key_pem(LineEnding::CRLF).unwrap().into(),
    };

    let new_key_bytes = NewPublicKeyV1::from(new_key_message.clone()).encode();

    let old_signature: Signature = old_key.sign(&new_key_bytes);
    let old_verifying_key = old_key.verifying_key();
    let old_verifying_public_key: PublicKey = old_verifying_key.into();

    let new_signature: Signature = new_key.sign(&new_key_bytes);

    RotateKey {
        payload: new_key_message,
        previous_signature: old_signature.to_vec(),
        previous_signing_key: old_verifying_public_key
            .to_public_key_pem(LineEnding::CRLF)
            .unwrap()
            .into(),
        new_signature: new_signature.to_vec(),
        new_signing_key: new_verifying_public_key
            .to_public_key_pem(LineEnding::CRLF)
            .unwrap()
            .into(),
    }
}

fn set_policy(id: impl AsRef<str>, policy: Vec<u8>) -> SetPolicy {
    SetPolicy { id: id.as_ref().to_owned(), policy: Policy::new(policy) }
}

enum BuildingMessage {
    BootstrapRoot(BootstrapRoot),
    RegisterKey(SignedOperation),
    RotateKey(SignedOperation),
    SetPolicy(SignedOperation),
}

pub struct SubmissionBuilder {
    message: Option<BuildingMessage>,
}

impl SubmissionBuilder {
    pub fn bootstrap_root(public_key: VerifyingKey) -> Self {
        Self { message: Some(BuildingMessage::BootstrapRoot(bootstrap_root(public_key))) }
    }

    pub async fn register_key(
        id: impl AsRef<str>,
        new_key: &str,
        signer: &ChronicleSigning,
        overwrite_existing: bool,
    ) -> Result<Self, SecretError> {
        let operation = SignedOperationPayload {
            operation: Operation::RegisterKey(register_key(
                id,
                &signer.verifying_key(OPA_NAMESPACE, new_key).await?,
                overwrite_existing,
            )),
        };

        let signature = signer
            .opa_sign(&SignedOperationPayloadV1::from(operation.clone()).encode())
            .await?;
        let key: PublicKey = signer.opa_verifying().await?.into();
        let signed_operation = SignedOperation {
            payload: operation,
            signature: signature.to_vec(),
            verifying_key: key.to_public_key_pem(LineEnding::CRLF).unwrap().into(),
        };
        Ok(Self { message: Some(BuildingMessage::RegisterKey(signed_operation)) })
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

        let operation = SignedOperationPayload {
            operation: Operation::RotateKey(rotate_key(id, &old_key, &new_key)),
        };

        let signature = signer
            .opa_sign(&SignedOperationPayloadV1::from(operation.clone()).encode())
            .await?;
        let key: PublicKey = signer.opa_verifying().await?.into();

        let signed_operation = SignedOperation {
            payload: operation,
            signature,
            verifying_key: key.to_public_key_pem(LineEnding::CRLF).unwrap().into(),
        };
        Ok(Self { message: Some(BuildingMessage::RotateKey(signed_operation)) })
    }

    pub async fn set_policy(
        id: &str,
        policy: Vec<u8>,
        signer: &ChronicleSigning,
    ) -> Result<Self, SecretError> {
        let operation =
            SignedOperationPayload { operation: Operation::SetPolicy(set_policy(id, policy)) };
        let signature = signer
            .opa_sign(&SignedOperationPayloadV1::from(operation.clone()).encode())
            .await?;
        let key: PublicKey = signer.opa_verifying().await?.into();

        let signed_operation = SignedOperation {
            payload: operation,
            signature,
            verifying_key: key.to_public_key_pem(LineEnding::CRLF).unwrap().into(),
        };

        Ok(Self { message: Some(BuildingMessage::SetPolicy(signed_operation)) })
    }

    pub fn build(self, span_id: u64, correlation_id: Uuid) -> OpaSubmission {
        OpaSubmission {
            span_id,
            correlation_id: correlation_id.into_bytes(),
            version: "1.0".to_string(),
            payload: match self.message {
                Some(BuildingMessage::BootstrapRoot(message)) => Payload::BootstrapRoot(message),
                Some(BuildingMessage::RotateKey(message)) => Payload::SignedOperation(message),
                Some(BuildingMessage::SetPolicy(message)) => Payload::SignedOperation(message),
                Some(BuildingMessage::RegisterKey(message)) => Payload::SignedOperation(message),
                None => panic!("No message to build"),
            },
        }
    }
}
