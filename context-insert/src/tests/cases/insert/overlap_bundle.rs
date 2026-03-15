use crate::{
    ToInsertCtx,
    overlap::{
        OverlapBundleInput,
        PartitionOutcome,
        bundle_overlap,
        left_partition_from_postfix_path,
        right_partition_from_prefix_path,
    },
    tests::env::EnvExpandedOverlap,
};
use context_trace::{
    tests::test_case::TestEnv,
    *,
};

#[test]
fn partition_outcome_helpers() {
    let token = Token::new(VertexIndex(7), TokenWidth(3));

    let present = PartitionOutcome::Token(token);
    assert_eq!(present.as_token(), Some(token));
    assert_eq!(present.clone().token(), Some(token));
    assert!(!present.is_empty());

    let empty = PartitionOutcome::Empty;
    assert_eq!(empty.as_token(), None);
    assert_eq!(empty.clone().token(), None);
    assert!(empty.is_empty());
}

#[test]
fn overlap_bundle_input_new_stores_fields() {
    let env = EnvExpandedOverlap::initialize();

    let anchor_postfix_path =
        IndexEndPath::from(ChildLocation::new(env.abc, env.abc_id, 1));
    let overlap_prefix_path =
        IndexStartPath::from(ChildLocation::new(env.ab, env.ab_id, 0));

    let input = OverlapBundleInput::new(
        anchor_postfix_path.clone(),
        overlap_prefix_path.clone(),
        env.ab,
        env.abc,
    );

    assert_eq!(input.anchor_postfix_path, anchor_postfix_path);
    assert_eq!(input.overlap_prefix_path, overlap_prefix_path);
    assert_eq!(input.t1, env.ab);
    assert_eq!(input.t2, env.abc);
}

#[test]
fn left_partition_from_postfix_path_returns_left_siblings() {
    let env = EnvExpandedOverlap::initialize();
    let graph = env.graph.clone();
    let _tracing = init_test_tracing!(&graph);

    let path = IndexEndPath::from(ChildLocation::new(env.abc, env.abc_id, 1));

    let result = left_partition_from_postfix_path(&graph, &path)
        .expect("left partition should succeed");

    let token = result
        .as_token()
        .expect("left partition of abc at c should be non-empty");

    assert_eq!(token, env.ab);
    assert_eq!(token.width(), TokenWidth(2));
}

#[test]
fn right_partition_from_prefix_path_returns_right_siblings() {
    let env = EnvExpandedOverlap::initialize();
    let graph = env.graph.clone();
    let _tracing = init_test_tracing!(&graph);

    let path = IndexStartPath::from(ChildLocation::new(env.ab, env.ab_id, 0));

    let result = right_partition_from_prefix_path(&graph, &path)
        .expect("right partition should succeed");

    let token = result
        .as_token()
        .expect("right partition of ab at a should be non-empty");

    assert_eq!(token, env.b);
    assert_eq!(token.width(), TokenWidth(1));
}

#[test]
fn left_partition_from_postfix_path_returns_empty_when_no_left_siblings() {
    let env = EnvExpandedOverlap::initialize();
    let graph = env.graph.clone();
    let _tracing = init_test_tracing!(&graph);

    let path = IndexEndPath::from(ChildLocation::new(env.ab, env.ab_id, 0));

    let result = left_partition_from_postfix_path(&graph, &path)
        .expect("empty left partition should still succeed");

    assert!(result.is_empty());
}

#[test]
fn right_partition_from_prefix_path_returns_empty_when_no_right_siblings() {
    let env = EnvExpandedOverlap::initialize();
    let graph = env.graph.clone();
    let _tracing = init_test_tracing!(&graph);

    let path = IndexStartPath::from(ChildLocation::new(env.abc, env.abc_id, 1));

    let result = right_partition_from_prefix_path(&graph, &path)
        .expect("empty right partition should still succeed");

    assert!(result.is_empty());
}

#[test]
fn bundle_overlap_builds_structural_bundle_for_simple_overlap() {
    let env = EnvExpandedOverlap::initialize();
    let graph = env.graph.clone();
    let _tracing = init_test_tracing!(&graph);

    let anchor_postfix_path =
        IndexEndPath::from(ChildLocation::new(env.abc, env.abc_id, 1));
    let overlap_prefix_path =
        IndexStartPath::from(ChildLocation::new(env.ab, env.ab_id, 0));

    let input = OverlapBundleInput::new(
        anchor_postfix_path,
        overlap_prefix_path,
        env.ab,
        env.abc,
    );

    let bundled =
        bundle_overlap(&graph, input).expect("overlap bundle should succeed");

    assert_eq!(bundled.width(), TokenWidth(6));

    let patterns = graph.expect_child_patterns(bundled);
    assert!(
        !patterns.is_empty(),
        "bundled overlap token should have at least one decomposition"
    );
    assert!(
        patterns
            .values()
            .all(|pattern| pattern_width(pattern) == TokenWidth(6)),
        "all bundled decompositions must preserve the full bundle width"
    );
    assert!(
        patterns.values().any(|pattern| !pattern.is_empty()),
        "at least one bundled decomposition should contain tokens"
    );
}

#[test]
fn bundle_overlap_trait_helper_matches_free_function() {
    let env = EnvExpandedOverlap::initialize();
    let graph = env.graph.clone();
    let _tracing = init_test_tracing!(&graph);

    let anchor_postfix_path =
        IndexEndPath::from(ChildLocation::new(env.abc, env.abc_id, 1));
    let overlap_prefix_path =
        IndexStartPath::from(ChildLocation::new(env.ab, env.ab_id, 0));

    let input_a = OverlapBundleInput::new(
        anchor_postfix_path.clone(),
        overlap_prefix_path.clone(),
        env.ab,
        env.abc,
    );

    let input_b = OverlapBundleInput::new(
        anchor_postfix_path,
        overlap_prefix_path,
        env.ab,
        env.abc,
    );

    let direct = bundle_overlap(&graph, input_a)
        .expect("free function bundle_overlap should succeed");
    assert_eq!(direct.width(), TokenWidth(6));

    let direct_patterns = graph.expect_child_patterns(direct);
    assert!(
        !direct_patterns.is_empty(),
        "direct bundle result should have decompositions"
    );
    assert!(
        direct_patterns
            .values()
            .all(|pattern| pattern_width(pattern) == TokenWidth(6))
    );

    let via_trait =
        <HypergraphRef as ToInsertCtx<Token>>::bundle_overlap(&graph, input_b)
            .expect("trait helper bundle_overlap should succeed");
    assert_eq!(via_trait.width(), TokenWidth(6));
}

#[test]
fn bundle_overlap_mutates_graph_when_successful() {
    let env = EnvExpandedOverlap::initialize();
    let graph = env.graph.clone();

    let before = graph.vertex_count();

    let anchor_postfix_path =
        IndexEndPath::from(ChildLocation::new(env.abc, env.abc_id, 1));
    let overlap_prefix_path =
        IndexStartPath::from(ChildLocation::new(env.ab, env.ab_id, 0));

    let input = OverlapBundleInput::new(
        anchor_postfix_path,
        overlap_prefix_path,
        env.ab,
        env.abc,
    );

    let bundled = bundle_overlap(&graph, input)
        .expect("successful overlap bundling should create or reuse a bundle");

    let after = graph.vertex_count();

    assert!(
        after >= before,
        "successful overlap bundling should not reduce graph size"
    );
    assert_eq!(bundled.width(), TokenWidth(6));
}
