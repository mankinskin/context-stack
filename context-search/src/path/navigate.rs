//! Search-only navigation of end paths (search plan 2.4): the largest token
//! ending at a position ([`PathNavigate::end_at`]) and the update of an end
//! path to a later position ([`PathNavigate::advance_end_path`]). Written
//! once for every store on top of [`Navigate`].

use context_trace::{
    AtomPosition,
    positioned::{
        CompositionRead,
        Located,
        Navigate,
        Occurrence,
        OccurrenceLocation,
        VertexRead,
        descend,
        location,
    },
};

/// End-path navigation on a vertex object (blanket over [`Navigate`]).
pub trait PathNavigate<'a>: Navigate<'a> {
    /// The largest retained proper descendant ending at `e`, and the
    /// occurrence steps from `self` down to it (mirror of
    /// [`Navigate::start_at`]).
    ///
    /// Descends through the first occurrence ending at or after `e` until
    /// one ends exactly at `e`. Correctness: a token ending at `e` lies in
    /// an occurrence `O` with `O.end ≥ e`; the first such occurrence starts
    /// no later than `O` and so contains it. `None` iff `e == 0`,
    /// `e > width` or `self` is an atom.
    fn end_at(
        self,
        e: impl Into<AtomPosition>,
    ) -> Option<Located<Self::Index>> {
        let mut parent = self;
        let mut e = e.into();
        let mut steps = Vec::new();
        loop {
            let occurrence = parent.node().composite()?.ending_at(e)?;
            steps.push(location(parent.vertex_index(), occurrence));
            if occurrence.end() == e {
                return Some((occurrence.child, steps));
            }
            e = AtomPosition(e.0 - occurrence.offset.0);
            parent = parent.resolve(occurrence.child.index)?;
        }
    }

    /// Move an end path inside `self` (the root) to position `p`: the
    /// result equals [`Navigate::start_at`]`(p)`, recomputing only the changed
    /// tail.
    ///
    /// `steps` is a path from `self` (as returned by `start_at` for an
    /// earlier position). Corrected pop rule (search plan, "End-path
    /// update"): a step `(P, o, C)` with absolute child offset `a` is kept
    /// iff `C` covers `p`, no later occurrence of `P` starts at or before
    /// `p` and `a < p`. The path is truncated at the first step that fails
    /// and continued with `start_at(P, p − offset(P))`; if every step is
    /// kept, the descent continues from the last child.
    fn advance_end_path(
        self,
        mut steps: Vec<OccurrenceLocation<Self::Index>>,
        p: impl Into<AtomPosition>,
    ) -> Option<Located<Self::Index>> {
        let p = p.into();
        debug_assert!(
            steps
                .first()
                .is_none_or(|s| s.parent == self.vertex_index())
        );
        // Absolute offset of the current parent in `self`.
        let mut base = 0;
        let mut parent = self.vertex_index();
        let mut kept = 0;
        for step in &steps {
            let composition = self.resolve(step.parent)?.node().composite()?;
            let index =
                partition_point(composition, |o| o.offset < step.offset);
            let occurrence = composition.get(index)?;
            debug_assert_eq!(occurrence.offset, step.offset);
            debug_assert_eq!(occurrence.child.index, step.child);
            let start = base + occurrence.offset.0;
            let covers = start <= p.0 && p.0 < start + occurrence.child.width.0;
            let last = composition
                .get(index + 1)
                .is_none_or(|next| base + next.offset.0 > p.0);
            if !(covers && last && start < p.0) {
                break;
            }
            base = start;
            parent = step.child;
            kept += 1;
        }
        steps.truncate(kept);
        descend(self.resolve(parent)?, AtomPosition(p.0 - base), steps)
    }
}

impl<'a, V: VertexRead<'a>> PathNavigate<'a> for V {}

/// First `k` with `!pred(get(k))` (binary search; `pred` holds on a
/// prefix of the occurrences).
fn partition_point<C: CompositionRead>(
    composition: C,
    pred: impl Fn(&Occurrence<C::Index>) -> bool,
) -> usize {
    let (mut lo, mut hi) = (0, composition.len());
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if pred(&composition.get(mid).expect("in range")) {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}
