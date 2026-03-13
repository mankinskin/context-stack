//! Integration tests for context-api.
//!
//! These tests exercise end-to-end workflows that span multiple modules
//! (workspace lifecycle, atom/pattern commands, persistence, command dispatch).
//! Per-module unit tests live alongside the code they test; these tests verify
//! that the pieces work together correctly.

use std::collections::HashSet;

use crate::{
    commands::{
        Command,
        CommandResult,
        WorkspaceApi,
        execute,
    },
    error::{
        ApiError,
        AtomError,
        PatternError,
        WorkspaceError,
    },
    types::AtomInfo,
    workspace::manager::WorkspaceManager,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a `WorkspaceManager` backed by a temporary directory.
fn tmp_manager() -> (tempfile::TempDir, WorkspaceManager) {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let mgr = WorkspaceManager::new(tmp.path().to_path_buf());
    (tmp, mgr)
}

/// Create a manager with an already-open workspace named `ws`.
fn setup_with_workspace() -> (tempfile::TempDir, WorkspaceManager) {
    let (tmp, mut mgr) = tmp_manager();
    mgr.create_workspace("ws").unwrap();
    (tmp, mgr)
}

/// Add atoms for all characters in a string, returning the atom infos sorted by index.
fn add_atoms_str(
    mgr: &mut WorkspaceManager,
    ws: &str,
    chars: &str,
) -> Vec<AtomInfo> {
    let char_vec: Vec<char> = chars.chars().collect();
    mgr.add_atoms(ws, char_vec).unwrap()
}

// ===========================================================================
// End-to-end workspace lifecycle
// ===========================================================================

#[test]
fn lifecycle_create_save_close_open_verify() {
    let (_tmp, mut mgr) = tmp_manager();

    // Create
    let create_info = mgr.create_workspace("lifecycle").unwrap();
    assert_eq!(create_info.name, "lifecycle");
    assert_eq!(create_info.vertex_count, 0);

    // Add atoms
    let atoms = add_atoms_str(&mut mgr, "lifecycle", "abc");
    assert_eq!(atoms.len(), 3);

    // Add pattern
    let pattern = mgr.add_simple_pattern("lifecycle", vec!['a', 'b']).unwrap();
    assert_eq!(pattern.label, "ab");
    assert_eq!(pattern.width, 2);
    assert_eq!(pattern.children.len(), 2);

    // Verify statistics
    let stats = mgr.get_statistics("lifecycle").unwrap();
    assert_eq!(stats.vertex_count, 4); // 3 atoms + 1 pattern
    assert_eq!(stats.atom_count, 3);
    assert_eq!(stats.pattern_count, 1);

    // Save
    mgr.save_workspace("lifecycle").unwrap();

    // Close
    mgr.close_workspace("lifecycle").unwrap();
    assert!(!mgr.is_open("lifecycle"));

    // Reopen
    let open_info = mgr.open_workspace("lifecycle").unwrap();
    assert_eq!(open_info.name, "lifecycle");
    assert_eq!(
        open_info.vertex_count, 4,
        "data should persist across save/close/open"
    );
    assert_eq!(open_info.atom_count, 3);
    assert_eq!(open_info.pattern_count, 1);

    // Verify atoms survived
    let listed_atoms = mgr.list_atoms("lifecycle").unwrap();
    assert_eq!(listed_atoms.len(), 3);
    let chars: HashSet<char> = listed_atoms.iter().map(|a| a.ch).collect();
    assert!(chars.contains(&'a'));
    assert!(chars.contains(&'b'));
    assert!(chars.contains(&'c'));

    // Verify pattern survived via vertex lookup
    let vertices = mgr.list_vertices("lifecycle").unwrap();
    assert_eq!(vertices.len(), 4);
    let pattern_vertex = vertices.iter().find(|v| v.width == 2);
    assert!(pattern_vertex.is_some(), "pattern should be in vertex list");
    assert_eq!(pattern_vertex.unwrap().label, "ab");

    // Cleanup
    mgr.close_workspace("lifecycle").unwrap();
    mgr.delete_workspace("lifecycle").unwrap();
}

// ===========================================================================
// Persistence round-trips
// ===========================================================================

#[test]
fn persistence_empty_workspace_round_trip() {
    let (_tmp, mut mgr) = tmp_manager();

    mgr.create_workspace("empty-rt").unwrap();
    mgr.save_workspace("empty-rt").unwrap();
    mgr.close_workspace("empty-rt").unwrap();

    let info = mgr.open_workspace("empty-rt").unwrap();
    assert_eq!(info.vertex_count, 0);
    assert_eq!(info.atom_count, 0);
    assert_eq!(info.pattern_count, 0);
}

#[test]
fn persistence_atoms_survive_round_trip() {
    let (_tmp, mut mgr) = tmp_manager();

    mgr.create_workspace("atom-rt").unwrap();
    mgr.add_atom("atom-rt", 'x').unwrap();
    mgr.add_atom("atom-rt", 'y').unwrap();
    mgr.add_atom("atom-rt", 'z').unwrap();
    mgr.save_workspace("atom-rt").unwrap();
    mgr.close_workspace("atom-rt").unwrap();

    mgr.open_workspace("atom-rt").unwrap();
    let atoms = mgr.list_atoms("atom-rt").unwrap();
    assert_eq!(atoms.len(), 3);

    let chars: HashSet<char> = atoms.iter().map(|a| a.ch).collect();
    assert!(chars.contains(&'x'));
    assert!(chars.contains(&'y'));
    assert!(chars.contains(&'z'));
}

#[test]
fn persistence_patterns_survive_round_trip() {
    let (_tmp, mut mgr) = tmp_manager();

    mgr.create_workspace("pat-rt").unwrap();
    add_atoms_str(&mut mgr, "pat-rt", "abcd");
    mgr.add_simple_pattern("pat-rt", vec!['a', 'b']).unwrap();
    mgr.add_simple_pattern("pat-rt", vec!['c', 'd']).unwrap();

    let stats_before = mgr.get_statistics("pat-rt").unwrap();
    let snapshot_before = mgr.get_snapshot("pat-rt").unwrap();

    mgr.save_workspace("pat-rt").unwrap();
    mgr.close_workspace("pat-rt").unwrap();

    mgr.open_workspace("pat-rt").unwrap();

    let stats_after = mgr.get_statistics("pat-rt").unwrap();
    assert_eq!(stats_before.vertex_count, stats_after.vertex_count);
    assert_eq!(stats_before.atom_count, stats_after.atom_count);
    assert_eq!(stats_before.pattern_count, stats_after.pattern_count);
    assert_eq!(stats_before.edge_count, stats_after.edge_count);
    assert_eq!(stats_before.max_width, stats_after.max_width);

    let snapshot_after = mgr.get_snapshot("pat-rt").unwrap();
    assert_eq!(snapshot_before.nodes.len(), snapshot_after.nodes.len());
    assert_eq!(snapshot_before.edges.len(), snapshot_after.edges.len());
}

#[test]
fn persistence_multiple_save_cycles() {
    let (_tmp, mut mgr) = tmp_manager();
    mgr.create_workspace("multi").unwrap();

    // Round 1: add atoms
    add_atoms_str(&mut mgr, "multi", "ab");
    mgr.save_workspace("multi").unwrap();

    // Round 2: add pattern
    mgr.add_simple_pattern("multi", vec!['a', 'b']).unwrap();
    mgr.save_workspace("multi").unwrap();

    mgr.close_workspace("multi").unwrap();
    mgr.open_workspace("multi").unwrap();

    let stats = mgr.get_statistics("multi").unwrap();
    assert_eq!(stats.vertex_count, 3); // 2 atoms + 1 pattern
    assert_eq!(stats.atom_count, 2);
    assert_eq!(stats.pattern_count, 1);
}

// ===========================================================================
// Workspace lifecycle edge cases
// ===========================================================================

#[test]
fn workspace_list_includes_both_open_and_closed() {
    let (_tmp, mut mgr) = tmp_manager();

    mgr.create_workspace("ws-a").unwrap();
    mgr.save_workspace("ws-a").unwrap();
    mgr.close_workspace("ws-a").unwrap(); // closed on disk

    mgr.create_workspace("ws-b").unwrap(); // still open

    let list = mgr.list_workspaces().unwrap();
    let names: Vec<&str> = list.iter().map(|i| i.name.as_str()).collect();
    assert!(names.contains(&"ws-a"), "closed workspace should appear");
    assert!(names.contains(&"ws-b"), "open workspace should appear");
    assert_eq!(list.len(), 2);
}

#[test]
fn workspace_delete_while_open() {
    let (_tmp, mut mgr) = tmp_manager();
    mgr.create_workspace("del-open").unwrap();
    assert!(mgr.is_open("del-open"));

    mgr.delete_workspace("del-open").unwrap();
    assert!(!mgr.is_open("del-open"));

    // Should not be listable
    let list = mgr.list_workspaces().unwrap();
    assert!(list.is_empty());
}

#[test]
fn workspace_unsaved_changes_warning() {
    // This test just verifies the code path doesn't panic; the warning is
    // logged via tracing.
    let (_tmp, mut mgr) = tmp_manager();
    mgr.create_workspace("dirty").unwrap();
    mgr.add_atom("dirty", 'x').unwrap(); // marks dirty
    mgr.close_workspace("dirty").unwrap(); // should warn but not error
}

// ===========================================================================
// Error handling
// ===========================================================================

#[test]
fn error_open_nonexistent() {
    let (_tmp, mut mgr) = tmp_manager();
    match mgr.open_workspace("ghost") {
        Err(WorkspaceError::NotFound { name }) => {
            assert_eq!(name, "ghost");
        },
        other => panic!("expected NotFound, got: {other:?}"),
    }
}

#[test]
fn error_close_not_open() {
    let (_tmp, mut mgr) = tmp_manager();
    match mgr.close_workspace("nope") {
        Err(WorkspaceError::NotOpen { name }) => {
            assert_eq!(name, "nope");
        },
        other => panic!("expected NotOpen, got: {other:?}"),
    }
}

#[test]
fn error_save_not_open() {
    let (_tmp, mut mgr) = tmp_manager();
    match mgr.save_workspace("nope") {
        Err(WorkspaceError::NotOpen { name }) => {
            assert_eq!(name, "nope");
        },
        other => panic!("expected NotOpen, got: {other:?}"),
    }
}

#[test]
fn error_delete_nonexistent() {
    let (_tmp, mut mgr) = tmp_manager();
    match mgr.delete_workspace("void") {
        Err(WorkspaceError::NotFound { name }) => {
            assert_eq!(name, "void");
        },
        other => panic!("expected NotFound, got: {other:?}"),
    }
}

#[test]
fn error_add_atom_workspace_not_open() {
    let (_tmp, mut mgr) = tmp_manager();
    match mgr.add_atom("nope", 'a') {
        Err(AtomError::WorkspaceNotOpen { workspace }) => {
            assert_eq!(workspace, "nope");
        },
        other => panic!("expected WorkspaceNotOpen, got: {other:?}"),
    }
}

#[test]
fn error_add_simple_pattern_cascading() {
    let (_tmp, mut mgr) = setup_with_workspace();
    add_atoms_str(&mut mgr, "ws", "abc");

    // Create pattern "ab" — now a and b have parents
    mgr.add_simple_pattern("ws", vec!['a', 'b']).unwrap();

    // Attempt to use 'a' again
    match mgr.add_simple_pattern("ws", vec!['a', 'c']) {
        Err(PatternError::AtomAlreadyInPattern { ch, .. }) => {
            assert_eq!(ch, 'a');
        },
        other => panic!("expected AtomAlreadyInPattern, got: {other:?}"),
    }

    // Attempt with too few atoms
    match mgr.add_simple_pattern("ws", vec!['c']) {
        Err(PatternError::TooShort { len }) => {
            assert_eq!(len, 1);
        },
        other => panic!("expected TooShort, got: {other:?}"),
    }

    // Attempt with non-existent atom
    match mgr.add_simple_pattern("ws", vec!['c', 'z']) {
        Err(PatternError::AtomNotFound { ch }) => {
            assert_eq!(ch, 'z');
        },
        other => panic!("expected AtomNotFound, got: {other:?}"),
    }

    // Attempt with duplicate in input
    match mgr.add_simple_pattern("ws", vec!['c', 'c']) {
        Err(PatternError::DuplicateAtomInInput { ch }) => {
            assert_eq!(ch, 'c');
        },
        other => panic!("expected DuplicateAtomInInput, got: {other:?}"),
    }
}

// ===========================================================================
// Command enum round-trip via execute
// ===========================================================================

#[test]
fn command_json_round_trip_full_workflow() {
    let (_tmp, mut mgr) = tmp_manager();

    // Build commands from JSON strings (simulating what an adapter would do)
    let create_json = r#"{"command":"create_workspace","name":"json-ws"}"#;
    let create_cmd: Command = serde_json::from_str(create_json).unwrap();
    let create_result = execute(&mut mgr, create_cmd).unwrap();
    match &create_result {
        CommandResult::WorkspaceInfo(info) => assert_eq!(info.name, "json-ws"),
        other => panic!("expected WorkspaceInfo, got: {other:?}"),
    }

    // Serialize the result to JSON and back
    let result_json = serde_json::to_string(&create_result).unwrap();
    let _: CommandResult = serde_json::from_str(&result_json).unwrap();

    // Add atoms via command
    let add_atom_json =
        r#"{"command":"add_atom","workspace":"json-ws","ch":"a"}"#;
    let add_atom_cmd: Command = serde_json::from_str(add_atom_json).unwrap();
    let add_result = execute(&mut mgr, add_atom_cmd).unwrap();
    match &add_result {
        CommandResult::AtomInfo(info) => assert_eq!(info.ch, 'a'),
        other => panic!("expected AtomInfo, got: {other:?}"),
    }

    // Add another atom
    let add_atom_b_json =
        r#"{"command":"add_atom","workspace":"json-ws","ch":"b"}"#;
    let add_atom_b_cmd: Command =
        serde_json::from_str(add_atom_b_json).unwrap();
    execute(&mut mgr, add_atom_b_cmd).unwrap();

    // Add pattern
    let add_pat_json = r#"{"command":"add_simple_pattern","workspace":"json-ws","atoms":["a","b"]}"#;
    let add_pat_cmd: Command = serde_json::from_str(add_pat_json).unwrap();
    let pat_result = execute(&mut mgr, add_pat_cmd).unwrap();
    match &pat_result {
        CommandResult::PatternInfo(info) => {
            assert_eq!(info.label, "ab");
            assert_eq!(info.width, 2);
        },
        other => panic!("expected PatternInfo, got: {other:?}"),
    }

    // Get statistics
    let stats_json = r#"{"command":"get_statistics","workspace":"json-ws"}"#;
    let stats_cmd: Command = serde_json::from_str(stats_json).unwrap();
    let stats_result = execute(&mut mgr, stats_cmd).unwrap();
    match &stats_result {
        CommandResult::Statistics(stats) => {
            assert_eq!(stats.vertex_count, 3); // 2 atoms + 1 pattern
            assert_eq!(stats.atom_count, 2);
            assert_eq!(stats.pattern_count, 1);
        },
        other => panic!("expected Statistics, got: {other:?}"),
    }

    // Snapshot
    let snap_json = r#"{"command":"get_snapshot","workspace":"json-ws"}"#;
    let snap_cmd: Command = serde_json::from_str(snap_json).unwrap();
    let snap_result = execute(&mut mgr, snap_cmd).unwrap();
    match &snap_result {
        CommandResult::Snapshot(snap) => {
            assert_eq!(snap.nodes.len(), 3);
            assert_eq!(snap.edges.len(), 2);
            // Verify snapshot is JSON-serializable
            let snap_json = serde_json::to_string(snap).unwrap();
            assert!(!snap_json.is_empty());
        },
        other => panic!("expected Snapshot, got: {other:?}"),
    }
}

// ===========================================================================
// Vertex introspection
// ===========================================================================

#[test]
fn vertex_info_reflects_parent_relationships() {
    let (_tmp, mut mgr) = setup_with_workspace();
    add_atoms_str(&mut mgr, "ws", "abc");

    // Before adding a pattern, atoms have no parents
    let a_before = mgr.get_atom("ws", 'a').unwrap().unwrap();
    let v_before = mgr.get_vertex("ws", a_before.index).unwrap().unwrap();
    assert_eq!(v_before.parent_count, 0);

    // Add pattern "ab"
    let pat = mgr.add_simple_pattern("ws", vec!['a', 'b']).unwrap();

    // Now atom 'a' should have 1 parent
    let v_after = mgr.get_vertex("ws", a_before.index).unwrap().unwrap();
    assert_eq!(v_after.parent_count, 1);
    assert!(v_after.is_atom);

    // The pattern vertex should have 2 children and 0 parents
    let pat_v = mgr.get_vertex("ws", pat.index).unwrap().unwrap();
    assert!(!pat_v.is_atom);
    assert_eq!(pat_v.parent_count, 0);
    assert_eq!(pat_v.children.len(), 1); // one child pattern
    assert_eq!(pat_v.children[0].len(), 2); // with two tokens in it
}

#[test]
fn list_vertices_sorted_by_index() {
    let (_tmp, mut mgr) = setup_with_workspace();
    add_atoms_str(&mut mgr, "ws", "dcba");

    let vertices = mgr.list_vertices("ws").unwrap();
    assert_eq!(vertices.len(), 4);

    // Must be sorted by index regardless of insertion order
    for i in 0..vertices.len() - 1 {
        assert!(
            vertices[i].index < vertices[i + 1].index,
            "vertices should be sorted by index: {} vs {}",
            vertices[i].index,
            vertices[i + 1].index
        );
    }
}

// ===========================================================================
// Atom deduplication
// ===========================================================================

#[test]
fn atom_deduplication_across_operations() {
    let (_tmp, mut mgr) = setup_with_workspace();

    let a1 = mgr.add_atom("ws", 'a').unwrap();
    let a2 = mgr.add_atom("ws", 'a').unwrap();
    assert_eq!(a1.index, a2.index, "same char must give same index");

    // Also via bulk add
    let chars: Vec<char> = vec!['a', 'b'];
    let bulk = mgr.add_atoms("ws", chars).unwrap();
    let a_bulk = bulk.iter().find(|i| i.ch == 'a').unwrap();
    assert_eq!(a_bulk.index, a1.index, "bulk add should also deduplicate");

    // Total unique atoms should be 2 (a and b)
    let all_atoms = mgr.list_atoms("ws").unwrap();
    assert_eq!(all_atoms.len(), 2);
}

// ===========================================================================
// Graph statistics correctness
// ===========================================================================

#[test]
fn statistics_edge_count_multiple_patterns() {
    let (_tmp, mut mgr) = setup_with_workspace();
    add_atoms_str(&mut mgr, "ws", "abcdef");

    // Pattern "ab" → 2 edges
    mgr.add_simple_pattern("ws", vec!['a', 'b']).unwrap();
    // Pattern "cde" → 3 edges
    mgr.add_simple_pattern("ws", vec!['c', 'd', 'e']).unwrap();

    let stats = mgr.get_statistics("ws").unwrap();
    assert_eq!(stats.vertex_count, 8); // 6 atoms + 2 patterns
    assert_eq!(stats.atom_count, 6);
    assert_eq!(stats.pattern_count, 2);
    assert_eq!(stats.edge_count, 5); // 2 + 3
    assert_eq!(stats.max_width, 3);
}

// ===========================================================================
// WorkspaceApi trait usage (verifies the trait is usable generically)
// ===========================================================================

#[test]
fn workspace_api_trait_is_object_safe_enough() {
    // Verify we can call methods through the trait
    fn use_api(api: &mut impl WorkspaceApi) {
        let info = api.create_workspace("trait-ws").unwrap();
        assert_eq!(info.name, "trait-ws");

        let atom = api.add_atom("trait-ws", 'x').unwrap();
        assert_eq!(atom.ch, 'x');

        let atoms = api.list_atoms("trait-ws").unwrap();
        assert_eq!(atoms.len(), 1);

        let stats = api.get_statistics("trait-ws").unwrap();
        assert_eq!(stats.atom_count, 1);

        api.save_workspace("trait-ws").unwrap();
        api.close_workspace("trait-ws").unwrap();
    }

    let (_tmp, mut mgr) = tmp_manager();
    use_api(&mut mgr);
}

// ===========================================================================
// Error response serialization
// ===========================================================================

#[test]
fn api_error_to_error_response_round_trip() {
    use crate::error::ErrorResponse;

    let err = ApiError::Pattern(PatternError::TooShort { len: 1 });
    let resp = ErrorResponse::from(&err);
    assert_eq!(resp.kind, "pattern");
    assert!(resp.message.contains("too short"));

    let json = serde_json::to_string(&resp).unwrap();
    let deser: ErrorResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(deser.kind, resp.kind);
    assert_eq!(deser.message, resp.message);
}

// ===========================================================================
// Multiple workspaces simultaneously
// ===========================================================================

#[test]
fn multiple_workspaces_independent() {
    let (_tmp, mut mgr) = tmp_manager();

    mgr.create_workspace("ws-1").unwrap();
    mgr.create_workspace("ws-2").unwrap();

    mgr.add_atom("ws-1", 'a').unwrap();
    mgr.add_atom("ws-1", 'b').unwrap();
    mgr.add_atom("ws-2", 'x').unwrap();

    let atoms_1 = mgr.list_atoms("ws-1").unwrap();
    let atoms_2 = mgr.list_atoms("ws-2").unwrap();

    assert_eq!(atoms_1.len(), 2);
    assert_eq!(atoms_2.len(), 1);
    assert_eq!(atoms_2[0].ch, 'x');

    // Saving one doesn't affect the other
    mgr.save_workspace("ws-1").unwrap();

    let ws2 = mgr.get_workspace("ws-2").unwrap();
    assert!(ws2.is_dirty(), "ws-2 should still be dirty");
}
