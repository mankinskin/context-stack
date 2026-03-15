use context_insert::*;
use context_trace::{
    graph::vertex::location::pattern::PatternLocation,
    *,
};
use derive_more::{
    Deref,
    DerefMut,
};
use derive_new::new;
use tracing::debug;

use crate::expansion::chain::BandState;

#[derive(Debug, Clone, Deref, DerefMut, new)]
pub(crate) struct RootManager {
    #[deref]
    #[deref_mut]
    pub(crate) graph: HypergraphRef,
    #[new(default)]
    pub(crate) root: Option<Token>,
    /// The last token committed by `commit_state`: T2 for `WithOverlap`, the
    /// single token for `Single`.  The next overlap search uses this as its
    /// left-side context.
    #[new(default)]
    pub(crate) anchor: Option<Token>,
    /// `true` while the root is a flat, work-in-progress container built
    /// exclusively by `append_token` / `append_pattern` (unknown-atom path).
    /// Such a root may be extended in-place without invalidating semantic
    /// compound tokens.  Cleared to `false` by every `commit_state` call.
    #[new(default)]
    pub(crate) flat_root: bool,
}

impl RootManager {
    pub(crate) fn anchor(&self) -> Option<Token> {
        self.anchor
    }

    /// `true` when the root has exactly one child pattern and no parents,
    /// AND appending `token` would not create a self-referential pattern.
    pub(crate) fn can_extend_with(
        &self,
        token: Token,
    ) -> bool {
        let Some(root) = self.root else {
            return false;
        };
        if token.vertex_index() == root.vertex_index() {
            return false;
        }
        let vertex = root.vertex(&self.graph);
        vertex.child_patterns().len() == 1 && vertex.parents().is_empty()
    }

    /// `true` when the root has exactly one child pattern and no parents.
    pub(crate) fn can_extend(&self) -> bool {
        let Some(root) = self.root else {
            return false;
        };
        let vertex = root.vertex(&self.graph);
        vertex.child_patterns().len() == 1 && vertex.parents().is_empty()
    }

    // -------------------------------------------------------------------------
    // Primitive root mutations
    // -------------------------------------------------------------------------

    pub(crate) fn set_root(
        &mut self,
        token: Token,
    ) {
        self.root = Some(token);
    }

    /// Append `token` to the root in-place (requires `can_extend_with(token)`).
    pub(crate) fn extend_root(
        &mut self,
        token: Token,
    ) {
        let root = self.root.expect("extend_root requires an existing root");
        let vertex = root.vertex(&self.graph);
        let (&pid, _) = vertex.expect_any_child_pattern();
        let new_root = self.graph.append_to_owned_pattern(root, pid, token);
        self.root = Some(new_root);
    }

    /// Wrap the current root together with `token` into a new compound:
    /// `insert_pattern([root, token])`.
    pub(crate) fn wrap_root(
        &mut self,
        token: Token,
    ) {
        let root = self.root.expect("wrap_root requires an existing root");
        let new_root = self.graph.insert_pattern(vec![root, token]);
        self.root = Some(new_root);
    }

    /// Replace the last child of the root's pattern with `replacement`.
    ///
    /// When the root is extendable in-place (`can_extend_with(replacement)`)
    /// the replacement is done via `replace_in_pattern` followed by a
    /// re-read of the updated token.  Otherwise the root is rebuilt as a
    /// fresh `insert_pattern` call with the last element swapped.
    pub(crate) fn replace_last_child(
        &mut self,
        replacement: Token,
    ) {
        let root = self
            .root
            .expect("replace_last_child requires an existing root");

        if self.can_extend_with(replacement) {
            let vertex = root.vertex(&self.graph);
            let (&pid, pattern) = vertex.expect_any_child_pattern();
            let last_idx = pattern.len() - 1;
            let loc = PatternLocation::new(root, pid);
            self.graph.replace_in_pattern(
                loc,
                last_idx..last_idx + 1,
                vec![replacement],
            );
            let updated = root.vertex(&self.graph).to_token();
            self.root = Some(updated);
        } else {
            let vertex = root.vertex(&self.graph);
            let (&_pid, pattern) = vertex.expect_any_child_pattern();
            let mut new_pat: Vec<Token> = pattern.iter().copied().collect();
            new_pat.pop();
            new_pat.push(replacement);
            let new_root = self.graph.insert_pattern(new_pat);
            self.root = Some(new_root);
        }
    }

