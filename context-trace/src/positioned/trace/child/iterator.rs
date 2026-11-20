//! Queue of child states (legacy `trace::child::iterator::ChildQueue`).

use std::collections::VecDeque;

use derive_more::{
    Deref,
    DerefMut,
    From,
    IntoIterator,
};

use super::state::ChildState;

#[derive(Debug, Clone, Default, Deref, DerefMut, IntoIterator, From)]
pub struct ChildQueue<S> {
    queue: VecDeque<S>,
}

impl<S> FromIterator<S> for ChildQueue<S> {
    fn from_iter<T: IntoIterator<Item = S>>(iter: T) -> Self {
        Self {
            queue: VecDeque::from_iter(iter),
        }
    }
}

impl From<ChildState> for ChildQueue<ChildState> {
    fn from(state: ChildState) -> Self {
        FromIterator::from_iter([state])
    }
}
