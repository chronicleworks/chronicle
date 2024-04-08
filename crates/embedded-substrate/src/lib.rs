use node_chronicle::{cli::Cli, service};
use protocol_substrate::SubxtClientError;
use protocol_substrate_chronicle::ChronicleSubstrateClient;
use sc_cli::{print_node_infos, CliConfiguration, Signals, SubstrateCli};
use subxt::{
	config::ExtrinsicParams,
	ext::futures::{pin_mut, FutureExt},
	utils::{AccountId32, MultiAddress, MultiSignature},
};
use tempfile::TempDir;
use thiserror::Error;
use tokio::{
	select,
	sync::oneshot::{channel, Sender},
};

use lazy_static::lazy_static;
use std::{
	collections::BTreeMap,
	sync::{Arc, Mutex},
	time::Duration,
};
use tracing::info;

#[derive(Debug, Error)]
pub enum Error {
	#[error("Substrate invocation error: {source}")]
	Cli { source: anyhow::Error },
	#[error("No free ports")]
	NoFreePorts,
}

// Substrate initialization is costly and includes log configuration, so we need to keep and reuse
// instances in most circumnstances
lazy_static! {
	static ref SUBSTRATE_INSTANCES: Mutex<BTreeMap<u16, Arc<EmbeddedSubstrate>>> =
		Mutex::new(BTreeMap::new());
}

pub struct EmbeddedSubstrate {
	shutdown: Option<Sender<()>>,
	_state: TempDir,
	rpc_port: u16,
}

impl EmbeddedSubstrate {
	pub async fn connect_chronicle<C>(
		&self,
	) -> Result<ChronicleSubstrateClient<C>, SubxtClientError>
	where
		C: subxt::Config<
			Hash = subxt::utils::H256,
			Address = MultiAddress<AccountId32, ()>,
			AccountId = AccountId32,
			Signature = MultiSignature,
		>,
		<C::ExtrinsicParams as ExtrinsicParams<C>>::OtherParams: Default,
	{
		ChronicleSubstrateClient::<C>::connect(format!("ws://127.0.0.1:{}", self.rpc_port)).await
	}

	pub fn port(&self) -> u16 {
		self.rpc_port
	}
}

impl Drop for EmbeddedSubstrate {
	fn drop(&mut self) {
		if let Some(shutdown) = self.shutdown.take() {
			if let Err(e) = shutdown.send(()) {
				tracing::error!("Failed to send shutdown signal: {:?}", e);
			}
		} else {
			tracing::warn!("Shutdown signal was already taken");
		}
	}
}

pub async fn shared_dev_node_rpc_on_arbitrary_port() -> Result<Arc<EmbeddedSubstrate>, Error> {
	shared_dev_node_rpc_on_port(
		portpicker::pick_unused_port().ok_or_else(|| Error::NoFreePorts)?,
		false,
	)
	.await
}
// Utilize the CLI run command to bring up a substrate-chronicle dev mode node with a new runtime
// Utilize the CLI run command to bring up a substrate-chronicle dev mode node with a new runtime
// thread. Execute node until receipt of a drop channel message or signal
pub async fn shared_dev_node_rpc_on_port(
	port: u16,
	configure_logging: bool,
) -> Result<Arc<EmbeddedSubstrate>, Error> {
	let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
	let handle = rt.handle().clone();

	if let Some(substrate_instance) = SUBSTRATE_INSTANCES.lock().unwrap().get(&port) {
		return Ok(substrate_instance.clone());
	}

	let (live_tx, live_rx) = channel::<()>();
	let (tx, rx) = channel();
	let tmp_dir = tempfile::tempdir().unwrap();
	let tmp_path = format!("{}", tmp_dir.path().to_string_lossy());

	std::thread::spawn(move || {
		let cli = Cli::from_iter([
			"--chain dev",
			"--force-authoring",
			"--alice",
			&*format!("--rpc-port={}", port),
			"--rpc-cors=all",
			&*format!("-d{}", tmp_path),
		]);

		let signals = handle
			.block_on(async { Signals::capture() })
			.map_err(|e| tracing::error!("{}", e))
			.unwrap();

		let config = cli
			.create_configuration(&cli.run, handle.clone())
			.map_err(|e| tracing::error!("{}", e))
			.unwrap();

		print_node_infos::<Cli>(&config);

		if configure_logging {
			cli.run
				.init(
					&"https://chronicle.works".to_owned(),
					&"2.0.dev".to_owned(),
					|_, _| {},
					&config,
				)
				.unwrap();
		}

		let mut task_manager = handle
			.block_on(async move { service::new_full(config).map_err(sc_cli::Error::Service) })
			.map_err(|e| tracing::error!("{}", e))
			.unwrap();

		live_tx.send(()).unwrap();

		let task_manager = handle.block_on(async move {
			let signal_exit = signals.future().fuse();
			let task_exit = task_manager.future().fuse();
			let drop_exit = async move {
				let _ = rx.await;
				tracing::info!("Shutdown message");
			};

			pin_mut!(signal_exit, drop_exit);

			select! {
				_ = signal_exit => {},
				_ = drop_exit => {},
				_ = task_exit => {},
			}

			task_manager
		});

		let task_registry = task_manager.into_task_registry();
		let shutdown_timeout = Duration::from_secs(60);
		rt.shutdown_timeout(shutdown_timeout);

		let running_tasks = task_registry.running_tasks();

		if !running_tasks.is_empty() {
			tracing::error!("Detected running(potentially stalled) tasks on shutdown:");
			running_tasks.iter().for_each(|(task, count)| {
				let instances_desc =
					if *count > 1 { format!("with {} instances ", count) } else { "".to_string() };

				if task.is_default_group() {
					tracing::error!(
						"Task \"{}\" was still running {}after waiting {} seconds to finish.",
						task.name,
						instances_desc,
						60
					);
				} else {
					tracing::error!(
						"Task \"{}\" (Group: {}) was still running {}after waiting {} seconds to finish.",
						task.name,
						task.group,
						instances_desc,
						60
					);
				}
			});
		}

		info!("Shut down embedded substrate instance on port {}", port);
	});

	tracing::info!("Await substrate boot");
	let _ = live_rx.await;
	tracing::info!("Substrate booted");

	let instance =
		Arc::new(EmbeddedSubstrate { shutdown: tx.into(), rpc_port: port, _state: tmp_dir });

	SUBSTRATE_INSTANCES.lock().unwrap().insert(port, instance.clone());

	Ok(instance)
}

