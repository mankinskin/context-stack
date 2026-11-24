//! Characters → atoms, split into runs of unknown and known atoms (reader
//! plan §3.1, §8.1; legacy `segment.rs`, `input.rs`).
//!
//! An atom is *unknown* if the graph has no vertex for it when it is read.
//! It is created at once through `GraphEdit::atom`, so a later repeat in the
//! same input is *known* (legacy `NewAtomIndex::{New, Known}`).

use context_trace::{
    StageIndex,
    ToToken,
    Token,
    positioned::{
        GraphEdit,
        GraphRead,
        StageHandle,
    },
};

/// One run of unknown atoms followed by one run of known atoms (legacy
/// `NextSegment`). Either run may be empty, but not both.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Segment {
    /// Atoms created by this read; they occur in no other token.
    pub(crate) unknown: Vec<Token<StageIndex>>,
    /// Atoms that had a vertex when they were read.
    pub(crate) known: Vec<Token<StageIndex>>,
}

/// Split `chars` into [`Segment`]s, creating unknown atoms on first sight.
pub(crate) fn segments<H: StageHandle<Kind = context_trace::BaseGraphKind>>(
    handle: &mut H,
    chars: impl IntoIterator<Item = char>,
) -> Vec<Segment> {
    let mut view = handle.stage();
    let mut edit = GraphEdit::new(&mut view);
    let mut segments: Vec<Segment> = Vec::new();
    for c in chars {
        let known = edit.graph().atom(&c).map(|atom| atom.to_token());
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
    use context_trace::positioned::{
        HasGraph,
        OverlayHandle,
        PositionedGraphRef,
        VertexRead,
        VertexState,
    };

    use super::*;
    use crate::tests::fixture;

    #[test]
    fn unknown_and_known_runs() {
        // `linear_abc` knows `a`, `b`, `c`.
        let (graph, _) = fixture("linear_abc");
        let generation = graph.read().generation();
        let mut handle = OverlayHandle::open(&graph);
        let segments = segments(&mut handle, "xabyyc".chars());
        let view = handle.graph();
        let atom = |c| view.atom(&c).unwrap().to_token();
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
        assert_eq!(view.vertex(atom('x')).unwrap().state(), VertexState::Staged);
        assert_eq!(view.vertex(atom('a')).unwrap().state(), VertexState::Committed);
        assert_eq!(view.generation().stage, 1);
        drop(view);
        handle.commit(atom_token(&graph, 'a')).unwrap();
        assert_eq!(graph.read().generation().base, generation.base + 1);
        graph.read().validate().unwrap();
    }

    fn atom_token(
        graph: &PositionedGraphRef,
        c: char,
    ) -> Token<StageIndex> {
        graph.read().atom(&c).unwrap().to_token().into()
    }

    #[test]
    fn known_input_does_not_edit() {
        let (graph, _) = fixture("linear_abc");
        let mut handle = OverlayHandle::open(&graph);
        let segments = segments(&mut handle, "cab".chars());
        assert_eq!(segments.len(), 1);
        assert!(segments[0].unknown.is_empty());
        assert_eq!(segments[0].known.len(), 3);
        assert_eq!(handle.graph().generation().stage, 0);
        assert_eq!(handle.staged_count(), 0);
    }

    #[test]
    fn repeat_inside_one_input_is_known() {
        let graph = PositionedGraphRef::default();
        let mut handle = OverlayHandle::open(&graph);
        let segments = segments(&mut handle, "aab".chars());
        let view = handle.graph();
        let atom = |c| view.atom(&c).unwrap().to_token();
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
