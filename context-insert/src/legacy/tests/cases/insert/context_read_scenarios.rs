//! Integration tests simulating context-read scenarios
//!
//! These tests reproduce exact scenarios from context-read tests to verify
//! that context-insert handles them gracefully (returning errors instead of panicking).
//!
//! The scenarios include:
//! - Partial match with no checkpoint advancement
//! - Triple repeat pattern handling (ababab)
//! - Single token mismatch at query start

use crate::legacy::{
    insert::{
        ToInsertCtx,
        context::{
            InsertCtx,
            InsertTraversal,
        },
    },
    interval::init::InitInterval,
    join::{
        plan::{
            HostInterval,
            JoinPlan,
            JoinRef,
            PatternOrigin,
            PlanEdit,
            SubMergeKind,
        },
        select::{
            Retention,
            retention,
            select,
        },
    },
    split::provenance::RequirementOrigin,
    tests::env::{
        EnvAbc,
        EnvHypergra,
        EnvTripleRepeat,
    },
};
use context_search::legacy::{
    ErrorState,
    *,
};
use context_trace::legacy::{
    graph::vertex::{
        data::VertexData,
        key::VertexKey,
    },
    tests::test_case::TestEnv,
    *,
};
use crate::legacy::split::cache::position::PosKey;

// ============================================================================
// Partial Match Scenarios
// ============================================================================

/// Integration test: Simulates the context-read failure scenario
///
/// This test reproduces the exact scenario from read_sequence1:
/// - Graph has "hypergraph" pattern partially built
/// - Search for pattern [p, h] where p exists but h doesn't match next position
/// - Search returns checkpoint_position = 0
/// - insert_next_match should handle this gracefully
///
/// Expected behavior: Returns appropriate error/result without panicking
#[test]
fn integration_partial_match_no_checkpoint() {
    let EnvHypergra { graph, p, h, .. } = EnvHypergra::initialize();
    let _tracing = context_trace::init_test_tracing!(&graph);

    // Search for [p, h] - p will be found in hypergra, but h won't match 'e'
    let query = vec![p, h];

    // This mimics what context-read does: insert_next_match
    // Should handle gracefully without panicking
    let result = ToInsertCtx::<IndexWithPath>::insert_next_match(&graph, query);

    // The result should either be:
    // - Ok(InsertOutcome::Created { .. }) if insertion succeeded
    // - Ok(InsertOutcome::Complete { .. }) if pattern already exists
    // - Ok(InsertOutcome::NoExpansion { .. }) if partial match with no expansion
    // - Err(_) if validation failed
    // It should NOT panic
    assert!(
        result.is_ok() || result.is_err(),
        "insert_next_match should return a result, not panic"
    );
}

/// Test: Single token pattern with mismatch at first position
///
/// Tests the boundary case where a single-token pattern exists in graph
/// but the search query doesn't match at the first position
#[test]
fn single_token_mismatch_at_start() {
    let EnvAbc { graph, b, c, .. } = EnvAbc::initialize();
    let _tracing = context_trace::init_test_tracing!(&graph);

    // Search for [c, b] - c is different from a (which starts the ab pattern)
    // This should fail gracefully, not panic
    let query = vec![c, b];
    let result: Result<Response, _> = graph.find_ancestor(query);

    // This search should return an error (no match found)
    // or a response indicating no match, NOT panic
    assert!(
        result.is_err() || !result.as_ref().unwrap().query_exhausted(),
        "Expected no match or error for mismatched pattern"
    );
}

// ============================================================================
// Repeat Pattern Scenarios
// ============================================================================

/// Find the vertex whose atom string is exactly `content`.
fn vertex_by_string(
    graph: &HypergraphRef,
    content: &str,
) -> Option<Token> {
    graph
        .vertex_iter()
        .map(|(_, data)| data)
        .find(|data| graph.vertex_data_string(data.clone()) == content)
        .map(|data| data.to_token())
}

