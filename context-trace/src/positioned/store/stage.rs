//! The [`Stage`] contract of a store with an open stage (staging plan
//! §3.6), used by `GraphEdit`'s dissolution and stabilization. The overlay
//! implementation lives in `context_read::stage`.

use crate::{
    graph::vertex::{
        StageIndex,
        StagedIndex,
    },
    positioned::store::{
        GraphWrite,
        raw,
    },
};

/// A store with an open stage: indices are [`StageIndex`], new vertices are
/// staged, unstable staged vertices can be removed and staged vertices can
/// be marked stable. Staged vertices never move: a removal leaves a hole and
/// no index is ever renamed.
pub trait Stage: GraphWrite<Index = StageIndex> + raw::Stabilize {
    /// The live staged vertices in creation order (without removed slots).
    fn staged(&self) -> impl Iterator<Item = StagedIndex> + '_;
    /// Whether `index` is a live stable staged vertex. Stable vertices are
    /// never removed (cheap stabilization plan 20261126).
    fn is_stable(
        &self,
        index: StageIndex,
    ) -> bool;
}
