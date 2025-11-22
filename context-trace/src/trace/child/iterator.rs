use std::{
    collections::VecDeque,
    fmt::Debug,
};

use crate::*;
use derive_more::{
    Deref,
    DerefMut,
    From,
    IntoIterator,
};
pub trait TraceKind: Debug {
    type Trav: HasGraph;
}
impl<'a, K: TraceKind> TraceKind for &'a K {
    type Trav = &'a K::Trav;
}
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
pub struct ChildIterator<K: TraceKind, S: QueuedState = ChildState> {
    pub queue: ChildQueue<S>,
    pub trav: K::Trav,
}
impl<T: TraceKind, S: QueuedState> ChildIterator<T, S> {
    pub fn new(
        trav: T::Trav,
        queue: impl Into<ChildQueue<S>>,
    ) -> Self {
        Self {
            queue: queue.into(),
            trav,
        }
    }
}

impl<T: TraceKind, S: QueuedState> Iterator for ChildIterator<T, S> {
    type Item = S;
    fn next(&mut self) -> Option<Self::Item> {
        self.queue.pop_front()
    }
}
