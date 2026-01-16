//! Test environment for insert_infix1 test
//!
//! Graph with patterns: yz, xxabyzw
//! Tests infix/range matching behavior

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
pub struct EnvInsertInfix1 {
    pub graph: HypergraphRef,
    pub a: Token,
    pub b: Token,
    pub w: Token,
    pub x: Token,
    pub y: Token,
    pub z: Token,
    pub yz: Token,
    pub xxabyzw: Token,
}

impl TestEnv for EnvInsertInfix1 {
    fn initialize() -> Self {
        let graph = Hypergraph::default();
        let [a, b, w, x, y, z] = graph.insert_atoms([
            Atom::Element('a'),
            Atom::Element('b'),
            Atom::Element('w'),
            Atom::Element('x'),
            Atom::Element('y'),
            Atom::Element('z'),
        ])[..] else {
            panic!()
        };

        let yz = graph.insert_pattern(vec![y, z]);
        let xxabyzw = graph.insert_pattern(vec![x, x, a, b, yz, w]);

        #[cfg(any(test, feature = "test-api"))]
        context_trace::graph::test_graph::register_test_graph(&graph);

        Self {
            graph: HypergraphRef::from(graph),
            a,
            b,
            w,
            x,
            y,
            z,
            yz,
            xxabyzw,
        }
    }

    fn get<'a>() -> RwLockReadGuard<'a, Self> {
        get_context_index_infix1().read().unwrap()
    }
    fn get_mut<'a>() -> RwLockWriteGuard<'a, Self> {
        get_context_index_infix1().write().unwrap()
    }

    fn graph(&self) -> &HypergraphRef {
        &self.graph
    }
}

fn get_context_index_infix1() -> &'static Arc<RwLock<EnvInsertInfix1>> {
    CONTEXT_INDEX_INFIX1.with(|cell| unsafe {
        let ptr = cell.get_or_init(|| {
            Arc::new(RwLock::new(EnvInsertInfix1::initialize()))
        });
        &*(ptr as *const Arc<RwLock<EnvInsertInfix1>>)
    })
}

thread_local! {
    static CONTEXT_INDEX_INFIX1: OnceLock<Arc<RwLock<EnvInsertInfix1>>> = const { OnceLock::new() };
}
