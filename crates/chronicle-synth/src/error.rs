use thiserror::Error;

#[derive(Error, Debug)]
pub enum ChronicleSynthError {
	#[error("Chronicle domain parsing error: {0}")]
	ModelError(
		#[from]
		#[source]
		common::domain::ModelError,
	),

	#[error("Invalid JSON: {0}")]
	JsonError(
		#[from]
		#[source]
		serde_json::Error,
	),

	#[error("I/O error: {0}")]
	IO(
		#[from]
		#[source]
		std::io::Error,
	),

	#[error("YAML parsing error: {0}")]
	YamlError(
		#[from]
		#[source]
		serde_yaml::Error,
	),
}
