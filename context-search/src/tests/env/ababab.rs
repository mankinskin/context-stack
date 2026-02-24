//! Test environment for ababab scenario (triple repeat)
//!
//! Graph structure:
//! - Atoms: a, b
//! - Pattern ab = [a, b]
//! - Pattern abab = [ab, ab]
//! - Pattern ababab = [abab, ab]  (NOT [ab, ab, ab])
//!
//! This creates a proper hierarchy where abab is a child of ababab,
//! allowing parent exploration to find ababab when searching for [ab, ab, ab].
//!
//! Key property: When searching for [ab, ab, ab], should find ababab as EntireRoot.

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
pub(crate) struct EnvAbabab {
    pub(crate) graph: HypergraphRef,
    pub(crate) a: Token,
    pub(crate) b: Token,
    pub(crate) ab: Token,
    pub(crate) ab_id: PatternId,
    pub(crate) abab: Token,
    pub(crate) abab_id: PatternId,
    pub(crate) ababab: Token,
    pub(crate) ababab_id: PatternId,
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
        // ababab = [abab, ab] so that abab is a child of ababab
        // This allows parent exploration to find ababab when matching [ab, ab, ab]
        let (ababab, ababab_id) = graph.insert_pattern_with_id(vec![abab, ab]);

        #[cfg(any(test, feature = "test-api"))]
        context_trace::graph::test_graph::register_test_graph(&graph);

        graph.emit_graph_snapshot();

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
