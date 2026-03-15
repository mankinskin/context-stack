//! Expansion context for known-atom blocks.
//!
//! `ExpansionCtx` is a one-shot iterator that consumes a slice of known atom
//! tokens and yields one `BandState` per step.  Each yielded state **must** be
//! committed to the `RootManager` before `next()` is called again (invariant
//! PI-5 / D8).
//!
//! ## Algorithm
//!
//! ```text
//! while cursor < atoms.len():
//!   remaining = atoms[cursor..]
//!
//!   fast-path A: single remaining token
//!     → yield Single { token: remaining[0] }; cursor += 1
//!
//!   fast-path B: anchor is an atom (width == 1, no true postfixes)
//!     → yield Single for remaining[0] from insert_next_match; cursor += width
//!
//!   normal path:
//!     t1      = insert_next_match(remaining).token()
//!     overlap = find_overlap(anchor, t1, remaining[t1.width..])
//!     if overlap:
//!       yield WithOverlap { t1, postfix, t2 }
//!       cursor = next_cursor   (inside overlap region of t2)
//!       anchor ← set by caller via commit_state
//!     else:
//!       yield Single { t1 }
//!       cursor += t1.width
//! ```

pub(crate) mod block;
pub(crate) mod chain;
pub(crate) mod cursor;
pub(crate) mod link;
pub(crate) mod stack;

use chain::{
    band::Band,
    BandState,
};
use context_insert::*;
use context_trace::*;
use tracing::debug;

use crate::{
    bands::HasTokenRoleIters,
    complement::ComplementBuilder,
    expansion::{
        chain::link::OverlapLink,
        link::ExpansionLink,
    },
};

// ---------------------------------------------------------------------------
// ExpansionCtx
// ---------------------------------------------------------------------------

/// Iterator over a known-atom block.
///
/// Yields one `BandState` per step.  Each yielded state must be committed to
/// `RootManager` before `next()` is called again.
#[derive(Debug)]
pub(crate) struct ExpansionCtx {
    pub(crate) graph: HypergraphRef,
    /// Full atom slice for the current known segment.
    atoms: Vec<Token>,
    /// Current position within `atoms` (in token-count units, not width units).
    cursor: usize,
    /// The anchor token: last committed expansion result, supplied externally
    /// by `RootManager` at each `next()` call.  `None` on the very first step.
    pub(crate) anchor: Option<Token>,
}

impl Iterator for ExpansionCtx {
    type Item = BandState;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor >= self.atoms.len() {
            return None;
        }

        let remaining = &self.atoms[self.cursor..];

        // ── Fast-path A: single remaining token ───────────────────────────
        // `insert_next_match` requires at least 2 tokens; handle 1 directly.
        if remaining.len() == 1 {
            let token = remaining[0];
            debug!(token = ?token, "ExpansionCtx::next fast-path A (single token)");
            self.cursor += 1;
            return Some(BandState::new(token));
        }

        // ── Fast-path B: anchor is an atom (width 1, no true postfixes) ──
        // Atoms have no true postfixes so `find_overlap` would always return
        // `None`; skip it entirely (OQ-2 / PI-14).
        let anchor_is_atom =
            self.anchor.map(|a| *a.width() == 1).unwrap_or(false);

        // ── Normal path: insert_next_match → maybe find_overlap ───────────
        let outcome = match ToInsertCtx::<IndexWithPath>::insert_next_match(
            &self.graph,
            remaining.to_vec(),
        ) {
            Ok(o) => o,
            Err(ErrorReason::SingleIndex(boxed)) => {
                // Exactly one atom matched — treat as a single-token advance.
                let token = boxed.index;
                debug!(token = ?token, "ExpansionCtx::next SingleIndex fallback");
                self.cursor += 1;
                return Some(BandState::new(token));
            },
            Err(e) => {
                // Unrecoverable error — surface as a single-token advance to
                // avoid an infinite loop.
                debug!(error = ?e, "ExpansionCtx::next insert_next_match error fallback");
                let token = remaining[0];
                self.cursor += 1;
                return Some(BandState::new(token));
            },
        };

        let t1 = outcome.token();
        let t1_width = t1.width().0;

