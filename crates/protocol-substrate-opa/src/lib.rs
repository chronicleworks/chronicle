#![cfg_attr(feature = "strict", deny(warnings))]

use std::marker::PhantomData;

use pallet_opa::ChronicleTransactionId;
use protocol_abstract::{LedgerEvent, LedgerEventCodec, Span};
use protocol_substrate::{SubstrateClient, SubxtClientError};
use serde::Serialize;
use subxt::{
	ext::{
		codec::Decode,
		sp_core::{blake2_256, Pair},
	},
	tx::Signer,
	utils::{AccountId32, MultiAddress, MultiSignature},
};
use transaction::OpaTransaction;
//pub mod submission;
pub mod loader;
pub mod submission_builder;
pub mod transaction;

pub use subxt::ext::sp_core::blake2_128 as policy_hash;

pub struct OpaEventCodec<C>
where
	C: subxt::Config,
{
	_p: PhantomData<C>,
}

//This type must match pallet::Event but we cannot reference it directly
#[derive(Debug, Clone, Serialize)]
pub enum OpaEvent {
	PolicyUpdate { policy: common::opa::PolicyMeta, correlation_id: ChronicleTransactionId },
	KeyUpdate { keys: common::opa::Keys, correlation_id: ChronicleTransactionId },
}

impl OpaEvent {
	fn new_policy_update(
		policy_meta: common::opa::PolicyMeta,
		transaction_id: ChronicleTransactionId,
	) -> Self {
		OpaEvent::PolicyUpdate { policy: policy_meta, correlation_id: transaction_id }
	}

	fn new_key_update(keys: common::opa::Keys, correlation_id: ChronicleTransactionId) -> Self {
		OpaEvent::KeyUpdate { keys, correlation_id }
	}
}

fn extract_event<C>(
	event: subxt::events::EventDetails<C>,
) -> Result<Option<OpaEvent>, SubxtClientError>
where
	C: subxt::Config,
{
	type PolicyUpdate = (common::opa::codec::PolicyMetaV1, ChronicleTransactionId);
	type KeyUpdate = (common::opa::codec::KeysV1, ChronicleTransactionId);
	match (event.pallet_name(), event.variant_name(), event.field_bytes()) {
		("Opa", "PolicyUpdate", mut event_bytes) => match PolicyUpdate::decode(&mut event_bytes) {
			Ok((meta, correlation_id)) =>
				Ok(Some(OpaEvent::new_policy_update(meta.try_into()?, correlation_id))),
			Err(e) => {
				tracing::error!("Failed to decode ProvModel: {}", e);
				Err(e.into())
			},
		},
		("Chronicle", "KeyUpdate", mut event_bytes) => match KeyUpdate::decode(&mut event_bytes) {
			Ok((keys, correlation_id)) =>
				Ok(OpaEvent::new_key_update(keys.try_into()?, correlation_id).into()),
			Err(e) => {
				tracing::error!("Failed to decode Contradiction: {}", e);
				Err(e.into())
			},
		},
		(_pallet, _event, _) => Ok(None),
	}
}

#[async_trait::async_trait]
impl<C> LedgerEventCodec for OpaEventCodec<C>
where
	C: subxt::Config,
{
	type Error = SubxtClientError;
	type Sink = OpaEvent;
	type Source = subxt::events::EventDetails<C>;

	async fn maybe_deserialize(
		source: Self::Source,
	) -> Result<Option<(Self::Sink, Span)>, Self::Error>
	where
		Self: Sized,
	{
		match extract_event(source) {
			Ok(Some(ev)) => Ok(Some((ev, Span::NotTraced))),
			Ok(None) => Ok(None),
			Err(e) => Err(e),
		}
	}
}

impl LedgerEvent for OpaEvent {
	fn correlation_id(&self) -> [u8; 16] {
		match self {
			Self::PolicyUpdate { correlation_id, .. } => **correlation_id,
			Self::KeyUpdate { correlation_id, .. } => **correlation_id,
		}
	}
}

impl<C> Signer<C> for OpaTransaction
where
	C: subxt::Config<
		AccountId = AccountId32,
		Address = MultiAddress<AccountId32, ()>,
		Signature = MultiSignature,
	>,
{
	// The account id for an ecdsa key is the blake2_256 hash of the compressed public key
	fn account_id(&self) -> AccountId32 {
		AccountId32::from(blake2_256(&self.account_key().public().0))
	}

	fn address(&self) -> MultiAddress<<C as subxt::Config>::AccountId, ()> {
		MultiAddress::Id(<Self as subxt::tx::Signer<C>>::account_id(self))
	}

	fn sign(&self, signer_payload: &[u8]) -> MultiSignature {
		self.account_key().sign(signer_payload).into()
	}
}

pub type OpaSubstrateClient<C> = SubstrateClient<C, OpaEventCodec<C>, OpaTransaction>;
