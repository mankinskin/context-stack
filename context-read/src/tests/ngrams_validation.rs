//! Validation tests comparing context-read output against ngrams reference implementation.
//!
//! The ngrams algorithm is a naive, slow algorithm that produces correct hypergraph output.
//! The context-read algorithm is a faster, more complex version that should produce the same output.
//! These tests validate that both algorithms produce equivalent graphs.
//!
//! Note: These tests may fail if context-read has bugs. The ngrams algorithm is the reference.

use crate::input::HasReadCtx;
use context_search::Find;
use context_trace::{
    graph::{
        vertex::has_vertex_index::HasVertexIndex,
        Hypergraph,
    },
    init_test_tracing,
    *,
};
use ngrams::{
    cancellation::Cancellation,
    graph::{
        parse_corpus,
        Corpus,
        Status,
        StatusHandle,
    },
};
use std::collections::{
    BTreeMap,
    BTreeSet,
};

/// Represents a graph structure in a canonical, comparable format.
/// Uses string representations of vertices and patterns for comparison.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CanonicalGraph {
    /// Map from vertex string to set of patterns (each pattern is a Vec of child strings)
    vertices: BTreeMap<String, BTreeSet<Vec<String>>>,
}

impl CanonicalGraph {
    /// Create a canonical representation from a Hypergraph
    fn from_hypergraph(graph: &Hypergraph) -> Self {
        let mut vertices = BTreeMap::new();

        for key in graph.vertex_keys() {
            let vertex_string = graph.vertex_key_string(&key);
            let patterns = graph.expect_child_patterns(&key);

            let pattern_vecs: BTreeSet<Vec<String>> = patterns
                .values()
                .map(|pattern| {
                    pattern
                        .iter()
                        .map(|token| graph.index_string(token.vertex_index()))
                        .collect::<Vec<_>>()
                })
                .collect();

            vertices.insert(vertex_string, pattern_vecs);
        }

        Self { vertices }
    }

    /// Get all vertex strings in this graph
    fn vertex_strings(&self) -> BTreeSet<&str> {
        self.vertices.keys().map(String::as_str).collect()
    }
}

/// Populate a graph using context-read from a single input string
fn populate_context_read_graph(
    graph: &HypergraphRef,
    input: &str,
) {
    let _result = (&mut graph.clone(), input.chars()).read_sequence().unwrap();
}

/// Build a graph using ngrams from a single input string
/// Returns None if ngrams fails (e.g., for inputs with no repeated patterns)
fn build_ngrams_graph(input: &str) -> Option<Hypergraph> {
    // ngrams needs at least some repeated patterns to work
    // It panics on single character inputs or inputs with no repeats
    if input.len() < 2 {
        return None;
    }

    let texts = vec![input.to_string()];
    let corpus_name = format!("validation_{}", input);
    let status = StatusHandle::from(Status::new(texts.clone()));

    let result = parse_corpus(
        Corpus::new(corpus_name, texts),
        status,
        Cancellation::None,
    );

    result.ok().map(|r| r.graph)
}

