use std::collections::HashSet;

use crate::workspace::manager::WorkspaceManager;

/// Helper: create a `WorkspaceManager` backed by a temporary directory
/// with a workspace already created and open.
fn setup(ws_name: &str) -> (tempfile::TempDir, WorkspaceManager) {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let mut mgr = WorkspaceManager::new(tmp.path().to_path_buf());
    mgr.create_workspace(ws_name).unwrap();
    (tmp, mgr)
}

/// Helper: add atoms for all characters in the string.
fn add_atoms(
    mgr: &mut WorkspaceManager,
    ws: &str,
    chars: &str,
) {
    let char_vec: Vec<char> = chars.chars().collect();
    mgr.add_atoms(ws, char_vec).unwrap();
}

#[test]
fn insert_sequence_creates_new() {
    let (_tmp, mut mgr) = setup("ws");

    let result = mgr.insert_sequence("ws", "abc").unwrap();
    assert!(!result.already_existed);
    assert_eq!(result.token.width, 3);
}

#[test]
fn insert_sequence_auto_creates_atoms() {
    let (_tmp, mut mgr) = setup("ws");

    let result = mgr.insert_sequence("ws", "abcde").unwrap();
    assert!(!result.already_existed);

    let atoms = mgr.list_atoms("ws").unwrap();
    let atom_chars: HashSet<char> = atoms.iter().map(|a| a.ch).collect();
    for ch in "abcde".chars() {
        assert!(
            atom_chars.contains(&ch),
            "atom '{ch}' should have been auto-created"
        );
    }
}

#[test]
fn insert_sequence_idempotent() {
    let (_tmp, mut mgr) = setup("ws");

    let first = mgr.insert_sequence("ws", "ab").unwrap();
    let second = mgr.insert_sequence("ws", "ab").unwrap();

    assert_eq!(first.token.index, second.token.index);
    assert!(second.already_existed);
}

#[test]
fn insert_sequence_too_short() {
    let (_tmp, mut mgr) = setup("ws");
    let result = mgr.insert_sequence("ws", "a").unwrap();
    assert_eq!(result.token.label, "a");
    assert_eq!(result.token.width, 1);

    let err = mgr.insert_sequence("ws", "").unwrap_err();
    match err {
        crate::error::InsertError::QueryTooShort => {},
        other => panic!("expected QueryTooShort for empty string, got: {other}"),
    }
}

#[test]
fn insert_sequence_workspace_not_open() {
    let (_tmp, mut mgr) = setup("ws");
    let err = mgr.insert_sequence("nope", "ab").unwrap_err();
    match err {
        crate::error::InsertError::WorkspaceNotOpen { workspace } => {
            assert_eq!(workspace, "nope");
        },
        other => panic!("expected WorkspaceNotOpen, got: {other}"),
    }
}

#[test]
fn insert_then_search_finds_it() {
    let (_tmp, mut mgr) = setup("ws");

    let insert_result = mgr.insert_sequence("ws", "abc").unwrap();
    assert!(!insert_result.already_existed);

    let search_result = mgr.search_sequence("ws", "abc").unwrap();
    assert!(search_result.complete);
    assert_eq!(
        search_result.token.as_ref().unwrap().index,
        insert_result.token.index
    );
}

#[test]
fn insert_sequence_marks_workspace_dirty() {
    let (_tmp, mut mgr) = setup("ws");

    mgr.save_workspace("ws").unwrap();
    assert!(!mgr.get_workspace("ws").unwrap().is_dirty());

    mgr.insert_sequence("ws", "ab").unwrap();
    assert!(mgr.get_workspace("ws").unwrap().is_dirty());
}

#[test]
fn insert_multiple_sequences_builds_graph() {
    let (_tmp, mut mgr) = setup("ws");

    mgr.insert_sequence("ws", "abc").unwrap();
    mgr.insert_sequence("ws", "def").unwrap();

    let r1 = mgr.search_sequence("ws", "abc").unwrap();
    assert!(r1.complete, "abc should be found");

    let r2 = mgr.search_sequence("ws", "def").unwrap();
    assert!(r2.complete, "def should be found");
}

#[test]
fn insert_first_match_by_index() {
    let (_tmp, mut mgr) = setup("ws");
    add_atoms(&mut mgr, "ws", "ab");

    let a = mgr.get_atom("ws", 'a').unwrap().unwrap();
    let b = mgr.get_atom("ws", 'b').unwrap().unwrap();

    let result = mgr
        .insert_first_match(
            "ws",
            vec![
                crate::types::TokenRef::Index(a.index),
                crate::types::TokenRef::Index(b.index),
            ],
        )
        .unwrap();

    assert!(!result.already_existed);
    assert_eq!(result.token.label, "ab");
    assert_eq!(result.token.width, 2);
}

