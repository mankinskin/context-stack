use crate::{
    insert::ToInsertCtx,
    interval::init::InitInterval,
};
use context_search::{
    tests::env::EnvInsertPrefix1,
    *,
};
use context_trace::{
    tests::test_case::TestEnv,
    trace::has_graph::HasGraph,
    *,
};
use pretty_assertions::assert_eq;

#[test]
fn insert_prefix1() {
    // Create independent test environment
    let EnvInsertPrefix1 {
        graph,
        h,
        e,
        l,
        d,
        ld,
        ld_id,
        heldld,
        heldld_id,
    } = EnvInsertPrefix1::initialize();

    let _tracing = context_trace::init_test_tracing!(&graph);

    // Expected InitInterval from search for [h, e, l, l]
    let expected_init = InitInterval {
        root: heldld,
        cache: build_trace_cache!(
            heldld => (
                BU {},
                TD { 2 => ld -> (heldld_id, 2) },
            ),
            ld => (
                BU {},
                TD { 2 => l -> (ld_id, 0) },
            ),
            h => (
                BU {},
                TD {},
            ),
            l => (
                BU {},
                TD { 2 },
            ),
        ),
        end_bound: 3.into(),
    };

    let hel: Token = graph.insert_init((), expected_init);
    assert_indices!(graph, he, held);
    assert_patterns! {
        graph,
        he => [[h, e]],
        hel => [[he, l]],
        held => [[hel, d], [he, ld]],
        heldld => [[held, ld]]
    };
}