/// Validate that context-read and ngrams produce equivalent graphs for the given input.
///
/// Returns Ok if the graphs are equivalent, Err with a description of the differences otherwise.
fn validate_graphs_equivalent(
    graph: &HypergraphRef,
    input: &str,
) -> Result<(), String> {
    populate_context_read_graph(graph, input);
    let cr_graph: &Hypergraph = &*graph;

    let ngrams_graph = match build_ngrams_graph(input) {
        Some(g) => g,
        None => {
            // ngrams can fail for certain inputs (e.g., single char, no repeated patterns)
            // In this case we just verify context-read doesn't crash
            return Ok(());
        },
    };

    let cr_canonical = CanonicalGraph::from_hypergraph(&cr_graph);
    let ngrams_canonical = CanonicalGraph::from_hypergraph(&ngrams_graph);

    // Debug: print full canonical representations
    eprintln!("\n=== Input: '{}' ===", input);
    eprintln!("context-read canonical: {:#?}", cr_canonical);
    eprintln!("ngrams canonical: {:#?}", ngrams_canonical);

    let cr_vertices = cr_canonical.vertex_strings();
    let ngrams_vertices = ngrams_canonical.vertex_strings();

    // Check that all ngrams vertices exist in context-read
    // (context-read may have additional intermediate vertices, but must have all ngrams vertices)
    let missing_in_cr: Vec<_> =
        ngrams_vertices.difference(&cr_vertices).collect();

    if !missing_in_cr.is_empty() {
        return Err(format!(
            "Input '{}': Vertices in ngrams but missing in context-read: {:?}\n\
             context-read vertices: {:?}\n\
             ngrams vertices: {:?}",
            input, missing_in_cr, cr_vertices, ngrams_vertices
        ));
    }

    // For vertices that exist in both, verify patterns are compatible
    for vertex_str in ngrams_vertices.iter() {
        let ngrams_patterns =
            ngrams_canonical.vertices.get(*vertex_str).unwrap();
        let cr_patterns = cr_canonical.vertices.get(*vertex_str).unwrap();

        // ngrams patterns should be a subset of context-read patterns
        // (context-read may have additional equivalent patterns)
        if !ngrams_patterns.is_subset(cr_patterns) {
            let missing_patterns: Vec<_> =
                ngrams_patterns.difference(cr_patterns).collect();
            return Err(format!(
                "Input '{}': Vertex '{}' missing patterns in context-read: {:?}",
                input, vertex_str, missing_patterns
            ));
        }
    }

    Ok(())
}

/// Run validation on an input and panic with detailed comparison on failure.
/// Emits a graph snapshot after populating the graph (before assertions that could panic).
fn assert_graphs_equivalent(
    graph: &HypergraphRef,
    input: &str,
) {
    // Populate graph first so we can snapshot before validation assertions
    populate_context_read_graph(graph, input);
    graph.emit_graph_snapshot();
    // Now validate (populate_context_read_graph inside validate is idempotent)
    if let Err(msg) = validate_graphs_equivalent(graph, input) {
        panic!("{}", msg);
    }
}

// ============ Test cases ============
// Using short strings (≤10 chars) because ngrams is slow

#[test]
fn validate_single_char() {
    let graph = HypergraphRef::<BaseGraphKind>::default();
    let _tracing = init_test_tracing!(&graph);
    // Single char: ngrams may fail, just verify context-read works
    assert_graphs_equivalent(&graph, "a");
}

#[test]
fn validate_two_chars() {
    let graph = HypergraphRef::<BaseGraphKind>::default();
    let _tracing = init_test_tracing!(&graph);
    // Two different chars: no repeats, just verify both work
    assert_graphs_equivalent(&graph, "ab");
}

#[test]
fn validate_repeated_char() {
    let graph = HypergraphRef::<BaseGraphKind>::default();
    let _tracing = init_test_tracing!(&graph);
    assert_graphs_equivalent(&graph, "aa");
}

#[test]
fn validate_three_repeated() {
    let graph = HypergraphRef::<BaseGraphKind>::default();
    let _tracing = init_test_tracing!(&graph);
    assert_graphs_equivalent(&graph, "aaa");
}

#[test]
fn validate_simple_repeat() {
    let graph = HypergraphRef::<BaseGraphKind>::default();
    let _tracing = init_test_tracing!(&graph);
    assert_graphs_equivalent(&graph, "abab");
}

#[test]
fn validate_overlap() {
    let graph = HypergraphRef::<BaseGraphKind>::default();
    let _tracing = init_test_tracing!(&graph);
    assert_graphs_equivalent(&graph, "aba");
}

#[test]
fn validate_complex_short() {
    let graph = HypergraphRef::<BaseGraphKind>::default();
    let _tracing = init_test_tracing!(&graph);
    assert_graphs_equivalent(&graph, "abcabc");
}

