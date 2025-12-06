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

/// Test environment for cdefghi pattern tests
/// Used by context-search trace cache tests and context-insert interval tests
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
    fn initialize_expected() -> Self {
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
        
        let (abcdefghijk, abcdefghijk_id) = graph.insert_pattern_with_id([a, b, cdefghi, j, k]);

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
    
    fn get_expected<'a>() -> RwLockReadGuard<'a, Self> {
        get_context2().read().unwrap()
    }
    fn get_expected_mut<'a>() -> RwLockWriteGuard<'a, Self> {
        get_context2().write().unwrap()
    }
}

fn get_context2() -> &'static Arc<RwLock<Env2>> {
    CONTEXT2.with(|cell| {
        unsafe {
            let ptr = cell.get_or_init(|| Arc::new(RwLock::new(Env2::initialize_expected())));
            &*(ptr as *const Arc<RwLock<Env2>>)
        }
    })
}

thread_local! {
    static CONTEXT2: OnceLock<Arc<RwLock<Env2>>> = OnceLock::new();
}

/// Test environment for index_prefix1 test
/// Graph: heldld with patterns ld and heldld
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
    fn initialize_expected() -> Self {
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
        let (heldld, heldld_id) = graph.insert_pattern_with_id(vec![h, e, ld, ld]);
        
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
    
    fn get_expected<'a>() -> RwLockReadGuard<'a, Self> {
        get_context_index_prefix1().read().unwrap()
    }
    fn get_expected_mut<'a>() -> RwLockWriteGuard<'a, Self> {
        get_context_index_prefix1().write().unwrap()
    }
}

fn get_context_index_prefix1() -> &'static Arc<RwLock<EnvIndexPrefix1>> {
    CONTEXT_INDEX_PREFIX1.with(|cell| {
        unsafe {
            let ptr = cell.get_or_init(|| Arc::new(RwLock::new(EnvIndexPrefix1::initialize_expected())));
            &*(ptr as *const Arc<RwLock<EnvIndexPrefix1>>)
        }
    })
}

thread_local! {
    static CONTEXT_INDEX_PREFIX1: OnceLock<Arc<RwLock<EnvIndexPrefix1>>> = OnceLock::new();
}

/// Test environment for index_postfix1 test
/// Graph: ababcd with patterns ab and ababcd
pub struct EnvIndexPostfix1 {
    pub graph: HypergraphRef,
    pub a: Token,
    pub b: Token,
    pub c: Token,
    pub d: Token,
    pub ab: Token,
    pub ab_id: PatternId,
    pub ababcd: Token,
    pub ababcd_id: PatternId,
}

impl TestEnv for EnvIndexPostfix1 {
    fn initialize_expected() -> Self {
        let mut graph = Hypergraph::default();
        let [a, b, c, d] = graph.insert_atoms([
            Atom::Element('a'),
            Atom::Element('b'),
            Atom::Element('c'),
            Atom::Element('d'),
        ])[..] else {
            panic!()
        };
        
        let (ab, ab_id) = graph.insert_pattern_with_id(vec![a, b]);
        let (ababcd, ababcd_id) = graph.insert_pattern_with_id(vec![ab, ab, c, d]);
        
        #[cfg(any(test, feature = "test-api"))]
        crate::graph::test_graph::register_test_graph(&graph);
        
        Self {
            graph: HypergraphRef::from(graph),
            a,
            b,
            c,
            d,
            ab,
            ab_id: ab_id.unwrap(),
            ababcd,
            ababcd_id: ababcd_id.unwrap(),
        }
    }
    
    fn get_expected<'a>() -> RwLockReadGuard<'a, Self> {
        get_context_index_postfix1().read().unwrap()
    }
    fn get_expected_mut<'a>() -> RwLockWriteGuard<'a, Self> {
        get_context_index_postfix1().write().unwrap()
    }
}

fn get_context_index_postfix1() -> &'static Arc<RwLock<EnvIndexPostfix1>> {
    CONTEXT_INDEX_POSTFIX1.with(|cell| {
        unsafe {
            let ptr = cell.get_or_init(|| Arc::new(RwLock::new(EnvIndexPostfix1::initialize_expected())));
            &*(ptr as *const Arc<RwLock<EnvIndexPostfix1>>)
        }
    })
}

thread_local! {
    static CONTEXT_INDEX_POSTFIX1: OnceLock<Arc<RwLock<EnvIndexPostfix1>>> = OnceLock::new();
}

