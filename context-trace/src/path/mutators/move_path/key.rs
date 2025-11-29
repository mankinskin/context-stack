use derive_more::{
    Add,
    Deref,
    DerefMut,
    Sub,
};

use crate::{
    TokenWidth,
    direction::{
        Direction,
        Left,
        Right,
    },
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
pub struct AtomPosition(pub(crate) usize);

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

pub trait MoveKey<D: Direction> {
    fn move_key(
        &mut self,
        delta: usize,
    );
}

impl<D: Direction, T: MoveKey<D>> MoveKey<D> for &'_ mut T {
    fn move_key(
        &mut self,
        delta: usize,
    ) {
        (*self).move_key(delta)
    }
}

pub(crate) trait AdvanceKey: MoveKey<Right> {
    fn advance_key(
        &mut self,
        delta: usize,
    ) {
        self.move_key(delta)
    }
}

impl<T: MoveKey<Right>> AdvanceKey for T {}

pub(crate) trait RetractKey: MoveKey<Left> {
    fn retract_key(
        &mut self,
        delta: usize,
    ) {
        self.move_key(delta)
    }
}

impl<T: MoveKey<Left>> RetractKey for T {}

impl MoveKey<Right> for AtomPosition {
    fn move_key(
        &mut self,
        delta: usize,
    ) {
        *self += delta;
    }
}

impl MoveKey<Left> for AtomPosition {
    fn move_key(
        &mut self,
        delta: usize,
    ) {
        *self -= delta;
    }
}
