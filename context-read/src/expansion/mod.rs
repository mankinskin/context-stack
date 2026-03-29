pub(crate) mod block;
pub(crate) mod chain;
pub(crate) mod link;

use chain::{
    band::Band,
    BandState,
};
use context_insert::*;
use context_trace::*;
use tracing::debug;

use crate::expansion::chain::link::OverlapLink;

/// Iterator over a known-atom block.  Yields one `BandState` per step.
///
/// Each yielded state must be committed to `RootManager` before `next()` is
/// called again, and the caller must refresh `self.anchor` from `RootManager`
/// after each commit so the next step sees the up-to-date left-side context.
#[derive(Debug)]
pub(crate) struct ExpansionCtx {
    pub(crate) graph: HypergraphRef,
    atoms: Vec<Token>,
    cursor: usize,
    /// Last committed expansion result supplied externally by `RootManager`.
    /// `None` on the very first step.
    pub(crate) anchor: Option<Token>,
}

impl Iterator for ExpansionCtx {
    type Item = BandState;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor >= self.atoms.len() {
            return None;
        }

        let remaining = &self.atoms[self.cursor..];

        // Single remaining token: insert_next_match requires ≥ 2 tokens,
        // so handle the final atom directly — but still check for an overlap
        // between a non-atom anchor and this trailing atom.
        if remaining.len() == 1 {
            let token = remaining[0];
            debug!(token = ?token, anchor = ?self.anchor, "ExpansionCtx::next: single remaining token");

            let anchor_is_atom =
                self.anchor.map(|a| *a.width() == 1).unwrap_or(true);
            if !anchor_is_atom {
                if let Some(anchor) = self.anchor {
                    if let Some((postfix, t2, next_cursor)) =
                        self.find_overlap(anchor, token, self.cursor)
                    {
                        debug!(postfix = ?postfix, t2 = ?t2, next_cursor, "ExpansionCtx::next: overlap on single trailing token");
                        self.cursor = next_cursor;
                        return Some(
                            self.build_overlap_state(
                                anchor, token, postfix, t2,
                            ),
                        );
                    }
                }
            }

            self.cursor += 1;
            return Some(BandState::new(token));
        }

        // Atom anchors have no true postfixes, so skip overlap detection.
        let anchor_is_atom =
            self.anchor.map(|a| *a.width() == 1).unwrap_or(false);

        let outcome = match ToInsertCtx::<IndexWithPath>::insert_next_match(
            &self.graph,
            remaining.to_vec(),
        ) {
            Ok(o) => o,
            Err(ErrorReason::SingleIndex(boxed)) => {
                let token = boxed.index;
                debug!(token = ?token, "ExpansionCtx::next: SingleIndex fallback");
                self.cursor += 1;
                return Some(BandState::new(token));
            },
            Err(e) => {
                debug!(error = ?e, "ExpansionCtx::next: insert_next_match error fallback");
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
            "ExpansionCtx::next: insert_next_match result"
        );

        assert!(
            t1_width > 0,
            "insert_next_match returned zero-width token {:?}",
            t1
        );

        if !anchor_is_atom {
            if let Some(anchor) = self.anchor {
                if let Some((postfix, t2, next_cursor)) =
                    self.find_overlap(anchor, t1, self.cursor)
                {
                    debug!(postfix = ?postfix, t2 = ?t2, next_cursor, "ExpansionCtx::next: overlap found");
                    self.cursor = next_cursor;
                    return Some(
                        self.build_overlap_state(anchor, t1, postfix, t2),
                    );
                }
            }
        }

        debug!(t1 = ?t1, "ExpansionCtx::next: no overlap");
        self.cursor += t1_width;
        Some(BandState::new(t1))
    }
}

impl ExpansionCtx {
    pub(crate) fn new(
        graph: HypergraphRef,
        atoms: Vec<Token>,
        anchor: Option<Token>,
    ) -> Self {
        debug!(atoms_len = atoms.len(), anchor = ?anchor, "New ExpansionCtx");
        Self {
            graph,
            atoms,
            cursor: 0,
            anchor,
        }
    }

    /// Search postfixes of `anchor` for an overlap with `t1`.
    ///
    /// For each postfix `P` of `anchor` with width `pw`, the candidate query
    /// starts at `t1_cursor + t1.width - pw` in `self.atoms`.  If
    /// `insert_next_match` on that query returns a token strictly wider than
    /// `pw`, the postfix was genuinely expanded — that is the overlap.
    ///
    /// Overlap predicate: `result.width > postfix.width`
    ///
    /// Returns `Some((postfix, t2, next_cursor))` for the first qualifying
    /// postfix, `None` otherwise.
    fn find_overlap(
        &self,
        anchor: Token,
        t1: Token,
        t1_cursor: usize,
    ) -> Option<(Token, Token, usize)> {
        let t1_width = t1.width().0;
        let mut postfix_iter = anchor.postfix_iter(self.graph.clone());

        while let Some((_location, postfix)) = postfix_iter.next() {
            let postfix_width = postfix.width().0;

            if postfix_width > t1_width {
                continue;
            }
            let overlap_start = t1_cursor + t1_width - postfix_width;

            // Build query as [postfix_token, remaining_atoms...].  Using the
            // postfix *token* (not its atoms) lets the search walk its parents
            // to find wider tokens that start with the postfix and extend into
            // the remaining atoms.
            let mut query: Vec<Token> = vec![postfix];
            query.extend_from_slice(&self.atoms[overlap_start..]);

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
                overlap_start,
                result_token = ?result.token(),
                result_width,
                "find_overlap: checking postfix"
            );

            if result_width > postfix_width {
                let t2 = result.token();
                let next_cursor = overlap_start + result_width;
                debug!(t2 = ?t2, overlap_start, result_width, postfix_width, next_cursor, "find_overlap: overlap found");
                return Some((postfix, t2, next_cursor));
            }
        }

        None
    }

    /// Build a `BandState::WithOverlap` for the `(anchor → t1, postfix → t2)` overlap.
    fn build_overlap_state(
        &self,
        anchor: Token,
        t1: Token,
        postfix: Token,
        t2: Token,
    ) -> BandState {
        debug!(anchor = ?anchor, t1 = ?t1, postfix = ?postfix, t2 = ?t2, "build_overlap_state");

        let root_postfix = {
            let mut iter = anchor.postfix_iter(self.graph.clone());
            let (entry_loc, entry_tok) = iter.next().unwrap();
            let mut path = IndexEndPath::from(entry_loc);
            // Only descend further if the initial entry isn't already the
            // postfix token (compound postfixes are yielded at the first
            // level by the postfix iterator).
            if entry_tok != postfix {
                for (loc, tok) in iter {
                    path.path_append(loc);
                    if tok == postfix {
                        break;
                    }
                }
            }
            path
        };

        let expansion_prefix = t2.prefix_path(&self.graph, postfix);

        let start_bound = anchor.width().0 - postfix.width().0;

        // Build Band patterns that carry t1/t2 as their last element.
        // The collapse() method extracts only the last token from each
        // pattern before delegating to bundle_overlap, so the prefix
        // elements here are purely informational.
        let primary = Band::from((0.into(), Pattern::from(vec![t1])));
        let overlap = Band::from((0.into(), Pattern::from(vec![t2])));

        let overlap_link = OverlapLink {
            child_path: root_postfix,
            search_path: expansion_prefix,
            start_bound,
            self_overlap: t2.vertex_index() == anchor.vertex_index(),
        };

        BandState::WithOverlap {
            primary,
            overlap,
            link: overlap_link,
        }
    }
}
