use core::fmt;
#[cfg(not(feature = "std"))]
use parity_scale_codec::alloc::string::String;

#[cfg(not(feature = "std"))]
use scale_info::{prelude::vec, prelude::vec::Vec};

#[cfg_attr(
	feature = "parity-encoding",
	derive(
		scale_info::TypeInfo,
		parity_scale_codec::Encode,
		parity_scale_codec::Decode,
		scale_encode::EncodeAsType,
		scale_decode::DecodeAsType,
		parity_scale_codec::MaxEncodedLen
	)
)]
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize,))]
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct H128([u8; 16]);

impl H128 {
	pub fn new(value: [u8; 16]) -> Self {
		H128(value)
	}

	pub fn into(self) -> [u8; 16] {
		self.0
	}
}

#[cfg(not(feature = "std"))]
use scale_info::prelude::format;
#[cfg_attr(
	feature = "parity-encoding",
	derive(
		scale_info::TypeInfo,
		parity_scale_codec::Encode,
		parity_scale_codec::Decode,
		scale_encode::EncodeAsType,
		scale_decode::DecodeAsType,
		parity_scale_codec::MaxEncodedLen
	)
)]
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize,))]
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct PolicyAddress(H128);

impl fmt::Display for PolicyAddress {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "PolicyAddress({})", hex::encode(self.0.into()))
	}
}

impl From<[u8; 16]> for PolicyAddress {
	fn from(value: [u8; 16]) -> Self {
		tracing::debug!("Converting [u8; 16] to PolicyAddress");
		PolicyAddress(H128::new(value))
	}
}

#[cfg_attr(
	feature = "parity-encoding",
	derive(
		scale_info::TypeInfo,
		parity_scale_codec::Encode,
		parity_scale_codec::Decode,
		scale_encode::EncodeAsType,
		parity_scale_codec::MaxEncodedLen
	)
)]
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PolicyMetaAddress(H128);

impl PolicyMetaAddress {
	pub fn new(value: H128) -> Self {
		PolicyMetaAddress(value)
	}

	pub fn into(self) -> H128 {
		self.0
	}
}

impl From<[u8; 16]> for H128 {
	fn from(value: [u8; 16]) -> Self {
		H128(value)
	}
}

impl From<[u8; 16]> for PolicyMetaAddress {
	fn from(value: [u8; 16]) -> Self {
		PolicyMetaAddress(H128::new(value))
	}
}

impl From<[u8; 16]> for KeyAddress {
	fn from(value: [u8; 16]) -> Self {
		KeyAddress(H128::new(value))
	}
}

impl From<H128> for PolicyMetaAddress {
	fn from(value: H128) -> Self {
		PolicyMetaAddress(value)
	}
}

impl From<H128> for PolicyAddress {
	fn from(value: H128) -> Self {
		PolicyAddress(value)
	}
}

impl From<H128> for KeyAddress {
	fn from(value: H128) -> Self {
		KeyAddress(value)
	}
}

#[cfg_attr(
	feature = "parity-encoding",
	derive(
		scale_info::TypeInfo,
		parity_scale_codec::Encode,
		parity_scale_codec::Decode,
		scale_encode::EncodeAsType,
		parity_scale_codec::MaxEncodedLen
	)
)]
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct KeyAddress(H128);

#[derive(Debug, Clone, Eq, PartialEq)]
// This message is used to bootstrap the root key for a newly created authz tp,
// it can only be executed once
pub struct BootstrapRoot {
	pub public_key: PemEncoded,
}
#[cfg_attr(
	feature = "parity-encoding",
	derive(
		scale_info::TypeInfo,
		parity_scale_codec::Encode,
		parity_scale_codec::Decode,
		scale_encode::EncodeAsType,
		scale_decode::DecodeAsType,
	)
)]
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize,))]
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PemEncoded(String);

impl PemEncoded {
	pub fn as_str(&self) -> &str {
		&self.0
	}

	pub fn as_bytes(&self) -> &[u8] {
		self.0.as_bytes()
	}
}

impl PemEncoded {
	pub fn new(encoded: String) -> Self {
		PemEncoded(encoded)
	}
}

