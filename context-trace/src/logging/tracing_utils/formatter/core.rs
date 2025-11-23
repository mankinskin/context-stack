//! Core formatter struct and constructor

use super::{
    config::FormatConfig,
    timer::CompactTimer,
};

/// Compact formatter that works with tracing_subscriber's FormatEvent trait
/// and uses our custom CompactTimer for timing information.
pub struct CompactFieldsFormatter {
    pub(super) timer: CompactTimer,
    pub(super) config: FormatConfig,
}

impl CompactFieldsFormatter {
    /// Create a new compact formatter with the given config
    pub fn new(config: FormatConfig) -> Self {
        Self {
            timer: CompactTimer::new(),
            config,
        }
    }
}