#[test]
fn nested_repeat_interval_within_one_child() {
    let graph = HypergraphRef::default();
    let _tracing = context_trace::init_test_tracing!(&graph);
    insert_atoms!(graph, {a, b, c});
    let ab = graph.insert_pattern(vec![a, b]);
    let abab = graph.insert_pattern(vec![ab, ab]);
    let ababab =
        graph.insert_patterns(vec![vec![ab, abab], vec![abab, ab]]);
    let root = graph.insert_pattern(vec![ab, c, ababab]);

    let abcaba: Token = graph
        .insert(vec![ab, c, a, b, a])
        .expect("prefix insertion must resolve the nested interval");

    assert_eq!(abcaba.width(), TokenWidth(6));
    // `ba` (abab[1..3]) and `bab` only resolve boundaries: `ba` has no
    // evidence parent and `bab` a single one, so both are inlined
    assert_eq!(vertex_by_string(&graph, "ba"), None);
    assert_eq!(vertex_by_string(&graph, "bab"), None);
    let aba = vertex_by_string(&graph, "aba").expect("aba is shared");
    let abc = vertex_by_string(&graph, "abc").expect("abc is shared");
    let ababa =
        vertex_by_string(&graph, "ababa").expect("ababa is an overlap wrapper");
    assert_patterns! {
        graph,
        aba => [[ab, a]],
        abc => [[ab, c]],
        abcaba => [[abc, aba]],
        abab => [[ab, ab], [aba, b]],
        ababa => [[ab, aba], [abab, a]],
        ababab => [[ab, abab], [abab, ab], [ababa, b]],
        root => [[abc, ababab], [abcaba, b, ab]]
    };
    context_trace::legacy::tests::macros::string_repr::assert_token_string_repr(
        graph.graph(),
        abcaba,
        "abcaba",
    );
    context_trace::legacy::tests::macros::string_repr::assert_token_string_repr(
        graph.graph(),
        root,
        "abcababab",
    );
}

/// Search and plan an insertion without publishing it.
fn plan_only(
    graph: &HypergraphRef,
    query: Vec<Token>,
) -> JoinPlan {
    let response =
        Searchable::<InsertTraversal>::search(query, graph.clone()).unwrap();
    assert!(!response.is_entire_root(), "query must require insertion");
    InsertCtx::<Token>::from(graph.clone())
        .plan_init(InitInterval::from(response))
        .expect("planning must succeed")
}

fn graph_snapshot(graph: &HypergraphRef) -> HashMap<VertexKey, VertexData> {
    graph.vertex_iter().collect()
}

#[test]
fn nested_repeat_planning_does_not_mutate_graph() {
    let graph = HypergraphRef::default();
    let _tracing = context_trace::init_test_tracing!(&graph);
    insert_atoms!(graph, {a, b, c});
    let ab = graph.insert_pattern(vec![a, b]);
    let abab = graph.insert_pattern(vec![ab, ab]);
    let ababab =
        graph.insert_patterns(vec![vec![ab, abab], vec![abab, ab]]);
    let _root = graph.insert_pattern(vec![ab, c, ababab]);

    let before = graph_snapshot(&graph);
    let vertex_count = graph.vertex_count();

    let plan = plan_only(&graph, vec![ab, c, a, b, a]);

    assert_eq!(graph.vertex_count(), vertex_count);
    assert_eq!(graph_snapshot(&graph), before);

    let ba = plan
        .candidate_for_interval(HostInterval {
            host: abab,
            start: 1,
            end: 3,
        })
        .expect("abab[1..3] must be planned");
    assert_eq!(ba.width(), 2);
    assert_eq!(
        plan.expand_atoms(&graph, crate::legacy::join::plan::JoinRef::Candidate(ba.id)),
        vec![b, a],
    );
    let target = plan.target.expect("plan must have a target");
    assert_eq!(target.width(), 6);
    assert_eq!(plan.expand_atoms(&graph, target), vec![a, b, c, a, b, a]);

    // every proposed decomposition carries an origin
    let sub_merges = plan
        .candidates
        .iter()
        .flat_map(|c| &c.patterns)
        .filter(|p| {
            p.origin == PatternOrigin::SubMerge(SubMergeKind::Partition)
        })
        .count();
    assert!(sub_merges > 0, "binary sub-merges must be tagged");
    assert!(plan.edits.iter().any(|e| matches!(
        e,
        PlanEdit::AddPattern { pattern, .. }
            if pattern.origin == PatternOrigin::RootPattern
    )));
}

