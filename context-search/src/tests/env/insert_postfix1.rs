//! Test environment for insert_postfix1 test
//!
//! Graph: ababcd with patterns ab and ababcd
//! Tests postfix matching behavior

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

#[derive(Debug)]
pub struct EnvInsertPostfix1 {
    pub graph: HypergraphRef,
    pub a: Token,
    pub b: Token,
    pub c: Token,
    pub d: Token,
    pub ab: Token,
    pub ab_id: PatternId,
    pub ababcd: Token,
    pub ababcd_id: PatternId,
}

impl TestEnv for EnvInsertPostfix1 {
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

        let (ab, ab_id) = graph.insert_pattern_with_id(vec![a, b]);
        let (ababcd, ababcd_id) =
            graph.insert_pattern_with_id(vec![ab, ab, c, d]);

        #[cfg(any(test, feature = "test-api"))]
        context_trace::graph::test_graph::register_test_graph(&graph);

        Self {
            graph: HypergraphRef::from(graph),
            a,
            b,
            c,
            d,
            ab,
            ab_id: ab_id.unwrap(),
            ababcd,
            ababcd_id: ababcd_id.unwrap(),
        }
    }

    fn get<'a>() -> RwLockReadGuard<'a, Self> {
        get_context_index_postfix1().read().unwrap()
    }
    fn get_mut<'a>() -> RwLockWriteGuard<'a, Self> {
        get_context_index_postfix1().write().unwrap()
    }

    fn graph(&self) -> &HypergraphRef {
        &self.graph
    }
}

fn get_context_index_postfix1() -> &'static Arc<RwLock<EnvInsertPostfix1>> {
    CONTEXT_INDEX_POSTFIX1.with(|cell| unsafe {
        let ptr = cell.get_or_init(|| {
            Arc::new(RwLock::new(EnvInsertPostfix1::initialize()))
        });
        &*(ptr as *const Arc<RwLock<EnvInsertPostfix1>>)
    })
}

thread_local! {
    static CONTEXT_INDEX_POSTFIX1: OnceLock<Arc<RwLock<EnvInsertPostfix1>>> = const { OnceLock::new() };
}
