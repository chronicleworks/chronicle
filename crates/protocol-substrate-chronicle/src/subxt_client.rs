use std::{convert::Infallible, marker::PhantomData, sync::Arc};

use chronicle_signing::{
    ChronicleSigning, OwnedSecret, SecretError, BATCHER_NAMESPACE, BATCHER_PK,
};
use common::{
    identity::SignedIdentity, ledger::OperationSubmission, opa::OpaSettings,
    prov::operations::ChronicleOperation,
};

use protocol_substrate::{SubstrateClient, SubxtClientError};
use subxt::ext::{
    codec::Decode,
    scale_value::Composite,
    sp_core::{blake2_256, Pair},
};

use subxt::{
    tx::Signer,
    utils::{AccountId32, MultiAddress, MultiSignature},
};

use protocol_abstract::{LedgerEvent, LedgerEventCodec, LedgerTransaction, Span};

//This type must match pallet::Event but we cannot reference it directly
#[derive(Debug, Clone)]
pub enum ChronicleEvent {
    Committed {
        diff: common::prov::ProvModel,
        identity: SignedIdentity,
        correlation_id: [u8; 16],
    },
    Contradicted {
        contradiction: common::prov::Contradiction,
        identity: SignedIdentity,
        correlation_id: [u8; 16],
    },
}

//This type must match pallet::Event but we cannot reference it directly
pub struct ChronicleEventCodec<C>
    where
        C: subxt::Config,
{
    _p: PhantomData<C>,
}

impl ChronicleEvent {
    #[tracing::instrument(level = "trace", skip(diff, identity), fields(
        diff = tracing::field::debug(& diff), identity = tracing::field::debug(& identity), correlation_id = tracing::field::debug(& correlation_id)
    ))]
    pub fn new_committed(
        diff: common::prov::ProvModel,
        identity: SignedIdentity,
        correlation_id: [u8; 16],
    ) -> Self {
        ChronicleEvent::Committed { diff, identity, correlation_id }
    }

    pub fn new_contradicted(
        contradiction: common::prov::Contradiction,
        identity: SignedIdentity,
        correlation_id: [u8; 16],
    ) -> Self {
        ChronicleEvent::Contradicted { contradiction, identity, correlation_id }
    }
}

fn extract_event<C>(
    event: subxt::events::EventDetails<C>,
) -> Result<Option<ChronicleEvent>, SubxtClientError>
    where
        C: subxt::Config,
{
    type Applied = (common::prov::ProvModel, common::identity::SignedIdentity, [u8; 16]);
    type Contradicted = (common::prov::Contradiction, common::identity::SignedIdentity, [u8; 16]);
    match (event.pallet_name(), event.variant_name(), event.field_bytes()) {
        ("Chronicle", "Applied", mut event_bytes) => match Applied::decode(&mut event_bytes) {
            Ok((prov_model, identity, correlation_id)) =>
                Ok(Some(ChronicleEvent::new_committed(prov_model, identity, correlation_id))),
            Err(e) => {
                tracing::error!("Failed to decode ProvModel: {}", e);
                Err(e.into())
            }
        },
        ("Chronicle", "Contradicted", mut event_bytes) => {
            match Contradicted::decode(&mut event_bytes) {
                Ok((contradiction, identity, correlation_id)) =>
                    Ok(ChronicleEvent::new_contradicted(contradiction, identity, correlation_id)
                        .into()),
                Err(e) => {
                    tracing::error!("Failed to decode Contradiction: {}", e);
                    Err(e.into())
                }
            }
        }
        (_pallet, _event, _) => Ok(None),
    }
}

impl LedgerEvent for ChronicleEvent {
    fn correlation_id(&self) -> [u8; 16] {
        match self {
            Self::Committed { correlation_id, .. } => *correlation_id,
            Self::Contradicted { correlation_id, .. } => *correlation_id,
        }
    }
}