    // -------------------------------------------------------------------------
    // commit_state — dispatch over BandState variants
    // -------------------------------------------------------------------------

    pub(crate) fn commit_state(
        &mut self,
        state: BandState,
    ) {
        match state {
            BandState::Single { band } => {
                let token = *band
                    .pattern
                    .first()
                    .expect("Single band must have at least one token");

                debug!(
                    token = ?token,
                    has_root = self.root.is_some(),
                    can_extend = self.can_extend(),
                    flat_root = self.flat_root,
                    "commit_state Single"
                );

                if self.try_extend_tail_with(token) {
                    return;
                }

                match self.root {
                    None => {
                        debug!("commit_state Single: no root — set_root");
                        self.set_root(token);
                        self.flat_root = false;
                    },
                    Some(_)
                        if self.flat_root && self.can_extend_with(token) =>
                    {
                        debug!("commit_state Single: flat root — extend_root");
                        self.extend_root(token);
                        // flat_root remains true
                    },
                    Some(_) => {
                        debug!(
                            "commit_state Single: semantic root — wrap_root"
                        );
                        self.wrap_root(token);
                        self.flat_root = false;
                    },
                }

                self.anchor = self.root;
            },

            BandState::WithOverlap { ref overlap, .. } => {
                let t2 = *overlap
                    .pattern
                    .last()
                    .expect("overlap pattern must have at least one token");

                debug!(
                    t2 = ?t2,
                    has_root = self.root.is_some(),
                    anchor = ?self.anchor,
                    "commit_state WithOverlap"
                );

                let bundled_pattern = state.collapse(&mut self.graph);
                debug_assert!(
                    bundled_pattern.len() == 1,
                    "collapse() of WithOverlap should yield exactly one bundled token, got {}",
                    bundled_pattern.len()
                );
                let bundled = bundled_pattern[0];

                debug!(bundled = ?bundled, "WithOverlap collapsed");

                match self.root {
                    None => {
                        debug!("commit_state WithOverlap: no root — set_root(bundled)");
                        self.set_root(bundled);
                    },
                    Some(t1) if Some(t1) == self.anchor => {
                        debug!("commit_state WithOverlap: root==anchor — set_root(bundled)");
                        self.set_root(bundled);
                    },
                    Some(_) => {
                        debug!("commit_state WithOverlap: accumulated root — replace_last_child(bundled)");
                        self.replace_last_child(bundled);
                    },
                }

                self.anchor = Some(t2);
            },
        }
    }

    /// When the root is a semantic compound (not a flat unknown-atom container)
    /// and both its last child and the incoming `token` are single atoms, combine
    /// them into a wider compound and rebuild the root with the combined token as
    /// its new last child.
    ///
    /// Returns `true` and updates `self.root` / `self.anchor` when the extension
    /// was applied; returns `false` when the preconditions are not met.
    ///
    /// Example: root = `[[aa, b]]`, token = `b` (atom)
    ///   last_child = `b` (atom), combined = `bb = [[b, b]]`
    ///   new root = `[[aa, bb]]`
    fn try_extend_tail_with(
        &mut self,
        token: Token,
    ) -> bool {
        if self.flat_root || *token.width() != 1 {
            return false;
        }
        let last_child = match self.last_child_token() {
            Some(t) if *t.width() == 1 => t,
            _ => return false,
        };

        let combined = self.graph.insert_pattern(vec![last_child, token]);
        if *combined.width() <= *last_child.width() {
            return false;
        }

        // Rebuild the root pattern with `last_child` replaced by `combined`.
        // We rebuild via insert_pattern rather than replace_in_pattern to avoid
        // a vertex-width mismatch when the replacement is wider than the original.
        let new_root = if let Some(root) = self.root {
            let vertex = root.vertex(&self.graph);
            let (_pid, pattern) = vertex.expect_any_child_pattern();
            let mut new_pat: Vec<Token> = pattern.iter().copied().collect();
            new_pat.pop();
            new_pat.push(combined);
            self.graph.insert_pattern(new_pat)
        } else {
            combined
        };

        debug!(
            last_child = ?last_child,
            token = ?token,
            combined = ?combined,
            new_root = ?new_root,
            "commit_state: tail extension — rebuilt root"
        );

        self.root = Some(new_root);
        self.flat_root = false;
        self.anchor = self.root;
        true
    }

