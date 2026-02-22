//! Test environments for validation/edge case testing
//!
//! These environments provide pre-configured graph states for testing
//! input validation and error handling in context-insert.

use context_trace::{
    PatternId,
    graph::{
        Hypergraph,
        HypergraphRef,
        vertex::{
            atom::Atom,
            token::Token,
        },
    },
    tests::test_case::TestEnv,
};
use std::sync::{
    Arc,
    OnceLock,
    RwLock,
    RwLockReadGuard,
    RwLockWriteGuard,
};

// ============================================================================
// EnvAbcd - Simple 4-atom pattern for testing InitInterval validation
// ============================================================================

/// Test environment with pattern "abcd" for InitInterval edge cases.
///
/// Graph structure:
/// - atoms: a, b, c, d
/// - patterns: abcd = [a, b, c, d]
///
/// Used for testing:
/// - Invalid end_bound (0) rejection
/// - Cache validation
#[derive(Debug)]
#[allow(dead_code)]
pub(crate) struct EnvAbcd {
    pub(crate) graph: HypergraphRef,
    pub(crate) a: Token,
    pub(crate) b: Token,
    pub(crate) c: Token,
    pub(crate) d: Token,
    pub(crate) abcd: Token,
    pub(crate) abcd_id: PatternId,
}

impl TestEnv for EnvAbcd {
    fn initialize() -> Self {
        let graph = Hypergraph::default();
        let [a, b, c, d] = graph.insert_atoms([
            Atom::Element('a'),
            Atom::Element('b'),
            Atom::Element('c'),
            Atom::Element('d'),
        ])[..] else {
            panic!()
        };

        let (abcd, abcd_id) = graph.insert_pattern_with_id(vec![a, b, c, d]);

        #[cfg(any(test, feature = "test-api"))]
        context_trace::graph::test_graph::register_test_graph(&graph);

        graph.emit_graph_snapshot();

        Self {
            graph: HypergraphRef::from(graph),
            a,
            b,
            c,
            d,
            abcd,
            abcd_id: abcd_id.unwrap(),
        }
    }

    fn get<'a>() -> RwLockReadGuard<'a, Self> {
        get_env_abcd().read().unwrap()
    }
    fn get_mut<'a>() -> RwLockWriteGuard<'a, Self> {
        get_env_abcd().write().unwrap()
    }

    fn graph(&self) -> &HypergraphRef {
        &self.graph
    }
}

fn get_env_abcd() -> &'static Arc<RwLock<EnvAbcd>> {
    ENV_ABCD.with(|cell| unsafe {
        let ptr =
            cell.get_or_init(|| Arc::new(RwLock::new(EnvAbcd::initialize())));
        &*(ptr as *const Arc<RwLock<EnvAbcd>>)
    })
}

thread_local! {
    static ENV_ABCD: OnceLock<Arc<RwLock<EnvAbcd>>> = const { OnceLock::new() };
}

// ============================================================================
// EnvAb - Minimal 2-atom pattern for empty pattern testing
// ============================================================================

/// Test environment with pattern "ab" for empty pattern rejection tests.
///
/// Graph structure:
/// - atoms: a, b
/// - patterns: ab = [a, b]
///
/// Used for testing:
/// - Empty pattern rejection in search
/// - Empty pattern rejection in insert
#[derive(Debug)]
#[allow(dead_code)]
pub(crate) struct EnvAb {
    pub(crate) graph: HypergraphRef,
    pub(crate) a: Token,
    pub(crate) b: Token,
    pub(crate) ab: Token,
    pub(crate) ab_id: PatternId,
}

impl TestEnv for EnvAb {
    fn initialize() -> Self {
        let graph = Hypergraph::default();
        let [a, b] =
            graph.insert_atoms([Atom::Element('a'), Atom::Element('b')])[..]
        else {
            panic!()
        };

        let (ab, ab_id) = graph.insert_pattern_with_id(vec![a, b]);

        #[cfg(any(test, feature = "test-api"))]
        context_trace::graph::test_graph::register_test_graph(&graph);

        graph.emit_graph_snapshot();

        Self {
            graph: HypergraphRef::from(graph),
            a,
            b,
            ab,
            ab_id: ab_id.unwrap(),
        }
    }

    fn get<'a>() -> RwLockReadGuard<'a, Self> {
        get_env_ab().read().unwrap()
    }
    fn get_mut<'a>() -> RwLockWriteGuard<'a, Self> {
        get_env_ab().write().unwrap()
    }

    fn graph(&self) -> &HypergraphRef {
        &self.graph
    }
}

fn get_env_ab() -> &'static Arc<RwLock<EnvAb>> {
    ENV_AB.with(|cell| unsafe {
        let ptr =
            cell.get_or_init(|| Arc::new(RwLock::new(EnvAb::initialize())));
        &*(ptr as *const Arc<RwLock<EnvAb>>)
    })
}

thread_local! {
    static ENV_AB: OnceLock<Arc<RwLock<EnvAb>>> = const { OnceLock::new() };
}

// ============================================================================
// EnvAbc - 3-atom pattern for mismatch testing
// ============================================================================

/// Test environment with pattern "ab" and extra atom c for mismatch tests.
///
/// Graph structure:
/// - atoms: a, b, c
/// - patterns: ab = [a, b]
///
/// Used for testing:
/// - Single token mismatch at start
/// - Query patterns that don't match graph structure
#[derive(Debug)]
#[allow(dead_code)]
pub(crate) struct EnvAbc {
    pub(crate) graph: HypergraphRef,
    pub(crate) a: Token,
    pub(crate) b: Token,
    pub(crate) c: Token,
    pub(crate) ab: Token,
    pub(crate) ab_id: PatternId,
}

impl TestEnv for EnvAbc {
    fn initialize() -> Self {
        let graph = Hypergraph::default();
        let [a, b, c] = graph.insert_atoms([
            Atom::Element('a'),
            Atom::Element('b'),
            Atom::Element('c'),
        ])[..] else {
            panic!()
        };

        let (ab, ab_id) = graph.insert_pattern_with_id(vec![a, b]);

        #[cfg(any(test, feature = "test-api"))]
        context_trace::graph::test_graph::register_test_graph(&graph);

        graph.emit_graph_snapshot();

        Self {
            graph: HypergraphRef::from(graph),
            a,
            b,
            c,
            ab,
            ab_id: ab_id.unwrap(),
        }
    }

    fn get<'a>() -> RwLockReadGuard<'a, Self> {
        get_env_abc().read().unwrap()
    }
    fn get_mut<'a>() -> RwLockWriteGuard<'a, Self> {
        get_env_abc().write().unwrap()
    }

    fn graph(&self) -> &HypergraphRef {
        &self.graph
    }
}

fn get_env_abc() -> &'static Arc<RwLock<EnvAbc>> {
    ENV_ABC.with(|cell| unsafe {
        let ptr =
            cell.get_or_init(|| Arc::new(RwLock::new(EnvAbc::initialize())));
        &*(ptr as *const Arc<RwLock<EnvAbc>>)
    })
}

thread_local! {
    static ENV_ABC: OnceLock<Arc<RwLock<EnvAbc>>> = const { OnceLock::new() };
}
