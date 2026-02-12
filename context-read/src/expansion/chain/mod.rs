pub(crate) mod band;
pub(crate) mod expand;
pub(crate) mod link;

use band::Band;
use context_insert::*;
use context_trace::*;
use tracing::debug;

use crate::expansion::chain::link::OverlapLink;

/// Represents the state of band expansion.
/// 
/// Constrains the state to only valid configurations:
/// - Single: one band, no overlap found yet (or after commit)
/// - WithOverlap: two bands with overlap link, ready for commit
#[derive(Clone, Debug)]
pub(crate) enum BandState {
    /// Single band, no overlap found (or after commit)
    Single(Band),
    /// Two bands with overlap link, ready for commit
    WithOverlap {
        /// Primary band: sequential expansion (appended tokens)
        primary: Band,
        /// Overlap band: [complement, expansion] decomposition  
        overlap: Band,
        /// Link containing paths for complement construction
        link: OverlapLink,
    },
}

impl Default for BandState {
    fn default() -> Self {
        // Default is a single empty band - should not normally be used
        BandState::Single(Band {
            pattern: Pattern::default(),
            start_bound: 0.into(),
            end_bound: 0.into(),
        })
    }
}

impl BandState {
    /// Create a new BandState with a single token
    pub(crate) fn new(index: Token) -> Self {
        let band = Band {
            pattern: Pattern::from(vec![index]),
            start_bound: 0.into(),
            end_bound: index.width().0.into(),
        };
        debug!(initial_band = ?band, "New BandState");
        BandState::Single(band)
    }

    /// Get the primary/single band reference
    pub(crate) fn primary(&self) -> &Band {
        match self {
            BandState::Single(band) => band,
            BandState::WithOverlap { primary, .. } => primary,
        }
    }

    /// Get the primary/single band mutably
    pub(crate) fn primary_mut(&mut self) -> &mut Band {
        match self {
            BandState::Single(band) => band,
            BandState::WithOverlap { primary, .. } => primary,
        }
    }

    /// Get the start token from the primary band
    pub(crate) fn start_token(&self) -> Token {
        self.primary().last_token()
    }

    /// Get the end bound of the primary band
    pub(crate) fn end_bound(&self) -> AtomPosition {
        self.primary().end_bound
    }

    /// Check if this state has an overlap
    pub(crate) fn has_overlap(&self) -> bool {
        matches!(self, BandState::WithOverlap { .. })
    }

    /// Append a token to the single band.
    /// Panics if called on WithOverlap state.
    pub(crate) fn append(&mut self, token: Token) {
        match self {
            BandState::Single(band) => {
                band.pattern.push(token);
                band.end_bound += token.width().0;
            }
            BandState::WithOverlap { .. } => {
                panic!("Cannot append to BandState::WithOverlap - must commit first");
            }
        }
    }

    /// Transition from Single to WithOverlap state
    pub(crate) fn set_overlap(
        self,
        overlap_band: Band,
        link: OverlapLink,
    ) -> Self {
        match self {
            BandState::Single(primary) => {
                debug!(
                    primary = ?primary,
                    overlap = ?overlap_band,
                    "Transitioning to WithOverlap"
                );
                BandState::WithOverlap {
                    primary,
                    overlap: overlap_band,
                    link,
                }
            }
            BandState::WithOverlap { .. } => {
                panic!("Already in WithOverlap state - must commit first");
            }
        }
    }

    /// Collapse the band state into a single pattern.
    /// 
    /// For Single: returns the band's pattern directly
    /// For WithOverlap: builds complements, creates bundled token with both decompositions
    pub(crate) fn collapse(self, graph: &mut HypergraphRef) -> Pattern {
        match self {
            BandState::Single(band) => {
                debug!(pattern = ?band.pattern, "Collapsing Single band");
                band.pattern
            }
            BandState::WithOverlap { primary, overlap, link } => {
                debug!(
                    primary = ?primary.pattern,
                    overlap = ?overlap.pattern,
                    link = ?link,
                    "Collapsing WithOverlap bands"
                );

                // Build prefix complement for primary band
                // (the part before overlap, from expansion's context)
                let prefix_complement = build_prefix_complement(&link, graph);
                
                // Build postfix complement for overlap band
                // (the part after overlap, from primary's context)
                let postfix_complement = build_postfix_complement(&link, &primary, graph);
                
                // Complete primary: [prefix_complement, ...original_pattern]
                let complete_primary: Pattern = if let Some(prefix) = prefix_complement {
                    let mut p = vec![prefix];
                    p.extend(primary.pattern.iter().cloned());
                    p.into()
                } else {
                    primary.pattern
                };
                
                // Complete overlap: [...overlap.pattern, postfix_complement]
                let complete_overlap: Pattern = if let Some(postfix) = postfix_complement {
                    let mut p = overlap.pattern.to_vec();
                    p.push(postfix);
                    p.into()
                } else {
                    overlap.pattern
                };

                debug!(
                    complete_primary = ?complete_primary,
                    complete_overlap = ?complete_overlap,
                    "Collapsed decompositions"
                );

                // Insert both decompositions as patterns of bundled token
                let bundled = graph.insert_patterns(vec![
                    complete_primary.to_vec(),
                    complete_overlap.to_vec(),
                ]);
                
                debug!(bundled = ?bundled, "Created bundled token");
                Pattern::from(vec![bundled])
            }
        }
    }
}