impl From<String> for PemEncoded {
	fn from(encoded: String) -> Self {
		PemEncoded(encoded)
	}
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RegisterKey {
	pub public_key: PemEncoded,
	pub id: String,
	pub overwrite_existing: bool,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct NewPublicKey {
	pub public_key: PemEncoded,
	pub id: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
// Rotate the key with name to the new public key, the SignedOperation for this
// message must be signed by the old key. The signature must be valid for
// the new one, to demonstrate ownership of both keys
pub struct RotateKey {
	pub payload: NewPublicKey,
	pub previous_signing_key: PemEncoded,
	pub previous_signature: Vec<u8>,
	pub new_signing_key: PemEncoded,
	pub new_signature: Vec<u8>,
}
#[derive(Debug, Clone, Eq, PartialEq)]
// Set the policy with name to the new policy, the SignedOperation for this must
// be signed by the root key
pub struct SetPolicy {
	pub id: String,
	pub policy: Policy,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SignedOperationPayload {
	pub operation: Operation,
}

#[derive(Debug, Clone, Eq, PartialEq)]
// An OPA TP operation and its signature
pub struct SignedOperation {
	pub payload: SignedOperationPayload,
	pub verifying_key: PemEncoded,
	pub signature: Vec<u8>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Operation {
	RegisterKey(RegisterKey),
	RotateKey(RotateKey),
	SetPolicy(SetPolicy),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct OpaSubmission {
	pub version: String,
	pub correlation_id: [u8; 16],
	pub span_id: u64,
	pub payload: Payload,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Payload {
	BootstrapRoot(BootstrapRoot),
	SignedOperation(SignedOperation),
}

#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize,))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyRegistration {
	pub key: PemEncoded,
	pub version: u64,
}

#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize,))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keys {
	pub id: String,
	pub current: KeyRegistration,
	pub expired: Option<KeyRegistration>,
}

#[cfg_attr(
	feature = "parity-encoding",
	derive(
		scale_info::TypeInfo,
		parity_scale_codec::Encode,
		parity_scale_codec::Decode,
		scale_encode::EncodeAsType,
		scale_decode::DecodeAsType,
	)
)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Policy(Vec<u8>);

impl Policy {
	pub fn new(data: Vec<u8>) -> Self {
		Policy(data)
	}

	pub fn as_bytes(&self) -> &[u8] {
		&self.0
	}

	pub fn into_vec(self) -> Vec<u8> {
		self.0
	}
}

#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize,))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyMeta {
	pub id: String,
	pub hash: H128,
	pub policy_address: PolicyAddress,
}

#[cfg_attr(
	feature = "parity-encoding",
	derive(
		scale_info::TypeInfo,
		parity_scale_codec::Encode,
		parity_scale_codec::Decode,
		scale_encode::EncodeAsType,
		scale_decode::DecodeAsType
	)
)]
#[derive(Debug, Clone)]
pub struct OpaSettings {
	pub policy_address: PolicyAddress,
	pub policy_name: String,
	pub entrypoint: String,
}

#[cfg(feature = "parity-encoding")]
use parity_scale_codec::MaxEncodedLen;

#[cfg(feature = "parity-encoding")]
impl MaxEncodedLen for OpaSettings {
	fn max_encoded_len() -> usize {
		PolicyAddress::max_encoded_len() + 1024
	}
}

#[cfg(feature = "parity-encoding")]
pub mod codec {
	use super::*;
	use parity_scale_codec::{Decode, Encode};

	use scale_decode::DecodeAsType;
	use scale_encode::EncodeAsType;
	#[cfg(not(feature = "std"))]
	use scale_info::prelude::vec::Vec;

	use scale_info::TypeInfo;

	#[derive(Encode, EncodeAsType, DecodeAsType, Decode, Debug, Eq, PartialEq, TypeInfo, Clone)]
	pub struct KeysV1 {
		pub id: String,
		pub current: KeyRegistrationV1,
		pub expired: Option<KeyRegistrationV1>,
	}

	impl MaxEncodedLen for KeysV1 {
		fn max_encoded_len() -> usize {
			1024 + KeyRegistrationV1::max_encoded_len() +
				Option::<KeyRegistrationV1>::max_encoded_len()
		}
	}

