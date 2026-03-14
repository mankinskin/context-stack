//! Test cases for read requests with overlapping patterns.
//!
//! These tests verify reading functionality with strings that contain
//! overlapping patterns - substrings that repeat and share common subsequences
//! that trigger the expansion/overlap detection mechanism.
//!
//! An "overlapping" string in this context means:
//! - Patterns repeat 3+ times adjacently (e.g., "abcabcabc")
//! - The overlap detection needs to recognize that repeated patterns
//!   can be combined using the overlap (e.g., "abc" overlapping with "abcabc")
//!
//! These tests validate the overlap expansion algorithm.

use crate::request::ReadRequest;
use context_search::*;
use context_trace::*;
use pretty_assertions::assert_eq;

/// Test "abcabcabc" - "abc" repeated three times, requires overlap detection.
///
/// With overlap: "abcabcabc" = "abc" + "abcabc" where "abc" overlaps
/// Structure: [abc, abcabc] or [abcabc, abc]
#[test]
fn repetition_abcabcabc() {
    let mut graph = HypergraphRef::<BaseGraphKind>::default();
    let _tracing = init_test_tracing!(&graph);
    let result = ReadRequest::from_text("abcabcabc").execute(&mut graph);
    graph.emit_graph_snapshot();

    expect_atoms!(graph, {a, b, c});
    assert_indices!(graph, abc, abcabc);

    let root = result.expect("should have root");
    assert_eq!(root.width(), TokenWidth(9));

    // "abcabcabc" with overlap should be [abc, abcabc] or [abcabc, abc]
    // where abcabc = [abc, abc] with overlap
    assert_patterns!(
        graph,
        abc => [[a, b, c]],
        abcabc => [[abc, abc]],
        root => [[abcabc, abc], [abc, abcabc]]
    );
}

/// Test "xyzxyzxyz" - "xyz" repeated three times, requires overlap detection.
#[test]
fn repetition_xyzxyzxyz() {
    let mut graph = HypergraphRef::<BaseGraphKind>::default();
    let _tracing = init_test_tracing!(&graph);
    let result = ReadRequest::from_text("xyzxyzxyz").execute(&mut graph);
    graph.emit_graph_snapshot();

    expect_atoms!(graph, {x, y, z});
    assert_indices!(graph, xyz, xyzxyz);

    let root = result.expect("should have root");
    assert_eq!(root.width(), TokenWidth(9));

    // "xyzxyzxyz" with overlap should be [xyz, xyzxyz] or [xyzxyz, xyz]
    assert_patterns!(
        graph,
        xyz => [[x, y, z]],
        xyzxyz => [[xyz, xyz]],
        root => [[xyzxyz, xyz], [xyz, xyzxyz]]
    );
}

/// Test "abcabababcaba" - complex overlapping pattern with mixed repeats.
///
/// Correct structure derived from the ngrams reference algorithm:
/// (ab)            -> [a, b]
/// (aba)           -> [ab, a]
/// (abab)          -> { [ab, ab], [aba, b] }
/// (ababa)         -> { [ab, aba], [abab, a] }
/// (ababab)        -> { [ab, abab], [ababa, b] }
/// (caba)          -> [c, aba]
/// (abc)           -> [ab, c]
/// (abcaba)        -> { [ab, caba], [abc, aba] }
/// (abcabab)       -> { [abc, abab], [abcaba, b] }
/// (abcababa)      -> { [abc, ababa], [abcabab, a] }
/// (abcababab)     -> { [abc, ababab], [abcababa, b] }
/// (ababcaba)      -> { [ab, abcaba], [abab, caba] }
/// (abababcaba)    -> { [ab, ababcaba], [ababab, caba] }
/// (abcabababcaba) -> { [abc, abababcaba], [abcababab, caba] }
#[test]
fn complex_abcabababcaba() {
    let mut graph = HypergraphRef::<BaseGraphKind>::default();
    let _tracing = init_test_tracing!(&graph);
    let result = ReadRequest::from_text("abcabababcaba").execute(&mut graph);
    graph.emit_graph_snapshot();

    expect_atoms!(graph, {a, b, c});
    assert_indices!(
        graph,
        ab, aba, abab, ababa, ababab,
        caba, abc, abcaba, abcabab, abcababa, abcababab,
        ababcaba, abababcaba
    );

    let root = result.expect("should have root");
    assert_eq!(root.width(), TokenWidth(13));

    assert_patterns!(
        graph,
        ab         => [[a, b]],
        aba        => [[ab, a]],
        abab       => [[ab, ab], [aba, b]],
        ababa      => [[ab, aba], [abab, a]],
        ababab     => [[ab, abab], [ababa, b]],
        caba       => [[c, aba]],
        abc        => [[ab, c]],
        abcaba     => [[ab, caba], [abc, aba]],
        abcabab    => [[abc, abab], [abcaba, b]],
        abcababa   => [[abc, ababa], [abcabab, a]],
        abcababab  => [[abc, ababab], [abcababa, b]],
        ababcaba   => [[ab, abcaba], [abab, caba]],
        abababcaba => [[ab, ababcaba], [ababab, caba]],
        root       => [[abc, abababcaba], [abcababab, caba]]
    );
}
