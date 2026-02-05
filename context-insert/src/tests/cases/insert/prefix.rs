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
        a,
        b,
        c,
        d,
        cd,
        cd_id,
        abcdcd,
        abcdcd_id,
    } = EnvInsertPrefix1::initialize();

    let _tracing = context_trace::init_test_tracing!(&graph);

    // Expected InitInterval from search for [h, e, l, l]
    let expected_init = InitInterval {
        root: abcdcd,
        cache: build_trace_cache!(
            abcdcd => (
                BU {},
                TD { 2 => cd -> (abcdcd_id, 2) },
            ),
            cd => (
                BU {},
                TD { 2 => c -> (cd_id, 0) },
            ),
            a => (
                BU {},
                TD {},
            ),
            c => (
                BU {},
                TD { 2 },
            ),
        ),
        end_bound: 3.into(),
    };

    let abc: Token = graph.insert_init((), expected_init).expect("insert_init should succeed");
    assert_indices!(graph, ab, abcd);
    assert_patterns! {
        graph,
        ab => [[a, b]],
        abc => [[ab, c]],
        abcd => [[abc, d], [ab, cd]],
        abcdcd => [[abcd, cd]]
    };
}