	#[derive(Encode, EncodeAsType, DecodeAsType, Decode, Debug, Eq, PartialEq, TypeInfo, Clone)]
	pub struct KeyRegistrationV1 {
		// Der encoded public key
		pub key: PemEncoded,
		pub version: u64,
	}

	impl MaxEncodedLen for KeyRegistrationV1 {
		fn max_encoded_len() -> usize {
			1024 + u64::max_encoded_len()
		}
	}

	impl From<super::Keys> for KeysV1 {
		fn from(keys: super::Keys) -> Self {
			Self {
				id: keys.id,
				current: KeyRegistrationV1 { key: keys.current.key, version: keys.current.version },
				expired: keys.expired.map(|expired_key| KeyRegistrationV1 {
					key: expired_key.key,
					version: expired_key.version,
				}),
			}
		}
	}

	impl core::convert::TryFrom<KeysV1> for super::Keys {
		type Error = core::convert::Infallible;

		fn try_from(keys_v1: KeysV1) -> Result<Self, Self::Error> {
			Ok(Self {
				id: keys_v1.id,
				current: super::KeyRegistration {
					key: keys_v1.current.key,
					version: keys_v1.current.version,
				},
				expired: keys_v1.expired.map(|expired_key_v1| super::KeyRegistration {
					key: expired_key_v1.key,
					version: expired_key_v1.version,
				}),
			})
		}
	}

	#[derive(Encode, EncodeAsType, DecodeAsType, Decode, Debug, TypeInfo, Clone, PartialEq, Eq)]
	pub struct BootstrapRootV1 {
		pub public_key: PemEncoded,
	}

	#[derive(Encode, EncodeAsType, DecodeAsType, Decode, Debug, TypeInfo, Clone, PartialEq, Eq)]
	pub struct RegisterKeyV1 {
		pub public_key: PemEncoded,
		pub id: String,
		pub overwrite_existing: bool,
	}
	#[derive(Encode, EncodeAsType, DecodeAsType, Decode, Debug, TypeInfo, Clone, PartialEq, Eq)]
	pub struct NewPublicKeyV1 {
		pub public_key: PemEncoded,
		pub id: String,
	}

	impl From<super::NewPublicKey> for NewPublicKeyV1 {
		fn from(new_public_key: super::NewPublicKey) -> Self {
			Self { public_key: new_public_key.public_key, id: new_public_key.id }
		}
	}

	impl core::convert::TryFrom<NewPublicKeyV1> for super::NewPublicKey {
		type Error = core::convert::Infallible;

		fn try_from(new_public_key_v1: NewPublicKeyV1) -> Result<Self, Self::Error> {
			Ok(Self { public_key: new_public_key_v1.public_key, id: new_public_key_v1.id })
		}
	}

	#[derive(Encode, EncodeAsType, DecodeAsType, Decode, Debug, TypeInfo, Clone, PartialEq, Eq)]
	pub struct RotateKeyV1 {
		pub payload: NewPublicKeyV1,
		pub previous_signing_key: PemEncoded,
		pub previous_signature: Vec<u8>,
		pub new_signing_key: PemEncoded,
		pub new_signature: Vec<u8>,
	}

	#[derive(Encode, EncodeAsType, DecodeAsType, Decode, Debug, TypeInfo, Clone, PartialEq, Eq)]
	pub struct SetPolicyV1 {
		pub id: String,
		pub policy: Policy,
	}

	#[derive(Encode, EncodeAsType, DecodeAsType, Decode, Debug, TypeInfo, Clone, PartialEq, Eq)]
	pub struct SignedOperationPayloadV1 {
		pub operation: OperationV1,
	}

	#[derive(Encode, EncodeAsType, DecodeAsType, Decode, Debug, TypeInfo, Clone, PartialEq, Eq)]
	pub struct SignedOperationV1 {
		pub payload: SignedOperationPayloadV1,
		pub verifying_key: PemEncoded,
		pub signature: Vec<u8>,
	}

	impl From<super::SignedOperation> for SignedOperationV1 {
		fn from(signed_operation: super::SignedOperation) -> Self {
			Self {
				payload: SignedOperationPayloadV1 {
					operation: signed_operation.payload.operation.into(),
				},
				verifying_key: signed_operation.verifying_key,
				signature: signed_operation.signature,
			}
		}
	}

