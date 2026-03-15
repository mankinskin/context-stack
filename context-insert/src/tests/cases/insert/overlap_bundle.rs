use crate::{
    ToInsertCtx,
    overlap::{
        OverlapBundleInput,
        PartitionOutcome,
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
    let graph = env.graph.clone();

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

    drop(graph);
}

#[test]
fn bundle_overlap_scaffold_returns_error_for_now() {
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

    let result =
        <HypergraphRef as ToInsertCtx<Token>>::bundle_overlap(&graph, input);

    assert!(
        result.is_err(),
        "scaffold implementation should return an error until path-based partition bundling is implemented"
    );
}

#[test]
fn bundle_overlap_scaffold_is_non_mutating_for_now() {
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

    let _ =
        <HypergraphRef as ToInsertCtx<Token>>::bundle_overlap(&graph, input);

    let after = graph.vertex_count();
    assert_eq!(
        before, after,
        "scaffold overlap bundling should not mutate the graph before the implementation exists"
    );
}
