use std::fmt::Debug;

use crate::legacy::{
    join::context::node::merge::{
        PartitionRange,
        RequiredPartitions,
    },
    split::{
        cache::{
            SplitCache,
            position::{
                PosKey,
                SplitPositionCache,
            },
        },
        provenance::SplitProvenance,
        trace::states::SplitStates,
    },
};
use derivative::Derivative;
use context_trace::legacy::*;

pub mod init;
pub(crate) mod partition;

#[derive(Debug, Clone, Derivative)]
#[derivative(PartialEq, Eq)]
pub struct IntervalGraph {
    pub(crate) states: SplitStates,
    pub(crate) cache: SplitCache,
    pub(crate) root: Token,
    pub(crate) target_range: PartitionRange, // range of target partition indices in root
    pub(crate) required: RequiredPartitions, // required partition ranges for selective merge
    /// Why positions were required and how children were reached.
    #[derivative(PartialEq = "ignore")]
    pub(crate) provenance: SplitProvenance,
}
impl IntervalGraph {
    pub(crate) fn get(
        &self,
        key: &PosKey,
    ) -> Option<&SplitPositionCache> {
        self.cache
            .get(&key.index.vertex_index())
            .and_then(|ve| ve.positions.get(&key.pos))
    }
    #[allow(dead_code)]
    pub(crate) fn get_mut(
        &mut self,
        key: &PosKey,
    ) -> Option<&mut SplitPositionCache> {
        self.cache
            .get_mut(&key.index.vertex_index())
            .and_then(|ve| ve.positions.get_mut(&key.pos))
    }
    pub(crate) fn expect(
        &self,
        key: &PosKey,
    ) -> &SplitPositionCache {
        self.get(key).unwrap()
    }
    #[allow(dead_code)]
    pub(crate) fn expect_mut(
        &mut self,
        key: &PosKey,
    ) -> &mut SplitPositionCache {
        self.get_mut(key).unwrap()
    }
}
