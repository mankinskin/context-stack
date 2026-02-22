//! Test environment for index_infix2 test
//!
//! Graph with patterns: yy, xx, xy, abcdx, yabcdx, abcdxx, xxy, xxyyabcdxxyy
//! Tests complex infix matching with repetitions

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
pub(crate) struct EnvInsertInfix2 {
    pub(crate) graph: HypergraphRef,
    pub(crate) a: Token,
    pub(crate) b: Token,
    pub(crate) c: Token,
    pub(crate) d: Token,
    pub(crate) x: Token,
    pub(crate) y: Token,
    pub(crate) yy: Token,
    pub(crate) xx: Token,
    pub(crate) xy: Token,
    pub(crate) abcdx: Token,
    pub(crate) yabcdx: Token,
    pub(crate) abcdxx: Token,
    pub(crate) xxy: Token,
    pub(crate) xxyyabcdxxyy: Token,
}

impl TestEnv for EnvInsertInfix2 {
    fn initialize() -> Self {
        let graph = Hypergraph::default();
        let [a, b, c, d, x, y] = graph.insert_atoms([
            Atom::Element('a'),
            Atom::Element('b'),
            Atom::Element('c'),
            Atom::Element('d'),
            Atom::Element('x'),
            Atom::Element('y'),
        ])[..] else {
            panic!()
        };

        let yy = graph.insert_pattern(vec![y, y]);
        let xx = graph.insert_pattern(vec![x, x]);
        let xy = graph.insert_pattern(vec![x, y]);
        let abcdx = graph.insert_pattern(vec![a, b, c, d, x]);
        let yabcdx = graph.insert_pattern(vec![y, abcdx]);
        let abcdxx = graph.insert_pattern(vec![abcdx, x]);
        let xxy = graph.insert_patterns([vec![xx, y], vec![x, xy]]);
        let xxyyabcdxxyy = graph.insert_patterns([
            vec![xx, yy, abcdxx, yy],
            vec![xxy, yabcdx, xy, y],
        ]);

        #[cfg(any(test, feature = "test-api"))]
        context_trace::graph::test_graph::register_test_graph(&graph);

        graph.emit_graph_snapshot();

        Self {
            graph: HypergraphRef::from(graph),
            a,
            b,
            c,
            d,
            x,
            y,
            yy,
            xx,
            xy,
            abcdx,
            yabcdx,
            abcdxx,
            xxy,
            xxyyabcdxxyy,
        }
    }

    fn get<'a>() -> RwLockReadGuard<'a, Self> {
        get_context_index_infix2().read().unwrap()
    }
    fn get_mut<'a>() -> RwLockWriteGuard<'a, Self> {
        get_context_index_infix2().write().unwrap()
    }

    fn graph(&self) -> &HypergraphRef {
        &self.graph
    }
}

fn get_context_index_infix2() -> &'static Arc<RwLock<EnvInsertInfix2>> {
    CONTEXT_INDEX_INFIX2.with(|cell| unsafe {
        let ptr = cell.get_or_init(|| {
            Arc::new(RwLock::new(EnvInsertInfix2::initialize()))
        });
        &*(ptr as *const Arc<RwLock<EnvInsertInfix2>>)
    })
}

thread_local! {
    static CONTEXT_INDEX_INFIX2: OnceLock<Arc<RwLock<EnvInsertInfix2>>> = const { OnceLock::new() };
}
