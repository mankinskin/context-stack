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

use crate::expansion::chain::BandState;

#[derive(Debug, Clone, Deref, DerefMut, new)]
pub(crate) struct RootManager {
    #[deref]
    #[deref_mut]
    pub(crate) graph: HypergraphRef,
    #[new(default)]
    pub(crate) root: Option<Token>,
    /// Anchor: the last expansion token committed (T2 for WithOverlap, the
    /// single token for Single).  Used to detect whether the current root is
    /// "fresh" (root == anchor) and should be replaced rather than extended.
    #[new(default)]
    pub(crate) anchor: Option<Token>,
    /// Flag: true when the root was built (or last extended) by the
    /// `append_pattern` / `append_token` unknown-atom path.  In this state the
    /// root vertex is a flat work-in-progress container that can safely be
    /// extended in-place via `append_to_owned_pattern` without destroying
    /// semantically meaningful intermediate compound tokens.
    ///
    /// Set to `true` by `append_token` / `append_pattern`.
    /// Cleared to `false` by every `commit_state` call (once we start
    /// committing known-atom states the root becomes semantic).
    #[new(default)]
    pub(crate) flat_root: bool,
}

impl RootManager {
    // -------------------------------------------------------------------------
    // Accessors
    // -------------------------------------------------------------------------

    /// Returns the current anchor token.
    pub(crate) fn anchor(&self) -> Option<Token> {
        self.anchor
    }

    // -------------------------------------------------------------------------
    // Structural predicate
    // -------------------------------------------------------------------------

    /// Returns `true` when the root has exactly one child pattern and no
    /// parents — meaning we can extend it in-place without branching.
    ///
    /// The `token` parameter is the token we intend to append.  If it is the
    /// same vertex as the current root we must NOT extend in-place (that would
    /// produce a self-referential pattern); `wrap_root` must be used instead.
    pub(crate) fn can_extend_with(
        &self,
        token: Token,
    ) -> bool {
        let Some(root) = self.root else {
            return false;
        };
        // Self-referential guard: cannot append a token to its own pattern.
        if token.vertex_index() == root.vertex_index() {
            return false;
        }
        let vertex = root.vertex(&self.graph);
        vertex.child_patterns().len() == 1 && vertex.parents().is_empty()
    }

    /// Returns `true` when the root has exactly one child pattern and no
    /// parents.  Does NOT check the self-referential case — use
    /// `can_extend_with(token)` when you know which token will be appended.
    pub(crate) fn can_extend(&self) -> bool {
        let Some(root) = self.root else {
            return false;
        };
        let vertex = root.vertex(&self.graph);
        vertex.child_patterns().len() == 1 && vertex.parents().is_empty()
    }

    // -------------------------------------------------------------------------
    // Op-1: set_root
    // -------------------------------------------------------------------------

    /// Op-1 — install `token` as the new root (overwrites any existing root).
    pub(crate) fn set_root(
        &mut self,
        token: Token,
    ) {
        self.root = Some(token);
    }

    // -------------------------------------------------------------------------
    // Op-2: extend_root
    // -------------------------------------------------------------------------

    /// Op-2 — append `token` to the owned (single-pattern, no-parent) root,
    /// extending it in place.
    ///
    /// Delegates to the same `append_to_owned_pattern` path used by
    /// `append_token` / `append_pattern`.
    pub(crate) fn extend_root(
        &mut self,
        token: Token,
    ) {
        let root = self
            .root
            .expect("extend_root requires an existing root (can_extend check)");
        let vertex = root.vertex(&self.graph);
        let (&pid, _) = vertex.expect_any_child_pattern();
        let new_root = self.graph.append_to_owned_pattern(root, pid, token);
        self.root = Some(new_root);
    }

    // -------------------------------------------------------------------------
    // Op-3: wrap_root
    // -------------------------------------------------------------------------

    /// Op-3 — wrap the current root together with `token` into a new pattern
    /// node: `insert_pattern([root, token])`.
    pub(crate) fn wrap_root(
        &mut self,
        token: Token,
    ) {
        let root = self.root.expect("wrap_root requires an existing root");
        let new_root = self.graph.insert_pattern(vec![root, token]);
        self.root = Some(new_root);
    }

