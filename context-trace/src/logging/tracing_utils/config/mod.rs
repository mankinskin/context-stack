//! Configuration for test tracing

mod builder;
mod loader;
mod types;

pub use loader::TracingConfig;
pub use types::{
    FormatConfig,
    PanicConfig,
    SpanCloseConfig,
    SpanEnterConfig,
    TraitContextConfig,
    WhitespaceConfig,
};
