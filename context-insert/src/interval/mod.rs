use std::fmt::Debug;

use crate::{
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
        trace::states::SplitStates,
    },
};
use context_trace::*;

pub mod init;
pub(crate) mod partition;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntervalGraph {
    pub(crate) states: SplitStates,
    pub(crate) cache: SplitCache,
    pub(crate) root: Token,
    pub(crate) target_range: PartitionRange, // range of target partition indices in root
    pub(crate) required: RequiredPartitions, // required partition ranges for selective merge
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
    pub(crate) fn expect_mut(
        &mut self,
        key: &PosKey,
    ) -> &mut SplitPositionCache {
        self.get_mut(key).unwrap()
    }
}
