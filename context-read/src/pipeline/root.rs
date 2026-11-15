use context_insert::*;
use context_trace::*;
use derive_more::{
    Deref,
    DerefMut,
};
use derive_new::new;
use tracing::debug;

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
    fn can_extend_with(
        &self,
        token: Token,
    ) -> bool {
        let Some(root) = self.root else {
            return false;
        };
        if !self.owns_root || token.vertex_index() == root.vertex_index() {
            return false;
        }
        let vertex = root.vertex(&self.graph);
        vertex.child_patterns().len() == 1 && vertex.parents().is_empty()
    }

    /// Append a finalized span without modifying an adopted semantic token.
    ///
    /// Known spans and unknown atoms have the same accumulation policy.
    /// Overlap resolution belongs to the block, not to this sink.
    pub(crate) fn append_completed(
        &mut self,
        token: Token,
    ) {
        debug!(token = ?token, root = ?self.root, owns_root = self.owns_root, "append_completed");
        match self.root {
            None => {
                self.root = Some(token);
                self.owns_root = false;
            },
            Some(root) if self.can_extend_with(token) => {
                let vertex = root.vertex(&self.graph);
                let (&pattern_id, _) = vertex.expect_any_child_pattern();
                self.root = Some(self.graph.append_to_owned_pattern(
                    root, pattern_id, token,
                ));
            },
            Some(root) => {
                self.root = Some(self.graph.insert_pattern(vec![root, token]));
                self.owns_root = true;
            },
        }
    }

    /// Append unknown atoms through the same sink as completed known spans.
    pub(crate) fn append_pattern(
        &mut self,
        atoms: Pattern,
    ) {
        for token in atoms {
            self.append_completed(token);
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
