//! Test environment for hypergraph-related scenarios
//!
//! Graph structure:
//! - atoms: h, y, p, e, r, g, a
//! - patterns: hypergra = [h, y, p, e, r, g, r, a] (partial "hypergraph")
//!
//! Used for testing:
//! - Partial match scenarios where query doesn't continue
//! - Integration tests simulating context-read failures

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
pub(crate) struct EnvHypergra {
    pub(crate) graph: HypergraphRef,
    pub(crate) h: Token,
    pub(crate) y: Token,
    pub(crate) p: Token,
    pub(crate) e: Token,
    pub(crate) r: Token,
    pub(crate) g: Token,
    pub(crate) a: Token,
    pub(crate) hypergra: Token,
    pub(crate) hypergra_id: PatternId,
}

impl TestEnv for EnvHypergra {
    fn initialize() -> Self {
        let graph = Hypergraph::default();
        let [h, y, p, e, r, g, a] = graph.insert_atoms([
            Atom::Element('h'),
            Atom::Element('y'),
            Atom::Element('p'),
            Atom::Element('e'),
            Atom::Element('r'),
            Atom::Element('g'),
            Atom::Element('a'),
        ])[..] else {
            panic!()
        };

        // Create "hypergra" pattern (partial "hypergraph")
        let (hypergra, hypergra_id) =
            graph.insert_pattern_with_id(vec![h, y, p, e, r, g, r, a]);

        #[cfg(any(test, feature = "test-api"))]
        context_trace::graph::test_graph::register_test_graph(&graph);

        graph.emit_graph_snapshot();

        Self {
            graph: HypergraphRef::from(graph),
            h,
            y,
            p,
            e,
            r,
            g,
            a,
            hypergra,
            hypergra_id: hypergra_id.unwrap(),
        }
    }

    fn get<'a>() -> RwLockReadGuard<'a, Self> {
        get_env_hypergra().read().unwrap()
    }
    fn get_mut<'a>() -> RwLockWriteGuard<'a, Self> {
        get_env_hypergra().write().unwrap()
    }

    fn graph(&self) -> &HypergraphRef {
        &self.graph
    }
}

fn get_env_hypergra() -> &'static Arc<RwLock<EnvHypergra>> {
    ENV_HYPERGRA.with(|cell| unsafe {
        let ptr = cell
            .get_or_init(|| Arc::new(RwLock::new(EnvHypergra::initialize())));
        &*(ptr as *const Arc<RwLock<EnvHypergra>>)
    })
}

thread_local! {
    static ENV_HYPERGRA: OnceLock<Arc<RwLock<EnvHypergra>>> = const { OnceLock::new() };
}
