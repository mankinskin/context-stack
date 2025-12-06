//! Test environment for index_prefix1 test
//!
//! Graph: heldld with patterns ld and heldld
//! Tests prefix matching behavior

use crate::{
    graph::{
        Hypergraph,
        HypergraphRef,
        vertex::{
            atom::Atom,
            pattern::id::PatternId,
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
pub struct EnvIndexPrefix1 {
    pub graph: HypergraphRef,
    pub h: Token,
    pub e: Token,
    pub l: Token,
    pub d: Token,
    pub ld: Token,
    pub ld_id: PatternId,
    pub heldld: Token,
    pub heldld_id: PatternId,
}

impl TestEnv for EnvIndexPrefix1 {
    fn initialize() -> Self {
        let mut graph = Hypergraph::default();
        let [h, e, l, d] = graph.insert_atoms([
            Atom::Element('h'),
            Atom::Element('e'),
            Atom::Element('l'),
            Atom::Element('d'),
        ])[..] else {
            panic!()
        };

        let (ld, ld_id) = graph.insert_pattern_with_id(vec![l, d]);
        let (heldld, heldld_id) =
            graph.insert_pattern_with_id(vec![h, e, ld, ld]);

        #[cfg(any(test, feature = "test-api"))]
        crate::graph::test_graph::register_test_graph(&graph);

        Self {
            graph: HypergraphRef::from(graph),
            h,
            e,
            l,
            d,
            ld,
            ld_id: ld_id.unwrap(),
            heldld,
            heldld_id: heldld_id.unwrap(),
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

fn get_context_index_prefix1() -> &'static Arc<RwLock<EnvIndexPrefix1>> {
    CONTEXT_INDEX_PREFIX1.with(|cell| unsafe {
        let ptr = cell.get_or_init(|| {
            Arc::new(RwLock::new(EnvIndexPrefix1::initialize()))
        });
        &*(ptr as *const Arc<RwLock<EnvIndexPrefix1>>)
    })
}

thread_local! {
    static CONTEXT_INDEX_PREFIX1: OnceLock<Arc<RwLock<EnvIndexPrefix1>>> = const { OnceLock::new() };
}
