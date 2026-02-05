//! Test environment for ababab scenario (triple repeat)
//!
//! Graph structure:
//! - Atoms: a, b
//! - Pattern ab = [a, b]
//! - Pattern abab = [ab, ab]
//! - Pattern ababab = [ab, ab, ab]
//!
//! This tests the scenario from context-read's validate_triple_repeat test.
//! Key property: When searching for [ab, ab, ab], should find ababab as EntireRoot,
//! not abab as a partial match.

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
    RwLockReadGuard,
    RwLockWriteGuard,
};

#[derive(Debug)]
pub struct EnvAbabab {
    pub graph: HypergraphRef,
    pub a: Token,
    pub b: Token,
    pub ab: Token,
    pub ab_id: PatternId,
    pub abab: Token,
    pub abab_id: PatternId,
    pub ababab: Token,
    pub ababab_id: PatternId,
}

impl TestEnv for EnvAbabab {
    fn initialize() -> Self {
        let graph = Hypergraph::default();
        let [a, b] =
            graph.insert_atoms([Atom::Element('a'), Atom::Element('b')])[..]
        else {
            panic!()
        };

        let (ab, ab_id) = graph.insert_pattern_with_id(vec![a, b]);
        let (abab, abab_id) = graph.insert_pattern_with_id(vec![ab, ab]);
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
            abab,
            abab_id: abab_id.unwrap(),
            ababab,
            ababab_id: ababab_id.unwrap(),
        }
    }

    fn get<'a>() -> RwLockReadGuard<'a, Self> {
        get_env().read().unwrap()
    }
    fn get_mut<'a>() -> RwLockWriteGuard<'a, Self> {
        get_env().write().unwrap()
    }

    fn graph(&self) -> &HypergraphRef {
        &self.graph
    }
}

use std::sync::{
    Arc,
    OnceLock,
    RwLock,
};

fn get_env() -> &'static Arc<RwLock<EnvAbabab>> {
    ENV_ABABAB.with(|cell| unsafe {
        let ptr =
            cell.get_or_init(|| Arc::new(RwLock::new(EnvAbabab::initialize())));
        &*(ptr as *const Arc<RwLock<EnvAbabab>>)
    })
}

thread_local! {
    static ENV_ABABAB: OnceLock<Arc<RwLock<EnvAbabab>>> = const { OnceLock::new() };
}
