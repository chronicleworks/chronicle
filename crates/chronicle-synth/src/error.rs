use thiserror::Error;

#[derive(Error, Debug)]
pub enum ChronicleSynthError {
    #[error("Chronicle domain parsing error: {0}")]
    ModelError(#[from] chronicle::codegen::model::ModelError),

    #[error("Invalid JSON: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("I/O error: {0}")]
    IO(#[from] std::io::Error),

    #[error("YAML parsing error: {0}")]
    YamlError(#[from] serde_yaml::Error),
}