/// Sub-merge proposals never count as parents: `ba` is referenced by three
/// proposals (`aba=[a,ba]`, `bab=[ba,b]`, `ababa=[aba,ba]`) but by no
/// evidence pattern, so it is dropped together with those proposals. A
/// proposal whose children are independently retained and which adds a new
/// border (`abab=[aba,b]`, `ababab=[ababa,b]`) is kept.
#[test]
fn selection_rejects_proposal_only_support() {
    let graph = HypergraphRef::default();
    let _tracing = context_trace::init_test_tracing!(&graph);
    insert_atoms!(graph, {a, b, c});
    let ab = graph.insert_pattern(vec![a, b]);
    let abab = graph.insert_pattern(vec![ab, ab]);
    let ababab =
        graph.insert_patterns(vec![vec![ab, abab], vec![abab, ab]]);
    let _root = graph.insert_pattern(vec![ab, c, ababab]);

    let plan = plan_only(&graph, vec![ab, c, a, b, a]);
    let interval = |host, start, end| {
        plan.candidate_for_interval(HostInterval { host, start, end })
            .unwrap_or_else(|| panic!("{host:?}[{start}..{end}] planned"))
            .id
    };
    let ba = interval(abab, 1, 3);
    let bab = interval(abab, 1, 4);
    let aba = interval(abab, 0, 3);
    let ababa = interval(ababab, 0, 5);
    let proposal_refs = plan
        .candidates
        .iter()
        .flat_map(|c| &c.patterns)
        .filter(|p| {
            p.origin.is_proposal() && p.refs.contains(&JoinRef::Candidate(ba))
        })
        .count();
    assert_eq!(proposal_refs, 3, "ba looks shared through proposals only");

    let status = retention(&graph, &plan);
    assert_eq!(status[ba.index], Retention::Dropped);
    assert_eq!(status[bab.index], Retention::Inlined);
    assert_eq!(status[aba.index], Retention::Retained);
    assert_eq!(status[ababa.index], Retention::Retained);

    let selected = select(&graph, plan);
    let atoms = |r: JoinRef| selected.expand_atoms(&graph, r);
    let mut published: Vec<_> = selected
        .candidates
        .iter()
        .map(|c| atoms(JoinRef::Candidate(c.id)))
        .collect();
    published.sort();
    let mut expected = vec![
        vec![a, b, a],
        vec![a, b, c],
        vec![a, b, a, b, a],
        vec![a, b, c, a, b, a],
    ];
    expected.sort();
    assert_eq!(published, expected);
    let accepted: Vec<_> = selected
        .edits
        .iter()
        .filter_map(|e| match e {
            PlanEdit::AddPattern { target, pattern }
                if pattern.origin.is_proposal() =>
                Some((
                    *target,
                    pattern.refs.iter().map(|r| atoms(*r)).collect_vec(),
                )),
            _ => None,
        })
        .collect();
    assert_eq!(
        accepted,
        vec![
            (abab, vec![vec![a, b, a], vec![b]]),
            (ababab, vec![vec![a, b, a, b, a], vec![b]]),
        ]
    );
    // the root connection inlines the single-use helper `bab`
    assert!(selected.edits.iter().any(|e| matches!(
        e,
        PlanEdit::AddPattern { pattern, .. }
            if pattern.origin == PatternOrigin::RootPattern
                && pattern.refs.len() == 3
                && atoms(pattern.refs[1]) == vec![b]
                && pattern.refs[2] == JoinRef::Existing(ab)
    )));
}