#[async_trait::async_trait]
impl<C> LedgerEventCodec for ChronicleEventCodec<C>
    where
        C: subxt::Config,
{
    type Error = SubxtClientError;
    type Sink = ChronicleEvent;
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

pub struct ChronicleTransaction {
    pub correlation_id: uuid::Uuid,
    key: subxt::ext::sp_core::ecdsa::Pair, //We need the batcher key to sign transactions
    pub identity: Arc<SignedIdentity>,
    pub operations: Arc<Vec<ChronicleOperation>>,
}

impl ChronicleTransaction {
    pub async fn new(
        signer: &ChronicleSigning,
        identity: SignedIdentity,
        operations: impl IntoIterator<Item=ChronicleOperation>,
    ) -> Result<Self, SecretError> {
        Ok(Self {
            correlation_id: uuid::Uuid::new_v4(),
            key: subxt::ext::sp_core::ecdsa::Pair::from_seed_slice(
                &signer.copy_signing_key(BATCHER_NAMESPACE, BATCHER_PK).await?.to_bytes(),
            )
                .unwrap(),
            identity: identity.into(),
            operations: Arc::new(operations.into_iter().collect::<Vec<_>>()),
        })
    }
}

// This type must match the signature of the extrinsic call
#[derive(
    scale_info::TypeInfo,
    scale_encode::EncodeAsType,
    parity_scale_codec::Encode,
    parity_scale_codec::Decode,
)]
pub struct ApplyArgs {
    pub operations: OperationSubmission,
}

#[async_trait::async_trait]
impl LedgerTransaction for ChronicleTransaction {
    type Error = Infallible;
    type Payload = ApplyArgs;

    async fn as_payload(&self) -> Result<Self::Payload, Self::Error> {
        Ok(ApplyArgs {
            operations: OperationSubmission {
                correlation_id: self.correlation_id.into_bytes(),
                identity: self.identity.clone(),
                items: self.operations.clone(),
            },
        })
    }

    fn correlation_id(&self) -> [u8; 16] {
        self.correlation_id.into_bytes()
    }
}

///Subxt signer needs to be infallible, so we need to keep a copy of key material here
impl<C> Signer<C> for ChronicleTransaction
    where
        C: subxt::Config<
            AccountId=AccountId32,
            Address=MultiAddress<AccountId32, ()>,
            Signature=MultiSignature,
        >,
{
    // The account id for an ecdsa key is the blake2_256 hash of the compressed public key
    fn account_id(&self) -> AccountId32 {
        AccountId32::from(blake2_256(&self.key.public().0))
    }

    fn address(&self) -> MultiAddress<<C as subxt::Config>::AccountId, ()> {
        MultiAddress::Id(<Self as subxt::tx::Signer<C>>::account_id(self))
    }

    fn sign(&self, signer_payload: &[u8]) -> MultiSignature {
        self.key.sign(signer_payload).into()
    }
}

#[async_trait::async_trait]
pub trait SettingsLoader {
    async fn load_settings_from_storage(&self) -> Result<Option<OpaSettings>, SubxtClientError>;
}

pub type ChronicleSubstrateClient<C> =
SubstrateClient<C, ChronicleEventCodec<C>, ChronicleTransaction>;

#[async_trait::async_trait]
impl<C> SettingsLoader for ChronicleSubstrateClient<C>
    where
        C: subxt::Config,
{
    async fn load_settings_from_storage(&self) -> Result<Option<OpaSettings>, SubxtClientError> {
        tracing::debug!("Loading OPA settings from storage.");
        let call = subxt::dynamic::runtime_api_call(
            "Chronicle",
            "get_opa_settings",
            Composite::unnamed(vec![]),
        );
        let settings: Option<OpaSettings> = self
            .client
            .runtime_api()
            .at_latest()
            .await?
            .call(call)
            .await
            .map_err(SubxtClientError::from)
            .and_then(|r| r.as_type::<Option<OpaSettings>>().map_err(SubxtClientError::from))?;

        Ok(settings)
    }
}
