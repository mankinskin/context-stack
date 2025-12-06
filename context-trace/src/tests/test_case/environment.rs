//! Graph environment trait for reusable test fixtures

use crate::HypergraphRef;
use std::sync::{RwLockReadGuard, RwLockWriteGuard};

/// A graph environment provides pre-initialized graph state for testing.
///
/// Environments encapsulate:
/// - Graph initialization with atoms and patterns
/// - All relevant tokens and pattern IDs
/// - Metadata for organization and discovery
///
/// Environments should be:
/// - **Immutable**: Each test gets a fresh instance
/// - **Self-contained**: All needed tokens/IDs included
/// - **Well-documented**: Clear description of graph structure
///
/// # Example
///
/// ```rust,ignore
/// pub struct EnvSimplePattern {
///     pub graph: HypergraphRef,
///     pub a: Token,
///     pub b: Token,
///     pub ab: Token,
///     pub ab_id: PatternId,
/// }
///
/// impl TestEnv for EnvSimplePattern {
///     fn initialize() -> Self {
///         let mut graph = HypergraphRef::default();
///         // ... initialize graph
///         Self { graph, a, b, ab, ab_id }
///     }
///
///     fn get<'a>() -> RwLockReadGuard<'a, Self> {
///         CONTEXT.with(|cell| {
///             cell.get_or_init(|| Arc::new(RwLock::new(Self::initialize())))
///                 .read()
///                 .unwrap()
///         })
///     }
///
///     fn get_mut<'a>() -> RwLockWriteGuard<'a, Self> {
///         CONTEXT.with(|cell| {
///             cell.get_or_init(|| Arc::new(RwLock::new(Self::initialize())))
///                 .write()
///                 .unwrap()
///         })
///     }
///
///     fn graph(&self) -> &HypergraphRef {
///         &self.graph
///     }
/// }
/// ```
pub trait TestEnv: Sized {
    /// Initialize a fresh instance of the environment.
    ///
    /// Called for each test to ensure isolation. Should be deterministic.
    fn initialize() -> Self;

    /// Get a read lock to the cached environment instance.
    ///
    /// The environment is cached per-thread to avoid repeated initialization.
    fn get<'a>() -> RwLockReadGuard<'a, Self>;

    /// Get a write lock to the cached environment instance.
    ///
    /// Allows tests to mutate the environment if needed.
    fn get_mut<'a>() -> RwLockWriteGuard<'a, Self>;

    /// Get a reference to the graph.
    fn graph(&self) -> &HypergraphRef;
}
