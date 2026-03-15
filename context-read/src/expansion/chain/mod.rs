pub(crate) mod band;
pub(crate) mod link;

use band::Band;
use context_insert::*;
use context_trace::*;
use tracing::debug;

use crate::expansion::chain::link::{
    BandCapLink,
    OverlapLink,
};

/// Represents the state of band expansion.
///
/// - `Single`: one band, no overlap found (or after commit).
/// - `WithOverlap`: two bands with an overlap link, ready for collapse and commit.
#[derive(Clone, Debug)]
pub(crate) enum BandState {
    Single {
        band: Band,
    },
    WithOverlap {
        /// Primary band: sequential expansion (appended tokens).
        primary: Band,
        /// Overlap band: `[complement, expansion]` decomposition.
        overlap: Band,
        /// Paths needed to build complement tokens during collapse.
        link: OverlapLink,
    },
}

impl Default for BandState {
    fn default() -> Self {
        BandState::Single {
            band: Band {
                pattern: Pattern::default(),
                start_bound: 0.into(),
                end_bound: 0.into(),
            },
        }
    }
}

impl BandState {
    /// Create a `Single` state for a single token.
    pub(crate) fn new(index: Token) -> Self {
        let band = Band {
            pattern: Pattern::from(vec![index]),
            start_bound: 0.into(),
            end_bound: index.width().0.into(),
        };
        debug!(initial_band = ?band, "New BandState");
        BandState::Single { band }
    }