	impl From<super::Operation> for OperationV1 {
		fn from(operation: super::Operation) -> Self {
			match operation {
				super::Operation::RegisterKey(register_key) =>
					OperationV1::RegisterKey(register_key.into()),
				super::Operation::RotateKey(rotate_key) =>
					OperationV1::RotateKey(rotate_key.into()),
				super::Operation::SetPolicy(set_policy) =>
					OperationV1::SetPolicy(set_policy.into()),
			}
		}
	}

	impl From<super::RegisterKey> for RegisterKeyV1 {
		fn from(register_key: super::RegisterKey) -> Self {
			Self {
				public_key: register_key.public_key,
				id: register_key.id,
				overwrite_existing: register_key.overwrite_existing,
			}
		}
	}

	impl From<super::RotateKey> for RotateKeyV1 {
		fn from(rotate_key: super::RotateKey) -> Self {
			Self {
				payload: rotate_key.payload.into(),
				previous_signing_key: rotate_key.previous_signing_key,
				previous_signature: rotate_key.previous_signature,
				new_signing_key: rotate_key.new_signing_key,
				new_signature: rotate_key.new_signature,
			}
		}
	}

	impl From<super::SetPolicy> for SetPolicyV1 {
		fn from(set_policy: super::SetPolicy) -> Self {
			Self { id: set_policy.id, policy: set_policy.policy }
		}
	}

	#[derive(Encode, EncodeAsType, DecodeAsType, Decode, Debug, Clone, TypeInfo, PartialEq, Eq)]
	pub enum OperationV1 {
		RegisterKey(RegisterKeyV1),
		RotateKey(RotateKeyV1),
		SetPolicy(SetPolicyV1),
	}

	#[derive(Encode, EncodeAsType, DecodeAsType, Decode, Debug, TypeInfo, Clone, PartialEq, Eq)]
	pub struct OpaSubmissionV1 {
		pub version: String,
		pub correlation_id: [u8; 16],
		pub span_id: u64,
		pub payload: PayloadV1,
	}

	#[derive(Encode, EncodeAsType, DecodeAsType, Decode, Debug, TypeInfo, Clone, PartialEq, Eq)]
	pub enum PayloadV1 {
		BootstrapRoot(BootstrapRootV1),
		SignedOperation(SignedOperationV1),
	}

	#[derive(Encode, EncodeAsType, DecodeAsType, Decode, Debug, TypeInfo, Clone, PartialEq)]
	pub struct PolicyV1(Vec<u8>);

	impl PolicyV1 {
		pub fn into_vec(self) -> Vec<u8> {
			self.0
		}
	}

	impl MaxEncodedLen for PolicyV1 {
		fn max_encoded_len() -> usize {
			1024 * 1024 * 10
		}
	}

	impl From<Policy> for codec::PolicyV1 {
		fn from(item: Policy) -> Self {
			Self(item.0)
		}
	}

	impl core::convert::TryFrom<codec::PolicyV1> for Policy {
		type Error = core::convert::Infallible;

		fn try_from(value: codec::PolicyV1) -> Result<Self, Self::Error> {
			Ok(Self(value.0))
		}
	}

	#[derive(Encode, Decode, Debug, TypeInfo, Clone, PartialEq)]
	pub struct PolicyMetaV1 {
		pub id: String,
		pub hash: H128,
		pub policy_address: PolicyAddress,
	}

	impl MaxEncodedLen for PolicyMetaV1 {
		fn max_encoded_len() -> usize {
			1024 + H128::max_encoded_len() + PolicyAddress::max_encoded_len()
		}
	}

	impl From<super::PolicyMeta> for codec::PolicyMetaV1 {
		fn from(item: super::PolicyMeta) -> Self {
			Self { id: item.id, hash: item.hash, policy_address: item.policy_address }
		}
	}

	impl core::convert::TryFrom<codec::PolicyMetaV1> for super::PolicyMeta {
		type Error = core::convert::Infallible;

