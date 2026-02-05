//! Test environment for xyyxy scenario
//!
//! Graph structure:
//! - Atoms: x, y
//! - Pattern xy = [x, y]
//! - Pattern xyyxy = [xy, y, xy]
//!
//! This tests the scenario from context-read's read_repeating_known1 test.
//! Key property: When searching for [x, y], should find xy as EntireRoot,
//! not as a Postfix within xyyxy.

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
pub struct EnvXyyxy {
    pub graph: HypergraphRef,
    pub x: Token,
    pub y: Token,
    pub xy: Token,
    pub xy_id: PatternId,
    pub xyyxy: Token,
    pub xyyxy_id: PatternId,
}

impl TestEnv for EnvXyyxy {
    fn initialize() -> Self {
        let graph = Hypergraph::default();
        let [x, y] =
            graph.insert_atoms([Atom::Element('x'), Atom::Element('y')])[..]
        else {
            panic!()
        };

        let (xy, xy_id) = graph.insert_pattern_with_id(vec![x, y]);
        let (xyyxy, xyyxy_id) = graph.insert_pattern_with_id(vec![xy, y, xy]);

        #[cfg(any(test, feature = "test-api"))]
        context_trace::graph::test_graph::register_test_graph(&graph);

        Self {
            graph: HypergraphRef::from(graph),
            x,
            y,
            xy,
            xy_id: xy_id.unwrap(),
            xyyxy,
            xyyxy_id: xyyxy_id.unwrap(),
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

fn get_env() -> &'static Arc<RwLock<EnvXyyxy>> {
    ENV_XYYXY.with(|cell| unsafe {
        let ptr =
            cell.get_or_init(|| Arc::new(RwLock::new(EnvXyyxy::initialize())));
        &*(ptr as *const Arc<RwLock<EnvXyyxy>>)
    })
}

thread_local! {
    static ENV_XYYXY: OnceLock<Arc<RwLock<EnvXyyxy>>> = const { OnceLock::new() };
}