        debug!(
            t1 = ?t1,
            t1_width,
            cursor = self.cursor,
            anchor = ?self.anchor,
            anchor_is_atom,
            "ExpansionCtx::next insert_next_match result"
        );

        assert!(
            t1_width > 0,
            "insert_next_match returned zero-width token {:?}",
            t1
        );

        // ── Overlap check ─────────────────────────────────────────────────
        // Only attempt overlap detection when:
        //   1. We have an anchor (previous committed token).
        //   2. The anchor is not an atom.
        //   3. There are atoms remaining after t1.
        let after_t1 = &self.atoms[self.cursor + t1_width..];

        if !anchor_is_atom {
            if let Some(anchor) = self.anchor {
                if let Some((postfix, t2, next_cursor)) =
                    self.find_overlap(anchor, t1, after_t1)
                {
                    debug!(
                        postfix = ?postfix,
                        t2 = ?t2,
                        next_cursor,
                        "ExpansionCtx::next overlap found"
                    );
                    // cursor advances into T2's overlap region (PI-10)
                    self.cursor = next_cursor;
                    let state =
                        self.build_overlap_state(anchor, t1, postfix, t2);
                    return Some(state);
                }
            }
        }

        // No overlap — plain sequential advance.
        debug!(t1 = ?t1, "ExpansionCtx::next no overlap");
        self.cursor += t1_width;
        Some(BandState::new(t1))
    }
}

impl ExpansionCtx {
    /// Create a new `ExpansionCtx`.
    ///
    /// - `graph`  — shared graph reference.
    /// - `atoms`  — the known-atom slice for this segment.
    /// - `anchor` — the last committed token from `RootManager`, used as the
    ///   starting point for postfix overlap search.  Pass `None` on the very
    ///   first segment.
    pub(crate) fn new(
        graph: HypergraphRef,
        atoms: Vec<Token>,
        anchor: Option<Token>,
    ) -> Self {
        debug!(
            atoms_len = atoms.len(),
            anchor = ?anchor,
            "New ExpansionCtx"
        );
        Self {
            graph,
            atoms,
            cursor: 0,
            anchor,
        }
    }

    // -----------------------------------------------------------------------
    // find_overlap
    // -----------------------------------------------------------------------

    /// Search postfixes of `anchor` for an overlap with `t1` and the
    /// `remaining_after_t1` slice.
    ///
    /// Returns `Some((postfix, t2, next_cursor))` on the **first** qualifying
    /// overlap:
    /// - `postfix`     — the postfix of `anchor` that was used as left context.
    /// - `t2`          — the expansion token returned by `insert_next_match`.
    /// - `next_cursor` — new absolute cursor position after the overlap
    ///   (pointing into the T2 overlap region).
    ///
    /// **Overlap predicate (PI-8):** `result.width() > postfix.width()` — the
    /// right-side context genuinely expanded the postfix into something larger.
    /// Equality (`result.width() == postfix.width()`) means the postfix was
    /// found verbatim and is NOT an overlap.
    ///
    /// **Complement resolution (PI-9):** complement tokens are resolved here
    /// via a second `insert_next_match` call *before* `collapse()` is invoked.
    fn find_overlap(
        &self,
        anchor: Token,
        t1: Token,
        remaining_after_t1: &[Token],
    ) -> Option<(Token, Token, usize)> {
        // No atoms follow t1 → no room for an overlap.
        if remaining_after_t1.is_empty() {
            return None;
        }

        // Iterate postfixes of anchor, largest first.
        let mut postfix_iter = anchor.postfix_iter(self.graph.clone());

        while let Some((_location, postfix)) = postfix_iter.next() {
            let postfix_width = postfix.width().0;

            // Build the query: [postfix, remaining_after_t1…]
            let mut query = Vec::with_capacity(1 + remaining_after_t1.len());
            query.push(postfix);
            query.extend_from_slice(remaining_after_t1);

            let result = match ToInsertCtx::<IndexWithPath>::insert_next_match(
                &self.graph,
                query,
            ) {
                Ok(o) => o,
                Err(_) => continue,
            };

            let result_width = result.token().width().0;

            debug!(
                postfix = ?postfix,
                postfix_width,
                result_token = ?result.token(),
                result_width,
                "find_overlap: checking postfix"
            );

            // Overlap predicate (PI-8): result must be strictly wider than
            // the postfix alone.
            if result_width > postfix_width {
                let t2 = result.token();
                // next_cursor: advance cursor to just inside T2's overlap
                // region — position of the largest true postfix of T2 that
                // starts at cursor + t1.width (i.e., the start of remaining_after_t1).
                // Conservative: cursor = current_cursor_base + t1.width + (t2.width - postfix_width)
                // which points to the atom just after the non-overlapping prefix of T2.
                // The caller's cursor base is implicit; we return an absolute index into
                // `self.atoms`.
                let base = self.cursor + t1.width().0;
                // Overlap covers [base .. base + postfix_width] from t1's postfix.
                // T2 starts at base - postfix_width (because postfix is a suffix of t1).
                // After the overlap, cursor is at t2.start + t2.width - largest_postfix_of_t2.
                // For simplicity: advance to base + (result_width - postfix_width).
                let next_cursor = base + (result_width - postfix_width);
                debug!(
                    t2 = ?t2,
                    base,
                    result_width,
                    postfix_width,
                    next_cursor,
                    "find_overlap: overlap found"
                );
                return Some((postfix, t2, next_cursor));
            }
        }

        None
    }

