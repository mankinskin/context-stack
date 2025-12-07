use crate::{
    insert::ToInsertCtx,
    interval::init::InitInterval,
};
use context_search::{
    tests::env::EnvInsertPostfix1,
    *,
};
use context_trace::{
    tests::test_case::TestEnv,
    trace::has_graph::HasGraph,
    *,
};
use pretty_assertions::assert_eq;

#[test]
fn insert_postfix1() {
    // Create independent test environment
    let EnvInsertPostfix1 {
        graph,
        a,
        b,
        c,
        d,
        ab,
        ab_id,
        ababcd,
        ababcd_id,
    } = EnvInsertPostfix1::initialize();

    let _tracing = context_trace::init_test_tracing!(&graph);

    // Expected InitInterval from search for [b, c, d, d]
    let expected_init = InitInterval {
        root: ababcd,
        cache: build_trace_cache!(
            ababcd => (
                BU { 1 => ab -> (ababcd_id, 1) },
                TD {},
            ),
            ab => (
                BU { 1 => b -> (ab_id, 1) },
                TD {},
            ),
            b => (
                BU {},
                TD {},
            ),
        ),
        end_bound: 3.into(),
    };

    let bcd: Token = graph.insert_init((), expected_init);
    assert_indices!(graph, cd, abcd);
    assert_patterns! {
        graph,
        cd => [[c, d]],
        bcd => [[b, cd]],
        abcd => [[a, bcd], [ab, cd]],
        ababcd => [[ab, abcd]]
    };
}
