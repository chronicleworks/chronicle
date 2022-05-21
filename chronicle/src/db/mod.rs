#[cfg(feature = "pg-embed")]
mod embedded;
#[cfg(feature = "pg-embed")]
pub use embedded::*;