    // -------------------------------------------------------------------------
    // Unknown-atom append helpers
    // -------------------------------------------------------------------------

    /// Append a pattern of new atom indices to the root.
    ///
    /// Used exclusively by the unknown-atom segment path; not called from the
    /// expansion loop.
    pub(crate) fn append_pattern(
        &mut self,
        new: Pattern,
    ) {
        match new.len() {
            0 => {},
            1 => self.append_token(new.first().unwrap()),
            _ =>
                if let Some(root) = &mut self.root {
                    let vertex = (*root).vertex(&self.graph);
                    let can_extend_inplace = self.flat_root
                        && vertex.child_patterns().len() == 1
                        && vertex.parents().is_empty();
                    *root = if can_extend_inplace {
                        let (&pid, _) = vertex.expect_any_child_pattern();
                        self.graph.append_to_owned_pattern(*root, pid, new)
                    } else {
                        let new = new.into_pattern();
                        let new_root = self.graph.insert_pattern(
                            [&[*root], new.as_slice()].concat(),
                        );
                        self.flat_root = false;
                        new_root
                    };
                } else {
                    let c = self.graph.insert_pattern(new);
                    self.root = Some(c);
                    self.flat_root = true;
                },
        }
    }

    /// Append a single token to the root.
    ///
    /// Used exclusively by the unknown-atom segment path.  Extends the root
    /// in-place only when `flat_root` is `true`; otherwise wraps the semantic
    /// root in a new compound via `insert_pattern`.
    #[context_trace::instrument_sig(skip(self, token))]
    pub(crate) fn append_token(
        &mut self,
        token: impl ToToken,
    ) {
        let token = token.to_token();
        if let Some(root) = &mut self.root {
            let vertex = (*root).vertex(&self.graph);
            let can_extend_inplace = self.flat_root
                && token.vertex_index() != root.vertex_index()
                && vertex.child_patterns().len() == 1
                && vertex.parents().is_empty();

            *root = if can_extend_inplace {
                let (&pid, _) = vertex.expect_any_child_pattern();
                self.graph.append_to_owned_pattern(*root, pid, token)
            } else {
                let new_root = self.graph.insert_pattern(vec![*root, token]);
                self.flat_root = false;
                new_root
            };
        } else {
            self.root = Some(token);
            self.flat_root = true;
        }
    }

    /// Returns the rightmost token in the root's pattern, or the root itself
    /// when the root is atomic.
    pub(crate) fn last_child_token(&self) -> Option<Token> {
        let root = self.root?;
        let vertex = root.vertex(&self.graph);
        if let Some((&_pid, pattern)) = vertex.child_patterns().iter().next() {
            pattern.last().copied()
        } else {
            Some(root)
        }
    }
}

impl_has_graph! {
    impl for RootManager,
    self => &**self;
    <'a> &'a Hypergraph
}
impl<R: InsertResult> ToInsertCtx<R> for RootManager {
    fn insert_context(&self) -> InsertCtx<R> {
        InsertCtx::from(self.graph.clone())
    }
}
