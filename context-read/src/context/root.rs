use context_insert::*;
use context_trace::*;
use derive_more::{
    Deref,
    DerefMut,
};
use derive_new::new;

use crate::expansion::chain::BandChain;

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

    /// Commit a band chain to the root, adding overlap decompositions.
    ///
    /// The band chain contains:
    /// - First band: the initial block pattern (sequential expansion)
    /// - Additional bands: overlap decompositions `[complement, expansion]`
    ///
    /// If root is fresh (created during this read), extends the existing pattern.
    /// Otherwise, creates a new root with multiple child patterns for overlaps.
    pub(crate) fn commit_chain(
        &mut self,
        chain: BandChain,
    ) {
        use tracing::debug;

        // Get the first band's pattern - this is what we need to append
        let first_band = chain.bands.first().unwrap();
        let append_pattern = first_band.pattern.clone();
        
        debug!(
            append_pattern = ?append_pattern,
            num_bands = chain.bands.len(),
            is_fresh = self.is_fresh,
            "commit_chain"
        );

        // Collect overlap bands (all bands after the first)
        let overlap_patterns: Vec<Pattern> = chain
            .overlap_bands()
            .map(|band| band.pattern.clone())
            .collect();

        if self.is_fresh_root() {
            // Extend the existing pattern if possible
            let root = self.root.unwrap();
            let vertex = root.vertex(&self.graph);

            // Check if we can extend the pattern:
            // - root must have single child pattern and no parents
            let can_extend = vertex.child_patterns().len() == 1
                && vertex.parents().is_empty();

            if can_extend {
                let (&pid, _) = vertex.expect_any_child_pattern();
                // Append the entire pattern (may be multiple tokens like [b, a])
                self.root =
                    Some(self.graph.append_to_pattern(root, pid, append_pattern));
            } else {
                // Can't extend - create new combined pattern [root, ...append_pattern]
                let mut combined = vec![root];
                combined.extend(append_pattern.iter().cloned());
                self.root = Some(self.graph.insert_pattern(combined));
            }

            // Add overlap decompositions as additional child patterns
            if !overlap_patterns.is_empty() {
                debug!(
                    num_overlaps = overlap_patterns.len(),
                    "adding overlap patterns to fresh root"
                );
                self.graph.add_patterns_with_update(
                    self.root.unwrap(),
                    overlap_patterns,
                );
            }
        } else {
            // Create new combined root
            if let Some(old_root) = self.root {
                // Create pattern [old_root, ...append_pattern]
                let mut combined = vec![old_root];
                combined.extend(append_pattern.iter().cloned());
                let new_root = self.graph.insert_pattern(combined);

                // Add overlap decompositions as additional child patterns
                if !overlap_patterns.is_empty() {
                    debug!(
                        num_overlaps = overlap_patterns.len(),
                        "adding overlap patterns to new root"
                    );
                    self.graph
                        .add_patterns_with_update(new_root, overlap_patterns);
                }

                self.root = Some(new_root);
            } else {
                // No previous root - the append_pattern becomes the root
                if append_pattern.len() == 1 {
                    self.root = Some(append_pattern[0]);
                } else {
                    self.root = Some(self.graph.insert_pattern(append_pattern.to_vec()));
                }
                self.is_fresh = true;

                // Add overlap decompositions if any
                if !overlap_patterns.is_empty() && self.root.is_some() {
                    self.graph.add_patterns_with_update(
                        self.root.unwrap(),
                        overlap_patterns,
                    );
                }
            }
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
