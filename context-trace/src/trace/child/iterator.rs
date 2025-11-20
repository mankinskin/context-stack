use std::{
    collections::VecDeque,
    fmt::Debug,
};

use derive_more::{
    Deref,
    DerefMut,
    From,
    IntoIterator,
};

use crate::*;

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
//pub(crate) type ChildQueue = VecDeque<TokenModeCtx>;
pub trait QueuedState {}
impl<T> QueuedState for T {}

#[derive(Debug)]
pub struct ChildIterator<G: HasGraph, S: QueuedState = ChildState> {
    pub queue: ChildQueue<S>,
    pub trav: G,
}
impl<G: HasGraph, S: QueuedState> ChildIterator<G, S> {
    pub fn new(
        trav: G,
        queue: impl Into<ChildQueue<S>>,
    ) -> Self {
        Self {
            queue: queue.into(),
            trav,
        }
    }
}

impl<G: HasGraph, S: QueuedState> Iterator for ChildIterator<G, S> {
    type Item = S;
    fn next(&mut self) -> Option<Self::Item> {
        self.queue.pop_front()
    }
}
