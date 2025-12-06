//! Test environment for index_pattern1 test
//!
//! Graph with patterns: ab, by, yz, xa, xab, xaby, xabyz
//! Tests complex pattern matching with overlapping structures

use context_trace::{
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

#[derive(Debug)]
pub struct EnvIndexPattern1 {
    pub graph: HypergraphRef,
    pub a: Token,
    pub b: Token,
    pub x: Token,
    pub y: Token,
    pub z: Token,
    pub ab: Token,
    pub by: Token,
    pub yz: Token,
    pub xa: Token,
    pub xab: Token,
    pub xaby: Token,
    pub xabyz: Token,
}

impl TestEnv for EnvIndexPattern1 {
    fn initialize() -> Self {
        let mut graph = Hypergraph::default();
        let [a, b, x, y, z] = graph.insert_atoms([
            Atom::Element('a'),
            Atom::Element('b'),
            Atom::Element('x'),
            Atom::Element('y'),
            Atom::Element('z'),
        ])[..] else {
            panic!()
        };

        let ab = graph.insert_pattern(vec![a, b]);
        let by = graph.insert_pattern(vec![b, y]);
        let yz = graph.insert_pattern(vec![y, z]);
        let xa = graph.insert_pattern(vec![x, a]);
        let xab = graph.insert_patterns([vec![x, ab], vec![xa, b]]);
        let xaby = graph.insert_patterns([vec![xab, y], vec![xa, by]]);
        let xabyz = graph.insert_patterns([vec![xaby, z], vec![xab, yz]]);

        #[cfg(any(test, feature = "test-api"))]
        context_trace::graph::test_graph::register_test_graph(&graph);

        Self {
            graph: HypergraphRef::from(graph),
            a,
            b,
            x,
            y,
            z,
            ab,
            by,
            yz,
            xa,
            xab,
            xaby,
            xabyz,
        }
    }

    fn get<'a>() -> RwLockReadGuard<'a, Self> {
        get_context_index_pattern1().read().unwrap()
    }
    fn get_mut<'a>() -> RwLockWriteGuard<'a, Self> {
        get_context_index_pattern1().write().unwrap()
    }

    fn graph(&self) -> &HypergraphRef {
        &self.graph
    }
}

fn get_context_index_pattern1() -> &'static Arc<RwLock<EnvIndexPattern1>> {
    CONTEXT_INDEX_PATTERN1.with(|cell| unsafe {
        let ptr = cell.get_or_init(|| {
            Arc::new(RwLock::new(EnvIndexPattern1::initialize()))
        });
        &*(ptr as *const Arc<RwLock<EnvIndexPattern1>>)
    })
}

thread_local! {
    static CONTEXT_INDEX_PATTERN1: OnceLock<Arc<RwLock<EnvIndexPattern1>>> = const { OnceLock::new() };
}
