//! Test environment for index_pattern2 test
//!
//! Graph with patterns: yz, xab, xyz, xabz, xabyz
//! Tests pattern matching with different compositions

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
#[allow(dead_code)]
pub struct EnvInsertPattern2 {
    pub graph: HypergraphRef,
    pub a: Token,
    pub b: Token,
    pub x: Token,
    pub y: Token,
    pub z: Token,
    pub yz: Token,
    pub xab: Token,
    pub xyz: Token,
    pub xabz: Token,
    pub xabyz: Token,
}

impl TestEnv for EnvInsertPattern2 {
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

        let yz = graph.insert_pattern(vec![y, z]);
        let xab = graph.insert_pattern(vec![x, a, b]);
        let xyz = graph.insert_pattern(vec![x, yz]);
        let xabz = graph.insert_pattern(vec![xab, z]);
        let xabyz = graph.insert_pattern(vec![xab, yz]);

        #[cfg(any(test, feature = "test-api"))]
        context_trace::graph::test_graph::register_test_graph(&graph);

        Self {
            graph: HypergraphRef::from(graph),
            a,
            b,
            x,
            y,
            z,
            yz,
            xab,
            xyz,
            xabz,
            xabyz,
        }
    }

    fn get<'a>() -> RwLockReadGuard<'a, Self> {
        get_context_index_pattern2().read().unwrap()
    }
    fn get_mut<'a>() -> RwLockWriteGuard<'a, Self> {
        get_context_index_pattern2().write().unwrap()
    }

    fn graph(&self) -> &HypergraphRef {
        &self.graph
    }
}

fn get_context_index_pattern2() -> &'static Arc<RwLock<EnvInsertPattern2>> {
    CONTEXT_INDEX_PATTERN2.with(|cell| unsafe {
        let ptr = cell.get_or_init(|| {
            Arc::new(RwLock::new(EnvInsertPattern2::initialize()))
        });
        &*(ptr as *const Arc<RwLock<EnvInsertPattern2>>)
    })
}

thread_local! {
    static CONTEXT_INDEX_PATTERN2: OnceLock<Arc<RwLock<EnvInsertPattern2>>> = const { OnceLock::new() };
}
