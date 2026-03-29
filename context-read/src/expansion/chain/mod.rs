pub(crate) mod band;
pub(crate) mod link;

use band::Band;
use context_insert::{
    OverlapBundleInput,
    ToInsertCtx,
};
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
        /// Paths needed to build overlap bundles during collapse.
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
    /// - `WithOverlap`: delegates structural overlap bundling to
    ///   `context-insert` and returns a one-element pattern containing the
    ///   bundled token.
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
                    "Collapsing WithOverlap bands via context-insert bundle_overlap"
                );

                let t1 = *primary.pattern.last().expect(
                    "primary pattern must contain the sequential token",
                );
                let t2 = *overlap
                    .pattern
                    .last()
                    .expect("overlap pattern must contain the expanded token");

                let bundled =
                    <HypergraphRef as ToInsertCtx<Token>>::bundle_overlap(
                        graph,
                        {
                            let mut input = OverlapBundleInput::new(
                                link.child_path,
                                link.search_path,
                                t1,
                                t2,
                            );
                            input.self_overlap = link.self_overlap;
                            input
                        },
                    )
                    .expect("bundle_overlap should succeed for WithOverlap collapse");

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
/// block expansion. The `head` token is the leftmost anchor; each
/// `OverlapLink` records one overlap step; and `tail` is the rightmost
/// expanded token.
///
/// # Usage
/// Chains are constructed via `BandState::into_chain` and grown with
/// `push`/`cap`. Full collapse is wired in Pass C3 once the semantic overlap
/// bundling path is stable.
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
        // TODO(Pass C3): implement chain termination and overlap collapse.
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
