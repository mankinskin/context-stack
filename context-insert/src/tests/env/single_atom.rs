//! Test environment for single-atom repeated patterns
//!
//! Graph structure (atoms only initially):
//! - atoms: a
//!
//! Used for testing:
//! - Inserting patterns like [a, a]
//! - Intermediate token creation for "aaa" -> "aa" + "a"

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
};
use std::sync::{
    Arc,
    OnceLock,
    RwLock,
    RwLockReadGuard,
    RwLockWriteGuard,
};

#[derive(Debug)]
pub(crate) struct EnvSingleAtom {
    pub(crate) graph: HypergraphRef,
    pub(crate) a: Token,
}

impl TestEnv for EnvSingleAtom {
    fn initialize() -> Self {
        let graph = Hypergraph::default();
        let [a] = graph.insert_atoms([
            Atom::Element('a'),
        ])[..] else {
            panic!()
        };

        #[cfg(any(test, feature = "test-api"))]
        context_trace::graph::test_graph::register_test_graph(&graph);

        graph.emit_graph_snapshot();

        Self {
            graph: HypergraphRef::from(graph),
            a,
        }
    }

    fn get<'a>() -> RwLockReadGuard<'a, Self> {
        get_env_single_atom().read().unwrap()
    }
    fn get_mut<'a>() -> RwLockWriteGuard<'a, Self> {
        get_env_single_atom().write().unwrap()
    }

    fn graph(&self) -> &HypergraphRef {
        &self.graph
    }
}

fn get_env_single_atom() -> &'static Arc<RwLock<EnvSingleAtom>> {
    ENV_SINGLE_ATOM.with(|cell| unsafe {
        let ptr = cell.get_or_init(|| Arc::new(RwLock::new(EnvSingleAtom::initialize())));
        &*(ptr as *const Arc<RwLock<EnvSingleAtom>>)
    })
}

thread_local! {
    static ENV_SINGLE_ATOM: OnceLock<Arc<RwLock<EnvSingleAtom>>> = const { OnceLock::new() };
}
