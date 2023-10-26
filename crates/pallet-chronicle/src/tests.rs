use crate::{mock::*, Event};
use common::prov::{
	operations::{ChronicleOperation, CreateNamespace},
	ExternalId, NamespaceId,
};
use frame_support::assert_ok;
use uuid::Uuid;

#[test]
fn it_works_for_default_value() {
	new_test_ext().execute_with(|| {
		// Go past genesis block so events get deposited
		System::set_block_number(1);
		// Dispatch a signed extrinsic.
		assert_ok!(ChronicleModule::apply(RuntimeOrigin::signed(1), vec![]));
		// Assert that the correct event was deposited
		System::assert_last_event(Event::Applied(common::prov::ProvModel::default()).into());
	});
}

#[test]
fn single_operation() {
	chronicle_telemetry::telemetry(None, chronicle_telemetry::ConsoleLogging::Pretty);
	new_test_ext().execute_with(|| {
		// Go past genesis block so events get deposited
		System::set_block_number(1);
		let uuid = Uuid::new_v4();
		let op = ChronicleOperation::CreateNamespace(CreateNamespace {
			id: NamespaceId::from_external_id("test", uuid),
			external_id: ExternalId::from("test"),
			uuid: uuid.into(),
		});
		// Dispatch our operation
		assert_ok!(ChronicleModule::apply(RuntimeOrigin::signed(1), vec![op.clone()]));

		// Apply that operation to a new prov model for assertion -
		// the pallet execution should produce an identical delta
		let mut delta_model = common::prov::ProvModel::default();
		delta_model.apply(&op).unwrap();
		// Assert that the delta is correct
		System::assert_last_event(Event::Applied(delta_model).into());
	});
}