/// Test environment for index_pattern1 test
/// Graph with patterns: ab, by, yz, xa, xab, xaby, xabyz
pub struct EnvIndexPattern1 {
    pub graph: HypergraphRef,
    pub a: Token,
    pub b: Token,
    pub x: Token,
    pub y: Token,
    pub z: Token,
    pub ab: Token,
    pub by: Token,
    pub yz: Token,
    pub xa: Token,
    pub xab: Token,
    pub xaby: Token,
    pub xabyz: Token,
}

impl TestEnv for EnvIndexPattern1 {
    fn initialize_expected() -> Self {
        let mut graph = Hypergraph::default();
        let [a, b, x, y, z] = graph.insert_atoms([
            Atom::Element('a'),
            Atom::Element('b'),
            Atom::Element('x'),
            Atom::Element('y'),
            Atom::Element('z'),
        ])[..] else {
            panic!()
        };
        
        let ab = graph.insert_pattern(vec![a, b]);
        let by = graph.insert_pattern(vec![b, y]);
        let yz = graph.insert_pattern(vec![y, z]);
        let xa = graph.insert_pattern(vec![x, a]);
        let xab = graph.insert_patterns([vec![x, ab], vec![xa, b]]);
        let xaby = graph.insert_patterns([vec![xab, y], vec![xa, by]]);
        let xabyz = graph.insert_patterns([vec![xaby, z], vec![xab, yz]]);
        
        #[cfg(any(test, feature = "test-api"))]
        crate::graph::test_graph::register_test_graph(&graph);
        
        Self {
            graph: HypergraphRef::from(graph),
            a,
            b,
            x,
            y,
            z,
            ab,
            by,
            yz,
            xa,
            xab,
            xaby,
            xabyz,
        }
    }
    
    fn get_expected<'a>() -> RwLockReadGuard<'a, Self> {
        get_context_index_pattern1().read().unwrap()
    }
    fn get_expected_mut<'a>() -> RwLockWriteGuard<'a, Self> {
        get_context_index_pattern1().write().unwrap()
    }
}

fn get_context_index_pattern1() -> &'static Arc<RwLock<EnvIndexPattern1>> {
    CONTEXT_INDEX_PATTERN1.with(|cell| {
        unsafe {
            let ptr = cell.get_or_init(|| Arc::new(RwLock::new(EnvIndexPattern1::initialize_expected())));
            &*(ptr as *const Arc<RwLock<EnvIndexPattern1>>)
        }
    })
}

thread_local! {
    static CONTEXT_INDEX_PATTERN1: OnceLock<Arc<RwLock<EnvIndexPattern1>>> = OnceLock::new();
}

/// Test environment for index_pattern2 test
/// Graph with patterns: yz, xab, xyz, xabz, xabyz
pub struct EnvIndexPattern2 {
    pub graph: HypergraphRef,
    pub a: Token,
    pub b: Token,
    pub x: Token,
    pub y: Token,
    pub z: Token,
    pub yz: Token,
    pub xab: Token,
    pub xyz: Token,
    pub xabz: Token,
    pub xabyz: Token,
}

impl TestEnv for EnvIndexPattern2 {
    fn initialize_expected() -> Self {
        let mut graph = Hypergraph::default();
        let [a, b, x, y, z] = graph.insert_atoms([
            Atom::Element('a'),
            Atom::Element('b'),
            Atom::Element('x'),
            Atom::Element('y'),
            Atom::Element('z'),
        ])[..] else {
            panic!()
        };
        
        let yz = graph.insert_pattern(vec![y, z]);
        let xab = graph.insert_pattern(vec![x, a, b]);
        let xyz = graph.insert_pattern(vec![x, yz]);
        let xabz = graph.insert_pattern(vec![xab, z]);
        let xabyz = graph.insert_pattern(vec![xab, yz]);
        
        #[cfg(any(test, feature = "test-api"))]
        crate::graph::test_graph::register_test_graph(&graph);
        
        Self {
            graph: HypergraphRef::from(graph),
            a,
            b,
            x,
            y,
            z,
            yz,
            xab,
            xyz,
            xabz,
            xabyz,
        }
    }
    
    fn get_expected<'a>() -> RwLockReadGuard<'a, Self> {
        get_context_index_pattern2().read().unwrap()
    }
    fn get_expected_mut<'a>() -> RwLockWriteGuard<'a, Self> {
        get_context_index_pattern2().write().unwrap()
    }
}

