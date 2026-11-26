pub use context_trace::position::AtomPosition;
use crate::direction::{
    Direction,
    Left,
    Right,
};

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