    // -------------------------------------------------------------------------
    // Op-4: replace_last_child
    // -------------------------------------------------------------------------

    /// Op-4 — replace the last child of the current root with `bundled`.
    ///
    /// - Op-4a (`can_extend` == true): mutate the root's single child pattern
    ///   in-place via `replace_in_pattern`, then re-read the root token to
    ///   pick up the updated width.
    /// - Op-4b (`can_extend` == false): read the root's first child pattern,
    ///   drop its last element, push `bundled`, create a fresh pattern node,
    ///   and set it as the new root.
    pub(crate) fn replace_last_child(
        &mut self,
        bundled: Token,
    ) {
        let root = self
            .root
            .expect("replace_last_child requires an existing root");

        if self.can_extend_with(bundled) {
            // Op-4a: in-place replacement
            let vertex = root.vertex(&self.graph);
            let (&pid, pattern) = vertex.expect_any_child_pattern();
            let last_idx = pattern.len() - 1;
            let loc = PatternLocation::new(root, pid);
            self.graph.replace_in_pattern(
                loc,
                last_idx..last_idx + 1,
                vec![bundled],
            );
            // Re-read the updated token (width may have changed).
            let updated = root.vertex(&self.graph).to_token();
            self.root = Some(updated);
        } else {
            // Op-4b: rebuild pattern with new last child
            let vertex = root.vertex(&self.graph);
            let (&_pid, pattern) = vertex.expect_any_child_pattern();
            let mut new_pat: Vec<Token> = pattern.iter().copied().collect();
            new_pat.pop();
            new_pat.push(bundled);
            let new_root = self.graph.insert_pattern(new_pat);
            self.root = Some(new_root);
        }
    }

    // -------------------------------------------------------------------------
    // commit_state — Case A/B/C/D/E/F dispatch
    // -------------------------------------------------------------------------

    /// Commit a `BandState` to the root using the Expansion Loop Redesign
    /// dispatch table.
    ///
    /// ```text
    /// Single { band }:
    ///   token = band.pattern[0]          (single-token band)
    ///   None        → set_root(token)    // Case A
    ///   can_extend  → extend_root(token) // Case B
    ///   else        → wrap_root(token)   // Case F
    ///   anchor = token
    ///
    /// WithOverlap { primary, overlap, link }:
    ///   t2      = overlap.pattern.last() // expansion token
    ///   bundled = collapse()             // collapses into single bundled token
    ///   None                  → set_root(bundled)         // Case C
    ///   Some(t1)==anchor      → set_root(bundled)         // Case C (replace)
    ///   else                  → replace_last_child(bundled) // Case D/E
    ///   anchor = t2
    /// ```
    pub(crate) fn commit_state(
        &mut self,
        state: BandState,
    ) {
        use tracing::debug;

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
                    "commit_state Single"
                );

                match self.root {
                    None => {
                        // Case A: no root yet — create a new semantic root.
                        debug!("Case A: set_root");
                        self.set_root(token);
                        // A freshly set root is not a flat container.
                        self.flat_root = false;
                    },
                    Some(_)
                        if self.flat_root && self.can_extend_with(token) =>
                    {
                        // Case B (flat-root continuation): the root was built
                        // by `append_pattern` / `append_token` (flat_root is
                        // true), so it is a flat work-in-progress container.
                        // Extend it in-place to preserve the flat atom pattern
                        // required by the split+join pipeline (e.g.
                        // "hypergraph" must be stored as a flat atom sequence
                        // on first read).
                        debug!("Case B (flat-root, extend): extend_root");
                        self.extend_root(token);
                        // flat_root stays true — we're still extending flatly.
                    },
                    Some(_) => {
                        // Case B/F (semantic wrap): the root was committed via
                        // a previous `commit_state` call (flat_root is false),
                        // or extending in-place would be self-referential.
                        // Use wrap_root so intermediate compound tokens
                        // (e.g. "aa" in "aaa") are preserved as distinct
                        // vertices and not mutated away.
                        debug!("Case B/F (semantic wrap): wrap_root");
                        self.wrap_root(token);
                        // wrap_root creates a new semantic compound — no longer flat.
                        self.flat_root = false;
                    },
                }

