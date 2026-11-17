//! Cancellation abstraction for cross-platform async cancellation
//!
//! This module provides a unified interface for checking cancellation
//! that works both on native platforms (using `tokio_util::sync::CancellationToken`)
//! and in WebAssembly (using `Arc<AtomicBool>`).

use std::sync::{
    atomic::{
        AtomicBool,
        Ordering,
    },
    Arc,
};

#[cfg(not(target_arch = "wasm32"))]
use tokio_util::sync::CancellationToken;

/// A cancellation handle that can be checked for cancellation
pub(crate) trait Cancellable {
    /// Check if cancellation has been requested
    fn is_cancelled(&self) -> bool;
}

/// Native cancellation using tokio's CancellationToken
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone)]
pub struct NativeCancellation {
    /// Shared Tokio cancellation state; cloning preserves the same signal.
    token: CancellationToken,
}

#[cfg(not(target_arch = "wasm32"))]
impl NativeCancellation {
    /// Wrap an existing cancellation token rather than creating a new signal.
    pub(crate) fn new(token: CancellationToken) -> Self {
        Self { token }
    }

    /// Expose the wrapped token to native callers.
    pub(crate) fn token(&self) -> &CancellationToken {
        &self.token
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Cancellable for NativeCancellation {
    /// Observe the wrapped token without consuming or resetting its signal.
    fn is_cancelled(&self) -> bool {
        self.token.is_cancelled()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl From<CancellationToken> for NativeCancellation {
    /// Preserve the supplied token's shared cancellation state.
    fn from(token: CancellationToken) -> Self {
        Self::new(token)
    }
}

/// Wasm cancellation using `Arc<AtomicBool>`
#[derive(Debug, Clone)]
pub struct WasmCancellation {
    /// Shared flag; callers request cancellation by storing `true`.
    cancelled: Arc<AtomicBool>,
}

impl WasmCancellation {
    /// Wrap an existing flag, usable on both native and wasm targets.
    pub(crate) fn new(cancelled: Arc<AtomicBool>) -> Self {
        Self { cancelled }
    }

    /// Borrow the flag used by cooperative cancellation checks.
    pub(crate) fn flag(&self) -> &Arc<AtomicBool> {
        &self.cancelled
    }
}

impl Cancellable for WasmCancellation {
    /// Read the shared flag using sequentially consistent ordering.
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

impl From<Arc<AtomicBool>> for WasmCancellation {
    /// Share the caller's flag rather than allocate a separate signal.
    fn from(cancelled: Arc<AtomicBool>) -> Self {
        Self::new(cancelled)
    }
}

/// Platform-appropriate cancellation type
#[cfg(not(target_arch = "wasm32"))]
pub(crate) type PlatformCancellation = NativeCancellation;

#[cfg(target_arch = "wasm32")]
/// Default cancellation wrapper on wasm targets.
pub(crate) type PlatformCancellation = WasmCancellation;

/// Unified cancellation enum that can hold either type
#[derive(Debug, Clone)]
pub enum Cancellation {
    /// Native Tokio token, unavailable on wasm targets.
    #[cfg(not(target_arch = "wasm32"))]
    Native(NativeCancellation),
    /// Shared atomic flag, also supported on native targets.
    Wasm(WasmCancellation),
    /// No cancellation support
    None,
}

impl Cancellation {
    /// Construct the native-token variant.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn native(token: CancellationToken) -> Self {
        Self::Native(NativeCancellation::new(token))
    }

    /// Construct the atomic-flag variant.
    pub(crate) fn wasm(cancelled: Arc<AtomicBool>) -> Self {
        Self::Wasm(WasmCancellation::new(cancelled))
    }

    /// Construct a handle whose checks never request cancellation.
    pub(crate) fn none() -> Self {
        Self::None
    }
}

impl Cancellable for Cancellation {
    /// Delegate to the selected source; `None` always returns `false`.
    fn is_cancelled(&self) -> bool {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Cancellation::Native(c) => c.is_cancelled(),
            Cancellation::Wasm(c) => c.is_cancelled(),
            Cancellation::None => false,
        }
    }
}

impl Default for Cancellation {
    /// Default parsing does not support caller-requested cancellation.
    fn default() -> Self {
        Self::None
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl From<CancellationToken> for Cancellation {
    /// Select native cancellation for a supplied Tokio token.
    fn from(token: CancellationToken) -> Self {
        Self::native(token)
    }
}

impl From<Arc<AtomicBool>> for Cancellation {
    /// Select atomic-flag cancellation on either supported platform.
    fn from(cancelled: Arc<AtomicBool>) -> Self {
        Self::wasm(cancelled)
    }
}
