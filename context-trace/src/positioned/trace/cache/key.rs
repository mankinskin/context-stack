//! Key accessors of trace states (legacy `cache::key::props`). The keys
//! themselves are the model-agnostic [`crate::key`] types.

use crate::{
    DirectedKey,
    UpKey,
};

pub trait RootKey {
    fn root_key(&self) -> UpKey;
}

pub trait TargetKey {
    fn target_key(&self) -> DirectedKey;
}
