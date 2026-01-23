//! Test environment for insert_prefix1 test
//!
//! Graph: heldld with patterns ld and heldld
//! Tests prefix matching behavior

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
pub struct EnvInsertPrefix1 {
    pub graph: HypergraphRef,
    pub a: Token,
    pub b: Token,
    pub c: Token,
    pub d: Token,
    pub cd: Token,
    pub cd_id: PatternId,
    pub abcdcd: Token,
    pub abcdcd_id: PatternId,
}

impl TestEnv for EnvInsertPrefix1 {
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

        let (cd, cd_id) = graph.insert_pattern_with_id(vec![c, d]);
        let (abcdcd, abcdcd_id) =
            graph.insert_pattern_with_id(vec![a, b, cd, cd]);

        #[cfg(any(test, feature = "test-api"))]
        context_trace::graph::test_graph::register_test_graph(&graph);

        Self {
            graph: HypergraphRef::from(graph),
            a,
            b,
            c,
            d,
            cd,
            cd_id: cd_id.unwrap(),
            abcdcd,
            abcdcd_id: abcdcd_id.unwrap(),
        }
    }

    fn get<'a>() -> RwLockReadGuard<'a, Self> {
        get_context_index_prefix1().read().unwrap()
    }
    fn get_mut<'a>() -> RwLockWriteGuard<'a, Self> {
        get_context_index_prefix1().write().unwrap()
    }

    fn graph(&self) -> &HypergraphRef {
        &self.graph
    }
}

fn get_context_index_prefix1() -> &'static Arc<RwLock<EnvInsertPrefix1>> {
    CONTEXT_INDEX_PREFIX1.with(|cell| unsafe {
        let ptr = cell.get_or_init(|| {
            Arc::new(RwLock::new(EnvInsertPrefix1::initialize()))
        });
        &*(ptr as *const Arc<RwLock<EnvInsertPrefix1>>)
    })
}

thread_local! {
    static CONTEXT_INDEX_PREFIX1: OnceLock<Arc<RwLock<EnvInsertPrefix1>>> = const { OnceLock::new() };
}
