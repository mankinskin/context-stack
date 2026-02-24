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
                        let new = new.into_pattern();
                        self.graph
                            .insert_pattern([&[*root], new.as_slice()].concat())
                    };
                } else {
                    let c = self.graph.insert_pattern(new);
                    self.root = Some(c);
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
                self.graph.insert_pattern(vec![*root, token])
            };
        } else {
            self.root = Some(token);
        }
    }

    /// Get the last child token of the root (for overlap detection).
    /// Returns the rightmost token in the root's pattern, or the root itself if atomic.
    pub(crate) fn last_child_token(&self) -> Option<Token> {
        let root = self.root?;
        let vertex = root.vertex(&self.graph);

        // If root has child patterns, get the last token from the first pattern
        if let Some((&_pid, pattern)) = vertex.child_patterns().iter().next() {
            pattern.last().copied()
        } else {
            // Atomic token - return root itself
            Some(root)
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
            has_root = self.root.is_some(),
            "commit_state"
        );

        // 2. Append to root using unified logic
        self.append_collapsed(append_pattern);
    }

    /// Append a collapsed pattern to the root.
    ///
    /// - No root → create fresh root from pattern
    /// - With root → check for compound overlap, then append
    ///
    /// Compound overlap: root's last child equals append[0]'s first child (both compound).
    /// This creates both decompositions: [root, append] and [overlap_prefix, rest].
    fn append_collapsed(
        &mut self,
        append_pattern: Pattern,
    ) {
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
            return;
        };

        let root_last = self.last_child_of(root);
        let append_first = append_pattern[0];
        let append_first_first = self.first_child_of(append_first);

        // Cursor-level overlap: root is atomic AND equals append[0]
        // e.g., "aaa": root="a", append=[a,a] → create "aa" with both decompositions
        // This only triggers when root itself is atomic (root == root_last)
        if let Some(r_last) = root_last {
            let root_is_atomic = root == r_last;
            if root_is_atomic && r_last == append_first {
                debug!(
                    root = ?root,
                    append_first = ?append_first,
                    "Cursor-level overlap (atomic root) - creating decompositions"
                );

                // Bundle the append pattern
                let bundled =
                    self.graph.insert_pattern(append_pattern.to_vec());

                // Standard decomposition: [root, bundled]
                let standard = vec![root, bundled];

                // Overlap decomposition: [root_extended, rest]
                let root_extended =
                    self.graph.insert_pattern(vec![root, append_first]);
                let mut overlap = vec![root_extended];
                overlap.extend(append_pattern[1..].iter().cloned());

                debug!(standard = ?standard, overlap = ?overlap, "Inserting both decompositions");
                self.root =
                    Some(self.graph.insert_patterns(vec![standard, overlap]));
                return;
            }
        }

        // Compound overlap: root's last child equals append[0]'s first child
        // Only applies when append[0] is compound (has children)
        if let (Some(r_last), Some(a_first)) = (root_last, append_first_first) {
            // Check that append_first is actually compound (not atomic)
            let is_compound = a_first != append_first;
            if r_last == a_first && is_compound {
                debug!(
                    root_last = ?r_last,
                    append_first_first = ?a_first,
                    "Compound overlap - creating both decompositions"
                );

                // Standard decomposition: [root, append...]
                let standard: Vec<Token> = std::iter::once(root)
                    .chain(append_pattern.iter().cloned())
                    .collect();

                // Overlap decomposition
                if let Some(overlap) =
                    self.build_overlap_decomposition(root, &append_pattern)
                {
                    debug!(standard = ?standard, overlap = ?overlap, "Inserting both decompositions");
                    self.root = Some(
                        self.graph.insert_patterns(vec![standard, overlap]),
                    );
                    return;
                }
            }
        }

        // No overlap - standard append logic
        let vertex = root.vertex(&self.graph);
        let can_extend = vertex.child_patterns().len() == 1
            && vertex.parents().is_empty()
            && !append_pattern
                .iter()
                .any(|t| t.vertex_index() == root.vertex_index());

        self.root = Some(if can_extend {
            debug!("Extending root in place");
            let (&pid, _) = vertex.expect_any_child_pattern();
            self.graph.append_to_pattern(root, pid, append_pattern)
        } else {
            debug!("Creating new combined root");
            let combined: Vec<Token> = std::iter::once(root)
                .chain(append_pattern.iter().cloned())
                .collect();
            self.graph.insert_pattern(combined)
        });
    }

    /// Get the last child token of a token (or the token itself if atomic)
    fn last_child_of(
        &self,
        token: Token,
    ) -> Option<Token> {
        let vertex = token.vertex(&self.graph);
        if let Some((&_pid, pattern)) = vertex.child_patterns().iter().next() {
            pattern.last().copied()
        } else {
            Some(token)
        }
    }

    /// Get the first child token of a token (or the token itself if atomic)
    fn first_child_of(
        &self,
        token: Token,
    ) -> Option<Token> {
        let vertex = token.vertex(&self.graph);
        if let Some((&_pid, pattern)) = vertex.child_patterns().iter().next() {
            pattern.first().copied()
        } else {
            Some(token)
        }
    }

    /// Build the overlap decomposition: [overlap_prefix, rest_of_append]
    /// where overlap_prefix = [root, first_child_of_append[0]]
    fn build_overlap_decomposition(
        &mut self,
        root: Token,
        append_pattern: &Pattern,
    ) -> Option<Vec<Token>> {
        let append_first = append_pattern[0];
        let vertex = append_first.vertex(&self.graph);

        // Get children of append[0]
        let (&_pid, children) = vertex.child_patterns().iter().next()?;

        if children.is_empty() {
            return None;
        }

        // overlap_prefix = [root, first_child]
        let first_child = children[0];
        let overlap_prefix = self.graph.insert_pattern(vec![root, first_child]);

        // rest_of_append = [children[1..], append[1..]]
        let mut rest = Vec::new();
        if children.len() > 1 {
            // Bundle remaining children of append[0]
            let remaining_children: Vec<Token> = children[1..].to_vec();
            let remaining = if remaining_children.len() == 1 {
                remaining_children[0]
            } else {
                self.graph.insert_pattern(remaining_children)
            };
            rest.push(remaining);
        }
        rest.extend(append_pattern[1..].iter().cloned());

        // Full overlap decomposition: [overlap_prefix, rest...]
        let mut result = vec![overlap_prefix];
        result.extend(rest);

        Some(result)
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
