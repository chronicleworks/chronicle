use std::path::Path;

use runtime_chronicle::{
	opaque::SessionKeys, pallet_chronicle, AccountId, AuraConfig, GrandpaConfig, ImOnlineConfig, Runtime, RuntimeGenesisConfig, SessionConfig, Signature, SudoConfig, SystemConfig, ValidatorSetConfig, WASM_BINARY
};
use sc_keystore::LocalKeystore;
use sc_service::ChainType;
use sc_telemetry::{log, serde_json};
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_consensus_grandpa::AuthorityId as GrandpaId;
use sp_core::{sr25519, Pair, Public};
use sp_runtime::{traits::{IdentifyAccount, Verify}, KeyTypeId};
use pallet_im_online::sr25519::AuthorityId as ImOnlineId;
use serde_json::to_value;

// The URL for the telemetry server.
// const STAGING_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";

/// Specialized `ChainSpec`. This is a specialization of the general Substrate ChainSpec type.
pub type ChainSpec = sc_service::GenericChainSpec<RuntimeGenesisConfig>;

/// Generate a crypto pair from seed.
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
	TPublic::Pair::from_string(&format!("//{}", seed), None)
		.expect("static values are valid; qed")
		.public()
}

type AccountPublic = <Signature as Verify>::Signer;

/// Generate an account ID from seed.
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
	AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
	AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

/// Generate an Aura authority key.
pub fn authority_keys_from_seed(s: &str) -> (AccountId, AuraId, GrandpaId, ImOnlineId) {
	(get_account_id_from_seed::<sr25519::Public>(s), get_from_seed::<AuraId>(s), get_from_seed::<GrandpaId>(s), get_from_seed::<ImOnlineId>(s))
}

use sp_keystore::Keystore;

pub fn authority_keys_from_keystore(p: &std::path::Path) -> (AccountId, AuraId, GrandpaId, ImOnlineId) {
    let keystore = LocalKeystore::open(p, None).expect("Local keystore should open");

    let sudo_key = Keystore::sr25519_public_keys(&keystore, KeyTypeId(*b"acco"))
        .into_iter()
        .next()
        .expect("Account key should be present in keystore");

    let aura_key = Keystore::sr25519_public_keys(&keystore, KeyTypeId(*b"aura"))
        .into_iter()
        .next()
        .expect("Aura key should be present in keystore");

    let grandpa_key = Keystore::ed25519_public_keys(&keystore, KeyTypeId(*b"gran"))
        .into_iter()
        .next()
        .expect("Grandpa key should be present in keystore");

    let im_online_key = Keystore::sr25519_public_keys(&keystore, KeyTypeId(*b"onli"))
        .into_iter()
        .next()
        .expect("ImOnline key should be present in keystore");

    (
        AccountPublic::from(sudo_key).into_account(),
        aura_key.into(),
        grandpa_key.into(),
        im_online_key.into(),
    )
}

pub fn development_config() -> Result<ChainSpec, String> {
	let wasm_binary = WASM_BINARY.ok_or_else(|| "Development wasm not available".to_string())?;
	log::info!("development configuration");

	Ok(ChainSpec::builder(wasm_binary, None)
		.with_name("Development")
		.with_id("dev")
		.with_chain_type(ChainType::Development)
		.with_genesis_config(
			serde_json::to_value(genesis(
				vec![authority_keys_from_seed("Alice")],
				get_account_id_from_seed::<sr25519::Public>("Alice"),
				true,
			)).expect("Genesis config should be serializable")
		)
		.with_protocol_id("chronicle")
		.build())
}

pub fn local_testnet_config() -> Result<ChainSpec, String> {
	let wasm_binary = WASM_BINARY.ok_or_else(|| "Development wasm not available".to_string())?;

	log::info!("testnet configuration");
	Ok(ChainSpec::builder(
			wasm_binary,
			None
		)
		.with_name("Local Testnet")
		.with_id("local_testnet")
		.with_chain_type(ChainType::Local)
		.with_genesis_config(
			to_value(genesis(
				vec![authority_keys_from_seed("Alice")],
				get_account_id_from_seed::<sr25519::Public>("Alice"),
				true,
			)).expect("Genesis config should be serializable")
		)
		.with_protocol_id("chronicle")
		.build())
}

fn session_keys(aura: AuraId, grandpa: GrandpaId, im_online: ImOnlineId) -> SessionKeys {
	SessionKeys { aura, grandpa, im_online }
}

/// Configure initial storage state for FRAME modules.
fn genesis(
	initial_authorities: Vec<(AccountId, AuraId, GrandpaId, ImOnlineId)>,
	root_key: AccountId,
	_enable_println: bool,
) -> RuntimeGenesisConfig {
	RuntimeGenesisConfig {
		system: SystemConfig {
			..Default::default()
		},
		sudo: SudoConfig {
			// Assign network admin rights.
			key: Some(root_key),
		},
		chronicle: pallet_chronicle::GenesisConfig::<Runtime> { ..Default::default() },
		validator_set: ValidatorSetConfig {
			initial_validators: initial_authorities.iter().map(|x| x.0.clone()).collect::<Vec<_>>(),
		},
		session: SessionConfig {
			keys: initial_authorities.iter().map(|x| {
				(
					x.0.clone(),
					x.0.clone(),
					session_keys(x.1.clone(), x.2.clone(), x.3.clone())
				)
			}).collect::<Vec<_>>(),
		},
		aura: AuraConfig {
			authorities: vec![],
		},
		grandpa: GrandpaConfig {
			..Default::default()
		},
		im_online: ImOnlineConfig { keys: vec![] },
	}
}



pub fn chronicle_config() -> Result<ChainSpec, String> {
	let wasm_binary = WASM_BINARY.ok_or_else(|| "Development wasm not available".to_string())?;

	let (root_key, aura_key, grandpa_key, im_online_key) = authority_keys_from_keystore(Path::new("/keystore/"));

	log::info!("Private network configuration");
	Ok(ChainSpec::builder(
			wasm_binary,
			None
		)
		.with_name("Chronicle")
		.with_id("chronicle")
		.with_chain_type(ChainType::Live)
		.with_genesis_config(
			to_value(genesis(
				vec![(root_key.clone(), aura_key, grandpa_key, im_online_key)],
				root_key,
				true,
			)).expect("Genesis config should be serializable")
		)
		.with_protocol_id("chronicle")
		.build())
}

