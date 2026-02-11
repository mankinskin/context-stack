//! Test cases for simple, linear read requests.
//!
//! These tests verify the basic reading functionality with strings that do NOT
//! contain overlapping patterns. This means the input tokens are all unique
//! and don't share common subsequences that would trigger the expansion/overlap
//! detection mechanism.
//!
//! A "linear" string in this context means:
//! - Each character appears at most once (or if repeated, not in a pattern that overlaps)
//! - No substring of length >= 2 repeats within the string
//! - The graph structure should be a simple sequence of atoms
//!
//! These tests validate the basic cursor advancement without the complexity
//! of overlap detection.

use crate::{
    context::has_read_context::HasReadCtx,
    request::ReadRequest,
};
use context_search::*;
use context_trace::*;
use pretty_assertions::assert_eq;

/// Test reading a simple 3-character string with no repeats.
#[test]
fn linear_read_abc() {
    let mut graph = HypergraphRef::<BaseGraphKind>::default();
    let _tracing = init_test_tracing!(&graph);
    let result = ReadRequest::from_text("abc").execute(&mut graph);

    expect_atoms!(graph, {a, b, c});

    let root = result.expect("should have root");
    assert_eq!(root.width(), TokenWidth(3));

    assert_patterns!(
        graph,
        root => [[a, b, c]]
    );
}

/// Test reading a longer linear string.
#[test]
fn linear_read_unique_chars() {
    let mut graph = HypergraphRef::<BaseGraphKind>::default();
    let _tracing = init_test_tracing!(&graph);
    let result = ReadRequest::from_text("abcdefgh").execute(&mut graph);

    expect_atoms!(graph, {a, b, c, d, e, f, g, h});

    let root = result.expect("should have root");
    assert_eq!(root.width(), TokenWidth(8));

    assert_patterns!(
        graph,
        root => [[a, b, c, d, e, f, g, h]]
    );
}

/// Test reading a single character - simplest case.
#[test]
fn linear_read_single_char() {
    let mut graph = HypergraphRef::<BaseGraphKind>::default();
    let _tracing = init_test_tracing!(&graph);
    let result = ReadRequest::from_text("x").execute(&mut graph);

    let root = result.expect("should have root");
    assert_eq!(root.width(), TokenWidth(1));

    // Single character is just an atom, no pattern needed
    let g = graph.graph();
    assert!(g.expect_atom_child('x').vertex_index() == root.vertex_index());
}

/// Test reading two characters - minimal sequence case.
#[test]
fn linear_read_two_chars() {
    let mut graph = HypergraphRef::<BaseGraphKind>::default();
    let _tracing = init_test_tracing!(&graph);
    let result = ReadRequest::from_text("xy").execute(&mut graph);

    expect_atoms!(graph, {x, y});

    let root = result.expect("should have root");
    assert_eq!(root.width(), TokenWidth(2));

    assert_patterns!(
        graph,
        root => [[x, y]]
    );
}

/// Test reading empty string - edge case.
#[test]
fn linear_read_empty() {
    let mut graph = HypergraphRef::<BaseGraphKind>::default();
    let _tracing = init_test_tracing!(&graph);

    let result = ReadRequest::from_text("").execute(&mut graph);

    assert!(result.is_none(), "empty string should produce no root");
}

/// Test reading a string with unique characters including space.
#[test]
fn linear_read_with_space() {
    let mut graph = HypergraphRef::<BaseGraphKind>::default();
    let _tracing = init_test_tracing!(&graph);

    let result = ReadRequest::from_text("a b c").execute(&mut graph);

    expect_atoms!(graph, {a, b, c});
    let g = graph.graph();
    let space = g.expect_atom_child(' ');
    drop(g);

    let root = result.expect("should have root");
    assert_eq!(root.width(), TokenWidth(5));

    assert_patterns!(
        graph,
        root => [[a, space, b, space, c]]
    );
}

/// Test reading multiple linear sequences into the same graph.
#[test]
fn linear_read_multiple_sequences() {
    let mut graph = HypergraphRef::<BaseGraphKind>::default();
    let _tracing = init_test_tracing!(&graph);

    let first = ReadRequest::from_text("abc").execute(&mut graph);
    let second = ReadRequest::from_text("xyz").execute(&mut graph);

    expect_atoms!(graph, {a, b, c, x, y, z});

    let first_root = first.expect("first should have root");
    let second_root = second.expect("second should have root");

    assert_eq!(first_root.width(), TokenWidth(3));
    assert_eq!(second_root.width(), TokenWidth(3));

    // The roots should be different since the sequences don't overlap
    assert_ne!(first_root.vertex_index(), second_root.vertex_index());

    assert_patterns!(
        graph,
        first_root => [[a, b, c]],
        second_root => [[x, y, z]]
    );
}

