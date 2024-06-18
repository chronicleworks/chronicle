use crate::{mock::*, Event};
use common::{
	ledger::OperationSubmission,
	prov::{
		operations::{ChronicleOperation, CreateNamespace},
		NamespaceId,
	},
};
use frame_support::assert_ok;
use uuid::Uuid;

#[test]
fn it_works_for_default_value() {
	new_test_ext().execute_with(|| {
		// Go past genesis block so events get deposited
		System::set_block_number(1);
		let op = OperationSubmission::new_anonymous(Uuid::from_bytes([0u8; 16]), vec![]);
		// Dispatch a signed extrinsic.
		assert_ok!(ChronicleModule::apply(RuntimeOrigin::signed(1), op.clone()));
		// Assert that the correct event was deposited
		System::assert_last_event(
			Event::Applied(
				common::prov::ProvModel::default(),
				common::identity::SignedIdentity::new_no_identity(),
				op.correlation_id,
			)
			.into(),
		);
	});
}

#[test]
fn single_operation() {
	chronicle_telemetry::telemetry(chronicle_telemetry::ConsoleLogging::Pretty);
	new_test_ext().execute_with(|| {
		// Go past genesis block so events get deposited
		System::set_block_number(1);
		let uuid = Uuid::from_u128(0u128);
		let op = ChronicleOperation::CreateNamespace(CreateNamespace {
			id: NamespaceId::from_external_id("test", uuid),
		});

		let sub = OperationSubmission::new_anonymous(Uuid::from_bytes([0u8; 16]), vec![op.clone()]);
		// Dispatch our operation
		assert_ok!(ChronicleModule::apply(RuntimeOrigin::signed(1), sub.clone(),));

		// Apply that operation to a new prov model for assertion -
		// the pallet execution should produce an identical delta
		let mut delta_model = common::prov::ProvModel::default();
		delta_model.apply(&op).unwrap();
		// Assert that the delta is correct
		System::assert_last_event(
			Event::Applied(
				delta_model,
				common::identity::SignedIdentity::new_no_identity(),
				sub.correlation_id,
			)
			.into(),
		);
	});
}
