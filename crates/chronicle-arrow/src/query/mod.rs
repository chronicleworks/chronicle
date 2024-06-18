use std::sync::Arc;

use arrow_array::{Array, ListArray, StringArray};
use arrow_buffer::{Buffer, ToByteSlice};
use arrow_data::ArrayData;
use arrow_schema::DataType;

pub use activity::*;
pub use agent::*;
pub use entity::*;

use crate::ChronicleArrowError;

mod activity;
mod agent;
mod entity;

// For simple id only relations, we can just reuse this mapping
fn vec_vec_string_to_list_array(
	vec_vec_string: Vec<Vec<String>>,
) -> Result<ListArray, ChronicleArrowError> {
	let offsets: Vec<i32> = std::iter::once(0)
		.chain(vec_vec_string.iter().map(|v| v.len() as i32))
		.scan(0, |state, len| {
			*state += len;
			Some(*state)
		})
		.collect();
	let values: Vec<String> = vec_vec_string.into_iter().flatten().collect();

	let values_array = Arc::new(StringArray::from(values)) as Arc<dyn arrow_array::Array>;
	// Create an OffsetBuffer from the offsets
	let offsets_buffer = Buffer::from(offsets.to_byte_slice());
	let data_type = DataType::new_list(DataType::Utf8, false);
	let list_array = ListArray::from(
		ArrayData::builder(data_type)
			.add_child_data(values_array.to_data())
			.len(offsets.len() - 1)
			.null_count(0)
			.add_buffer(offsets_buffer)
			.build()?,
	);

	Ok(list_array)
}
