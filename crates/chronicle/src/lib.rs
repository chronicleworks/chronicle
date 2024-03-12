#![cfg_attr(feature = "strict", deny(warnings))]
pub mod bootstrap;
pub mod codegen;
/// Re-export dependencies for generated code
pub use api;
pub use async_graphql;
pub use chronicle_persistence as persistence;
pub use chrono;
pub use common;
pub use serde_json;
pub use tokio;
pub use uuid;

pub use crate::bootstrap::bootstrap;
pub use codegen::{generate_chronicle_domain_schema, Builder, PrimitiveType};
