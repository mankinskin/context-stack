//! Search test cases for EnvAbabab (triple repeat scenario)
//!
//! Tests the scenario from context-read's validate_triple_repeat test:
//! Graph: ab = [a, b], abab = [ab, ab], ababab = [abab, ab]
//! Key property: search for [ab, ab, ab] should find ababab, not abab
//!
//! This is a critical test for the repeat pattern bug where search
//! was returning a shorter pattern (abab) instead of the exact pattern (ababab).

use crate::{
    cursor::{
        checkpointed::Checkpointed,
        PatternCursor,
    },
    state::{
        end::PathCoverage,
        matched::{
            CheckpointedCursor,
            MatchResult,
        },
    },
    tests::{
        env::EnvAbabab,
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

/// Test case: Search for [ab, ab, ab] should find ababab as EntireRoot
///
/// This is the critical test for the triple repeat bug.
/// When searching for a pattern that exists exactly (ababab = [ab, ab, ab]),
/// we should get EntireRoot for ababab, NOT a match within abab.
pub(crate) struct SearchAbababExact;

impl TestCase for SearchAbababExact {
    type Env = EnvAbabab;

    fn name(&self) -> &'static str {
        "ababab_search_exact"
    }
}

impl SearchTestCase for SearchAbababExact {
    fn query(&self) -> Vec<Token> {
        let EnvAbabab { ab, .. } = *<Self as TestCase>::Env::get();
        vec![ab, ab, ab]
    }

    fn expected_response(&self) -> Response {
        let env = <Self as TestCase>::Env::get();
        let ab = env.ab;
        let ababab = env.ababab;
        let ababab_id = env.ababab_id;

        let query: Vec<Token> = vec![ab, ab, ab];

        Response {
            end: MatchResult {
                cursor: CheckpointedCursor::AtCheckpoint(Checkpointed::new(
                    PatternCursor {
                        path: RootedRangePath::new(
                            Pattern::from(query.clone()),
                            RolePath::new_empty(0),
                            RolePath::new_empty(2),
                        ),
                        atom_position: 6.into(), // 3 'ab' patterns * 2 atoms each
                        _state: Default::default(),
                    },
                )),
                // ababab = [abab, ab] has 2 children (indices 0 and 1)
                path: PathCoverage::EntireRoot(RootedRangePath::new(
                    PatternLocation::new(ababab, ababab_id),
                    RolePath::new_empty(0),
                    RolePath::new_empty(1), // end at index 1 (second child)
                )),
            },
            // Cache only contains patterns that were searched through,
            // not the final matched parent from parent exploration
            cache: build_trace_cache!(
                ab => (
                    BU {},
                    TD {},
                ),
            ),
        }
    }
}

/// Test case: Search for [ab, ab] should find abab as EntireRoot (not as part of ababab)
pub(crate) struct SearchAbabExact;

impl TestCase for SearchAbabExact {
    type Env = EnvAbabab;

    fn name(&self) -> &'static str {
        "ababab_search_abab_exact"
    }
}

impl SearchTestCase for SearchAbabExact {
    fn query(&self) -> Vec<Token> {
        let EnvAbabab { ab, .. } = *<Self as TestCase>::Env::get();
        vec![ab, ab]
    }

    fn expected_response(&self) -> Response {
        let env = <Self as TestCase>::Env::get();
        let ab = env.ab;
        let abab = env.abab;
        let abab_id = env.abab_id;

        let query: Vec<Token> = vec![ab, ab];

        Response {
            end: MatchResult {
                cursor: CheckpointedCursor::AtCheckpoint(Checkpointed::new(
                    PatternCursor {
                        path: RootedRangePath::new(
                            Pattern::from(query.clone()),
                            RolePath::new_empty(0),
                            RolePath::new_empty(1),
                        ),
                        atom_position: 4.into(), // 2 'ab' patterns * 2 atoms each
                        _state: Default::default(),
                    },
                )),
                path: PathCoverage::EntireRoot(RootedRangePath::new(
                    PatternLocation::new(abab, abab_id),
                    RolePath::new_empty(0),
                    RolePath::new_empty(1),
                )),
            },
            // Cache only contains patterns that were searched through,
            // not the final matched parent from parent exploration
            cache: build_trace_cache!(
                ab => (
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
    use crate::{
        search::{
            context::AncestorSearchTraversal,
            Find,
        },
        Searchable,
    };
    use context_trace::init_test_tracing;
    use pretty_assertions::assert_eq;

    /// Critical test: Searching for [ab, ab, ab] should find ababab
    ///
    /// This test exposes the bug where search was returning abab instead of ababab.
    /// The search algorithm must prefer the exact length match over partial matches.
    #[test]
    fn test_search_ababab_exact() {
        let test = SearchAbababExact;
        let query = test.query();
        let graph = <SearchAbababExact as TestCase>::Env::get().graph().clone();
        let _tracing = init_test_tracing!(&graph);

        let actual = Searchable::<AncestorSearchTraversal<_>>::search(
            query.clone(),
            graph.into(),
        )
        .expect("Search should succeed");

        // The found pattern should span 6 atoms (ababab), not 4 (abab)
        let found_width = match &actual.end.path {
            PathCoverage::EntireRoot(path) =>
                usize::from(*path.root_parent().width()),
            other => panic!("Expected EntireRoot, got {:?}", other),
        };

        assert_eq!(
            found_width, 6,
            "Should find ababab (width 6), not abab (width 4)"
        );

        // Also verify the full response matches
        let expected = test.expected_response();
        assert_eq!(actual, expected, "Response should match expected");
    }

    #[test]
    fn test_search_abab_exact() {
        let test = SearchAbabExact;
        let query = test.query();

        let actual = Searchable::<AncestorSearchTraversal<_>>::search(
            query.clone(),
            <SearchAbabExact as TestCase>::Env::get()
                .graph()
                .clone()
                .into(),
        )
        .expect("Search should succeed");

        let expected = test.expected_response();
        assert_eq!(actual, expected, "Response should match expected");
    }

    /// Test that searching for [a, b] finds the ab pattern exactly
    #[test]
    fn test_search_ab_from_atoms() {
        let env = EnvAbabab::get();
        let graph = env.graph();
        let a = env.a;
        let b = env.b;
        let ab = env.ab;
        let _ab_id = env.ab_id; // Use ab_id to silence unused field warning

        let result = graph.find_ancestor(&vec![a, b]);
        assert!(result.is_ok(), "Search for [a, b] should succeed");

        let response = result.unwrap();
        // Should find ab pattern as EntireRoot
        assert!(
            matches!(&response.end.path, PathCoverage::EntireRoot(path)
                if path.root_parent() == ab),
            "Should find [a, b] as EntireRoot matching ab, got {:?}",
            response.end.path
        );

        // Verify width is 2 (ab pattern)
        let found_width = match &response.end.path {
            PathCoverage::EntireRoot(path) =>
                usize::from(*path.root_parent().width()),
            _ => panic!("Expected EntireRoot"),
        };
        assert_eq!(found_width, 2, "Should find ab (width 2)");
    }
}