                self.anchor = Some(token);
            },

            BandState::WithOverlap { ref overlap, .. } => {
                // Extract T2 (expansion token) *before* consuming state in collapse().
                let t2 = *overlap.pattern.last().expect(
                    "overlap pattern must have at least one token (expansion)",
                );

                debug!(
                    t2 = ?t2,
                    has_root = self.root.is_some(),
                    anchor = ?self.anchor,
                    "commit_state WithOverlap"
                );

                // Collapse the WithOverlap state into a single bundled token.
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
                        // Case C: no root
                        debug!("Case C (no root): set_root(bundled)");
                        self.set_root(bundled);
                    },
                    Some(t1) if Some(t1) == self.anchor => {
                        // Case C: root equals anchor — the root *is* the
                        // previous expansion token; replace it wholesale.
                        debug!("Case C (root==anchor): set_root(bundled)");
                        self.set_root(bundled);
                    },
                    Some(_) => {
                        // Case D/E: root has accumulated content; replace its
                        // last child with the bundled token.
                        debug!("Case D/E: replace_last_child(bundled)");
                        self.replace_last_child(bundled);
                    },
                }

                self.anchor = Some(t2);
            },
        }
    }

    // -------------------------------------------------------------------------
    // Legacy helpers — kept for unknown-atom append paths
    // -------------------------------------------------------------------------

    /// Append a pattern of new atom indices.
    ///
    /// Used by unknown-atom code paths (not the main expansion loop).
    pub(crate) fn append_pattern(
        &mut self,
        new: Pattern,
    ) {
        // For single-token case, delegate to append_token which handles
        // flat_root tracking itself.  For the multi-token branch below we
        // manage flat_root inline.  Nothing to set here up-front.
        match new.len() {
            0 => {},
            1 => {
                let new = new.first().unwrap();
                self.append_token(new)
            },
            _ =>
                if let Some(root) = &mut self.root {
                    let vertex = (*root).vertex(&self.graph);
                    let can_extend_inplace = self.flat_root
                        && vertex.child_patterns().len() == 1
                        && vertex.parents().is_empty();
                    *root = if can_extend_inplace {
                        let (&pid, _) = vertex.expect_any_child_pattern();
                        // flat_root stays true — still extending the flat container.
                        self.graph.append_to_owned_pattern(*root, pid, new)
                    } else {
                        let new = new.into_pattern();
                        let new_root = self.graph.insert_pattern(
                            [&[*root], new.as_slice()].concat(),
                        );
                        // New wrapper compound is semantic — mark as not flat.
                        self.flat_root = false;
                        new_root
                    };
                } else {
                    let c = self.graph.insert_pattern(new);
                    self.root = Some(c);
                    // Fresh root from a multi-token pattern; treat as flat so
                    // subsequent unknown-atom appends can extend in-place.
                    self.flat_root = true;
                },
        }
    }

    /// Append a single token to the root.
    ///
    /// Used by unknown-atom code paths (not the main expansion loop).
    ///
    /// Extends in-place only when `flat_root` is already `true` (i.e. the
    /// root is a work-in-progress flat container built exclusively from
    /// unknown-atom appends).  When `flat_root` is `false` the root is a
    /// semantically meaningful compound token (created by a previous
    /// `commit_state` or `wrap_root` call) and must NOT be mutated — we
    /// create a new wrapper token via `insert_pattern` instead.
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
                // flat_root remains true — we are still extending the flat container.
                self.graph.append_to_owned_pattern(*root, pid, token)
            } else {
                // Either flat_root is false (semantic root) or the extend guard
                // failed — create a new wrapper compound token.
                let new_root = self.graph.insert_pattern(vec![*root, token]);
                // The new wrapper is itself a semantic compound; mark as NOT flat
                // so the next extend will not try to grow it in-place.
                self.flat_root = false;
                new_root
            };
        } else {
            // No root yet — the single atom is the starting root.
            // Mark as flat so subsequent unknown atoms can extend it in-place.
            self.root = Some(token);
            self.flat_root = true;
        }
    }

    /// Get the last child token of the root (for overlap detection).
    ///
    /// Returns the rightmost token in the root's pattern, or the root itself
    /// if it is atomic.
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

// RootManager derefs to HypergraphRef, which implements HasGraph
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
