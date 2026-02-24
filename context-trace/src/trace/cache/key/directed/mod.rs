use std::{
    fmt::Debug,
    ops::{
        Add,
        AddAssign,
    },
};

use crate::{
    TokenWidth,
    direction::Right,
    graph::vertex::{
        VertexIndex,
        has_vertex_index::HasVertexIndex,
        token::Token,
        wide::Wide,
    },
    path::mutators::move_path::key::{
        AtomPosition,
        MoveKey,
    },
};

pub mod down;
pub mod up;
use down::{
    DownKey,
    DownPosition,
};
use up::{
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

impl MoveKey<Right> for DirectedPosition {
    fn move_key(
        &mut self,
        delta: usize,
    ) {
        match self {
            DirectedPosition::BottomUp(UpPosition(p)) =>
                <AtomPosition as MoveKey<Right>>::move_key(p, delta),
            DirectedPosition::TopDown(DownPosition(p)) =>
                <AtomPosition as MoveKey<Right>>::move_key(p, delta),
        }
    }
}

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct DirectedKey {
    pub index: Token,
    pub pos: DirectedPosition,
}
impl Wide for DirectedKey {
    fn width(&self) -> TokenWidth {
        self.index.width()
    }
}
impl HasVertexIndex for DirectedKey {
    fn vertex_index(&self) -> VertexIndex {
        self.index.vertex_index()
    }
}
impl HasAtomPosition for DirectedKey {
    fn pos(&self) -> &AtomPosition {
        self.pos.pos()
    }
}
impl MoveKey<Right> for DirectedKey {
    fn move_key(
        &mut self,
        delta: usize,
    ) {
        self.pos.move_key(delta)
    }
}

impl DirectedKey {
    pub fn new(
        index: Token,
        pos: impl Into<DirectedPosition>,
    ) -> Self {
        Self {
            index,
            pos: pos.into(),
        }
    }
    pub fn up(
        index: Token,
        pos: impl Into<UpPosition>,
    ) -> Self {
        Self {
            index,
            pos: DirectedPosition::BottomUp(pos.into()),
        }
    }
    pub fn down(
        index: Token,
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

impl From<Token> for DirectedKey {
    fn from(index: Token) -> Self {
        Self {
            pos: DirectedPosition::BottomUp(index.width().0.into()),
            index,
        }
    }
}

impl From<UpKey> for DirectedKey {
    fn from(key: UpKey) -> Self {
        Self {
            index: key.index,
            pos: DirectedPosition::BottomUp(key.pos),
        }
    }
}

impl From<DownKey> for DirectedKey {
    fn from(key: DownKey) -> Self {
        Self {
            index: key.index,
            pos: DirectedPosition::TopDown(key.pos),
        }
    }
}
