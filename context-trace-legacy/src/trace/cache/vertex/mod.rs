use crate::{
    trace::cache::{
        key::directed::HasAtomPosition,
        position::PositionCache,
    },
    *,
};

pub mod positions;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VertexCache {
    pub bottom_up: DirectedPositions,
    pub top_down: DirectedPositions,
    pub index: Token,
}
impl VertexCache {
    pub fn start(index: Token) -> Self {
        let bottom_up = Default::default();
        //bottom_up.insert(
        //    index.width().into(),
        //    PositionCache::start(index)
        //);
        Self {
            bottom_up,
            index,
            top_down: Default::default(),
        }
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
    pub(crate) fn dir_mut(
        &mut self,
        pos: &DirectedPosition,
    ) -> &mut DirectedPositions {
        match pos {
            DirectedPosition::BottomUp(_) => &mut self.bottom_up,
            DirectedPosition::TopDown(_) => &mut self.top_down,
        }
    }
    pub(crate) fn get(
        &self,
        pos: &DirectedPosition,
    ) -> Option<&PositionCache> {
        self.dir(pos).get(pos.pos())
    }
    pub(crate) fn get_mut(
        &mut self,
        pos: &DirectedPosition,
    ) -> Option<&mut PositionCache> {
        self.dir_mut(pos).get_mut(pos.pos())
    }
    pub(crate) fn insert(
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
