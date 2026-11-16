pub(crate) mod cursor;

pub(crate) mod linear;
#[cfg(test)] // ngrams is a dev-dependency, only available in tests
pub(crate) mod ngrams_validation;
pub(crate) mod overlapping;
pub(crate) mod read;

/// Assert canonical content, complete occurrence paths and distinct boundaries.
pub(crate) fn assert_graph_invariants(graph: &context_trace::HypergraphRef) {
    use context_trace::*;
    use std::collections::{
        BTreeMap,
        HashSet,
    };

    fn content(
        graph: &HypergraphRef,
        token: Token,
    ) -> Vec<VertexIndex> {
        let patterns = graph.expect_child_patterns(token);
        if patterns.is_empty() {
            assert_eq!(token.width().0, 1);
            return vec![token.vertex_index()];
        }
        let mut contents = patterns.values().map(|pattern| {
            pattern
                .iter()
                .flat_map(|child| {
                    assert!(
                        child.width() < token.width(),
                        "cyclic or unary decomposition"
                    );
                    assert_eq!(
                        child.vertex(graph).width(),
                        child.width(),
                        "stale child width"
                    );
                    content(graph, *child)
                })
                .collect::<Vec<_>>()
        });
        let first = contents.next().unwrap();
        assert_eq!(first.len(), token.width().0);
        for alternative in contents {
            assert_eq!(
                alternative, first,
                "decompositions have different content"
            );
        }
        first
    }

    fn occurrences(
        graph: &HypergraphRef,
        token: Token,
        start: usize,
        found: &mut HashSet<(VertexIndex, usize)>,
    ) {
        if !found.insert((token.vertex_index(), start)) {
            return;
        }
        for pattern in graph.expect_child_patterns(token).values() {
            let mut offset = start;
            for child in pattern {
                occurrences(graph, *child, offset, found);
                offset += child.width().0;
            }
        }
    }

    let tokens: Vec<_> = graph
        .vertex_keys()
        .into_iter()
        .map(|key| graph.expect_vertex_data(key).to_token())
        .collect();
    let mut canonical = BTreeMap::new();
    for token in &tokens {
        let atoms = content(graph, *token);
        assert!(
            canonical.insert(atoms, *token).is_none(),
            "duplicate token content"
        );
        let mut boundaries = HashSet::new();
        for pattern in graph.expect_child_patterns(*token).values() {
            let mut offset = 0;
            for child in pattern.iter().take(pattern.len() - 1) {
                offset += child.width().0;
                assert!(
                    boundaries.insert(offset),
                    "repeated decomposition boundary"
                );
            }
        }
    }
    for (atoms, root) in &canonical {
        let mut found = HashSet::new();
        occurrences(graph, *root, 0, &mut found);
        for (sequence, token) in &canonical {
            if sequence.len() > atoms.len() {
                continue;
            }
            for (start, window) in atoms.windows(sequence.len()).enumerate() {
                if window == sequence {
                    assert!(
                        found.contains(&(token.vertex_index(), start)),
                        "unrepresented occurrence of {} at {} in {}",
                        graph.index_string(*token),
                        start,
                        graph.index_string(*root)
                    );
                }
            }
        }
    }
}