/// Test that reading the same linear sequence twice returns the same root.
#[test]
fn linear_read_same_sequence_twice() {
    let mut graph = HypergraphRef::<BaseGraphKind>::default();
    let _tracing = init_test_tracing!(&graph);

    let first = ReadRequest::from_text("abc").execute(&mut graph);
    let second = ReadRequest::from_text("abc").execute(&mut graph);

    let first_root = first.expect("first should have root");
    let second_root = second.expect("second should have root");

    // Reading the same sequence should return the same root
    assert_eq!(first_root.vertex_index(), second_root.vertex_index());
}

/// Test reading numbers (linear sequence of digit characters).
#[test]
fn linear_read_digits() {
    let mut graph = HypergraphRef::<BaseGraphKind>::default();
    let _tracing = init_test_tracing!(&graph);

    let result = ReadRequest::from_text("12345").execute(&mut graph);

    let root = result.expect("should have root");
    assert_eq!(root.width(), TokenWidth(5));

    let g = graph.graph();
    let d1 = g.expect_atom_child('1');
    let d2 = g.expect_atom_child('2');
    let d3 = g.expect_atom_child('3');
    let d4 = g.expect_atom_child('4');
    let d5 = g.expect_atom_child('5');
    drop(g);

    assert_patterns!(
        graph,
        root => [[d1, d2, d3, d4, d5]]
    );
}

/// Test reading special characters (all unique).
#[test]
fn linear_read_special_chars() {
    let mut graph = HypergraphRef::<BaseGraphKind>::default();
    let _tracing = init_test_tracing!(&graph);
    let result = ReadRequest::from_text("!@#$%").execute(&mut graph);

    let root = result.expect("should have root");
    assert_eq!(root.width(), TokenWidth(5));

    let g = graph.graph();
    let c1 = g.expect_atom_child('!');
    let c2 = g.expect_atom_child('@');
    let c3 = g.expect_atom_child('#');
    let c4 = g.expect_atom_child('$');
    let c5 = g.expect_atom_child('%');
    drop(g);

    assert_patterns!(
        graph,
        root => [[c1, c2, c3, c4, c5]]
    );
}

/// Test cursor advancement verification.
/// After reading, the cursor should have advanced past all tokens.
#[test]
fn linear_read_cursor_advancement() {
    use crate::context::ReadCtx;

    let mut graph = HypergraphRef::<BaseGraphKind>::default();
    let _tracing = init_test_tracing!(&graph);

    let input = "abcd";

    // Use ReadCtx directly to observe cursor behavior
    let mut ctx = ReadCtx::new(graph.clone(), input.chars());
    let result = ctx.read_sequence();

    let root = result.expect("should have root");
    assert_eq!(root.width(), TokenWidth(4));

    // Verify the root is correctly stored in the context
    assert!(ctx.root_token().is_some());
    assert_eq!(ctx.root_token().unwrap(), root);
}

/// Test reading a sequence that uses each letter exactly once (pangram-like but shorter).
#[test]
fn linear_read_no_letter_repeats() {
    let mut graph = HypergraphRef::<BaseGraphKind>::default();
    let _tracing = init_test_tracing!(&graph);

    // A short sequence where no character repeats
    let result = ReadRequest::from_text("qwerty").execute(&mut graph);

    expect_atoms!(graph, {q, w, e, r, t, y});

    let root = result.expect("should have root");
    assert_eq!(root.width(), TokenWidth(6));

    assert_patterns!(
        graph,
        root => [[q, w, e, r, t, y]]
    );
}

/// Test that a linear read produces a single pattern (no alternative decompositions).
#[test]
fn linear_read_single_pattern_only() {
    let mut graph = HypergraphRef::<BaseGraphKind>::default();
    let _tracing = init_test_tracing!(&graph);
    let result = ReadRequest::from_text("wxyz").execute(&mut graph);

    let root = result.expect("should have root");

    // For a linear read with no overlaps, there should be exactly one pattern
    let g = graph.graph();
    let patterns = g.expect_child_patterns(root.vertex_index());
    assert_eq!(
        patterns.len(),
        1,
        "linear read should produce exactly one pattern"
    );
}

/// Test using the builder pattern for ReadRequest.
#[test]
fn linear_read_with_builder() {
    use crate::request::ReadRequestBuilder;

    let mut graph = HypergraphRef::<BaseGraphKind>::default();
    let _tracing = init_test_tracing!(&graph);

    let request = ReadRequestBuilder::default()
        .text("builder")
        .build()
        .expect("should build successfully");

    let result = request.execute(&mut graph);
    let root = result.expect("should have root");
    assert_eq!(root.width(), TokenWidth(7));
}

