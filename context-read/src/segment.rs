//! Characters → atoms, split into runs of unknown and known atoms (reader
//! plan §3.1, §8.1; legacy `segment.rs`, `input.rs`).
//!
//! An atom is *unknown* if the graph has no vertex for it when it is read.
//! It is created at once through `GraphEdit::atom`, so a later repeat in the
//! same input is *known* (legacy `NewAtomIndex::{New, Known}`).

use context_trace::{
    Token,
    positioned::PositionedGraphRef,
};

/// One run of unknown atoms followed by one run of known atoms (legacy
/// `NextSegment`). Either run may be empty, but not both.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Segment {
    /// Atoms created by this read; they occur in no other token.
    pub(crate) unknown: Vec<Token>,
    /// Atoms that had a vertex when they were read.
    pub(crate) known: Vec<Token>,
}

/// Split `chars` into [`Segment`]s, creating unknown atoms on first sight.
pub(crate) fn segments(
    graph: &PositionedGraphRef,
    chars: impl IntoIterator<Item = char>,
) -> Vec<Segment> {
    let mut graph = graph.write();
    let mut edit = graph.edit();
    let mut segments: Vec<Segment> = Vec::new();
    for c in chars {
        let known = edit.graph().atom_token(&c);
        let token = known.unwrap_or_else(|| edit.atom(c));
        match (known.is_some(), segments.last_mut()) {
            (true, Some(segment)) => segment.known.push(token),
            (false, Some(segment)) if segment.known.is_empty() =>
                segment.unknown.push(token),
            (true, None) => segments.push(Segment {
                unknown: Vec::new(),
                known: vec![token],
            }),
            (false, _) => segments.push(Segment {
                unknown: vec![token],
                known: Vec::new(),
            }),
        }
    }
    edit.finish();
    segments
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::fixture;

    #[test]
    fn unknown_and_known_runs() {
        // `linear_abc` knows `a`, `b`, `c`.
        let (graph, _) = fixture("linear_abc");
        let generation = graph.read().generation();
        let segments = segments(&graph, "xabyyc".chars());
        let atom = |c| graph.read().atom_token(&c).unwrap();
        assert_eq!(
            segments,
            vec![
                Segment {
                    unknown: vec![atom('x')],
                    known: vec![atom('a'), atom('b')],
                },
                Segment {
                    unknown: vec![atom('y')],
                    known: vec![atom('y'), atom('c')],
                },
            ]
        );
        assert_eq!(graph.read().generation(), generation + 1);
        graph.read().validate().unwrap();
    }

    #[test]
    fn known_input_does_not_edit() {
        let (graph, _) = fixture("linear_abc");
        let generation = graph.read().generation();
        let segments = segments(&graph, "cab".chars());
        assert_eq!(segments.len(), 1);
        assert!(segments[0].unknown.is_empty());
        assert_eq!(segments[0].known.len(), 3);
        assert_eq!(graph.read().generation(), generation);
    }

    #[test]
    fn repeat_inside_one_input_is_known() {
        let graph = PositionedGraphRef::default();
        let segments = segments(&graph, "aab".chars());
        let atom = |c| graph.read().atom_token(&c).unwrap();
        assert_eq!(
            segments,
            vec![
                Segment {
                    unknown: vec![atom('a')],
                    known: vec![atom('a')],
                },
                Segment {
                    unknown: vec![atom('b')],
                    known: vec![],
                },
            ]
        );
    }
}
