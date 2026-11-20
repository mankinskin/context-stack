use std::ops::{
    Add,
    AddAssign,
};

use derive_more::derive::From;
use derive_new::new;

use crate::{
    AtomPosition,
    TokenWidth,
    graph::vertex::{
        token::Token,
        wide::Wide,
    },
    key::{
        HasAtomPosition,
        up::UpPosition,
    },
    logging::compact_format::CompactFormat,
};

#[derive(Clone, Debug, Copy, Hash, Eq, PartialEq, From)]
pub struct DownPosition(pub AtomPosition);

impl DownPosition {
    pub fn flipped(self) -> UpPosition {
        UpPosition(self.0)
    }
}
impl From<DownPosition> for AtomPosition {
    fn from(val: DownPosition) -> Self {
        val.0
    }
}
impl From<DownPosition> for usize {
    fn from(val: DownPosition) -> Self {
        usize::from(val.0)
    }
}
impl From<usize> for DownPosition {
    fn from(value: usize) -> Self {
        Self(value.into())
    }
}

impl AddAssign<usize> for DownPosition {
    fn add_assign(
        &mut self,
        rhs: usize,
    ) {
        self.0 += rhs;
    }
}

impl Add<usize> for DownPosition {
    type Output = Self;
    fn add(
        self,
        rhs: usize,
    ) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl std::fmt::Display for DownPosition {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter,
    ) -> std::fmt::Result {
        write!(f, "↓{}", self.0)
    }
}

impl CompactFormat for DownPosition {
    fn fmt_compact(
        &self,
        f: &mut std::fmt::Formatter,
    ) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }

    fn fmt_indented(
        &self,
        f: &mut std::fmt::Formatter,
        _indent: usize,
    ) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }
}

#[derive(Clone, Debug, Hash, Eq, PartialEq, Copy, new)]
pub struct DownKey {
    pub index: Token,
    pub pos: DownPosition,
}

impl HasAtomPosition for DownKey {
    fn pos(&self) -> &AtomPosition {
        &self.pos.0
    }
}

impl Wide for DownKey {
    fn width(&self) -> TokenWidth {
        self.index.width()
    }
}
