//! Category 2: Basic Read Tests (8 tests)
//!
//! These tests validate reading atoms, patterns, text sequences, and
//! the decomposition tree structure.

use crate::common::helpers::*;

#[test]
fn read_single_atom() {
    let mut ws = TestWorkspace::new("read-atom");

    let r = ws.add_atom('a');
    let a = unwrap_atom_info(&r);

    // Read as text
    let text_result = ws.read_as_text(a.index);
    let text = unwrap_text(&text_result);
    assert_eq!(text, "a");

    // Read as pattern (decomposition tree)
    let pattern_result = ws.read_pattern(a.index);
    let read = unwrap_read_result(&pattern_result);
    assert_eq!(read.text, "a");
    assert_eq!(read.root.width, 1);
    assert!(
        read.tree.children.is_empty(),
        "atom should have no children"
    );
}

#[test]
fn read_known_pattern() {
    let mut ws = TestWorkspace::new("read-pattern");

    // Insert "abc" to create atoms and pattern
    let insert_result = ws.insert_text("abc");
    let ir = unwrap_insert_result(&insert_result);

    // Read the root pattern
    let read_result = ws.read_pattern(ir.token.index);
    let read = unwrap_read_result(&read_result);

    assert_eq!(read.text, "abc");
    // The tree should have leaf nodes for a, b, c (possibly nested)
    assert!(
        !read.tree.children.is_empty(),
        "pattern should have children"
    );
}

#[test]
fn read_sequence_text() {
    let mut ws = TestWorkspace::new("read-seq-text");

    // Read a text sequence directly (auto-creates atoms)
    let result = ws.read_sequence("xyz");
    let read = unwrap_read_result(&result);

    assert_eq!(read.text, "xyz");
    assert_eq!(read.root.width, 3);
}

#[test]
fn read_sequence_single_char() {
    let mut ws = TestWorkspace::new("read-seq-1char");

    let result = ws.read_sequence("q");
    let read = unwrap_read_result(&result);

    assert_eq!(read.text, "q");
    assert_eq!(read.root.width, 1);
    assert!(
        read.tree.children.is_empty(),
        "single-char sequence should be an atom"
    );
}

#[test]
fn read_produces_decomposition_tree() {
    let mut ws = TestWorkspace::new("read-tree");

    // Insert then read to get a decomposition tree
    let ir = ws.insert_text("hello");
    let insert = unwrap_insert_result(&ir);

    let read_result = ws.read_pattern(insert.token.index);
    let read = unwrap_read_result(&read_result);

    assert_eq!(read.text, "hello");
    assert_eq!(read.root.width, 5);
    // Verify the tree has structure (at least the root has children)
    assert!(
        !read.tree.children.is_empty(),
        "hello should decompose into children"
    );
}

#[test]
fn read_text_output() {
    let mut ws = TestWorkspace::new("read-text-out");

    let ir = ws.insert_text("hello");
    let insert = unwrap_insert_result(&ir);

    let text_result = ws.read_as_text(insert.token.index);
    let text = unwrap_text(&text_result);
    assert_eq!(text, "hello");
}

#[test]
fn read_sequence_after_insert() {
    let mut ws = TestWorkspace::new("read-after-insert");

    // Insert first
    ws.insert_text("abc");

    // Read the same text via read_sequence
    let result = ws.read_sequence("abc");
    let read = unwrap_read_result(&result);
    assert_eq!(read.text, "abc");
}

#[test]
fn read_empty_sequence_returns_error() {
    let mut ws = TestWorkspace::new("read-empty");

    let result = ws.exec(context_api::commands::Command::ReadSequence {
        workspace: ws.name.clone(),
        text: String::new(),
    });

    assert!(result.is_err(), "empty text should return an error");
}
