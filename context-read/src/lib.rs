#![deny(clippy::disallowed_methods)]

/// Row-model reader, fenced off from the model-agnostic root.
#[cfg(feature = "legacy")]
pub mod legacy;
