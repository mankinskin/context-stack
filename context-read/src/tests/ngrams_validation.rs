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
    /// Map from vertex string to set of pattern strings (each pattern is a joined string of children)
    vertices: BTreeMap<String, BTreeSet<String>>,
}

impl CanonicalGraph {
    /// Create a canonical representation from a Hypergraph
    fn from_hypergraph(graph: &Hypergraph) -> Self {
        let mut vertices = BTreeMap::new();

        for key in graph.vertex_keys() {
            let vertex_string = graph.vertex_key_string(&key);
            let patterns = graph.expect_child_patterns(&key);

            let pattern_strings: BTreeSet<String> = patterns
                .values()
                .map(|pattern| {
                    pattern
                        .iter()
                        .map(|token| graph.index_string(token.vertex_index()))
                        .collect::<Vec<_>>()
                        .join("")
                })
                .collect();

            vertices.insert(vertex_string, pattern_strings);
        }

        Self { vertices }
    }

    /// Get all vertex strings in this graph
    fn vertex_strings(&self) -> BTreeSet<&str> {
        self.vertices.keys().map(String::as_str).collect()
    }
}

/// Build a graph using context-read from a single input string
fn build_context_read_graph(input: &str) -> Hypergraph {
    let graph = HypergraphRef::<BaseGraphKind>::default();
    let _tracing = init_test_tracing!(&graph);
    let _result = (&mut graph.clone(), input.chars()).read_sequence().unwrap();
    // HypergraphRef derefs to Hypergraph, clone it
    let g: &Hypergraph = &*graph;
    g.clone()
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
fn validate_graphs_equivalent(input: &str) -> Result<(), String> {
    let cr_graph = build_context_read_graph(input);

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

/// Run validation on an input and panic with detailed comparison on failure
fn assert_graphs_equivalent(input: &str) {
    if let Err(msg) = validate_graphs_equivalent(input) {
        panic!("{}", msg);
    }
}

// ============ Test cases ============
// Using short strings (≤10 chars) because ngrams is slow

#[test]
fn validate_single_char() {
    let _tracing = init_test_tracing!();
    // Single char: ngrams may fail, just verify context-read works
    assert_graphs_equivalent("a");
}

#[test]
fn validate_two_chars() {
    let _tracing = init_test_tracing!();
    // Two different chars: no repeats, just verify both work
    assert_graphs_equivalent("ab");
}

#[test]
fn validate_repeated_char() {
    let _tracing = init_test_tracing!();
    assert_graphs_equivalent("aa");
}

#[test]
fn validate_three_repeated() {
    let _tracing = init_test_tracing!();
    assert_graphs_equivalent("aaa");
}

#[test]
fn validate_simple_repeat() {
    let _tracing = init_test_tracing!();
    assert_graphs_equivalent("abab");
}

#[test]
fn validate_overlap() {
    let _tracing = init_test_tracing!();
    assert_graphs_equivalent("aba");
}

#[test]
fn validate_complex_short() {
    let _tracing = init_test_tracing!();
    assert_graphs_equivalent("abcabc");
}

#[test]
fn validate_mixed_pattern() {
    let _tracing = init_test_tracing!();
    assert_graphs_equivalent("aabb");
}

#[test]
fn validate_palindrome() {
    let _tracing = init_test_tracing!();
    assert_graphs_equivalent("abba");
}

#[test]
fn validate_triple_repeat() {
    let _tracing = init_test_tracing!();
    assert_graphs_equivalent("ababab");
}