    pub(crate) fn primary(&self) -> &Band {
        match self {
            BandState::Single { band, .. } => band,
            BandState::WithOverlap { primary, .. } => primary,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn primary_mut(&mut self) -> &mut Band {
        match self {
            BandState::Single { band, .. } => band,
            BandState::WithOverlap { primary, .. } => primary,
        }
    }

    pub(crate) fn anchor_token(&self) -> Option<Token> {
        match self {
            BandState::Single { band } => band.pattern.last().copied(),
            BandState::WithOverlap { primary, .. } =>
                primary.pattern.last().copied(),
        }
    }

    pub(crate) fn end_bound(&self) -> AtomPosition {
        self.primary().end_bound
    }

    pub(crate) fn has_overlap(&self) -> bool {
        matches!(self, BandState::WithOverlap { .. })
    }

    #[allow(dead_code)]
    pub(crate) fn is_empty(&self) -> bool {
        self.primary().pattern.is_empty()
    }

    /// If this is a `Single` state with exactly one token, return that token.
    pub(crate) fn single_token(&self) -> Option<Token> {
        match self {
            BandState::Single { band } if band.pattern.len() == 1 =>
                band.pattern.first().copied(),
            _ => None,
        }
    }

    /// Append a token to the single band.
    ///
    /// Panics when called on a `WithOverlap` state.
    pub(crate) fn append(
        &mut self,
        token: Token,
    ) {
        match self {
            BandState::Single { band, .. } => {
                band.pattern.push(token);
                band.end_bound += token.width().0;
            },
            BandState::WithOverlap { .. } => {
                panic!(
                    "Cannot append to BandState::WithOverlap — commit first"
                );
            },
        }
    }

    /// Transition from `Single` to `WithOverlap`.
    ///
    /// Panics when already in `WithOverlap` state.
    pub(crate) fn set_overlap(
        self,
        overlap_band: Band,
        link: OverlapLink,
    ) -> Self {
        match self {
            BandState::Single { band: primary, .. } => {
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
            },
            BandState::WithOverlap { .. } => {
                panic!("Already in WithOverlap state — commit first");
            },
        }
    }

    /// Collapse the band state into a single-element pattern.
    ///
    /// - `Single`: returns the band's pattern unchanged.
    /// - `WithOverlap`: builds complement tokens, inserts both decompositions,
    ///   and returns a one-element pattern containing the bundled token.
    pub(crate) fn collapse(
        self,
        graph: &mut HypergraphRef,
    ) -> Pattern {
        match self {
            BandState::Single { band, .. } => {
                debug!(pattern = ?band.pattern, "Collapsing Single band");
                band.pattern
            },
            BandState::WithOverlap {
                primary,
                overlap,
                link,
            } => {
                debug!(
                    primary = ?primary.pattern,
                    overlap = ?overlap.pattern,
                    link = ?link,
                    "Collapsing WithOverlap bands"
                );

                let prefix_complement = build_prefix_complement(&link, graph);
                let postfix_complement =
                    build_postfix_complement(&link, &primary, graph);

                let complete_primary: Pattern =
                    if let Some(prefix) = prefix_complement {
                        let mut p = vec![prefix];
                        p.extend(primary.pattern.iter().cloned());
                        p.into()
                    } else {
                        primary.pattern
                    };

                let complete_overlap: Pattern =
                    if let Some(postfix) = postfix_complement {
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

                let bundled = graph.insert_patterns(vec![
                    complete_primary.to_vec(),
                    complete_overlap.to_vec(),
                ]);

                debug!(bundled = ?bundled, "Created bundled token");
                Pattern::from(vec![bundled])
            },
        }
    }
}

// ---------------------------------------------------------------------------
// OverlapChain — ordered sequence of overlapping bands
// ---------------------------------------------------------------------------

/// A chain of overlapping expansion bands, ready for collapse.
///
/// `OverlapChain` captures a sequence of consecutive overlaps found during
/// block expansion.  The `head` token is the leftmost anchor; each
/// `OverlapLink` records one overlap step; and `tail` is the rightmost
/// expanded token.
///
/// # Usage
/// Chains are constructed via `BandState::into_chain` and grown with
/// `push`/`cap`.  Full collapse is wired in Pass C3 once the complement
/// design session is complete.
#[derive(Clone, Debug)]
pub(crate) struct OverlapChain {
    /// The leftmost anchor token at the start of the chain.
    pub(crate) head: Token,
    /// Ordered overlap steps from head toward tail.
    pub(crate) links: Vec<OverlapLink>,
    /// The rightmost expanded token at the end of the chain.
    pub(crate) tail: Token,
}

impl OverlapChain {
    /// Append another overlap link to the chain.
    ///
    /// # Panics
    /// Panics in debug builds if the chain has already been capped.
    /// (Full validation is deferred to Pass C3.)
    pub(crate) fn push(
        &mut self,
        link: OverlapLink,
    ) {
        // TODO(Pass C3): validate that `link.start_bound` is consistent with
        // the previous link's end position.
        self.links.push(link);
    }

    /// Terminate the chain with a cap link.
    ///
    /// After calling `cap`, no more `push` calls should be made.
    ///
    /// # Panics
    /// This is a stub — full implementation is deferred to Pass C3.
    #[allow(dead_code)]
    pub(crate) fn cap(
        &mut self,
        _link: BandCapLink,
    ) {
        // TODO(Pass C3): implement chain termination and complement collapse.
        unimplemented!("OverlapChain::cap — deferred to Pass C3");
    }
}

impl BandState {
    /// Lift a `WithOverlap` state into an `OverlapChain`.
    ///
    /// - `Single` states return `None` (there is no chain to build).
    /// - `WithOverlap` states return a chain whose `head` and `tail` are the
    ///   primary and overlap tokens respectively, with the single overlap link.
    ///
    /// Pass C3 will replace direct `collapse` calls with chain-based collapse.
    pub(crate) fn into_chain(self) -> Option<OverlapChain> {
        match self {
            BandState::WithOverlap {
                ref primary,
                ref overlap,
                ref link,
            } => {
                let head = *primary
                    .pattern
                    .last()
                    .expect("primary pattern must be non-empty");
                let tail = *overlap
                    .pattern
                    .last()
                    .expect("overlap pattern must be non-empty");
                Some(OverlapChain {
                    head,
                    links: vec![link.clone()],
                    tail,
                })
            },
            BandState::Single { .. } => None,
        }
    }
}

fn build_prefix_complement(
    link: &OverlapLink,
    graph: &HypergraphRef,
) -> Option<Token> {
    use context_trace::{
        GraphRootChild,
        HasRootChildIndex,
    };

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

    let cache = TraceCache::new(expansion_root);
    let init_interval = InitInterval {
        root: expansion_root,
        cache,
        end_bound: overlap_start_in_expansion.into(),
    };

    let prefix = graph
        .insert_init((), init_interval)
        .expect("prefix complement insert_init should succeed");

    debug!(prefix = ?prefix, "Built prefix complement");
    Some(prefix)
}

fn build_postfix_complement(
    link: &OverlapLink,
    primary: &Band,
    graph: &HypergraphRef,
) -> Option<Token> {
    use context_trace::{
        GraphRootChild,
        HasRootChildIndex,
    };

    let primary_root = link.child_path.graph_root_child(graph);
    let overlap_start_in_primary = link.child_path.root_child_index();

    let overlap_width = link
        .search_path
        .role_leaf_token::<Start, _>(graph)
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

    let mut acc = 0usize;
    let mut postfix_tokens = Vec::new();
    for token in primary.pattern.iter() {
        let token_end = acc + *token.width();
        if acc >= overlap_end_in_primary {
            postfix_tokens.push(*token);
        } else if token_end > overlap_end_in_primary {
            // Partial token overlap — skipped for now.
        }
        acc = token_end;
    }

    if postfix_tokens.is_empty() {
        debug!("No postfix tokens found");
        return None;
    }

    let postfix = if postfix_tokens.len() == 1 {
        postfix_tokens[0]
    } else {
        graph.insert_pattern(postfix_tokens)
    };

    debug!(postfix = ?postfix, "Built postfix complement");
    Some(postfix)
}