pub fn remove_shared_substrate_by_port(port: u16) {
	let mut instances = SUBSTRATE_INSTANCES.lock().unwrap();
	if let Some(_instance) = instances.get(&port) {
		instances.remove(&port);
	} else {
		tracing::warn!("No running substrate instance found on port {}", port);
	}
}

pub fn remove_shared_substrate(substrate: &EmbeddedSubstrate) {
	remove_shared_substrate_by_port(substrate.port())
}

#[cfg(test)]
pub mod test_runtime {
	use chronicle_signing::{
		chronicle_secret_names, ChronicleSecretsOptions, ChronicleSigning, BATCHER_NAMESPACE,
		CHRONICLE_NAMESPACE,
	};

	use protocol_abstract::{LedgerReader, LedgerWriter};
	use protocol_substrate_chronicle::{
		common::{
			attributes::Attributes,
			identity::SignedIdentity,
			prov::{
				operations::{AgentExists, ChronicleOperation, CreateNamespace, SetAttributes},
				AgentId, DomaintypeId, ExternalId, NamespaceId,
			},
		},
		ChronicleEvent, ChronicleTransaction,
	};
	use subxt::{
		ext::{
			futures::StreamExt,
			sp_core::{Pair, Public},
		},
		PolkadotConfig,
	};
	use uuid::Uuid;

	fn get_from_seed<TPublic: Public>(seed: &str) -> [u8; 32] {
		let k = TPublic::Pair::from_string(&format!("//{}", seed), None)
			.expect("static values are valid; qed");
		let mut buf = [0; 32];
		buf.copy_from_slice(&k.to_raw_vec());

		buf
	}

	#[tokio::test]
	pub async fn connect() {
		//chronicle_telemetry::telemetry(None, chronicle_telemetry::ConsoleLogging::Pretty);
		let handle = crate::shared_dev_node_rpc_on_port(2003, true).await.unwrap();

		let client = handle.connect_chronicle::<PolkadotConfig>().await.unwrap();

		let mut events =
			client.state_updates(protocol_abstract::FromBlock::Head, None).await.unwrap();

		let signing = ChronicleSigning::new(
			chronicle_secret_names(),
			vec![
				(
					CHRONICLE_NAMESPACE.to_string(),
					ChronicleSecretsOptions::seeded(
						vec![(
							"chronicle-pk".to_string(),
							get_from_seed::<subxt::ext::sp_core::ecdsa::Public>("Chronicle"),
						)]
						.into_iter()
						.collect(),
					),
				),
				(
					BATCHER_NAMESPACE.to_string(),
					ChronicleSecretsOptions::seeded(
						vec![(
							"batcher-pk".to_string(),
							get_from_seed::<subxt::ext::sp_core::ecdsa::Public>("Chronicle"),
						)]
						.into_iter()
						.collect(),
					),
				),
			],
		)
		.await
		.unwrap();

		let (submit, id) = client
			.pre_submit(
				ChronicleTransaction::new(
					&signing,
					SignedIdentity::new_no_identity(),
					vec![
						ChronicleOperation::CreateNamespace(CreateNamespace::new(
							NamespaceId::from_external_id(
								&ExternalId::from("test"),
								Uuid::default(),
							),
						)),
						ChronicleOperation::AgentExists(AgentExists::new(
							NamespaceId::from_external_id(
								&ExternalId::from("test"),
								Uuid::default(),
							),
							AgentId::from_external_id("test"),
						)),
						ChronicleOperation::SetAttributes(SetAttributes::agent(
							NamespaceId::from_external_id(
								&ExternalId::from("test"),
								Uuid::default(),
							),
							AgentId::from_external_id("test"),
							Attributes::type_only(Some(DomaintypeId::from_external_id("test"))),
						)),
					],
				)
				.await
				.unwrap(),
			)
			.await
			.unwrap();

		let _res = client
			.do_submit(protocol_abstract::WriteConsistency::Strong, submit)
			.await
			.unwrap();

		let (ev, _id, _block, _pos, _) = events.next().await.unwrap();

		match ev {
			ChronicleEvent::Committed { diff, .. } => {
				tracing::info!("{:?}", diff)
			},
			ChronicleEvent::Contradicted { .. } => panic!("Contradicted"),
		}
	}
}
