//! Integration tests demonstrating the Search API usage patterns
//! from SEARCH_API_EXAMPLES.md

#[cfg(test)]
use {
    crate::{
        search::Find,
        state::end::PathCoverage,
    },
    context_trace::*,
    pretty_assertions::assert_eq,
    tracing::{
        debug,
        trace,
    },
};

#[test]
fn example_basic_sequence_search() {
    // Create graph and insert atoms
    let graph = Hypergraph::<BaseGraphKind>::default();
    debug!("Created empty hypergraph");

    let a = graph.insert_atom(Atom::Element('a'));
    let b = graph.insert_atom(Atom::Element('b'));
    let c = graph.insert_atom(Atom::Element('c'));
    debug!(
        a = %pretty(&a),
        b = %pretty(&b),
        c = %pretty(&c),
        "Inserted atoms"
    );

    // Create a pattern [a, b, c]
    let abc = graph.insert_pattern([a, b, c]);
    debug!(abc = %pretty(&abc), "Created pattern [a, b, c]");

    // Search for [b, c] in the graph
    let graph = HypergraphRef::from(graph);
    // Initialize tracing for this test - log file will be cleaned up on success
    let _tracing = init_test_tracing!(&graph);
    let query = [b, c];
    debug!(query = %pretty(&query), "Searching for [b, c]");
    let result = graph.find_ancestor(&query[..]);

    // Verify we found the query pattern
    assert!(result.is_ok(), "Search should succeed");
    let response = result.unwrap();
    trace!(response = %pretty(&response), "Got response");

    // Since [b, c] was never inserted as a complete pattern, we expect a Postfix
    // (it matches indices 1-2 of the parent pattern [a, b, c])
    // But the query should be fully matched (Complete variant)
    assert!(
        response.end.query_exhausted(),
        "Query should be fully matched"
    );
    assert!(
        matches!(response.end.path(), crate::state::end::PathCoverage::Postfix(_)),
        "Path should be Postfix since [b, c] doesn't start at the beginning of [a, b, c]"
    );

    // Verify the path points to abc as the parent
    let parent = response.end.path().root_parent();
    assert_eq!(parent, abc);
}

#[test]
fn example_single_atom_error() {
    // Create graph and insert single atom
    let graph = Hypergraph::<BaseGraphKind>::default();
    let a = graph.insert_atom(Atom::Element('a'));

    // Single atoms have no parent patterns, so searching for them returns SingleIndex error
    // This is because there's no pattern containing just a single atom
    let graph = HypergraphRef::from(graph);
    let query = [a];
    let result = graph.find_ancestor(&query[..]);

    // Should return SingleIndex error - single atom has no parent pattern
    use context_trace::ErrorReason;
    assert_eq!(
        result,
        Err(ErrorReason::SingleIndex(Box::new(IndexWithPath {
            index: a,
            path: vec![a].into(),
        }))),
    );
}
#[test]
fn example_helper_methods() {
    let graph = Hypergraph::<BaseGraphKind>::default();
    let a = graph.insert_atom(Atom::Element('a'));
    let b = graph.insert_atom(Atom::Element('b'));
    let ab = graph.insert_pattern([a, b]);

    let graph = HypergraphRef::from(graph);
    let query = [a, b];
    let response = graph.find_ancestor(&query[..]).unwrap();

    // Check if complete
    assert!(response.query_exhausted());

    // Get as Option
    let opt_path = response.as_complete();
    assert!(opt_path.is_some());
    assert_eq!(opt_path.unwrap().root_pattern_location().parent, ab);
}

#[test]
fn example_token_pattern_element() {
    let graph = Hypergraph::<BaseGraphKind>::default();
    let h = graph.insert_atom(Atom::Element('h'));
    let e = graph.insert_atom(Atom::Element('e'));
    let l = graph.insert_atom(Atom::Element('l'));
    let o = graph.insert_atom(Atom::Element('o'));

    let hel = graph.insert_pattern([h, e, l]);
    let llo = graph.insert_pattern([l, l, o]);
    let _hello = graph.insert_pattern([hel, llo]);

    let graph = HypergraphRef::from(graph);
    let query = [e, l];
    let response = graph.find_ancestor(&query[..]).unwrap();

    // [e, l] matches at indices 1-2 within [h, e, l], so it's a Postfix match
    // but the query itself completed successfully (Complete variant)
    assert!(
        response.end.query_exhausted(),
        "Query should be fully matched"
    );

    // The path is Postfix since [e, l] starts at position 1 in [h, e, l]
    assert!(
        matches!(response.end.path(), crate::state::end::PathCoverage::Postfix(_)),
        "Path should be Postfix since [e, l] doesn't start at the beginning of [h, e, l]"
    );

    // Verify the path points to hel as the parent
    let parent = response.end.path().root_parent();
    assert_eq!(parent, hel);
}

