//! Search test cases for EnvIndexPrefix1
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
    tests::test_case::SearchTestCase,
    Response,
};
use context_trace::{
    build_trace_cache,
    graph::vertex::token::Token,
    tests::{
        env::EnvIndexPrefix1,
        test_case::{TestCase, TestEnv},
    },
    *,
};

/// Test case 1: Search for "hell" against "heldld" pattern - tests prefix matching
pub struct Prefix1;

impl TestCase for Prefix1 {
    type Env = EnvIndexPrefix1;

    fn name(&self) -> &'static str {
        "index_prefix1_search_1"
    }
}

impl SearchTestCase for Prefix1 {
    fn query(&self) -> Vec<Token> {
        let env = <Self as TestCase>::Env::get();
        vec![env.h, env.e, env.l, env.l]
    }

    fn expected_response(&self) -> Response {
        let env = <Self as TestCase>::Env::get();
        let query: Vec<Token> = vec![env.h, env.e, env.l, env.l];

        // Bring tokens into scope for macro
        let heldld = env.heldld;
        let ld = env.ld;
        let h = env.h;
        let l = env.l;
        let heldld_id = env.heldld_id;
        let ld_id = env.ld_id;

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
                        PatternLocation::new(heldld, heldld_id),
                        RolePath::new(
                            2,
                            vec![ChildLocation::new(ld, ld_id, 0)],
                        ),
                    ),
                    target: DownKey {
                        index: l,
                        pos: 2.into(),
                    },
                    exit_pos: 2.into(),
                    end_pos: 3.into(),
                }),
            },
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
