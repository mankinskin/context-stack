use crate::graph::{
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
};
use std::sync::{
    Arc,
    OnceLock,
    RwLock,
    RwLockReadGuard,
    RwLockWriteGuard,
};
pub trait TestEnv {
    fn initialize_expected() -> Self;
    fn get_expected<'a>() -> RwLockReadGuard<'a, Self>;
    fn get_expected_mut<'a>() -> RwLockWriteGuard<'a, Self>;
}
pub struct Env1 {
    pub graph: HypergraphRef,
    pub a: Token,
    pub b: Token,
    pub c: Token,
    pub d: Token,
    pub e: Token,
    pub f: Token,
    pub g: Token,
    pub h: Token,
    pub i: Token,

    pub ab: Token,
    pub bc: Token,
    pub bc_id: PatternId,

    pub cd: Token,
    pub cd_id: PatternId,

    pub bcd: Token,
    pub b_cd_id: PatternId,
    pub bc_d_id: PatternId,

    pub def: Token,
    pub d_ef_id: PatternId,

    pub abc: Token,
    pub a_bc_id: PatternId,

    pub abcd: Token,
    pub a_bcd_id: PatternId,
    pub abc_d_id: PatternId,

    pub ef: Token,
    pub e_f_id: PatternId,

    pub gh: Token,
    pub efgh: Token,

    pub ghi: Token,

    pub cdef: Token,
    pub c_def_id: PatternId,
    pub cd_ef_id: PatternId,

    pub efghi: Token,
    pub aba: Token,
    pub abab: Token,
    pub ababab: Token,
    pub ababcd: Token,
    pub ababababcd: Token,
    pub ababcdefghi: Token,

    pub abcdef: Token,
    pub abcd_ef_id: PatternId,
    pub abc_def_id: PatternId,
    pub ab_cdef_id: PatternId,

    pub abcdefghi: Token,
    pub abcd_efghi_id: PatternId,
    pub abcdef_ghi_id: PatternId,