#[test]
fn insert_first_match_existing() {
    let (_tmp, mut mgr) = setup("ws");
    add_atoms(&mut mgr, "ws", "ab");
    mgr.add_simple_pattern("ws", vec!['a', 'b']).unwrap();

    let a = mgr.get_atom("ws", 'a').unwrap().unwrap();
    let b = mgr.get_atom("ws", 'b').unwrap().unwrap();

    let result = mgr
        .insert_first_match(
            "ws",
            vec![
                crate::types::TokenRef::Index(a.index),
                crate::types::TokenRef::Index(b.index),
            ],
        )
        .unwrap();

    assert!(result.already_existed);
}

#[test]
fn insert_first_match_too_short() {
    let (_tmp, mut mgr) = setup("ws");
    let err = mgr
        .insert_first_match("ws", vec![crate::types::TokenRef::Index(0)])
        .unwrap_err();
    match err {
        crate::error::InsertError::QueryTooShort => {},
        other => panic!("expected QueryTooShort, got: {other}"),
    }
}

#[test]
fn insert_sequences_bulk() {
    let (_tmp, mut mgr) = setup("ws");

    let texts: HashSet<String> =
        ["abc", "def"].iter().map(|s| s.to_string()).collect();
    let results = mgr.insert_sequences("ws", texts).unwrap();
    assert_eq!(results.len(), 2);

    for result in &results {
        assert!(!result.already_existed);
    }
}

#[test]
fn insert_sequences_empty_set() {
    let (_tmp, mut mgr) = setup("ws");
    let texts: HashSet<String> = HashSet::new();
    let results = mgr.insert_sequences("ws", texts).unwrap();
    assert!(results.is_empty());
}

#[test]
fn insert_preserves_graph_integrity() {
    let (_tmp, mut mgr) = setup("ws");

    mgr.insert_sequence("ws", "abc").unwrap();
    mgr.insert_sequence("ws", "bcd").unwrap();
    mgr.insert_sequence("ws", "abcd").unwrap();

    let report = mgr.validate_graph("ws").unwrap();
    assert!(report.valid, "graph should be valid: {:?}", report.issues);
}

#[test]
fn insert_overlapping_sequences_shares_subpatterns() {
    let (_tmp, mut mgr) = setup("ws");

    let ab = mgr.insert_sequence("ws", "ab").unwrap();
    let _bc = mgr.insert_sequence("ws", "bc").unwrap();

    assert!(!ab.already_existed);

    let r1 = mgr.search_sequence("ws", "ab").unwrap();
    assert!(r1.complete, "ab should be found");

    let report = mgr.validate_graph("ws").unwrap();
    assert!(report.valid, "graph should be valid: {:?}", report.issues);
}

#[test]
fn insert_supersequence_of_existing() {
    let (_tmp, mut mgr) = setup("ws");

    mgr.insert_sequence("ws", "ab").unwrap();
    let abc = mgr.insert_sequence("ws", "abc").unwrap();

    assert!(!abc.already_existed);
    assert_eq!(abc.token.width, 3);

    let r1 = mgr.search_sequence("ws", "ab").unwrap();
    assert!(r1.complete, "ab should still be found");

    let report = mgr.validate_graph("ws").unwrap();
    assert!(report.valid, "graph should be valid: {:?}", report.issues);
}

#[test]
fn insert_subsequence_of_existing() {
    let (_tmp, mut mgr) = setup("ws");

    mgr.insert_sequence("ws", "abcd").unwrap();
    let _bc = mgr.insert_sequence("ws", "bc").unwrap();

    let report = mgr.validate_graph("ws").unwrap();
    assert!(report.valid, "graph should be valid: {:?}", report.issues);
}

#[test]
fn insert_sequence_with_repeated_chars() {
    let (_tmp, mut mgr) = setup("ws");

    let result = mgr.insert_sequence("ws", "hello").unwrap();
    assert!(!result.already_existed);
    assert_eq!(result.token.width, 5);
    assert_eq!(result.token.label, "hello");

    let text_result = mgr.read_as_text("ws", result.token.index).unwrap();
    assert_eq!(text_result, "hello");

    let report = mgr.validate_graph("ws").unwrap();
    assert!(report.valid, "graph should be valid: {:?}", report.issues);
}

#[test]
fn insert_sequence_correct_label() {
    let (_tmp, mut mgr) = setup("ws");

    let result = mgr.insert_sequence("ws", "xyz").unwrap();
    assert_eq!(result.token.label, "xyz");
    assert_eq!(result.token.width, 3);
}

#[test]
fn insert_disjoint_sequences_are_independent() {
    let (_tmp, mut mgr) = setup("ws");

    let r1 = mgr.insert_sequence("ws", "ab").unwrap();
    let r2 = mgr.insert_sequence("ws", "cd").unwrap();

    assert_ne!(r1.token.index, r2.token.index);

    let s1 = mgr.search_sequence("ws", "ab").unwrap();
    assert!(s1.complete, "ab should be found");

    let s2 = mgr.search_sequence("ws", "cd").unwrap();
    assert!(s2.complete, "cd should be found");

    let report = mgr.validate_graph("ws").unwrap();
    assert!(report.valid, "graph should be valid: {:?}", report.issues);
}