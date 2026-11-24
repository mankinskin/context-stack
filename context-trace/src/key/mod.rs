//! Trace-cache keys: a token with a directed atom position (bottom-up or
//! top-down). Model-agnostic; shared by the row model (`legacy`) and the
//! positioned trace cache.

use std::{
    fmt::Debug,
    ops::{
        Add,
        AddAssign,
    },
};

use crate::{
    AtomPosition,
    TokenWidth,
    graph::vertex::{
        id::{
            CommittedIndex,
            VertexId,
        },
        has_vertex_index::HasVertexIndex,
        token::Token,
        wide::Wide,
    },
};

pub mod down;
pub mod up;
pub use down::{
    DownKey,
    DownPosition,
};
pub use up::{
    UpKey,
    UpPosition,
};

#[derive(Clone, Debug, Copy, Hash, Eq, PartialEq)]
pub enum DirectedPosition {
    BottomUp(UpPosition),
    TopDown(DownPosition),
}
pub trait HasAtomPosition {
    fn pos(&self) -> &AtomPosition;
}
impl HasAtomPosition for DirectedPosition {
    fn pos(&self) -> &AtomPosition {
        match self {
            Self::BottomUp(pos) => &pos.0,
            Self::TopDown(pos) => &pos.0,
        }
    }
}
impl DirectedPosition {
    pub fn flipped(self) -> Self {
        match self {
            Self::BottomUp(pos) => Self::TopDown(pos.flipped()),
            Self::TopDown(pos) => Self::BottomUp(pos.flipped()),
        }
    }
}

impl From<usize> for DirectedPosition {
    fn from(value: usize) -> Self {
        Self::BottomUp(value.into())
    }
}

impl Add<usize> for DirectedPosition {
    type Output = Self;
    fn add(
        self,
        rhs: usize,
    ) -> Self::Output {
        match self {
            Self::BottomUp(p) => Self::BottomUp(p + rhs),
            Self::TopDown(p) => Self::TopDown(p + rhs),
        }
    }
}

impl AddAssign<usize> for DirectedPosition {
    fn add_assign(
        &mut self,
        rhs: usize,
    ) {
        match self {
            Self::BottomUp(p) => *p += rhs,
            Self::TopDown(p) => *p += rhs,
        }
    }
}

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct DirectedKey<I: VertexId = CommittedIndex> {
    pub index: Token<I>,
    pub pos: DirectedPosition,
}
impl<I: VertexId> Wide for DirectedKey<I> {
    fn width(&self) -> TokenWidth {
        self.index.width()
    }
}
impl<I: VertexId> HasVertexIndex for DirectedKey<I> {
    type Index = I;
    fn vertex_index(&self) -> I {
        self.index.vertex_index()
    }
}
impl<I: VertexId> HasAtomPosition for DirectedKey<I> {
    fn pos(&self) -> &AtomPosition {
        self.pos.pos()
    }
}
impl<I: VertexId> DirectedKey<I> {
    pub fn new(
        index: Token<I>,
        pos: impl Into<DirectedPosition>,
    ) -> Self {
        Self {
            index,
            pos: pos.into(),
        }
    }
    pub fn up(
        index: Token<I>,
        pos: impl Into<UpPosition>,
    ) -> Self {
        Self {
            index,
            pos: DirectedPosition::BottomUp(pos.into()),
        }
    }
    pub fn down(
        index: Token<I>,
        pos: impl Into<DownPosition>,
    ) -> Self {
        Self {
            index,
            pos: DirectedPosition::TopDown(pos.into()),
        }
    }
    pub fn flipped(self) -> Self {
        Self {
            index: self.index,
            pos: self.pos.flipped(),
        }
    }
}

impl<I: VertexId> From<Token<I>> for DirectedKey<I> {
    fn from(index: Token<I>) -> Self {
        Self {
            pos: DirectedPosition::BottomUp(index.width().0.into()),
            index,
        }
    }
}

impl<I: VertexId> From<UpKey<I>> for DirectedKey<I> {
    fn from(key: UpKey<I>) -> Self {
        Self {
            index: key.index,
            pos: DirectedPosition::BottomUp(key.pos),
        }
    }
}

impl<I: VertexId> From<DownKey<I>> for DirectedKey<I> {
    fn from(key: DownKey<I>) -> Self {
        Self {
            index: key.index,
            pos: DirectedPosition::TopDown(key.pos),
        }
    }
}
