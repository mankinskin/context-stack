//! Test environment for expanded overlap scenarios
//!
//! Tests cases where insert query starts at a postfix of an existing token.
//! For example, graph has "abc" and insert starts with "bcd" (bc is postfix of abc).

use context_trace::{
    graph::{
        vertex::{
            atom::Atom,
            token::Token,
        },
        Hypergraph,
        HypergraphRef,
    },
    tests::test_case::TestEnv,
    PatternId,
};
use std::sync::{
    Arc,
    OnceLock,
    RwLock,
    RwLockReadGuard,
    RwLockWriteGuard,
};

/// Environment with "abc" token for testing postfix overlaps
/// 
/// Graph structure:
/// - Atoms: a, b, c, d, e
/// - Patterns: ab=[a,b], abc=[ab,c]
/// 
/// Test scenario: Insert [b, c, d] where "bc" is postfix of "abc"
#[derive(Debug)]
pub struct EnvExpandedOverlap {
    pub graph: HypergraphRef,
    pub a: Token,
    pub b: Token,
    pub c: Token,
    pub d: Token,
    pub e: Token,
    pub ab: Token,
    pub ab_id: PatternId,
    pub abc: Token,
    pub abc_id: PatternId,
}

impl TestEnv for EnvExpandedOverlap {
    fn initialize() -> Self {
        let graph = Hypergraph::default();
        let [a, b, c, d, e] = graph.insert_atoms([
            Atom::Element('a'),
            Atom::Element('b'),
            Atom::Element('c'),
            Atom::Element('d'),
            Atom::Element('e'),
        ])[..] else {
            panic!()
        };

        // Build nested pattern: abc = [ab, c] = [[a, b], c]
        let (ab, ab_id) = graph.insert_pattern_with_id(vec![a, b]);
        let (abc, abc_id) = graph.insert_pattern_with_id(vec![ab, c]);

        #[cfg(any(test, feature = "test-api"))]
        context_trace::graph::test_graph::register_test_graph(&graph);

        graph.emit_graph_snapshot();

        Self {
            graph: HypergraphRef::from(graph),
            a,
            b,
            c,
            d,
            e,
            ab,
            ab_id: ab_id.unwrap(),
            abc,
            abc_id: abc_id.unwrap(),
        }
    }

    fn get<'a>() -> RwLockReadGuard<'a, Self> {
        get_context().read().unwrap()
    }
    
    fn get_mut<'a>() -> RwLockWriteGuard<'a, Self> {
        get_context().write().unwrap()
    }

    fn graph(&self) -> &HypergraphRef {
        &self.graph
    }
}

fn get_context() -> &'static Arc<RwLock<EnvExpandedOverlap>> {
    CTX_EXPANDED_OVERLAP.with(|cell| unsafe {
        let ptr = cell.get_or_init(|| {
            Arc::new(RwLock::new(EnvExpandedOverlap::initialize()))
        });
        &*(ptr as *const Arc<RwLock<EnvExpandedOverlap>>)
    })
}

thread_local! {
    static CTX_EXPANDED_OVERLAP: OnceLock<Arc<RwLock<EnvExpandedOverlap>>> = const { OnceLock::new() };
}

/// Environment with overlapping tokens for testing complex scenarios
/// 
/// Graph structure:
/// - Atoms: a, b, c, d
/// - Patterns: ab=[a,b], bc=[b,c], cd=[c,d]
/// - Pattern: abcd=[ab,cd]
/// 
/// Test scenario: Insert starting at various postfixes to test overlap detection
#[derive(Debug)]
pub struct EnvMultiOverlap {
    pub graph: HypergraphRef,
    pub a: Token,
    pub b: Token,
    pub c: Token,
    pub d: Token,
    pub ab: Token,
    pub ab_id: PatternId,
    pub bc: Token,
    pub bc_id: PatternId,
    pub cd: Token,
    pub cd_id: PatternId,
    pub abcd: Token,
    pub abcd_id: PatternId,
}

impl TestEnv for EnvMultiOverlap {
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

        // Build overlapping patterns
        let (ab, ab_id) = graph.insert_pattern_with_id(vec![a, b]);
        let (bc, bc_id) = graph.insert_pattern_with_id(vec![b, c]);
        let (cd, cd_id) = graph.insert_pattern_with_id(vec![c, d]);
        let (abcd, abcd_id) = graph.insert_pattern_with_id(vec![ab, cd]);

        #[cfg(any(test, feature = "test-api"))]
        context_trace::graph::test_graph::register_test_graph(&graph);

        graph.emit_graph_snapshot();

        Self {
            graph: HypergraphRef::from(graph),
            a,
            b,
            c,
            d,
            ab,
            ab_id: ab_id.unwrap(),
            bc,
            bc_id: bc_id.unwrap(),
            cd,
            cd_id: cd_id.unwrap(),
            abcd,
            abcd_id: abcd_id.unwrap(),
        }
    }

    fn get<'a>() -> RwLockReadGuard<'a, Self> {
        get_multi_context().read().unwrap()
    }
    
    fn get_mut<'a>() -> RwLockWriteGuard<'a, Self> {
        get_multi_context().write().unwrap()
    }

    fn graph(&self) -> &HypergraphRef {
        &self.graph
    }
}

fn get_multi_context() -> &'static Arc<RwLock<EnvMultiOverlap>> {
    CTX_MULTI_OVERLAP.with(|cell| unsafe {
        let ptr = cell.get_or_init(|| {
            Arc::new(RwLock::new(EnvMultiOverlap::initialize()))
        });
        &*(ptr as *const Arc<RwLock<EnvMultiOverlap>>)
    })
}

thread_local! {
    static CTX_MULTI_OVERLAP: OnceLock<Arc<RwLock<EnvMultiOverlap>>> = const { OnceLock::new() };
}
