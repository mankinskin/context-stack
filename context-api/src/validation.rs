//! Validation rules for API commands.
//!
//! Currently contains the `validate_simple_pattern` function which enforces
//! all invariants required before inserting a "simple pattern" (a pattern
//! composed entirely of existing, parentless atoms).

use std::collections::HashSet;

use context_trace::{
    graph::{
        kind::BaseGraphKind,
        vertex::atom::Atom,
        Hypergraph,
    },
    VertexSet,
};

use crate::error::PatternError;

/// Validate that `atoms` can be used to create a simple pattern.
///
/// A "simple pattern" is a pattern whose children are all existing atom
/// vertices that do not yet belong to any other pattern. This is the most
/// constrained (and safest) way to build up the graph incrementally.
///
/// # Rules
///
/// 1. **Length ≥ 2** — a pattern must contain at least two children.
/// 2. **No duplicate chars** — each character in the input must be unique.
/// 3. **Atoms exist** — every character must already be present in the graph
///    as an `Atom::Element`.
/// 4. **Atoms are parentless** — none of the atoms may already belong to an
///    existing pattern (i.e. they must have zero parents).
///
/// # Errors
///
/// Returns a specific `PatternError` variant describing the first rule that
/// was violated.
pub fn validate_simple_pattern(
    graph: &Hypergraph<BaseGraphKind>,
    atoms: &[char],
) -> Result<(), PatternError> {
    // 1. Length check
    if atoms.len() < 2 {
        return Err(PatternError::TooShort { len: atoms.len() });
    }

    // 2. Duplicate check within input
    let mut seen = HashSet::new();
    for &ch in atoms {
        if !seen.insert(ch) {
            return Err(PatternError::DuplicateAtomInInput { ch });
        }
    }

    // 3 + 4. Each char must be an existing atom with no parents
    for &ch in atoms {
        // Look up the atom by its character value via the atom_keys map.
        let atom_index = graph
            .get_atom_index(Atom::Element(ch))
            .map_err(|_| PatternError::AtomNotFound { ch })?;

        // Check that the atom has no parent relationships.
        let vertex_data = graph
            .get_vertex_data(atom_index)
            .map_err(|e| PatternError::InternalError(format!("{e:?}")))?;

        if !vertex_data.parents().is_empty() {
            // Find the first parent's index for a useful error message.
            let first_parent_index = vertex_data
                .parents()
                .keys()
                .next()
                .map(|idx| idx.0)
                .unwrap_or(0);

            return Err(PatternError::AtomAlreadyInPattern {
                ch,
                existing_parent: first_parent_index,
            });
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use context_trace::graph::vertex::atom::Atom;

    /// Helper: create a default graph and insert the given atoms.
    fn graph_with_atoms(chars: &[char]) -> Hypergraph<BaseGraphKind> {
        let g = Hypergraph::<BaseGraphKind>::default();
        for &ch in chars {
            g.insert_atom(Atom::Element(ch));
        }
        g
    }

    #[test]
    fn valid_simple_pattern() {
        let g = graph_with_atoms(&['a', 'b', 'c']);
        assert!(validate_simple_pattern(&g, &['a', 'b']).is_ok());
        assert!(validate_simple_pattern(&g, &['a', 'b', 'c']).is_ok());
    }

    #[test]
    fn too_short_zero() {
        let g = Hypergraph::<BaseGraphKind>::default();
        match validate_simple_pattern(&g, &[]) {
            Err(PatternError::TooShort { len: 0 }) => {},
            other => panic!("expected TooShort(0), got: {other:?}"),
        }
    }

    #[test]
    fn too_short_one() {
        let g = graph_with_atoms(&['x']);
        match validate_simple_pattern(&g, &['x']) {
            Err(PatternError::TooShort { len: 1 }) => {},
            other => panic!("expected TooShort(1), got: {other:?}"),
        }
    }

    #[test]
    fn duplicate_atom_in_input() {
        let g = graph_with_atoms(&['a', 'b']);
        match validate_simple_pattern(&g, &['a', 'a']) {
            Err(PatternError::DuplicateAtomInInput { ch: 'a' }) => {},
            other =>
                panic!("expected DuplicateAtomInInput('a'), got: {other:?}"),
        }
    }

    #[test]
    fn atom_not_found() {
        let g = graph_with_atoms(&['a']);
        match validate_simple_pattern(&g, &['a', 'z']) {
            Err(PatternError::AtomNotFound { ch: 'z' }) => {},
            other => panic!("expected AtomNotFound('z'), got: {other:?}"),
        }
    }

    #[test]
    fn atom_already_in_pattern() {
        let g = graph_with_atoms(&['a', 'b', 'c']);

        // Create a pattern using atoms 'a' and 'b' — they now have parents.
        let ta = g.expect_atom_child(Atom::Element('a'));
        let tb = g.expect_atom_child(Atom::Element('b'));
        let _pattern = g.insert_pattern(vec![ta, tb]);

        // Trying to use 'a' again should fail.
        match validate_simple_pattern(&g, &['a', 'c']) {
            Err(PatternError::AtomAlreadyInPattern { ch: 'a', .. }) => {},
            other =>
                panic!("expected AtomAlreadyInPattern('a'), got: {other:?}"),
        }

        // 'c' is still free, but 'b' is taken.
        match validate_simple_pattern(&g, &['c', 'b']) {
            Err(PatternError::AtomAlreadyInPattern { ch: 'b', .. }) => {},
            other =>
                panic!("expected AtomAlreadyInPattern('b'), got: {other:?}"),
        }
    }

    #[test]
    fn validation_order_length_before_duplicates() {
        // With a single-element input that is also duplicated (impossible, but
        // length should be checked first).
        let g = graph_with_atoms(&['a']);
        match validate_simple_pattern(&g, &['a']) {
            Err(PatternError::TooShort { .. }) => {},
            other => panic!("expected TooShort, got: {other:?}"),
        }
    }
}