    pub ababababcdefghi: Token,
}
pub fn atoms1(graph: &mut Hypergraph) -> Vec<Token> {
    graph.insert_atoms([
        Atom::Element('a'),
        Atom::Element('b'),
        Atom::Element('c'),
        Atom::Element('d'),
        Atom::Element('e'),
        Atom::Element('f'),
        Atom::Element('g'),
        Atom::Element('h'),
        Atom::Element('i'),
    ])
}
impl TestEnv for Env1 {
    fn initialize_expected() -> Self {
        let mut graph = Hypergraph::default();
        let [a, b, c, d, e, f, g, h, i] = atoms1(&mut graph)[..] else {
            panic!()
        };
        // abcdefghi
        // ababababcdbcdefdefcdefefghefghghi
        // ->
        // abab ab abcdbcdefdefcdefefghefghghi
        // ab abab abcdbcdefdefcdefefghefghghi

        // abcdbcdef def cdef efgh efgh ghi

        // abcd b cdef
        // abcd bcd ef

        // ab cd
        // abc d
        // a bcd
        // index: 9
        let ab = graph.insert_pattern(vec![a, b]);
        let (bc, bc_id) = graph.insert_pattern_with_id(vec![b, c]);
        let (abc, abc_ids) = graph.insert_patterns_with_ids([
            Pattern::from(vec![ab, c]),
            Pattern::from(vec![a, bc]),
        ]);

        let (cd, cd_id) = graph.insert_pattern_with_id(vec![c, d]);
        // 13
        let (bcd, bcd_ids) = graph.insert_patterns_with_ids([
            Pattern::from(vec![bc, d]),
            Pattern::from(vec![b, cd]),
        ]);
        //let abcd = graph.insert_pattern(&[abc, d]);
        //graph.insert_to_pattern(abcd, &[a, bcd]);
        let (abcd, abcd_ids) = graph.insert_patterns_with_ids([
            Pattern::from(vec![abc, d]),
            Pattern::from(vec![a, bcd]),
        ]);
        // index 15
        let (ef, e_f_id) = graph.insert_pattern_with_id(vec![e, f]);
        let gh = graph.insert_pattern(vec![g, h]);
        let ghi = graph.insert_pattern(vec![gh, i]);
        let efgh = graph.insert_pattern(vec![ef, gh]);
        let efghi = graph.insert_patterns([vec![efgh, i], vec![ef, ghi]]);
        let (def, d_ef_id) = graph.insert_pattern_with_id(vec![d, ef]);
        let (cdef, cdef_ids) = graph.insert_patterns_with_ids([
            Pattern::from(vec![c, def]),
            Pattern::from(vec![cd, ef]),
        ]);
        // index 22
        let (abcdef, abcdef_ids) = graph.insert_patterns_with_ids([
            Pattern::from(vec![abcd, ef]),
            Pattern::from(vec![abc, def]),
            Pattern::from(vec![ab, cdef]),
        ]);
        let (abcdefghi, abcdefghi_ids) = graph.insert_patterns_with_ids([
            Pattern::from(vec![abcd, efghi]),
            Pattern::from(vec![abcdef, ghi]),
        ]);
        let aba = graph.insert_pattern(vec![ab, a]);
        // 25
        let abab = graph.insert_patterns([vec![aba, b], vec![ab, ab]]);
        let ababab = graph.insert_patterns([vec![abab, ab], vec![ab, abab]]);
        let ababcd = graph.insert_patterns([
            vec![ab, abcd],
            vec![aba, bcd],
            vec![abab, cd],
        ]);
        // 28
        let ababababcd =
            graph.insert_patterns([vec![ababab, abcd], vec![abab, ababcd]]);
        let ababcdefghi =
            graph.insert_patterns([vec![ab, abcdefghi], vec![ababcd, efghi]]);
        // 30
        let ababababcdefghi = graph.insert_patterns([
            vec![ababababcd, efghi],
            vec![abab, ababcdefghi],
            vec![ababab, abcdefghi],
        ]);

        // Register the graph for token string representations in test output
        #[cfg(any(test, feature = "test-api"))]
        crate::graph::test_graph::register_test_graph(&graph);

        Env1 {
            graph: HypergraphRef::from(graph),
            a,
            b,
            c,
            d,
            e,
            f,
            g,
            h,
            i,
            ab,
            bc,
            bc_id: bc_id.unwrap(),
            cd,
            cd_id: cd_id.unwrap(),
            bcd,
            bc_d_id: bcd_ids[0],
            b_cd_id: bcd_ids[1],
            abc,
            a_bc_id: abc_ids[1],
            abcd,
            abc_d_id: abcd_ids[0],
            a_bcd_id: abcd_ids[1],
            ef,
            e_f_id: e_f_id.unwrap(),
            gh,
            efgh,
            def,
            d_ef_id: d_ef_id.unwrap(),
            ghi,
            abcdef,
            abcd_ef_id: abcdef_ids[0],
            abc_def_id: abcdef_ids[1],
            ab_cdef_id: abcdef_ids[2],
            abcdefghi,
            abcd_efghi_id: abcdefghi_ids[0],
            abcdef_ghi_id: abcdefghi_ids[1],
            cdef,
            c_def_id: cdef_ids[0],
            cd_ef_id: cdef_ids[1],
            efghi,
            aba,
            abab,
            ababab,
            ababcd,
            ababababcd,
            ababcdefghi,
            ababababcdefghi,
        }
    }
    
    fn get_expected<'a>() -> RwLockReadGuard<'a, Self> {
        get_context().read().unwrap()
    }
    fn get_expected_mut<'a>() -> RwLockWriteGuard<'a, Self> {
        get_context().write().unwrap()
    }
}

fn get_context() -> &'static Arc<RwLock<Env1>> {
    CONTEXT.with(|cell| {
        // SAFETY: OnceLock::get_or_init returns &T which is tied to the OnceLock's lifetime
        // We extend this to 'static because thread_local storage persists for thread lifetime
        unsafe {
            let ptr = cell.get_or_init(|| Arc::new(RwLock::new(Env1::initialize_expected())));
            &*(ptr as *const Arc<RwLock<Env1>>)
        }
    })
}

thread_local! {
    /// Thread-local test environment
    /// Each test thread maintains its own Env1 instance, allowing parallel test execution
    /// without lock contention or poisoning issues between tests
    static CONTEXT: OnceLock<Arc<RwLock<Env1>>> = OnceLock::new();
}