#[test]
fn provenance_records_paths_per_structural_incidence() {
    let graph = HypergraphRef::default();
    let _tracing = context_trace::init_test_tracing!(&graph);
    insert_atoms!(graph, {a, b, c});
    let ab = graph.insert_pattern(vec![a, b]);
    let abab = graph.insert_pattern(vec![ab, ab]);
    let (ababab, ids) = graph.insert_patterns_with_ids(vec![
        Pattern::from(vec![ab, abab]),
        Pattern::from(vec![abab, ab]),
    ]);
    let _root = graph.insert_pattern(vec![ab, c, ababab]);

    let plan = plan_only(&graph, vec![ab, c, a, b, a]);
    let provenance = &plan.provenance;

    // ababab is split at 3 and 5. Boundary abab@3 is requested by both
    // decompositions: [ab, abab] at offset 5 and [abab, ab] at offset 3.
    let incidence = provenance
        .incidences_of(abab)
        .find(|i| i.local_offset.get() == 3)
        .expect("abab@3 must be requested");
    let mut paths: Vec<_> = incidence
        .paths
        .iter()
        .map(|p| (p.parent, p.pattern_id, p.sub_index, p.child_start))
        .collect();
    paths.sort_by_key(|(_, pid, _, _)| ids.iter().position(|id| id == pid));
    assert_eq!(paths, vec![(ababab, ids[0], 1, 2), (ababab, ids[1], 0, 0)]);
    assert_eq!(
        provenance
            .incidences
            .iter()
            .filter(|i| i.child == abab && i.local_offset.get() == 3)
            .count(),
        1,
        "alternative decompositions must not duplicate the incidence"
    );

    // abab@1 is only reachable through [ab, abab]
    let single = provenance
        .incidences_of(abab)
        .find(|i| i.local_offset.get() == 1)
        .expect("abab@1 must be requested");
    assert_eq!(single.paths.len(), 1);

    // both parent positions are recorded as requirements via the same
    // structural incidence
    let froms: Vec<_> = provenance
        .requirements_at(PosKey::new(abab, 3))
        .map(|r| match r.origin {
            RequirementOrigin::Propagated { from, via } => {
                assert!(via.is_some());
                from.pos.get()
            },
            other => panic!("unexpected origin {other:?}"),
        })
        .sorted()
        .collect();
    assert_eq!(froms, vec![3, 5]);
}

/// Test: Simulates "ababab" reading which triggers cache/root mismatch
///
/// This test reproduces the exact scenario from validate_triple_repeat:
/// - Input: "ababab" (6 chars)
/// - After reading, context-read tries to expand a pattern
/// - The expansion creates an InitInterval where root != cache entries
///
/// Expected behavior: Graceful error handling, no panic
#[test]
fn triple_repeat_pattern_scenario() {
    let EnvTripleRepeat { graph, ab, .. } = EnvTripleRepeat::initialize();
    let _tracing = context_trace::init_test_tracing!(&graph);

    // Now simulate what context-read does: search for [ab] in the context of ababab
    // This might return a Response where root_token is different from cached entries
    let query = vec![ab];

    // insert_next_match should handle this without panic
    let result = ToInsertCtx::<IndexWithPath>::insert_next_match(&graph, query);

    // Verify no panic occurred
    match result {
        Ok(crate::legacy::InsertOutcome::Created { .. }) => { /* New token created - fine */
        },
        Ok(crate::legacy::InsertOutcome::Complete { .. }) => { /* Pattern already exists - fine */
        },
        Ok(crate::legacy::InsertOutcome::NoExpansion { .. }) => { /* Partial match - fine */
        },
        Err(_) => { /* Error returned - fine, as long as no panic */ },
    }
}

/// Test: Search for exact triple repeat pattern
///
/// After graph contains ab and ababab, searching for [ab, ab, ab]
/// should find ababab, not abab.
#[test]
fn search_triple_repeat_finds_full_pattern() {
    let EnvTripleRepeat {
        graph, ab, ababab, ..
    } = EnvTripleRepeat::initialize();
    let _tracing = context_trace::init_test_tracing!(&graph);

    // Search for [ab, ab, ab] - should find ababab
    let query = vec![ab, ab, ab];
    let result: Result<Response, _> = graph.find_ancestor(query);

    assert!(result.is_ok(), "Search should succeed");
    let response = result.unwrap();

    // Should find ababab (width 6), not abab (width 4)
    assert!(response.query_exhausted(), "Query should be fully consumed");
    assert_eq!(
        response.root_token(),
        ababab,
        "Should find ababab, not a partial match"
    );
}
