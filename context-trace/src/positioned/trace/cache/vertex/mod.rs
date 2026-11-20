//! Cache entries of one vertex (legacy `cache::vertex`).

pub mod positions;

use positions::DirectedPositions;

use crate::{
    DirectedPosition,
    HasAtomPosition,
    Token,
    positioned::trace::cache::position::PositionCache,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VertexCache {
    pub bottom_up: DirectedPositions,
    pub top_down: DirectedPositions,
    pub index: Token,
}

impl VertexCache {
    pub fn start(index: Token) -> Self {
        Self::from(index)
    }
    pub fn dir(
        &self,
        pos: &DirectedPosition,
    ) -> &DirectedPositions {
        match pos {
            DirectedPosition::BottomUp(_) => &self.bottom_up,
            DirectedPosition::TopDown(_) => &self.top_down,
        }
    }
    pub fn dir_mut(
        &mut self,
        pos: &DirectedPosition,
    ) -> &mut DirectedPositions {
        match pos {
            DirectedPosition::BottomUp(_) => &mut self.bottom_up,
            DirectedPosition::TopDown(_) => &mut self.top_down,
        }
    }
    pub fn get(
        &self,
        pos: &DirectedPosition,
    ) -> Option<&PositionCache> {
        self.dir(pos).get(pos.pos())
    }
    pub fn get_mut(
        &mut self,
        pos: &DirectedPosition,
    ) -> Option<&mut PositionCache> {
        self.dir_mut(pos).get_mut(pos.pos())
    }
    pub fn insert(
        &mut self,
        pos: &DirectedPosition,
        cache: PositionCache,
    ) {
        self.dir_mut(pos).insert(*pos.pos(), cache);
    }
}

impl From<Token> for VertexCache {
    fn from(index: Token) -> Self {
        Self {
            index,
            bottom_up: Default::default(),
            top_down: Default::default(),
        }
    }
}