#[test]
fn example_hierarchical_parent_search() {
    let graph = Hypergraph::<BaseGraphKind>::default();
    let a = graph.insert_atom(Atom::Element('a'));
    let b = graph.insert_atom(Atom::Element('b'));
    let c = graph.insert_atom(Atom::Element('c'));
    let d = graph.insert_atom(Atom::Element('d'));

    let ab = graph.insert_pattern([a, b]);
    let _bc = graph.insert_pattern([b, c]);
    let cd = graph.insert_pattern([c, d]);
    let abcd = graph.insert_pattern([ab, cd]);

    let graph = HypergraphRef::from(graph);

    // Find parent of [a, b]
    let query1 = [a, b];
    let response1 = graph.find_parent(query1).unwrap();
    assert!(response1.query_exhausted());
    assert_eq!(
        response1.unwrap_complete().root_pattern_location().parent,
        ab
    );

    // Find parent of [ab, cd]
    let query2 = [ab, cd];
    let response2 = graph.find_parent(query2).unwrap();
    assert!(response2.query_exhausted());
    assert_eq!(
        response2.unwrap_complete().root_pattern_location().parent,
        abcd
    );
}

#[test]
fn example_hierarchical_ancestor_search() {
    // This test demonstrates a hierarchical ancestor search scenario where:
    // - The graph has a single `abcd` vertex with multiple pattern compositions
    // - Pattern 1: [ab, cd] - hierarchical composition
    // - Pattern 2: [a, bc, d] - ensures reachability to the bc substring
    //
    // Expected behavior (once bug is fixed):
    // Searching for [b, c, d] should return a Postfix match:
    // - The match starts at position 1 in abcd (not at the beginning)
    // - It continues to the end through the [a, bc, d] pattern
    // - Root parent should be abcd
    // - Result should be QueryExhausted with a Postfix path
    //
    let graph = Hypergraph::<BaseGraphKind>::default();
    let a = graph.insert_atom(Atom::Element('a'));
    let b = graph.insert_atom(Atom::Element('b'));
    let c = graph.insert_atom(Atom::Element('c'));
    let d = graph.insert_atom(Atom::Element('d'));

    let ab = graph.insert_pattern([a, b]);
    let bc = graph.insert_pattern([b, c]);
    let cd = graph.insert_pattern([c, d]);

    // Create abcd with two ways to compose it:
    // 1. [ab, cd] - doesn't contain bc directly
    // 2. [a, bc, d] - contains bc and maintains reachability to bc substring
    // Both patterns are added to the same vertex
    insert_patterns!(graph,
        abcd => [[ab, cd], [a, bc, d]]
    );

    let graph = HypergraphRef::from(graph);
    // Initialize tracing for this test
    let _tracing = init_test_tracing!(&graph);

    // Search for [b, c, d] - this should find a hierarchical postfix match
    // In abcd with patterns [ab, cd] and [a, bc, d],
    // the query [b, c, d] should match via the [a, bc, d] pattern
    let query = [b, c, d];
    debug!(query = %pretty(&query), "Searching for [b, c, d]");
    let result = graph.find_ancestor(query);

    debug!(result = ?result, "Search result");

    // If the search succeeds, let's examine what we got
    if let Ok(response) = result {
        debug!(response = %pretty(&response), "Actual response");
        debug!(path = ?response.end.path(), "End path");

        // The query was matched to the end (query exhausted - QueryExhausted became Complete variant)
        // But the path is Postfix, not Complete
        assert!(
            response.end.query_exhausted(),
            "Query should be fully exhausted (Complete variant). Got query_exhausted={:?}",
            response.end.query_exhausted()
        );

        // The path should be Postfix since [b, c, d] starts at position 1 in abcd (not at the beginning)
        // not Complete (Complete means path covers entire root from start to end)
        assert!(
            matches!(response.end.path(), PathCoverage::Postfix(_)),
            "Path should be Postfix, not Complete. Got: {:?}",
            response.end.path()
        );
        match &response.end.path() {
            PathCoverage::Postfix(postfix) => {
                // The root parent should be abcd
                assert_eq!(
                    postfix.path.root_pattern_location().parent,
                    abcd,
                    "Root parent should be abcd"
                );
                // The postfix starts at position 1 in abcd (at 'bc')
                assert_eq!(
                    postfix.path.root_child_index(),
                    1,
                    "Postfix should start at position 1"
                );
            },
            _ =>
                panic!("Expected Postfix path, got: {:?}", response.end.path()),
        }
    } else {
        panic!("Search failed: {:?}", result);
    }
}

