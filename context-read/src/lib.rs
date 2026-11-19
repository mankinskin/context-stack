#![deny(clippy::disallowed_methods)]

mod read;
pub use read::read;

/// Row-model reader, fenced off from the model-agnostic root.
#[cfg(feature = "legacy")]
pub mod legacy;
