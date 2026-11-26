use derive_more::{
    Add,
    Deref,
    DerefMut,
    Sub,
};

use crate::{
    TokenWidth,
    logging::compact_format::CompactFormat,
};

#[derive(
    Clone,
    Debug,
    Copy,
    Hash,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Add,
    Sub,
    Deref,
    DerefMut,
    Default,
)]
pub struct AtomPosition(pub usize);

impl From<TokenWidth> for AtomPosition {
    fn from(width: TokenWidth) -> Self {
        Self(width.0)
    }
}
impl std::fmt::Display for AtomPosition {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter,
    ) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl CompactFormat for AtomPosition {
    fn fmt_compact(
        &self,
        f: &mut std::fmt::Formatter,
    ) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }

    fn fmt_indented(
        &self,
        f: &mut std::fmt::Formatter,
        _indent: usize,
    ) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsMut<usize> for AtomPosition {
    fn as_mut(&mut self) -> &mut usize {
        &mut self.0
    }
}
impl AsRef<usize> for AtomPosition {
    fn as_ref(&self) -> &usize {
        &self.0
    }
}
impl From<AtomPosition> for usize {
    fn from(val: AtomPosition) -> Self {
        val.0
    }
}
impl From<usize> for AtomPosition {
    fn from(pos: usize) -> Self {
        Self(pos)
    }
}

impl std::ops::Add<usize> for AtomPosition {
    type Output = Self;
    fn add(
        mut self,
        delta: usize,
    ) -> Self {
        self.0 += delta;
        self
    }
}

impl std::ops::Sub<usize> for AtomPosition {
    type Output = Self;
    fn sub(
        mut self,
        delta: usize,
    ) -> Self {
        self.0 -= delta;
        self
    }
}

impl std::ops::AddAssign<usize> for AtomPosition {
    fn add_assign(
        &mut self,
        delta: usize,
    ) {
        self.0 += delta;
    }
}

impl std::ops::SubAssign<usize> for AtomPosition {
    fn sub_assign(
        &mut self,
        delta: usize,
    ) {
        self.0 -= delta;
    }
}
