//! Test environment for repeated pattern scenarios (e.g., "ababab")
//!
//! Graph structure:
//! - atoms: a, b
//! - patterns: ab = [a, b], ababab = [ab, ab, ab]
//!
//! Used for testing:
//! - Cache/root mismatch scenarios from context-read
//! - Triple repeat pattern handling
//! - Intermediate token discovery

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

#[derive(Debug)]
#[allow(dead_code)]
pub struct EnvTripleRepeat {
    pub graph: HypergraphRef,
    pub a: Token,
    pub b: Token,
    pub ab: Token,
    pub ab_id: PatternId,
    pub ababab: Token,
    pub ababab_id: PatternId,
}

impl TestEnv for EnvTripleRepeat {
    fn initialize() -> Self {
        let graph = Hypergraph::default();
        let [a, b] =
            graph.insert_atoms([Atom::Element('a'), Atom::Element('b')])[..]
        else {
            panic!()
        };

        let (ab, ab_id) = graph.insert_pattern_with_id(vec![a, b]);
        let (ababab, ababab_id) =
            graph.insert_pattern_with_id(vec![ab, ab, ab]);

        #[cfg(any(test, feature = "test-api"))]
        context_trace::graph::test_graph::register_test_graph(&graph);

        Self {
            graph: HypergraphRef::from(graph),
            a,
            b,
            ab,
            ab_id: ab_id.unwrap(),
            ababab,
            ababab_id: ababab_id.unwrap(),
        }
    }

    fn get<'a>() -> RwLockReadGuard<'a, Self> {
        get_env_triple_repeat().read().unwrap()
    }
    fn get_mut<'a>() -> RwLockWriteGuard<'a, Self> {
        get_env_triple_repeat().write().unwrap()
    }

    fn graph(&self) -> &HypergraphRef {
        &self.graph
    }
}

fn get_env_triple_repeat() -> &'static Arc<RwLock<EnvTripleRepeat>> {
    ENV_TRIPLE_REPEAT.with(|cell| unsafe {
        let ptr = cell.get_or_init(|| {
            Arc::new(RwLock::new(EnvTripleRepeat::initialize()))
        });
        &*(ptr as *const Arc<RwLock<EnvTripleRepeat>>)
    })
}

thread_local! {
    static ENV_TRIPLE_REPEAT: OnceLock<Arc<RwLock<EnvTripleRepeat>>> = const { OnceLock::new() };
}
