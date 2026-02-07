//! Search test cases for EnvInsertPrefix1
//!
//! Concrete implementations of SearchTestCase trait with expected Response values.

use crate::{
    cursor::{
        checkpointed::Checkpointed,
        PatternCursor,
    },
    state::{
        end::{
            prefix::PrefixEnd,
            PathCoverage,
        },
        matched::{
            CheckpointedCursor,
            MatchResult,
        },
    },
    tests::{
        env::EnvInsertPrefix1,
        test_case::SearchTestCase,
    },
    Response,
};
use context_trace::{
    build_trace_cache,
    graph::vertex::token::Token,
    tests::test_case::{
        TestCase,
        TestEnv,
    },
    *,
};

/// Test case 1: Search for "hell" against "heldld" pattern - tests prefix matching
pub struct Prefix1;

impl TestCase for Prefix1 {
    type Env = EnvInsertPrefix1;

    fn name(&self) -> &'static str {
        "index_prefix1_search_1"
    }
}

impl SearchTestCase for Prefix1 {
    fn query(&self) -> Vec<Token> {
        let EnvInsertPrefix1 { a, b, c, .. } = *<Self as TestCase>::Env::get();
        vec![a, b, c, c]
    }

    fn expected_response(&self) -> Response {
        let env = <Self as TestCase>::Env::get();
        let query: Vec<Token> = vec![env.a, env.b, env.c, env.c];

        // Bring tokens into scope for macro
        let abcdcd = env.abcdcd;
        let cd = env.cd;
        let a = env.a;
        let c = env.c;
        let abcdcd_id = env.abcdcd_id;
        let cd_id = env.cd_id;

        Response {
            end: MatchResult {
                cursor: CheckpointedCursor::AtCheckpoint(Checkpointed::new(
                    PatternCursor {
                        path: RootedRangePath::new(
                            Pattern::from(query.clone()),
                            RolePath::new_empty(0),
                            RolePath::new_empty(2),
                        ),
                        atom_position: 3.into(),
                        _state: Default::default(),
                    },
                )),
                path: PathCoverage::Prefix(PrefixEnd {
                    path: RootedRolePath::new(
                        PatternLocation::new(abcdcd, abcdcd_id),
                        RolePath::new(
                            2,
                            vec![ChildLocation::new(cd, cd_id, 0)],
                        ),
                    ),
                    target: DownKey {
                        index: c,
                        pos: 2.into(),
                    },
                    exit_pos: 2.into(),
                    end_pos: 3.into(),
                }),
            },
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
        }
    }
}

#[cfg(test)]
use crate::{
    search::context::AncestorSearchTraversal,
    Searchable,
};
#[test]
fn test_search_1() {
    let _tracing = init_test_tracing!();
    let test = Prefix1;
    let query = test.query();

    let actual = Searchable::<AncestorSearchTraversal<_>>::search(
        query.clone(),
        <Prefix1 as TestCase>::Env::get().graph().clone().into(),
    )
    .expect("Search should succeed");

    let expected = test.expected_response();

    assert_eq!(actual, expected, "Response should match expected");
}
