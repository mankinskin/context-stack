use std::ops::{
    Add,
    AddAssign,
};

use derive_more::derive::From;
use derive_new::new;

use crate::{
    graph::vertex::token::Token,
    path::mutators::move_path::key::AtomPosition,
    trace::cache::key::directed::{
        HasAtomPosition,
        down::DownPosition,
    },
};

#[derive(Clone, Debug, Copy, Hash, Eq, PartialEq, From)]
pub struct UpPosition(pub AtomPosition);

impl UpPosition {
    pub fn flipped(self) -> DownPosition {
        DownPosition(self.0)
    }
}

impl From<UpPosition> for usize {
    fn from(val: UpPosition) -> Self {
        usize::from(val.0)
    }
}
impl From<UpPosition> for AtomPosition {
    fn from(val: UpPosition) -> Self {
        val.0
    }
}
impl From<usize> for UpPosition {
    fn from(value: usize) -> Self {
        Self(value.into())
    }
}

impl AddAssign<usize> for UpPosition {
    fn add_assign(
        &mut self,
        rhs: usize,
    ) {
        self.0 += rhs;
    }
}

impl Add<usize> for UpPosition {
    type Output = Self;
    fn add(
        self,
        rhs: usize,
    ) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl std::fmt::Display for UpPosition {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter,
    ) -> std::fmt::Result {
        write!(f, "↑{}", self.0)
    }
}

impl crate::logging::compact_format::CompactFormat for UpPosition {
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
pub struct UpKey {
    pub index: Token,
    pub pos: UpPosition,
}

impl HasAtomPosition for UpKey {
    fn pos(&self) -> &AtomPosition {
        &self.pos.0
    }
}
