use runtime_chronicle::{
	opaque::SessionKeys, pallet_chronicle, AccountId, AuraConfig, GrandpaConfig, ImOnlineConfig, Runtime, RuntimeGenesisConfig, SessionConfig, Signature, SudoConfig, SystemConfig, ValidatorSetConfig, WASM_BINARY
};
use sc_service::ChainType;
use sc_telemetry::{log, serde_json};
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_consensus_grandpa::AuthorityId as GrandpaId;
use sp_core::{sr25519, Pair, Public};
use sp_runtime::traits::{IdentifyAccount, Verify};
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
pub fn authority_keys_from_seed(s: &str) -> (AuraId, GrandpaId, ImOnlineId) {
	(get_from_seed::<AuraId>(s), get_from_seed::<GrandpaId>(s), get_from_seed::<ImOnlineId>(s))
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
				vec![authority_keys_from_seed("Alice"), authority_keys_from_seed("Bob")],
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
	initial_authorities: Vec<(AuraId, GrandpaId, ImOnlineId)>,
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
			initial_validators: initial_authorities.iter().map(|x| {
				sp_runtime::AccountId32::from(x.0.clone().into_inner())
			}).collect::<Vec<_>>(),
		},
		session: SessionConfig {
			keys: initial_authorities.iter().map(|x| {
				(
					sp_runtime::AccountId32::from(x.0.clone().into_inner()),
					sp_runtime::AccountId32::from(x.0.clone().into_inner()),
					session_keys(x.0.clone(), x.1.clone(), x.2.clone())
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

	log::info!("testnet configuration");
	Ok(ChainSpec::builder(
			wasm_binary,
			None
		)
		.with_name("Chronicle Mainnet")
		.with_id("chronicle")
		.with_chain_type(ChainType::Local)
		.with_genesis_config(
			serde_json::to_value(genesis(
				vec![],
				get_account_id_from_seed::<sr25519::Public>("Alice"),
				true,
			)).expect("Genesis config should be serializable")
		)
		.with_protocol_id("chronicle")
		.build())
}