		fn try_from(value: codec::PolicyMetaV1) -> Result<Self, Self::Error> {
			Ok(Self { id: value.id, hash: value.hash, policy_address: value.policy_address })
		}
	}

	impl From<codec::BootstrapRootV1> for BootstrapRoot {
		fn from(item: codec::BootstrapRootV1) -> Self {
			Self { public_key: item.public_key }
		}
	}

	impl From<BootstrapRoot> for codec::BootstrapRootV1 {
		fn from(item: BootstrapRoot) -> Self {
			tracing::debug!(target: "codec_conversion", "Converting BootstrapRoot to BootstrapRootV1");
			Self { public_key: item.public_key }
		}
	}

	impl From<codec::PayloadV1> for Payload {
		fn from(item: codec::PayloadV1) -> Self {
			match item {
				codec::PayloadV1::BootstrapRoot(v) => Self::BootstrapRoot(v.into()),
				codec::PayloadV1::SignedOperation(v) => Self::SignedOperation(v.into()),
			}
		}
	}

	impl From<codec::OpaSubmissionV1> for OpaSubmission {
		fn from(item: codec::OpaSubmissionV1) -> Self {
			Self {
				correlation_id: item.correlation_id,
				version: item.version,
				span_id: item.span_id,
				payload: item.payload.into(),
			}
		}
	}

	impl From<OpaSubmission> for codec::OpaSubmissionV1 {
		fn from(item: OpaSubmission) -> Self {
			tracing::debug!(target: "codec_conversion", "Converting OpaSubmission to OpaSubmissionV1");
			Self {
				version: item.version,
				correlation_id: item.correlation_id,
				span_id: item.span_id,
				payload: match item.payload {
					Payload::BootstrapRoot(v) => {
						tracing::trace!(target: "codec_conversion", "Payload is BootstrapRoot");
						codec::PayloadV1::BootstrapRoot(v.into())
					},
					Payload::SignedOperation(v) => {
						tracing::trace!(target: "codec_conversion", "Payload is SignedOperation");
						codec::PayloadV1::SignedOperation(v.into())
					},
				},
			}
		}
	}

	impl From<codec::OperationV1> for Operation {
		fn from(item: codec::OperationV1) -> Self {
			match item {
				codec::OperationV1::RegisterKey(v) => Self::RegisterKey(v.into()),
				codec::OperationV1::RotateKey(v) => Self::RotateKey(v.into()),
				codec::OperationV1::SetPolicy(v) => Self::SetPolicy(v.into()),
			}
		}
	}

	impl From<codec::SignedOperationV1> for SignedOperation {
		fn from(item: codec::SignedOperationV1) -> Self {
			Self {
				payload: SignedOperationPayload { operation: item.payload.operation.into() },
				verifying_key: item.verifying_key,
				signature: item.signature,
			}
		}
	}

	impl From<codec::RotateKeyV1> for RotateKey {
		fn from(item: codec::RotateKeyV1) -> Self {
			Self {
				payload: NewPublicKey { public_key: item.payload.public_key, id: item.payload.id },
				previous_signing_key: item.previous_signing_key,
				previous_signature: item.previous_signature,
				new_signing_key: item.new_signing_key,
				new_signature: item.new_signature,
			}
		}
	}

	impl From<codec::RegisterKeyV1> for RegisterKey {
		fn from(item: codec::RegisterKeyV1) -> Self {
			Self {
				public_key: item.public_key,
				id: item.id,
				overwrite_existing: item.overwrite_existing,
			}
		}
	}

	impl From<codec::SetPolicyV1> for SetPolicy {
		fn from(item: codec::SetPolicyV1) -> Self {
			Self { id: item.id, policy: item.policy }
		}
	}

	impl From<SignedOperationPayload> for codec::SignedOperationPayloadV1 {
		fn from(item: SignedOperationPayload) -> Self {
			codec::SignedOperationPayloadV1 {
				operation: match item.operation {
					Operation::RegisterKey(v) => codec::OperationV1::RegisterKey(v.into()),
					Operation::RotateKey(v) => codec::OperationV1::RotateKey(v.into()),
					Operation::SetPolicy(v) => codec::OperationV1::SetPolicy(v.into()),
				},
			}
		}
	}
}
