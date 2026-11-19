use crate::legacy::{
    UpKey,
    graph::vertex::location::child::ChildLocation,
};

pub trait LeafKey {
    fn leaf_location(&self) -> ChildLocation;
}
use crate::legacy::path::mutators::move_path::key::AtomPosition;

use crate::legacy::trace::cache::key::directed::DirectedKey;

/// get the atom position in a cursor
pub trait CursorPosition {
    fn cursor_pos(&self) -> &AtomPosition;
    fn cursor_pos_mut(&mut self) -> &mut AtomPosition;
}
#[macro_export]
macro_rules! impl_cursor_pos {
    {
        $(< $( $par:ident $( : $bhead:tt $( + $btail:tt )*)? ),* >)? CursorPosition for $target:ty, $self_:ident => $func:expr
    } => {
        impl <$( $( $par $(: $bhead $( + $btail )* )? ),* )?> $crate::legacy::CursorPosition for $target {
            fn cursor_pos(& $self_) -> &$crate::legacy::AtomPosition {
                &$func
            }
            fn cursor_pos_mut(&mut $self_) -> &mut $crate::legacy::AtomPosition {
                &mut $func
            }
        }
    };
}
pub trait RootKey {
    fn root_key(&self) -> UpKey;
}

pub trait TargetKey {
    fn target_key(&self) -> DirectedKey;
}
