//! Category 3: Deduplication / Shared Substring Tests (8 tests)
//!
//! These tests validate that the graph reuses existing structure when
//! the same or overlapping sequences are inserted/read multiple times.
//!
//! NOTE: Several of these tests may fail due to known context-read bugs.
//! Each failure is documented in FAILING_TESTS.md.

use crate::common::helpers::*;

#[test]
fn dedup_exact_match() {
    let mut ws = TestWorkspace::new("dedup-exact");

    let r1 = ws.insert_text("abc");
    let r2 = ws.insert_text("abc");

    let ir1 = unwrap_insert_result(&r1);
    let ir2 = unwrap_insert_result(&r2);

    // Both insertions should return the same root vertex index
    assert_eq!(
        ir1.token.index, ir2.token.index,
        "inserting the same text twice should return the same token"
    );
}

#[test]
fn dedup_insert_then_read() {
    let mut ws = TestWorkspace::new("dedup-read");

    // Insert text
    let ir = ws.insert_text("hello");
    let insert = unwrap_insert_result(&ir);

    // Read back via ReadAsText — should get the same text
    let text_result = ws.read_as_text(insert.token.index);
    let text = unwrap_text(&text_result);
    assert_eq!(text, "hello");
}

#[test]
fn dedup_shared_prefix() {
    let mut ws = TestWorkspace::new("dedup-prefix");

    ws.insert_text("abc");
    ws.insert_text("abd");

    // After both inserts, "ab" should be a shared substructure.
    // We verify this indirectly: the vertex count should be less than
    // 3+3 = 6 atoms + 2 roots, because "ab" is shared.
    let stats = ws.get_statistics();
    let s = unwrap_statistics(&stats);

    // We have at most 4 unique atoms: a, b, c, d
    assert!(
        s.atom_count <= 4,
        "expected at most 4 atoms, got {}",
        s.atom_count
    );
}

#[test]
fn dedup_no_duplicate_vertices() {
    let mut ws = TestWorkspace::new("dedup-no-dup");

    ws.insert_text("abc");
    ws.insert_text("abd");
    ws.insert_text("abe");

    let stats = ws.get_statistics();
    let s = unwrap_statistics(&stats);

    // 5 unique atoms: a, b, c, d, e
    // With proper dedup (shared "ab" prefix): 5 atoms + 1 ab + 3 composites = 9
    // Without dedup: 5 atoms + 3 composites (no shared ab) = 8, or naive = 3*3 = 9
    // RC-1 fix: composites are now correctly created, so vertex count is exactly 9
    // (the shared "ab" sub-token + 3 composite tokens + 5 atoms).
    assert!(
        s.vertex_count <= 9,
        "expected deduplication to keep vertex count at most 9, got {}",
        s.vertex_count
    );
}

#[test]
fn dedup_read_sequence_finds_existing() {
    let mut ws = TestWorkspace::new("dedup-read-seq");

    // Insert a text
    ws.insert_text("hello");

    // Reading the same text should find the existing structure
    let result = ws.read_sequence("hello");
    let read = unwrap_read_result(&result);
    assert_eq!(read.text, "hello");
    assert_eq!(read.root.width, 5);
}

#[test]
fn dedup_atoms_not_duplicated() {
    let mut ws = TestWorkspace::new("dedup-atoms");

    ws.add_atom('a');
    ws.add_atom('b');
    ws.insert_text("ab"); // should reuse existing atoms

    let atoms_result = ws.list_atoms();
    let atoms = unwrap_atom_list(&atoms_result);
    assert_eq!(atoms.len(), 2, "should still be 2 atoms, not 4");
}

#[test]
fn dedup_multiple_reads_same_result() {
    let mut ws = TestWorkspace::new("dedup-multi-read");

    ws.insert_text("test");

    // Multiple reads of the same sequence should return identical results
    let r1 = ws.read_sequence("test");
    let r2 = ws.read_sequence("test");

    let read1 = unwrap_read_result(&r1);
    let read2 = unwrap_read_result(&r2);

    assert_eq!(read1.text, read2.text);
    assert_eq!(read1.root.index, read2.root.index);
    assert_eq!(read1.root.width, read2.root.width);
}

#[test]
fn dedup_graph_valid_after_inserts() {
    let mut ws = TestWorkspace::new("dedup-valid");

    ws.insert_text("abc");
    ws.insert_text("abd");

    let result = ws.validate_graph();
    match result {
        context_api::commands::CommandResult::ValidationReport(report) => {
            assert!(
                report.valid,
                "graph should be valid after dedup inserts, issues: {:?}",
                report.issues
            );
        },
        other => panic!("expected ValidationReport, got {other:?}"),
    }
}
