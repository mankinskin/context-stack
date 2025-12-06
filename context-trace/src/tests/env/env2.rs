//! Test environment for cdefghi pattern tests
//!
//! Used by context-search trace cache tests and context-insert interval tests

use crate::{
    graph::{
        Hypergraph,
        HypergraphRef,
        vertex::{
            atom::Atom,
            pattern::{
                Pattern,
                id::PatternId,
            },
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
pub struct Env2 {
    pub graph: HypergraphRef,
    // Atoms
    pub a: Token,
    pub b: Token,
    pub c: Token,
    pub d: Token,
    pub e: Token,
    pub f: Token,
    pub g: Token,
    pub h: Token,
    pub i: Token,
    pub j: Token,
    pub k: Token,

    // Patterns
    pub cd: Token,
    pub c_d_id: PatternId,

    pub hi: Token,
    pub h_i_id: PatternId,

    pub efg: Token,
    pub e_f_g_id: PatternId,

    pub cdefg: Token,
    pub cd_efg_id: PatternId,

    pub efghi: Token,
    pub efg_hi_id: PatternId,

    pub cdefghi: Token,
    pub cdefghi_ids: Vec<PatternId>,

    pub abcdefghijk: Token,
    pub abcdefghijk_id: PatternId,
}

impl TestEnv for Env2 {
    fn initialize() -> Self {
        let mut graph = Hypergraph::default();
        let atoms = graph.insert_atoms([
            Atom::Element('a'),
            Atom::Element('b'),
            Atom::Element('c'),
            Atom::Element('d'),
            Atom::Element('e'),
            Atom::Element('f'),
            Atom::Element('g'),
            Atom::Element('h'),
            Atom::Element('i'),
            Atom::Element('j'),
            Atom::Element('k'),
        ]);
        let [a, b, c, d, e, f, g, h, i, j, k] = atoms[..] else {
            panic!()
        };

        let (cd, c_d_id) = graph.insert_pattern_with_id([c, d]);
        let (hi, h_i_id) = graph.insert_pattern_with_id([h, i]);
        let (efg, e_f_g_id) = graph.insert_pattern_with_id([e, f, g]);
        let (cdefg, cd_efg_id) = graph.insert_pattern_with_id([cd, efg]);
        let (efghi, efg_hi_id) = graph.insert_pattern_with_id([efg, hi]);

        let (cdefghi, cdefghi_ids) = graph.insert_patterns_with_ids([
            Pattern::from(vec![cdefg, hi]),
            Pattern::from(vec![cd, efghi]),
        ]);

        let (abcdefghijk, abcdefghijk_id) =
            graph.insert_pattern_with_id([a, b, cdefghi, j, k]);

        let graph = graph.to_ref();

        Self {
            graph,
            a,
            b,
            c,
            d,
            e,
            f,
            g,
            h,
            i,
            j,
            k,
            cd,
            c_d_id: c_d_id.unwrap(),
            hi,
            h_i_id: h_i_id.unwrap(),
            efg,
            e_f_g_id: e_f_g_id.unwrap(),
            cdefg,
            cd_efg_id: cd_efg_id.unwrap(),
            efghi,
            efg_hi_id: efg_hi_id.unwrap(),
            cdefghi,
            cdefghi_ids,
            abcdefghijk,
            abcdefghijk_id: abcdefghijk_id.unwrap(),
        }
    }

    fn get<'a>() -> RwLockReadGuard<'a, Self> {
        get_context2().read().unwrap()
    }
    fn get_mut<'a>() -> RwLockWriteGuard<'a, Self> {
        get_context2().write().unwrap()
    }

    fn graph(&self) -> &HypergraphRef {
        &self.graph
    }
}

fn get_context2() -> &'static Arc<RwLock<Env2>> {
    CONTEXT2.with(|cell| unsafe {
        let ptr =
            cell.get_or_init(|| Arc::new(RwLock::new(Env2::initialize())));
        &*(ptr as *const Arc<RwLock<Env2>>)
    })
}

thread_local! {
    static CONTEXT2: OnceLock<Arc<RwLock<Env2>>> = const { OnceLock::new() };
}