    // -----------------------------------------------------------------------
    // build_overlap_state
    // -----------------------------------------------------------------------

    /// Build a `BandState::WithOverlap` for the `(anchor → t1, postfix → t2)`
    /// overlap.
    ///
    /// This mirrors the logic previously in `ExpansionCtx::apply_op /
    /// ChainOp::Expansion`, but expressed purely in terms of tokens rather
    /// than path objects.
    fn build_overlap_state(
        &self,
        anchor: Token,
        t1: Token,
        postfix: Token,
        t2: Token,
    ) -> BandState {
        debug!(
            anchor = ?anchor,
            t1 = ?t1,
            postfix = ?postfix,
            t2 = ?t2,
            "build_overlap_state"
        );

        // Build the ExpansionLink that ComplementBuilder needs.
        // root_postfix:      path from anchor down to postfix
        // expansion_prefix:  path from t2 down to the same overlap region
        use crate::bands::HasTokenRoleIters;
        let root_postfix = {
            let mut iter = anchor.postfix_iter(self.graph.clone());
            // Find the location entry for `postfix` in anchor's postfix walk.
            let entry = iter.next().map(|(loc, _)| loc);
            let mut path = entry.map(IndexEndPath::from).unwrap_or_else(|| {
                IndexEndPath::from(ChildLocation::new(
                    anchor,
                    Default::default(),
                    0,
                ))
            });
            for (loc, tok) in iter {
                path.path_append(loc);
                if tok == postfix {
                    break;
                }
            }
            path
        };

        let expansion_prefix = t2.prefix_path(&self.graph, postfix);

        let start_bound = {
            // start_bound is the position within the combined band where
            // the overlap region begins (= anchor.width - postfix.width).
            anchor.width().0 - postfix.width().0
        };

        let expansion_link = ExpansionLink {
            start_bound,
            root_postfix: root_postfix.clone(),
            expansion_prefix,
        };

        // Build the complement token (prefix of t1 before the overlap region).
        let complement =
            ComplementBuilder::new(expansion_link.clone()).build(&self.graph);

        // Primary band: [complement, t1] — the sequential view
        // The complement covers [0 .. start_bound], t1 covers from start_bound.
        let primary =
            Band::from((0.into(), Pattern::from(vec![complement, t1])));

        // Overlap band: [complement_of_primary, t2]
        // complement_of_primary covers same prefix as `complement` above.
        let overlap =
            Band::from((0.into(), Pattern::from(vec![complement, t2])));

        let overlap_link = OverlapLink {
            child_path: root_postfix,
            search_path: expansion_link.expansion_prefix,
            start_bound,
        };

        BandState::WithOverlap {
            primary,
            overlap,
            link: overlap_link,
        }
    }
}
