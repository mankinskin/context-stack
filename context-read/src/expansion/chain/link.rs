//! Overlap link types used to represent one step in a resolved overlap chain.
//!
//! An [`OverlapLink`] captures the dual-path view of a single overlap region
//! between two consecutive tokens during band expansion.  A [`BandCapLink`]
//! records the terminal node of a fully-resolved chain.

use context_trace::*;

/// Represents the overlap between two tokens in a decomposition.
///
/// When a postfix of the current root expands into remaining pattern, it creates
/// an overlap region that can be viewed from two perspectives:
///
/// 1. From the first token's view: `child_path` - a top-down path from the starting
///    root to the expandable postfix (the overlap region token)
/// 2. From the second token's view: `search_path` - a bottom-up then top-down path
///    from the expansion (the same overlap region, but from expansion's perspective)
///
/// This link helps retrieve or build complement tokens to convert from an overlap
/// chain representation to the full set of decompositions.
#[derive(Clone, Debug)]
pub(crate) struct OverlapLink {
    /// Top-down child path from starting root to the expandable postfix.
    /// This represents the overlap region token from the first token's perspective.
    pub(crate) child_path: IndexEndPath,

    /// Bottom-up then top-down search path from the expansion.
    /// This represents the overlap region token from the second token's perspective.
    pub(crate) search_path: IndexStartPath,

    /// Position where the overlap starts in the input sequence.
    #[allow(dead_code)]
    pub(crate) start_bound: usize,
}

/// Terminal node of a resolved overlap chain.
///
/// A `BandCapLink` records the final postfix path of a fully-resolved chain.
/// Unlike `OverlapLink`, it has no `search_path` because the chain has been
/// terminated — there is no further expansion to anchor.
///
/// Invariant: a `BandCapLink` appears only as the last element of an
/// `OverlapChain::links` list, after all intermediate `OverlapLink`s.
#[derive(Clone, Debug)]
pub(crate) struct BandCapLink {
    /// Top-down child path from the root to the terminal postfix.
    pub(crate) child_path: IndexEndPath,
    /// Position where the terminal overlap starts in the input sequence.
    pub(crate) start_bound: usize,
}
