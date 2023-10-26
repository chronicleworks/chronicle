use sawtooth_sdk::{
	messages::processor::TpProcessRequest,
	processor::handler::{ApplyError, TransactionContext},
};
pub trait TP {
	fn apply(
		&self,
		request: &TpProcessRequest,
		context: &mut dyn TransactionContext,
	) -> Result<(), ApplyError>;
}
