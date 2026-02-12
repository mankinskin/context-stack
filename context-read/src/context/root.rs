use context_insert::*;
use context_trace::*;
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
    /// Whether the root was created fresh during this read request.
    /// Fresh roots can have their pattern extended directly.
    #[new(default)]
    is_fresh: bool,
}

impl RootManager {
    /// append a pattern of new atom indices
    /// returns index of possible new index
    pub(crate) fn append_pattern(
        &mut self,
        new: Pattern,
    ) {
        match new.len() {
            0 => {},
            1 => {
                let new = new.first().unwrap();
                self.append_token(new)
            },
            _ => {
                if let Some(root) = &mut self.root {
                    let vertex = (*root).vertex(&self.graph);
                    *root = if vertex.child_patterns().len() == 1
                        && vertex.parents().is_empty()
                    {
                        let (&pid, _) = vertex.expect_any_child_pattern();
                        self.graph.append_to_pattern(*root, pid, new)
                    } else {
                        // some old overlaps though
                        self.is_fresh = false;
                        let new = new.into_pattern();
                        self.graph
                            .insert_pattern([&[*root], new.as_slice()].concat())
                    };
                } else {
                    let c = self.graph.insert_pattern(new);
                    self.root = Some(c);
                    self.is_fresh = true;
                }
            },
        }
    }

    #[context_trace::instrument_sig(skip(self, token))]
    pub(crate) fn append_token(
        &mut self,
        token: impl ToToken,
    ) {
        let token = token.to_token();
        if let Some(root) = &mut self.root {
            let vertex = (*root).vertex(&self.graph);
            *root = if token.vertex_index() != root.vertex_index()
                && vertex.child_patterns().len() == 1
                && vertex.parents().is_empty()
            {
                let (&pid, _) = vertex.expect_any_child_pattern();
                self.graph.append_to_pattern(*root, pid, token)
            } else {
                self.is_fresh = false;
                self.graph.insert_pattern(vec![*root, token])
            };
        } else {
            self.root = Some(token);
            self.is_fresh = true;
        }
    }

    /// Check if root was freshly created and can be extended directly.
    /// Returns true if: root exists, was created during this read,
    /// has single child pattern, and has no parents.
    pub(crate) fn is_fresh_root(&self) -> bool {
        if !self.is_fresh {
            return false;
        }
        if let Some(root) = self.root {
            let vertex = root.vertex(&self.graph);
            vertex.child_patterns().len() == 1 && vertex.parents().is_empty()
        } else {
            false
        }
    }

    /// Commit a band state to the root.
    /// 
    /// Linear control flow:
    /// 1. Collapse the state to a single append_pattern
    /// 2. Append to existing root or create new root
    pub(crate) fn commit_state(
        &mut self,
        state: BandState,
    ) {
        use tracing::debug;

        // 1. Collapse state to single append_pattern
        let append_pattern = state.collapse(&mut self.graph);
        
        debug!(
            append_pattern = ?append_pattern,
            is_fresh = self.is_fresh,
            has_root = self.root.is_some(),
            "commit_state"
        );

        // 2. Append to root using unified logic
        self.append_collapsed(append_pattern);
    }

    /// Append a collapsed pattern to the root.
    /// 
    /// - No root → create fresh root from pattern
    /// - Fresh root with single pattern, no parents → extend in place
    /// - Otherwise → create new root [prev_root, ...append_pattern]
    fn append_collapsed(&mut self, append_pattern: Pattern) {
        use tracing::debug;

        // Handle empty pattern
        if append_pattern.is_empty() {
            debug!("Empty append_pattern, nothing to do");
            return;
        }

        // No root → create fresh from pattern
        let Some(root) = self.root else {
            debug!("No existing root, creating fresh");
            self.root = Some(if append_pattern.len() == 1 {
                append_pattern[0]
            } else {
                self.graph.insert_pattern(append_pattern.to_vec())
            });
            self.is_fresh = true;
            return;
        };

        // Check if we can extend in place
        let vertex = root.vertex(&self.graph);
        let can_extend = self.is_fresh 
            && vertex.child_patterns().len() == 1 
            && vertex.parents().is_empty()
            // Prevent self-reference: don't append if pattern contains root
            && !append_pattern.iter().any(|t| t.vertex_index() == root.vertex_index());

        self.root = Some(if can_extend {
            debug!("Extending fresh root in place");
            let (&pid, _) = vertex.expect_any_child_pattern();
            self.graph.append_to_pattern(root, pid, append_pattern)
        } else {
            debug!("Creating new combined root");
            self.is_fresh = false;
            let combined: Vec<Token> = std::iter::once(root)
                .chain(append_pattern.iter().cloned())
                .collect();
            self.graph.insert_pattern(combined)
        });
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
