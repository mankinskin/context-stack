//! Bottom-up join of a positioned insertion (insert plan rev 3, §5; task
//! 3.3).
//!
//! Split nodes are joined in increasing `(width, index)`, so children come
//! before parents; inside a node its required intervals in increasing width.
//! An interval inside a child resolves to that child or to the child's
//! interval token; otherwise a new token is created from the maximal pieces.
//! Each node finally gets its new tokens through one `add_occurrences`.

use std::{
    collections::BTreeMap,
    ops::Range,
};

use context_trace::{
    AtomPosition,
    GraphKind,
    Token,
    graph::vertex::VertexIndex,
    positioned::{
        GraphEdit,
        Occurrence,
        PositionedError,
        maximal,
    },
};

use crate::split::SplitGraph;

/// Resolved intervals of this insertion: `(vertex, start, end) → token`.
type Resolved = BTreeMap<(VertexIndex, AtomPosition, AtomPosition), Token>;

/// Create every required token of `split` through `edit` and add the new
/// tokens to their split nodes. Returns the target token `N` (created last).
pub fn join<G: GraphKind>(
    edit: &mut GraphEdit<'_, G>,
    split: &SplitGraph,
) -> Result<Token, PositionedError> {
    let mut resolved = Resolved::new();
    #[cfg(debug_assertions)]
    let mut composed: Vec<Vec<Occurrence>> = Vec::new();
    for (vertex, _) in split.bottom_up() {
        let occurrences = edit
            .graph()
            .vertex(vertex)
            .and_then(|v| v.composition())
            .expect("split node is composite")
            .occurrences()
            .to_vec();
        let mut own: Vec<Occurrence> = Vec::new();
        for interval in split.intervals(vertex) {
            let token = match inside_child(&occurrences, &interval) {
                Some(child) => resolve_in_child(&resolved, child, &interval),
                None => {
                    let pieces = pieces(&resolved, &occurrences, &own, &interval);
                    let composition = maximal(pieces);
                    debug_assert!(
                        composition.len() >= 2,
                        "J3: interval {interval:?} of {vertex:?} has fewer than 2 pieces",
                    );
                    #[cfg(debug_assertions)]
                    {
                        debug_assert!(
                            !composed.contains(&composition),
                            "J2: composition {composition:?} created twice",
                        );
                        composed.push(composition.clone());
                    }
                    let token = edit.create(composition)?;
                    own.push(Occurrence::new(interval.start, token));
                    token
                },
            };
            resolved.insert((vertex, interval.start, interval.end), token);
        }
        if !own.is_empty() {
            edit.add_occurrences(vertex, own)?;
        }
    }
    let span = split.span();
    Ok(resolved[&(split.root().index, span.start, span.end)])
}

/// The occurrence containing `interval`, if any (§5.2 step 1). J1: an
/// interval in no child is not a vertex, so only those are composed.
fn inside_child<'o>(
    occurrences: &'o [Occurrence],
    interval: &Range<AtomPosition>,
) -> Option<&'o Occurrence> {
    occurrences
        .iter()
        .find(|o| o.offset <= interval.start && interval.end <= o.end())
}

/// The token of the part `range` (relative to `child`) of an original
/// child: the child itself or its resolved interval token.
fn child_part(
    resolved: &Resolved,
    child: Token,
    range: Range<AtomPosition>,
) -> Token {
    if *range.start == 0 && *range.end == child.width.0 {
        return child;
    }
    *resolved
        .get(&(child.index, range.start, range.end))
        .unwrap_or_else(|| {
            panic!("child {child:?} has no resolved interval {range:?}")
        })
}

fn resolve_in_child(
    resolved: &Resolved,
    child: &Occurrence,
    interval: &Range<AtomPosition>,
) -> Token {
    let a = *child.offset;
    child_part(resolved, child.child, interval.start - a..interval.end - a)
}

/// Pieces of `interval`, relative to its start (§5.2 step 2): children
/// inside it, the resolved parts of children crossing its borders and the
/// vertex's own new tokens inside it.
fn pieces(
    resolved: &Resolved,
    occurrences: &[Occurrence],
    own: &[Occurrence],
    interval: &Range<AtomPosition>,
) -> Vec<Occurrence> {
    let (x, y) = (interval.start, interval.end);
    let mut pieces = Vec::new();
    for occurrence in occurrences {
        let (a, end) = (occurrence.offset, occurrence.end());
        let (lo, hi) = (x.max(a), y.min(end));
        if lo >= hi {
            continue;
        }
        let token = child_part(resolved, occurrence.child, lo - *a..hi - *a);
        pieces.push(Occurrence::new(lo - *x, token));
    }
    pieces.extend(
        own.iter()
            .filter(|o| x <= o.offset && o.end() <= y)
            .map(|o| Occurrence::new(o.offset - *x, o.child)),
    );
    pieces
}
