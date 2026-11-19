use context_insert::*;
use context_trace::legacy::*;
use derive_more::{
    Deref,
    DerefMut,
};
use derive_new::new;
use tracing::debug;

/// A finalized accumulation operation handed from expansion to the sink.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Span {
    /// Starts directly after everything committed so far.
    Append(Token),
    /// A prepared result covering the previous root and new input.
    ReplaceRoot { previous: Token, token: Token },
    /// Replace the last child of the expected editable root with a wider token.
    /// `old` identifies the current tail; `token` represents its extension.
    ReplaceTail {
        previous: Token,
        old: Token,
        token: Token,
    },
}

/// The committed root used by the next expansion step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CommitReceipt {
    /// Root after applying the span; editability must be checked separately.
    pub(crate) root: Token,
}

/// Accumulates prepared spans without searching or constructing overlaps.
///
/// Only sink-allocated, single-pattern, parentless roots may grow in place.
/// Adopted roots are never edited, even when they have no graph parents.
#[derive(Debug, Deref, DerefMut, new)]
pub(crate) struct RootManager {
    #[deref]
    #[deref_mut]
    pub(crate) graph: HypergraphRef,
    /// Materialized token covering everything committed so far.
    #[new(default)]
    pub(crate) root: Option<Token>,
    /// Only containers allocated by this sink may be extended in place.
    #[new(default)]
    owns_root: bool,
}

impl RootManager {
    /// Check ownership and current graph structure for in-place root edits.
    ///
    /// Recheck after materialization, which can add parents or decompositions.
    pub(crate) fn is_editable(
        &self,
        root: Token,
    ) -> bool {
        if self.root != Some(root) || !self.owns_root {
            return false;
        }
        let vertex = root.vertex(&self.graph);
        vertex.child_patterns().len() == 1 && vertex.parents().is_empty()
    }

    fn can_extend_with(
        &self,
        token: Token,
    ) -> bool {
        let Some(root) = self.root else {
            return false;
        };
        if token.vertex_index() == root.vertex_index() {
            return false;
        }
        self.is_editable(root)
    }

    /// Commit a finalized span and return the current root.
    ///
    /// Whole-root replacement adopts the result; tail replacement retains
    /// ownership and updates widths and child-parent metadata.
    ///
    /// # Panics
    ///
    /// Panics on a stale replacement root, an ineligible or stale tail,
    /// non-growing replacement, or self-referential tail.
    pub(crate) fn commit(
        &mut self,
        span: Span,
    ) -> CommitReceipt {
        match span {
            Span::Append(token) => self.append(token),
            Span::ReplaceTail {
                previous,
                old,
                token,
            } => {
                assert_eq!(
                    self.root,
                    Some(previous),
                    "replacement root is stale"
                );
                assert!(
                    self.is_editable(previous),
                    "replacement root is not editable"
                );
                let vertex = previous.vertex(&self.graph);
                let (&pattern_id, _) = vertex.expect_any_child_pattern();
                self.root = Some(
                    self.graph
                        .replace_owned_tail(previous, pattern_id, old, token),
                );
            }
            Span::ReplaceRoot { previous, token } => {
                assert_eq!(
                    self.root,
                    Some(previous),
                    "replacement root is stale"
                );
                assert!(
                    token.width().0 > previous.width().0,
                    "replacement must extend the committed root"
                );
                self.root = Some(token);
                self.owns_root = false;
            }
        }
        CommitReceipt {
            root: self.root.expect("a committed span must have a root"),
        }
    }

    /// Append a finalized span without modifying an adopted semantic token.
    ///
    /// Known spans and unknown atoms have the same accumulation policy.
    /// Overlaps are resolved by the block before they reach this sink.
    pub(crate) fn append_completed(
        &mut self,
        token: Token,
    ) {
        self.append(token);
    }

    fn append(
        &mut self,
        token: Token,
    ) {
        debug!(token = ?token, root = ?self.root, owns_root = self.owns_root, "append_completed");
        match self.root {
            None => {
                self.root = Some(token);
                self.owns_root = false;
            }
            Some(root) if self.can_extend_with(token) => {
                let vertex = root.vertex(&self.graph);
                let (&pattern_id, _) = vertex.expect_any_child_pattern();
                self.root = Some(
                    self.graph.append_to_owned_pattern(root, pattern_id, token),
                );
            }
            Some(root) => {
                self.root = Some(self.graph.insert_pattern(vec![root, token]));
                self.owns_root = true;
            }
        }
    }

    /// Append unknown atoms through the same sink as completed known spans.
    pub(crate) fn append_pattern(
        &mut self,
        atoms: Pattern,
    ) {
        for token in atoms {
            self.append(token);
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