/// Build the prefix complement for the primary band.
/// Returns None if the overlap starts at position 0 (no prefix needed).
fn build_prefix_complement(
    link: &OverlapLink,
    graph: &HypergraphRef,
) -> Option<Token> {
    use context_trace::GraphRootChild;
    use context_trace::HasRootChildIndex;

    // The prefix complement comes from the expansion's perspective (search_path)
    // It's the part of the expansion before the overlap region
    let expansion_root = link.search_path.graph_root_child(graph);
    let overlap_start_in_expansion = link.search_path.root_child_index();

    debug!(
        expansion_root = ?expansion_root,
        overlap_start = overlap_start_in_expansion,
        "build_prefix_complement"
    );

    if overlap_start_in_expansion == 0 {
        debug!("No prefix complement needed (overlap at start)");
        return None;
    }

    // Build trace cache for the prefix range
    let cache = TraceCache::new(expansion_root);
    
    let init_interval = InitInterval {
        root: expansion_root,
        cache,
        end_bound: overlap_start_in_expansion.into(),
    };
    
    let prefix = graph.insert_init((), init_interval)
        .expect("prefix complement insert_init should succeed");
    
    debug!(prefix = ?prefix, "Built prefix complement");
    Some(prefix)
}

/// Build the postfix complement for the overlap band.
/// Returns None if the overlap ends at the primary's end (no postfix needed).
fn build_postfix_complement(
    link: &OverlapLink,
    primary: &Band,
    graph: &HypergraphRef,
) -> Option<Token> {
    use context_trace::GraphRootChild;
    use context_trace::HasRootChildIndex;

    // The postfix complement comes from the primary's perspective (child_path)
    // It's the part of the primary after the overlap region
    let primary_root = link.child_path.graph_root_child(graph);
    let overlap_start_in_primary = link.child_path.root_child_index();
    
    // The overlap ends at: overlap_start + overlap_width
    // We need to figure out where in the primary the overlap ends
    // The overlap width can be computed from the search_path's leaf
    let overlap_width = link.search_path.role_leaf_token::<Start, _>(graph)
        .map(|t| *t.width())
        .unwrap_or(0);
    
    let overlap_end_in_primary = overlap_start_in_primary + overlap_width;
    let primary_end = *primary.end_bound;

    debug!(
        primary_root = ?primary_root,
        overlap_start = overlap_start_in_primary,
        overlap_end = overlap_end_in_primary,
        primary_end = primary_end,
        "build_postfix_complement"
    );

    if overlap_end_in_primary >= primary_end {
        debug!("No postfix complement needed (overlap at end)");
        return None;
    }

    // Build the postfix from overlap_end to primary_end
    // We need to extract [overlap_end_in_primary..primary_end] from the primary root
    let cache = TraceCache::new(primary_root);
    
    // For postfix, we need range [overlap_end..primary_end]
    // This is a suffix extraction - might need different approach
    // For now, use the pattern directly from primary band's remaining tokens
    
    // Calculate which tokens in primary.pattern are after the overlap
    let mut acc = 0usize;
    let mut postfix_tokens = Vec::new();
    for token in primary.pattern.iter() {
        let token_end = acc + *token.width();
        if acc >= overlap_end_in_primary {
            // This token is entirely after the overlap
            postfix_tokens.push(*token);
        } else if token_end > overlap_end_in_primary {
            // This token partially overlaps - need to extract suffix
            // For simplicity, skip partial tokens for now
            // TODO: Handle partial token extraction
        }
        acc = token_end;
    }

    if postfix_tokens.is_empty() {
        debug!("No postfix tokens found");
        return None;
    }

    // Bundle postfix tokens if multiple
    let postfix = if postfix_tokens.len() == 1 {
        postfix_tokens[0]
    } else {
        graph.insert_pattern(postfix_tokens)
    };

    debug!(postfix = ?postfix, "Built postfix complement");
    Some(postfix)
}
