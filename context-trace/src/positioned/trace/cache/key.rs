//! Key accessors of trace states (legacy `cache::key::props`). The keys
//! themselves are the model-agnostic [`crate::key`] types.

use crate::{
    CommittedIndex,
    DirectedKey,
    UpKey,
    VertexId,
};

pub trait RootKey<I: VertexId = CommittedIndex> {
    fn root_key(&self) -> UpKey<I>;
}

pub trait TargetKey<I: VertexId = CommittedIndex> {
    fn target_key(&self) -> DirectedKey<I>;
}