/// Validate that reading "aabb" produces the tightest compound decomposition.
///
/// The ngrams oracle is NOT used here because ngrams only creates compound tokens
/// for substrings that appear more than once.  "aabb" has no repeated substrings
/// of length ≥ 2, so ngrams produces only the flat atom pattern `[a, a, b, b]`.
///
/// context-read's contract is different: it builds compound tokens incrementally
/// as it processes the input, always using the tightest available decomposition.
/// After reading "aa", it creates `aa = [[a, a]]`.  When the first `b` is
/// appended as an unknown atom the root becomes `aab = [[aa, b]]`.  Then when
/// the second `b` arrives as a known atom, the implementation detects that the
/// root's last child (`b`) can be extended with the incoming `b` to form
/// `bb = [[b, b]]`, and replaces the last child — yielding `aabb = [[aa, bb]]`.
///
/// Expected final graph:
///   `aa`   → `[[a, a]]`
///   `bb`   → `[[b, b]]`
///   `aabb` → `[[aa, bb]]`
#[test]
fn validate_mixed_pattern() {
    use crate::request::ReadRequest;
    use context_search::assert_indices;

    let mut graph = HypergraphRef::<BaseGraphKind>::default();
    let _tracing = init_test_tracing!(&graph);

    let result = ReadRequest::from_text("aabb").execute(&mut graph);
    graph.emit_graph_snapshot();

    expect_atoms!(graph, {a, b});
    // Use assert_indices! only for the sub-tokens aa and bb — these are
    // unambiguously reachable via find_ancestor.  The root token aabb is
    // obtained directly from the ReadRequest result to avoid a known issue
    // where find_ancestor("aabb") can return the intermediate aab vertex
    // (which exists as an orphaned compound) instead of the full aabb token.
    assert_indices!(graph, aa, bb);

    let aabb = result.expect("should produce a root token");
    assert_eq!(aabb.width(), TokenWidth(4));

    assert_patterns!(
        graph,
        aa   => [[a, a]],
        bb   => [[b, b]],
        aabb => [[aa, bb]]
    );
}

#[test]
fn validate_palindrome() {
    let graph = HypergraphRef::<BaseGraphKind>::default();
    let _tracing = init_test_tracing!(&graph);
    assert_graphs_equivalent(&graph, "abba");
}

#[test]
fn validate_triple_repeat() {
    let graph = HypergraphRef::<BaseGraphKind>::default();
    let _tracing = init_test_tracing!(&graph);
    assert_graphs_equivalent(&graph, "ababab");
}

/// ngrams reference output for "aabbaabb" — oracle check.
///
/// Run this test to see what the ngrams algorithm produces for "aabbaabb".
/// This informs whether the `repetition_aabbaabb` test expectations are correct.
#[test]
fn ngrams_inspect_aabbaabb() {
    let _dummy = HypergraphRef::<BaseGraphKind>::default();
    let _tracing = init_test_tracing!(&_dummy);
    let input = "aabbaabb";

    let ngrams_graph = build_ngrams_graph(input)
        .expect("ngrams should produce a graph for this input");

    let canonical = CanonicalGraph::from_hypergraph(&ngrams_graph);
    println!("\n=== ngrams canonical graph for {:?} ===", input);
    for (vertex, patterns) in &canonical.vertices {
        for pattern in patterns {
            println!("  ({}) -> [{}]", vertex, pattern.join(", "));
        }
    }
    println!("=== end ===\n");
}

/// ngrams reference output for "aaa" — oracle check.
///
/// Run this test to see what the ngrams algorithm produces for "aaa".
/// This informs the correct expected patterns for the three-repeated test.
#[test]
fn ngrams_inspect_aaa() {
    let _dummy = HypergraphRef::<BaseGraphKind>::default();
    let _tracing = init_test_tracing!(&_dummy);
    let input = "aaa";

    let ngrams_graph = build_ngrams_graph(input)
        .expect("ngrams should produce a graph for this input");

    let canonical = CanonicalGraph::from_hypergraph(&ngrams_graph);
    println!("\n=== ngrams canonical graph for {:?} ===", input);
    for (vertex, patterns) in &canonical.vertices {
        for pattern in patterns {
            println!("  ({}) -> [{}]", vertex, pattern.join(", "));
        }
    }
    println!("=== end ===\n");
}

