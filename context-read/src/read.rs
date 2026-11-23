//! Positioned reader (reader plan §3.1; legacy `pipeline/mod.rs`,
//! `expansion/block.rs`).

use context_trace::{
    Token,
    positioned::PositionedGraphRef,
};

pub use crate::root::Commit;
use crate::{
    expand::expand,
    forward::forward,
    root::RootManager,
    root::Span,
    segment::{
        Segment,
        segments,
    },
};

/// Read `input` into `graph` and return the token of the whole input, or
/// `None` for empty input.
///
/// Every step (an unknown atom, a root expansion or a forward match) is
/// committed to the root before the next search, so the input read so far is
/// always a vertex of `graph`.
pub fn read(
    graph: &PositionedGraphRef,
    input: impl IntoIterator<Item = char>,
) -> Option<Token> {
    read_steps(graph, input, |_| {})
}

/// One committed reader step: what was consumed and how the root changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Step {
    pub kind: StepKind,
    pub commit: Commit,
    /// Width of the root after the commit.
    pub root: usize,
}

/// What a [`Step`] consumed (widths in atoms).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepKind {
    /// An atom read for the first time.
    Unknown,
    /// A forward match of this width.
    Forward(usize),
    /// A root postfix of width `postfix` extended to `expansion`.
    Expansion { postfix: usize, expansion: usize },
}

/// [`read`], also returning the committed steps.
pub fn read_traced(
    graph: &PositionedGraphRef,
    input: impl IntoIterator<Item = char>,
) -> (Option<Token>, Vec<Step>) {
    let mut steps = Vec::new();
    let root = read_steps(graph, input, |step| steps.push(step));
    (root, steps)
}

fn read_steps(
    graph: &PositionedGraphRef,
    input: impl IntoIterator<Item = char>,
    mut observe: impl FnMut(Step),
) -> Option<Token> {
    let mut root = RootManager::default();
    let mut commit = |root: &mut RootManager, kind, span| {
        let commit = root.commit(graph, span);
        let width = root.token().map_or(0, |token| token.width.0);
        observe(Step {
            kind,
            commit,
            root: width,
        });
    };
    for Segment { unknown, known } in segments(graph, input) {
        for atom in unknown {
            commit(&mut root, StepKind::Unknown, Span::Append(atom));
        }
        let mut cursor = 0;
        while cursor < known.len() {
            let rest = &known[cursor..];
            let (kind, span) = match root
                .token()
                .and_then(|token| expand(graph, token, rest, cursor))
            {
                Some(accepted) => (
                    StepKind::Expansion {
                        postfix: accepted.postfix.width.0,
                        expansion: accepted.expansion.width.0,
                    },
                    accepted.span,
                ),
                None => {
                    let span = forward(graph, root.token(), rest);
                    (StepKind::Forward(root.consumed(span)), span)
                },
            };
            cursor += root.consumed(span);
            commit(&mut root, kind, span);
        }
    }
    root.token()
}
