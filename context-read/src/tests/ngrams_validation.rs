//! Validation tests comparing context-read output against ngrams reference implementation.
//!
//! The ngrams algorithm is a naive, slow algorithm that produces correct hypergraph output.
//! The context-read algorithm is a faster, more complex version that should produce the same output.
//! These tests validate that both algorithms produce equivalent graphs.
//!
//! Note: These tests may fail if context-read has bugs. The ngrams algorithm is the reference.

use crate::context::has_read_context::HasReadCtx;
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

#[test]
fn validate_mixed_pattern() {
    let graph = HypergraphRef::<BaseGraphKind>::default();
    let _tracing = init_test_tracing!(&graph);
    assert_graphs_equivalent(&graph, "aabb");
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