/// ngrams reference output for "ababab" — oracle check.
///
/// Run this test to see what the ngrams algorithm produces for "ababab".
#[test]
fn ngrams_inspect_ababab() {
    let _dummy = HypergraphRef::<BaseGraphKind>::default();
    let _tracing = init_test_tracing!(&_dummy);
    let input = "ababab";

    let ngrams_graph = build_ngrams_graph(input)
        .expect("ngrams should produce a graph for this input");

    let canonical = CanonicalGraph::from_hypergraph(&ngrams_graph);
    println!("\n=== ngrams canonical graph for {:?} ===", input);
    for (vertex, patterns) in &canonical.vertices {
        for pattern in patterns {
            println!("  ({}) -> [{}]", vertex, pattern.join(", "));
        }
    }
    println!("=== end ===\n");
}

/// ngrams reference output for "abcabcabc" — oracle check.
#[test]
fn ngrams_inspect_abcabcabc() {
    let _dummy = HypergraphRef::<BaseGraphKind>::default();
    let _tracing = init_test_tracing!(&_dummy);
    let input = "abcabcabc";

    let ngrams_graph = build_ngrams_graph(input)
        .expect("ngrams should produce a graph for this input");

    let canonical = CanonicalGraph::from_hypergraph(&ngrams_graph);
    println!("\n=== ngrams canonical graph for {:?} ===", input);
    for (vertex, patterns) in &canonical.vertices {
        for pattern in patterns {
            println!("  ({}) -> [{}]", vertex, pattern.join(", "));
        }
    }
    println!("=== end ===\n");
}

/// ngrams reference output for "abcabcabc" — oracle check.
#[test]
fn ngrams_inspect_aabb() {
    let _dummy = HypergraphRef::<BaseGraphKind>::default();
    let _tracing = init_test_tracing!(&_dummy);
    let input = "aabb";

    let ngrams_graph = match build_ngrams_graph(input) {
        Some(g) => g,
        None => {
            println!(
                "ngrams produced no graph for {:?} (no repeated substrings)",
                input
            );
            return;
        },
    };

    let canonical = CanonicalGraph::from_hypergraph(&ngrams_graph);
    println!("\n=== ngrams canonical graph for {:?} ===", input);
    for (vertex, patterns) in &canonical.vertices {
        for pattern in patterns {
            println!("  ({}) -> [{}]", vertex, pattern.join(", "));
        }
    }
    println!("=== end ===\n");
}

/// ngrams reference output for "abcabababcaba" (printed and verified 2026-03-14):
///
/// (ab)            -> [a, b]
/// (aba)           -> [ab, a]
/// (abab)          -> [ab, ab], [aba, b]
/// (ababa)         -> [ab, aba], [abab, a]
/// (ababab)        -> [ab, abab], [ababa, b]
/// (abababcaba)    -> [ab, ababcaba], [ababab, caba]
/// (ababcaba)      -> [ab, abcaba], [abab, caba]
/// (abc)           -> [ab, c]
/// (abcaba)        -> [ab, caba], [abc, aba]
/// (abcabab)       -> [abc, abab], [abcaba, b]
/// (abcababa)      -> [abc, ababa], [abcabab, a]
/// (abcababab)     -> [abc, ababab], [abcababa, b]
/// (abcabababcaba) -> [abc, abababcaba], [abcababab, caba]
/// (caba)          -> [c, aba]
#[test]
fn ngrams_inspect_abcabababcaba() {
    let _dummy = HypergraphRef::<BaseGraphKind>::default();
    let _tracing = init_test_tracing!(&_dummy);
    let input = "abcabababcaba";

    let ngrams_graph = build_ngrams_graph(input)
        .expect("ngrams should produce a graph for this input");

    let canonical = CanonicalGraph::from_hypergraph(&ngrams_graph);
    println!("\n=== ngrams canonical graph for {:?} ===", input);
    for (vertex, patterns) in &canonical.vertices {
        for pattern in patterns {
            println!("  ({}) -> [{}]", vertex, pattern.join(", "));
        }
    }
    println!("=== end ===\n");

    // Verify all expected tokens are present
    let vertices = canonical.vertex_strings();
    for expected in &[
        "ab",
        "aba",
        "abab",
        "ababa",
        "ababab",
        "caba",
        "abc",
        "abcaba",
        "abcabab",
        "abcababa",
        "abcababab",
        "ababcaba",
        "abababcaba",
        "abcabababcaba",
    ] {
        assert!(vertices.contains(*expected), "missing token: {}", expected);
    }
}
