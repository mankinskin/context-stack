//! Configuration options for the ngrams crate

use std::sync::atomic::{
    AtomicBool,
    Ordering,
};

/// Process-wide cache flag for native image reads and optional image writes.
///
/// Disabled by default; changing it does not clear already stored images.
static CACHE_ENABLED: AtomicBool = AtomicBool::new(false);

/// Process-wide switch for optional labelling-image persistence.
pub struct CacheConfig;

impl CacheConfig {
    /// Whether optional cache access is enabled.
    pub fn is_enabled() -> bool {
        CACHE_ENABLED.load(Ordering::Relaxed)
    }

    /// Enable native image lookup and optional platform-specific image writes.
    pub fn enable() {
        CACHE_ENABLED.store(true, Ordering::Relaxed);
    }

    /// Disable optional image reads/writes without removing stored data.
    pub fn disable() {
        CACHE_ENABLED.store(false, Ordering::Relaxed);
    }

    /// Set the process-wide cache switch without synchronizing an active parse.
    pub fn set_enabled(enabled: bool) {
        CACHE_ENABLED.store(enabled, Ordering::Relaxed);
    }
}