fn get_context_index_pattern2() -> &'static Arc<RwLock<EnvIndexPattern2>> {
    CONTEXT_INDEX_PATTERN2.with(|cell| {
        unsafe {
            let ptr = cell.get_or_init(|| Arc::new(RwLock::new(EnvIndexPattern2::initialize_expected())));
            &*(ptr as *const Arc<RwLock<EnvIndexPattern2>>)
        }
    })
}

thread_local! {
    static CONTEXT_INDEX_PATTERN2: OnceLock<Arc<RwLock<EnvIndexPattern2>>> = OnceLock::new();
}

/// Test environment for index_infix1 test
/// Graph with patterns: yz, xxabyzw
pub struct EnvIndexInfix1 {
    pub graph: HypergraphRef,
    pub a: Token,
    pub b: Token,
    pub w: Token,
    pub x: Token,
    pub y: Token,
    pub z: Token,
    pub yz: Token,
    pub xxabyzw: Token,
}

impl TestEnv for EnvIndexInfix1 {
    fn initialize_expected() -> Self {
        let mut graph = Hypergraph::default();
        let [a, b, w, x, y, z] = graph.insert_atoms([
            Atom::Element('a'),
            Atom::Element('b'),
            Atom::Element('w'),
            Atom::Element('x'),
            Atom::Element('y'),
            Atom::Element('z'),
        ])[..] else {
            panic!()
        };
        
        let yz = graph.insert_pattern(vec![y, z]);
        let xxabyzw = graph.insert_pattern(vec![x, x, a, b, yz, w]);
        
        #[cfg(any(test, feature = "test-api"))]
        crate::graph::test_graph::register_test_graph(&graph);
        
        Self {
            graph: HypergraphRef::from(graph),
            a,
            b,
            w,
            x,
            y,
            z,
            yz,
            xxabyzw,
        }
    }
    
    fn get_expected<'a>() -> RwLockReadGuard<'a, Self> {
        get_context_index_infix1().read().unwrap()
    }
    fn get_expected_mut<'a>() -> RwLockWriteGuard<'a, Self> {
        get_context_index_infix1().write().unwrap()
    }
}

fn get_context_index_infix1() -> &'static Arc<RwLock<EnvIndexInfix1>> {
    CONTEXT_INDEX_INFIX1.with(|cell| {
        unsafe {
            let ptr = cell.get_or_init(|| Arc::new(RwLock::new(EnvIndexInfix1::initialize_expected())));
            &*(ptr as *const Arc<RwLock<EnvIndexInfix1>>)
        }
    })
}

thread_local! {
    static CONTEXT_INDEX_INFIX1: OnceLock<Arc<RwLock<EnvIndexInfix1>>> = OnceLock::new();
}

/// Test environment for index_infix2 test
/// Graph with patterns: yy, xx, xy, abcdx, yabcdx, abcdxx, xxy, xxyyabcdxxyy
pub struct EnvIndexInfix2 {
    pub graph: HypergraphRef,
    pub a: Token,
    pub b: Token,
    pub c: Token,
    pub d: Token,
    pub x: Token,
    pub y: Token,
    pub yy: Token,
    pub xx: Token,
    pub xy: Token,
    pub abcdx: Token,
    pub yabcdx: Token,
    pub abcdxx: Token,
    pub xxy: Token,
    pub xxyyabcdxxyy: Token,
}

impl TestEnv for EnvIndexInfix2 {
    fn initialize_expected() -> Self {
        let mut graph = Hypergraph::default();
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
        crate::graph::test_graph::register_test_graph(&graph);
        
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
    
    fn get_expected<'a>() -> RwLockReadGuard<'a, Self> {
        get_context_index_infix2().read().unwrap()
    }
    fn get_expected_mut<'a>() -> RwLockWriteGuard<'a, Self> {
        get_context_index_infix2().write().unwrap()
    }
}

fn get_context_index_infix2() -> &'static Arc<RwLock<EnvIndexInfix2>> {
    CONTEXT_INDEX_INFIX2.with(|cell| {
        unsafe {
            let ptr = cell.get_or_init(|| Arc::new(RwLock::new(EnvIndexInfix2::initialize_expected())));
            &*(ptr as *const Arc<RwLock<EnvIndexInfix2>>)
        }
    })
}

thread_local! {
    static CONTEXT_INDEX_INFIX2: OnceLock<Arc<RwLock<EnvIndexInfix2>>> = OnceLock::new();
}
