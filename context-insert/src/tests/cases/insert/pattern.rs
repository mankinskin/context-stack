//! Insert test case for index_pattern1 environment
//!
//! Tests inserting patterns with overlapping structures.

use crate::{
    insert::ToInsertCtx,
    tests::{
        env::{
            EnvInsertPattern1,
            EnvInsertPattern2,
        },
        test_case::InsertTestCase,
    },
};
use context_search::legacy::*;
use context_trace::legacy::{
    GraphRoot,
    HasGraph,
    HashSet,
    Token,
    VertexSet,
    Wide,
    graph::vertex::{
        VertexIndex,
        pattern::Pattern,
    },
    tests::{
        macros::string_repr::{
            assert_all_vertices_unique,
            assert_token_string_repr,
        },
        test_case::{
            TestCase,
            TestEnv,
        },
    },
};
use pretty_assertions::assert_matches;
/// Test case: Insert "byz" into pattern1 environment
pub(crate) struct Pattern1Byz;

impl TestCase for Pattern1Byz {
    type Env = EnvInsertPattern1;

    fn name(&self) -> &'static str {
        "index_pattern1_insert_byz"
    }
}

impl InsertTestCase for Pattern1Byz {
    fn input_tokens(&self) -> Vec<Token> {
        let EnvInsertPattern1 { by, z, .. } = *Self::Env::get();
        vec![by, z]
    }

    fn expected_token(&self) -> Token {
        let EnvInsertPattern1 { .. } = *Self::Env::get();
        Token::new(VertexIndex(0), 3)
    }

    fn expected_string(&self) -> &str {
        "byz"
    }

    //fn expected_vertex_data(&self) -> VertexData {
    //    let env = Self::Env::get();
    //    let token = self.expected_token();
    //    let mut vertex = VertexData::new(token);

    //    // Add expected child pattern
    //    let pattern = Pattern::from(vec![env.by, env.z]);
    //    let pattern_id = PatternId::default();
    //    vertex.child_patterns_mut().insert(pattern_id, pattern);

    //    vertex
    //}
}

/// Test case: Insert "aby" into pattern1 environment
pub(crate) struct Pattern1Aby;

impl TestCase for Pattern1Aby {
    type Env = EnvInsertPattern1;

    fn name(&self) -> &'static str {
        "index_pattern1_insert_aby"
    }
}
impl InsertTestCase for Pattern1Aby {
    fn input_tokens(&self) -> Vec<Token> {
        let EnvInsertPattern1 { ab, y, .. } = *Self::Env::get();
        vec![ab, y]
    }

    fn expected_token(&self) -> Token {
        Token::new(VertexIndex(0), 3)
    }

    fn expected_string(&self) -> &str {
        "aby"
    }

    //fn expected_vertex_data(&self) -> VertexData {
    //    let env = Self::Env::get();
    //    let token = self.expected_token();
    //    let mut vertex = VertexData::new(token);

    //    // Add expected child pattern
    //    let pattern = Pattern::from(vec![env.ab, env.y]);
    //    let pattern_id = PatternId::default();
    //    vertex.child_patterns_mut().insert(pattern_id, pattern);

    //    vertex
    //}
}

#[test]
fn insert_pattern1() {
    // Test case 1: Insert "byz"
    let case = Pattern1Byz;
    let env = case.environment();
    let _tracing = context_trace::init_test_tracing!(env.graph());

    let query = case.input_tokens();
    let result_token: Token =
        env.graph.insert(query.clone()).expect("Indexing failed");

    // Assert the token has the expected string representation
    {
        let g = env.graph.graph();
        assert_token_string_repr(g, result_token, case.expected_string());
        assert_all_vertices_unique(g);
    }
    assert_eq!(
        result_token.width(),
        case.expected_token().width(),
        "byz should have expected width"
    );

    let found = env.graph.find_ancestor(&query);
    assert_matches!(
        found,
        Ok(ref response) if response.query_exhausted() && response.is_entire_root() && response.root_token() == result_token,
        "byz"
    );

    // Test case 2: Insert "aby"
    let case2 = Pattern1Aby;
    let query2 = case2.input_tokens();
    let result_token2: Token =
        env.graph.insert(query2.clone()).expect("Indexing failed");

    // Assert aby has the expected string representation
    {
        let g = env.graph.graph();
        assert_token_string_repr(g, result_token2, case2.expected_string());
        assert_all_vertices_unique(g);
    }

    let found2 = env.graph.find_parent(&query2);
    assert_matches!(
        found2,
        Ok(ref response) if response.query_exhausted() && response.is_entire_root() && response.root_token() == result_token2,
        "aby"
    );
}