// =============================================================================
// Non-overlapping repetition tests
// =============================================================================
// These tests have repeated patterns but the repetitions don't overlap with
// each other (i.e., they are separated or adjacent, not sharing characters).

/// Test "abab" - repeated "ab" pattern, adjacent but not overlapping.
#[test]
fn repetition_abab() {
    let mut graph = HypergraphRef::<BaseGraphKind>::default();
    let _tracing = init_test_tracing!(&graph);
    let result = ReadRequest::from_text("abab").execute(&mut graph);

    expect_atoms!(graph, {a, b});
    assert_indices!(graph, ab);

    let root = result.expect("should have root");
    assert_eq!(root.width(), TokenWidth(4));

    // "abab" should be decomposed as [ab, ab]
    assert_patterns!(
        graph,
        ab => [[a, b]],
        root => [[ab, ab]]
    );
}

/// Test "abcxyzabc" - "abc" repeated with different content in between.
#[test]
fn repetition_abcxyzabc() {
    let _tracing = init_test_tracing!();
    let mut graph = HypergraphRef::<BaseGraphKind>::default();
    let _tracing = init_test_tracing!(&graph);
    let result = ReadRequest::from_text("abcxyzabc").execute(&mut graph);

    expect_atoms!(graph, {a, b, c, x, y, z});
    assert_indices!(graph, abc);

    let root = result.expect("should have root");
    assert_eq!(root.width(), TokenWidth(9));

    // "abcxyzabc" should use the "abc" pattern twice
    assert_patterns!(
        graph,
        abc => [[a, b, c]],
        root => [[abc, x, y, z, abc]]
    );
}

/// Test "aabbaabb" - "aabb" repeated twice, adjacent.
#[test]
fn repetition_aabbaabb() {
    let mut graph = HypergraphRef::<BaseGraphKind>::default();
    let _tracing = init_test_tracing!(&graph);
    let result = ReadRequest::from_text("aabbaabb").execute(&mut graph);

    expect_atoms!(graph, {a, b});
    assert_indices!(graph, aa, bb, aabb);

    let root = result.expect("should have root");
    assert_eq!(root.width(), TokenWidth(8));

    // "aabbaabb" should be decomposed as [aabb, aabb]
    assert_patterns!(
        graph,
        aa => [[a, a]],
        bb => [[b, b]],
        aabb => [[aa, bb]],
        root => [[aabb, aabb]]
    );
}

/// Test "abcabc" - "abc" repeated twice, adjacent.
#[test]
fn repetition_abcabc() {
    let mut graph = HypergraphRef::<BaseGraphKind>::default();
    let _tracing = init_test_tracing!(&graph);
    let result = ReadRequest::from_text("abcabc").execute(&mut graph);

    expect_atoms!(graph, {a, b, c});
    assert_indices!(graph, abc);

    let root = result.expect("should have root");
    assert_eq!(root.width(), TokenWidth(6));

    // "abcabc" should be decomposed as [abc, abc]
    assert_patterns!(
        graph,
        abc => [[a, b, c]],
        root => [[abc, abc]]
    );
}
/// Test "abXXab" - "ab" repeated with different content between.
#[test]
#[allow(non_snake_case)]
fn repetition_ab_separated() {
    let mut graph = HypergraphRef::<BaseGraphKind>::default();
    let _tracing = init_test_tracing!(&graph);
    let result = ReadRequest::from_text("abXXab").execute(&mut graph);

    expect_atoms!(graph, {a, b, X});

    let abXXab = result.expect("should have root");
    assert_eq!(abXXab.width(), TokenWidth(6));
    assert_indices!(graph, ab, abXXab);

    assert_patterns!(
        graph,
        ab => [[a, b]],
        abXXab => [[ab, X, X, ab]]
    );
}

/// Test "helloXXXhello" - longer pattern repeated with separator.
#[test]
#[allow(non_snake_case)]
fn repetition_hello_separated() {
    let mut graph = HypergraphRef::<BaseGraphKind>::default();
    let _tracing = init_test_tracing!(&graph);
    let result = ReadRequest::from_text("helloXXhello").execute(&mut graph);

    expect_atoms!(graph, {h, e, l, o, X});

    let helloXXhello = result.expect("should have root");
    assert_eq!(helloXXhello.width(), TokenWidth(12));

    assert_indices!(graph, hello, helloXXhello);
    assert_patterns!(
        graph,
        hello => [[h, e, l, l, o]],
        helloXXhello => [[hello, X, X, hello]]
    );
}