#[test]
fn example_incomplete_postfix() {
    let graph = Hypergraph::<BaseGraphKind>::default();
    let a = graph.insert_atom(Atom::Element('a'));
    let b = graph.insert_atom(Atom::Element('b'));
    let c = graph.insert_atom(Atom::Element('c'));
    let abc = graph.insert_pattern([a, b, c]);

    let graph = HypergraphRef::from(graph);

    // Search for [b, c] using find_ancestor
    let query = [b, c];
    let response = graph.find_ancestor(query).unwrap();

    // Query is fully exhausted (Complete), but path is Postfix (doesn't start at beginning)
    assert!(
        response.query_exhausted(),
        "Query should be fully exhausted"
    );
    match &response.end.path() {
        PathCoverage::Postfix(postfix) => {
            assert_eq!(postfix.entry_pos, 1.into());
            assert_eq!(postfix.path.root_pattern_location().parent, abc);
        },
        _ => panic!("Expected Postfix"),
    }
}

#[test]
fn example_incomplete_prefix() {
    let graph = Hypergraph::<BaseGraphKind>::default();
    let a = graph.insert_atom(Atom::Element('a'));
    let b = graph.insert_atom(Atom::Element('b'));
    let c = graph.insert_atom(Atom::Element('c'));
    let abc = graph.insert_pattern([a, b, c]);

    let graph = HypergraphRef::from(graph);

    // Search for [a, b] using find_ancestor
    let query = [a, b];
    let response = graph.find_ancestor(query).unwrap();

    // Query is fully exhausted (Complete), but path is Prefix (doesn't reach end of parent)
    assert!(
        response.query_exhausted(),
        "Query should be fully exhausted"
    );
    match &response.end.path() {
        PathCoverage::Prefix(prefix) => {
            assert_eq!(prefix.path.root_pattern_location().parent, abc);
        },
        _ => panic!("Expected Prefix"),
    }
}

#[test]
fn example_pattern_location_access() {
    let graph = Hypergraph::<BaseGraphKind>::default();
    let a = graph.insert_atom(Atom::Element('a'));
    let b = graph.insert_atom(Atom::Element('b'));
    let c = graph.insert_atom(Atom::Element('c'));

    let ab = graph.insert_pattern([a, b]);
    let abc = graph.insert_pattern([ab, c]);

    let graph = HypergraphRef::from(graph);

    // Search for [ab, c]
    let query = [ab, c];
    let response = graph.find_ancestor(&query[..]).unwrap();
    let path = response.unwrap_complete();

    // Access the pattern location
    let location = path.root_pattern_location();
    assert_eq!(location.parent, abc);
}

#[test]
fn example_pattern_width() {
    let graph = Hypergraph::<BaseGraphKind>::default();
    let a = graph.insert_atom(Atom::Element('a'));
    let b = graph.insert_atom(Atom::Element('b'));

    // Create pattern with multiple ways to compose [a, b]
    let _ab1 = graph.insert_pattern([a, b]);
    let _ab2 = graph.insert_pattern([a, b]);

    let graph = HypergraphRef::from(graph);

    // Search can find any of the alternatives
    let query = [a, b];
    let response = graph.find_ancestor(&query[..]).unwrap();
    assert!(response.query_exhausted());
}

#[test]
fn example_expect_complete() {
    let graph = Hypergraph::<BaseGraphKind>::default();
    let a = graph.insert_atom(Atom::Element('a'));
    let b = graph.insert_atom(Atom::Element('b'));
    let ab = graph.insert_pattern([a, b]);

    let graph = HypergraphRef::from(graph);
    let query = [a, b];
    let response = graph.find_ancestor(&query[..]).unwrap();

    // Using expect_complete with custom message
    let path = response.expect_complete("Expected complete match for [a, b]");
    assert_eq!(path.root_pattern_location().parent, ab);
}
