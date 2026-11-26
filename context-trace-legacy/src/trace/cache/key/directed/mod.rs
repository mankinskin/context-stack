//! Re-exports of the promoted root keys ([`context_trace::key`]) plus their
//! row-model `MoveKey` impls.

use crate::{
    direction::Right,
    path::mutators::move_path::key::{
        AtomPosition,
        MoveKey,
    },
};

pub use context_trace::key::{
    DirectedKey,
    DirectedPosition,
    HasAtomPosition,
};

pub mod down {
    pub use context_trace::key::down::*;
}
pub mod up {
    pub use context_trace::key::up::*;
}

impl MoveKey<Right> for DirectedPosition {
    fn move_key(
        &mut self,
        delta: usize,
    ) {
        match self {
            DirectedPosition::BottomUp(up::UpPosition(p)) =>
                <AtomPosition as MoveKey<Right>>::move_key(p, delta),
            DirectedPosition::TopDown(down::DownPosition(p)) =>
                <AtomPosition as MoveKey<Right>>::move_key(p, delta),
        }
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
