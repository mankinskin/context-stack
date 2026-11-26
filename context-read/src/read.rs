//! Positioned reader (reader plan §3.1; legacy `pipeline/mod.rs`,
//! `expansion/block.rs`).

use crate::stage::overlay::OverlayHandle;
use context_trace::{
    positioned::{
        Candidates,
        PositionedGraphRef,
        Publication,
    },
    StageIndex,
    Token,
};

pub use crate::root::Commit;
use crate::{
    expand::expand,
    forward::forward,
    root::{
        ReadStage,
        Span,
    },
    segment::{
        Segment,
        segments,
    },
};

/// Read `input` into `graph` through an overlay stage, committed at the end
/// of the input; the committed token of the whole input, or `None` for empty
/// input.
///
/// Every step (an unknown atom, a root expansion or a forward match) is
/// committed to the root before the next search, so the input read so far is
/// always a vertex of the stage. After every step, staged tokens left with
/// at most one parent occurrence are dissolved or discarded (dissolution
/// plan §3.3), so the stage always holds the graph of the input read so far;
/// then staged tokens with two fixed parent occurrences (ending strictly
/// inside the parent, or in a committed or stable parent) are marked stable
/// (cheap stabilization plan §2.2, Rule S′): no continuation removes them.
pub fn read(
    graph: &PositionedGraphRef,
    input: impl IntoIterator<Item = char>,
) -> Option<Token> {
    read_steps(graph, input, |_| {})
}

/// One committed reader step: what was consumed, how the root changed, what
/// the following `settle` removed and how many tokens became stable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Step {
    pub kind: StepKind,
    pub commit: Commit,
    /// Width of the root after the commit.
    pub root: usize,
    /// Staged tokens dissolved into their only parent occurrence.
    pub dissolved: usize,
    /// Staged tokens without parent occurrences removed.
    pub discarded: usize,
    /// Staged tokens marked stable by Rule S′ in this step (never removed
    /// again).
    pub stabilized: usize,
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
    let root = read_steps(graph, input, |view| steps.push(view.step));
    (root, steps)
}

/// [`read`], passing every committed step to `observe` as it happens (e.g.
/// for progress reports).
pub fn read_observed(
    graph: &PositionedGraphRef,
    input: impl IntoIterator<Item = char>,
    mut observe: impl FnMut(&Step),
) -> Option<Token> {
    read_steps(graph, input, |view| observe(&view.step))
}

/// A settled step and the stage after it (for the per-step tests).
pub(crate) struct StepView<'a, 'g> {
    pub(crate) step: Step,
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) handle: &'a OverlayHandle<'g>,
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) root: Option<Token<StageIndex>>,
}

pub(crate) fn read_steps(
    graph: &PositionedGraphRef,
    input: impl IntoIterator<Item = char>,
    mut observe: impl FnMut(&StepView<'_, '_>),
) -> Option<Token> {
    let mut root = ReadStage::open(graph);
    // Commit `span`, then settle the candidates of the step: the records of
    // the insertion and of the commit, the previous root if it lost its
    // exemption and the root's previous last child if the root grew in place.
    let mut commit = |root: &mut ReadStage<'_>,
                      kind,
                      span,
                      inserted: Option<Publication<StageIndex>>| {
        let previous = root.token();
        let (commit, committed, last) = root.commit(span);
        let mut candidates = Candidates::default();
        for publication in inserted.iter().chain(committed.iter()) {
            candidates.record(publication);
        }
        if let Some(last) = last {
            candidates.push_gained(last);
        }
        if let Some(previous) = previous.filter(|previous| {
            root.token().map(|token| token.index) != Some(previous.index)
        }) {
            candidates.push(previous.index);
        }
        let settlement = root.settle(candidates);
        let token = root.token();
        observe(&StepView {
            step: Step {
                kind,
                commit,
                root: token.map_or(0, |token| token.width.0),
                dissolved: settlement.settled.dissolved,
                discarded: settlement.settled.discarded,
                stabilized: settlement.stabilized,
            },
            handle: &root.handle,
            root: token,
        });
    };
    for Segment { unknown, known } in segments(&mut root.handle, input) {
        for atom in unknown {
            commit(&mut root, StepKind::Unknown, Span::Append(atom), None);
        }
        let mut cursor = 0;
        while cursor < known.len() {
            let rest = &known[cursor..];
            let (kind, span, inserted) = match root
                .token()
                .and_then(|token| expand(&mut root.handle, token, rest, cursor))
            {
                Some(accepted) => (
                    StepKind::Expansion {
                        postfix: accepted.postfix.width.0,
                        expansion: accepted.expansion.width.0,
                    },
                    accepted.span,
                    accepted.publication,
                ),
                None => {
                    let token = root.token();
                    let (span, inserted) = forward(&mut root.handle, token, rest);
                    (StepKind::Forward(root.consumed(span)), span, inserted)
                },
            };
            cursor += root.consumed(span);
            commit(&mut root, kind, span, inserted);
        }
    }
    root.finish().map(|(token, _)| token)
}
