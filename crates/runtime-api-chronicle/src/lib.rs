#![cfg_attr(not(feature = "std"), no_std)]
pub type Hash = sp_core::H256;

pub mod chronicle_core {
	pub use common::*;
}

// Here we declare the runtime API. It is implemented it the `impl` block in
// runtime file (the `runtime/src/lib.rs`)
sp_api::decl_runtime_apis! {
	pub trait ChronicleApi {
		fn placeholder() -> u32;
	}
}
