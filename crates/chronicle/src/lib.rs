#![cfg_attr(feature = "strict", deny(warnings))]

pub use async_graphql;
pub use chrono;
pub use serde_json;
pub use tokio;
pub use uuid;

/// Re-export dependencies for generated code
pub use api;
pub use chronicle_persistence as persistence;
pub use codegen::{Builder, generate_chronicle_domain_schema, PrimitiveType};
pub use common;

pub use crate::bootstrap::bootstrap;

pub mod bootstrap;
pub mod codegen;

