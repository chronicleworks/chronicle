use crate::{mock::*, Error, Event};
use frame_support::{assert_noop, assert_ok};

#[test]
fn it_works_for_default_value() {
    new_test_ext().execute_with(|| {
        // Go past genesis block so events get deposited
        System::set_block_number(1);
        // Dispatch a signed extrinsic.
        assert_ok!(TemplateModule::apply(RuntimeOrigin::signed(1), vec![]));
        // Assert that the correct event was deposited
        System::assert_last_event(Event::Applied(common::prov::ProvModel::default()).into());
    });
}
