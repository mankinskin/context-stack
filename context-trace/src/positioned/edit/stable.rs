//! Early stabilization by fixed parent occurrences (cheap stabilization
//! plan 20261126 rev. 2, §2, Rule S′ and Theorem S′):
//! [`GraphEdit::stabilize`] and the event-driven worklist
//! [`GraphEdit::stabilize_candidates`] (§3).

use super::{
    Error,
    GraphEdit,
    check::expect_vertex,
};
use crate::{
    graph::vertex::{
        StageIndex,
        wide::Wide as _,
    },
    positioned::{
        model::PositionedError,
        stage::Stage,
        store::{
            CompositionRead as _,
            VertexRead,
            raw::SEAL,
        },
    },
};

impl<S: Stage + ?Sized> GraphEdit<'_, S> {
    /// Mark the staged composite `t ≠ root` stable: it is never removed
    /// again (`settle` skips it). Rule S′ (§2.2): `t` must have ≥ 2 fixed
    /// parent occurrences (ending strictly inside the parent, or in a
    /// committed or stable parent); by Theorem S′ it is then a vertex of
    /// every later settled stage. Only a flag changes; no index moves.
    ///
    /// # Errors
    /// Graph unchanged: `UnknownVertex`, `Atom`, `NotStaged` (committed),
    /// `IsRoot`, `FixedParentCount` (< 2). `Ok(false)` if `t` was already
    /// stable.
    pub fn stabilize(
        &mut self,
        t: StageIndex,
        root: StageIndex,
    ) -> Result<bool, Error<S>> {
        if expect_vertex(&*self.graph, t)?.node().composite().is_none() {
            return Err(PositionedError::Atom(t));
        }
        if !t.is_staged() {
            return Err(PositionedError::NotStaged(t));
        }
        if t == root {
            return Err(PositionedError::IsRoot(t));
        }
        if self.graph.is_stable(t) {
            return Ok(false);
        }
        let count = fixed_parent_occurrences(&*self.graph, t)?;
        if count < 2 {
            return Err(PositionedError::FixedParentCount { vertex: t, count });
        }
        self.graph.stabilize(SEAL, t);
        Ok(true)
    }

    /// Stabilize by Rule S′, starting from `candidates` (events E1 and E3 of
    /// §3: children of added occurrences, the root's previous last child)
    /// and continuing with the children of every stabilized vertex (E2,
    /// top-down cascade).
    /// Afterwards no unstable staged composite `≠ root` among the reached
    /// vertices has ≥ 2 fixed parent occurrences. Returns the number of
    /// vertices stabilized.
    pub fn stabilize_candidates(
        &mut self,
        candidates: impl IntoIterator<Item = StageIndex>,
        root: StageIndex,
    ) -> usize {
        let mut work: Vec<StageIndex> = candidates.into_iter().collect();
        let mut stabilized = 0;
        while let Some(t) = work.pop() {
            if !t.is_staged() || t == root || self.graph.is_stable(t) {
                continue;
            }
            if !matches!(fixed_parent_occurrences(&*self.graph, t), Ok(2..)) {
                continue;
            }
            self.stabilize(t, root)
                .unwrap_or_else(|error| panic!("stabilize: {error}"));
            stabilized += 1;
            let vertex = expect_vertex(&*self.graph, t).expect("stabilized vertex");
            let composition = vertex.node().composite().expect("composite");
            work.extend(composition.occurrences().map(|c| c.child.index));
        }
        stabilized
    }
}

/// The fixed parent occurrences of `t` (Rule S′, §2.2), counted up to 2:
/// `(U, o)` is fixed iff `t` ends strictly inside `U` (`o + |t| < |U|`: its
/// right neighbor is read, immutable content of any `U`, the root
/// included) or `U` is committed or stable. O(1) per parent occurrence, no
/// walk.
fn fixed_parent_occurrences<S: Stage + ?Sized>(
    graph: &S,
    t: StageIndex,
) -> Result<usize, PositionedError<StageIndex>> {
    let vertex = expect_vertex(graph, t)?;
    let width = vertex.width().0;
    Ok(vertex
        .parents()
        .filter(|(parent, offset)| {
            offset.0 + width < parent.width.0
                || !parent.index.is_staged()
                || graph.is_stable(parent.index)
        })
        .take(2)
        .count())
}
