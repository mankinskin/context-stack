//! Search test cases for EnvXyyxy
//!
//! Tests the scenario from context-read's read_repeating_known1 test:
//! Graph: xy = [x, y], xyyxy = [xy, y, xy]
//! Key property: search for [x, y] should find xy as EntireRoot

use crate::{
    cursor::{
        checkpointed::Checkpointed,
        PatternCursor,
    },
    search::{
        context::AncestorSearchTraversal,
        Find,
    },
    state::{
        end::PathCoverage,
        matched::{
            CheckpointedCursor,
            MatchResult,
        },
    },
    tests::{
        env::EnvXyyxy,
        test_case::SearchTestCase,
    },
    Response,
    Searchable,
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

/// Test case: Search for "xy" (atoms [x, y]) should find pattern xy as EntireRoot
///
/// This verifies that when searching for a pattern that exists exactly,
/// we get EntireRoot (exact match) rather than finding it as a sub-part of xyyxy.
pub struct SearchXyExact;

impl TestCase for SearchXyExact {
    type Env = EnvXyyxy;

    fn name(&self) -> &'static str {
        "xyyxy_search_xy_exact"
    }
}

impl SearchTestCase for SearchXyExact {
    fn query(&self) -> Vec<Token> {
        let EnvXyyxy { x, y, .. } = *<Self as TestCase>::Env::get();
        vec![x, y]
    }

    fn expected_response(&self) -> Response {
        let env = <Self as TestCase>::Env::get();
        let query: Vec<Token> = vec![env.x, env.y];

        let xy = env.xy;
        let xy_id = env.xy_id;
        let x = env.x;
        let y = env.y;

        Response {
            end: MatchResult {
                cursor: CheckpointedCursor::AtCheckpoint(Checkpointed::new(
                    PatternCursor {
                        path: RootedRangePath::new(
                            Pattern::from(query.clone()),
                            RolePath::new_empty(0),
                            RolePath::new_empty(1),
                        ),
                        atom_position: 2.into(),
                        _state: Default::default(),
                    },
                )),
                path: PathCoverage::EntireRoot(RootedRangePath::new(
                    PatternLocation::new(xy, xy_id),
                    RolePath::new_empty(0),
                    RolePath::new_empty(1),
                )),
            },
            cache: build_trace_cache!(
                xy => (
                    BU {},
                    TD {},
                ),
                x => (
                    BU {},
                    TD {},
                ),
                y => (
                    BU {},
                    TD {},
                ),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_search_xy_exact() {
        let test = SearchXyExact;
        let query = test.query();

        let actual = Searchable::<AncestorSearchTraversal<_>>::search(
            query.clone(),
            <SearchXyExact as TestCase>::Env::get().graph().clone().into(),
        )
        .expect("Search should succeed");

        let expected = test.expected_response();
        assert_eq!(actual, expected, "Response should match expected");
    }

    /// Test that searching for just 'y' (which appears multiple times in xyyxy)
    /// finds it correctly
    #[test]
    fn test_search_y_alone() {
        let env = EnvXyyxy::get();
        let graph = env.graph();
        let y = env.y;

        let result = graph.find_ancestor(&vec![y]);
        assert!(result.is_ok(), "Search for 'y' should succeed");

        let response = result.unwrap();
        // y is an atom, so it should be an EntireRoot match
        assert!(
            matches!(response.end.path, PathCoverage::EntireRoot(_)),
            "Should find 'y' as EntireRoot, got {:?}",
            response.end.path
        );
    }
}