#[test]
fn insert_pattern2() {
    // Create independent test environment
    let EnvInsertPattern2 {
        graph, a, b, x, y, ..
    } = EnvInsertPattern2::initialize();

    let _tracing = context_trace::init_test_tracing!(&graph);

    // Verify all vertices have unique string representations before insertion
    {
        let g = graph.graph();
        assert_all_vertices_unique(g);
    }

    let query = vec![a, b, y, x];
    let aby: Token = graph.insert(query.clone()).expect("Indexing failed");

    // Assert the token has the expected string representation and width
    {
        let g = graph.graph();
        assert_token_string_repr(g, aby, "aby");
        assert_all_vertices_unique(g);
    }
    assert_eq!(aby.width(), 3);

    let ab = graph
        .find_ancestor("ab".chars())
        .unwrap()
        .expect_complete("ab")
        .root_parent();
    let g = graph.graph();
    let aby_vertex = g.expect_vertex_data(aby);
    assert_eq!(aby_vertex.parents().len(), 1, "aby");
    assert_eq!(
        aby_vertex
            .child_pattern_set()
            .into_iter()
            .collect::<HashSet<_>>(),
        HashSet::from_iter([Pattern::from(vec![ab, y]),])
    );
    let query = vec![a, b, y];
    let aby_found = graph.find_ancestor(&query);
    assert_matches!(
        aby_found,
        Ok(ref response) if response.query_exhausted() && response.is_entire_root() && response.root_token() == aby,
        "aby"
    );
}

/// Nested and disjoint replacements proposed for one original pattern are
/// composed from original index ranges at publication.
#[test]
fn publish_composes_replacements_in_one_pattern() {
    use crate::join::plan::{
        JoinPlan,
        JoinRef,
        PatternOrigin,
        PlanEdit,
        ProposedPattern,
        publish,
    };
    use context_trace::legacy::{
        HypergraphRef,
        assert_parents,
        graph::vertex::parent::PatternIndex,
        insert_atoms,
    };

    let graph = HypergraphRef::default();
    insert_atoms!(graph, {a, b, c, d, e, f});
    let (root, pid) = graph.insert_pattern_with_id(vec![a, b, c, d, e, f]);
    let pid = pid.unwrap();

    let joined = |refs: Vec<JoinRef>| {
        vec![ProposedPattern {
            refs,
            origin: PatternOrigin::Joined { source: pid },
        }]
    };
    let ex = JoinRef::Existing;
    let mut plan = JoinPlan::default();
    let x = plan.propose(root, joined(vec![ex(b), ex(c)]));
    let y = plan.propose(root, joined(vec![x, ex(d)]));
    let z = plan.propose(root, joined(vec![ex(e), ex(f)]));
    // proposal order of planning: inner before outer, then a later range
    for (range, replacement) in [(1..3, x), (1..4, y), (4..6, z)] {
        plan.push_edit(PlanEdit::ReplaceRange {
            target: root,
            pattern_id: pid,
            range,
            replacement,
            origin: PatternOrigin::PerfectReplacement,
        });
    }
    plan.target = Some(y);

    let (y, _) = publish(&graph, plan);
    let g = graph.graph();

    let root_patterns = g.expect_child_patterns(root);
    assert_eq!(root_patterns.len(), 1, "no duplicate root patterns");
    let pattern = root_patterns[&pid].clone();
    let z = pattern[2];
    assert_eq!(pattern, Pattern::from(vec![a, y, z]));
    let y_patterns = g.expect_child_patterns(y);
    assert_eq!(y_patterns.len(), 1);
    let (y_pid, y_pattern) = y_patterns.into_iter().next().unwrap();
    let x = y_pattern[0];
    assert_eq!(y_pattern, Pattern::from(vec![x, d]));
    assert_eq!(
        g.expect_child_patterns(x).into_values().collect::<Vec<_>>(),
        vec![Pattern::from(vec![b, c])]
    );
    assert_eq!(
        g.expect_child_patterns(z).into_values().collect::<Vec<_>>(),
        vec![Pattern::from(vec![e, f])]
    );
    assert_eq!(
        (y.width(), x.width(), z.width(), root.width()),
        (3.into(), 2.into(), 2.into(), 6.into())
    );
    assert_eq!(g.vertex_count(), 10);

    assert_parents(g, a, root, [PatternIndex::new(pid, 0)]);
    assert_parents(g, y, root, [PatternIndex::new(pid, 1)]);
    assert_parents(g, z, root, [PatternIndex::new(pid, 2)]);
    assert_parents(g, x, y, [PatternIndex::new(y_pid, 0)]);
    assert_parents(g, d, y, [PatternIndex::new(y_pid, 1)]);
    let x_pid = *g.expect_child_patterns(x).keys().next().unwrap();
    assert_parents(g, b, x, [PatternIndex::new(x_pid, 0)]);
    assert_parents(g, c, x, [PatternIndex::new(x_pid, 1)]);
}
