//! Cache entries of one vertex (legacy `cache::vertex`).

pub mod positions;

use positions::DirectedPositions;

use crate::{
    DirectedPosition,
    HasAtomPosition,
    trace::cache::position::PositionCache,
};
use context_trace::{
    graph::{
        vertex::{
            CommittedIndex,
            VertexId,
        },
    },
    Token,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VertexCache<I: VertexId = CommittedIndex> {
    pub bottom_up: DirectedPositions<I>,
    pub top_down: DirectedPositions<I>,
    pub index: Token<I>,
}

impl<I: VertexId> VertexCache<I> {
    pub fn start(index: Token<I>) -> Self {
        Self::from(index)
    }
    pub fn dir(
        &self,
        pos: &DirectedPosition,
    ) -> &DirectedPositions<I> {
        match pos {
            DirectedPosition::BottomUp(_) => &self.bottom_up,
            DirectedPosition::TopDown(_) => &self.top_down,
        }
    }
    pub fn dir_mut(
        &mut self,
        pos: &DirectedPosition,
    ) -> &mut DirectedPositions<I> {
        match pos {
            DirectedPosition::BottomUp(_) => &mut self.bottom_up,
            DirectedPosition::TopDown(_) => &mut self.top_down,
        }
    }
    pub fn get(
        &self,
        pos: &DirectedPosition,
    ) -> Option<&PositionCache<I>> {
        self.dir(pos).get(pos.pos())
    }
    pub fn get_mut(
        &mut self,
        pos: &DirectedPosition,
    ) -> Option<&mut PositionCache<I>> {
        self.dir_mut(pos).get_mut(pos.pos())
    }
    pub fn insert(
        &mut self,
        pos: &DirectedPosition,
        cache: PositionCache<I>,
    ) {
        self.dir_mut(pos).insert(*pos.pos(), cache);
    }
}

impl<I: VertexId> From<Token<I>> for VertexCache<I> {
    fn from(index: Token<I>) -> Self {
        Self {
            index,
            bottom_up: Default::default(),
            top_down: Default::default(),
        }
    }
}
