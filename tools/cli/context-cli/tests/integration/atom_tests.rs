//! Category 1: Atom Management Tests (6 tests)
//!
//! These tests validate basic atom creation, deduplication, listing, and
//! auto-creation during insert operations.

use crate::common::helpers::*;

#[test]
fn atom_create_basic() {
    let mut ws = TestWorkspace::new("atom-basic");

    let r_a = ws.add_atom('a');
    let r_b = ws.add_atom('b');
    let r_c = ws.add_atom('c');

    let a = unwrap_atom_info(&r_a);
    let b = unwrap_atom_info(&r_b);
    let c = unwrap_atom_info(&r_c);

    assert_eq!(a.ch, 'a');
    assert_eq!(b.ch, 'b');
    assert_eq!(c.ch, 'c');

    // Each atom must have a unique index
    assert_ne!(a.index, b.index);
    assert_ne!(b.index, c.index);
    assert_ne!(a.index, c.index);
}

#[test]
fn atom_create_unicode() {
    let mut ws = TestWorkspace::new("atom-unicode");

    let r_emoji = ws.add_atom('🎯');
    let r_cjk = ws.add_atom('漢');

    let emoji = unwrap_atom_info(&r_emoji);
    let cjk = unwrap_atom_info(&r_cjk);

    assert_eq!(emoji.ch, '🎯');
    assert_eq!(cjk.ch, '漢');
    assert_ne!(emoji.index, cjk.index);
}

#[test]
fn atom_create_duplicate() {
    let mut ws = TestWorkspace::new("atom-dup");

    let first = ws.add_atom('x');
    let second = ws.add_atom('x');

    let first_info = unwrap_atom_info(&first);
    let second_info = unwrap_atom_info(&second);

    // Must return the same vertex — no duplicate created
    assert_eq!(first_info.index, second_info.index);
    assert_eq!(first_info.ch, second_info.ch);
}

#[test]
fn atom_list_all() {
    let mut ws = TestWorkspace::new("atom-list");

    ws.add_atom('a');
    ws.add_atom('b');
    ws.add_atom('c');

    let atoms_result = ws.list_atoms();
    let atoms = unwrap_atom_list(&atoms_result);

    assert_eq!(atoms.len(), 3);
    let chars: Vec<char> = atoms.iter().map(|a| a.ch).collect();
    assert!(chars.contains(&'a'), "atom 'a' missing");
    assert!(chars.contains(&'b'), "atom 'b' missing");
    assert!(chars.contains(&'c'), "atom 'c' missing");
}

#[test]
fn atom_get_by_index() {
    let mut ws = TestWorkspace::new("atom-get");

    let r_a = ws.add_atom('z');
    let a = unwrap_atom_info(&r_a);

    // Verify we can get the vertex at the atom's index
    let vertex_result = ws.get_vertex(a.index);
    // Getting a vertex should succeed (returns OptionalVertexInfo)
    match vertex_result {
        context_api::commands::CommandResult::OptionalVertexInfo { vertex } => {
            assert!(vertex.is_some(), "vertex should exist at atom index");
            let v = vertex.unwrap();
            assert_eq!(v.index, a.index);
            assert!(v.is_atom);
        },
        other => panic!("expected OptionalVertexInfo, got {other:?}"),
    }
}

#[test]
fn atom_auto_create_on_insert() {
    let mut ws = TestWorkspace::new("atom-auto");

    // Insert "abc" — should auto-create atoms a, b, c
    ws.insert_text("abc");

    let atoms_result = ws.list_atoms();
    let atoms = unwrap_atom_list(&atoms_result);

    let chars: Vec<char> = atoms.iter().map(|a| a.ch).collect();
    assert!(chars.contains(&'a'), "atom 'a' missing after insert");
    assert!(chars.contains(&'b'), "atom 'b' missing after insert");
    assert!(chars.contains(&'c'), "atom 'c' missing after insert");
}
