use crate::{
    cursor::{
        CursorState,
        PathCursor,
    },
    state::start::StartFoldPath,
};
use context_trace::*;

impl_cursor_pos! {
    <R: StartFoldPath, S: CursorState> CursorPosition for PathCursor<R, S>, self => self.atom_position
}
impl<D: Direction, P, S: CursorState> MoveKey<D> for PathCursor<P, S>
where
    AtomPosition: MoveKey<D>,
{
    fn move_key(
        &mut self,
        delta: usize,
    ) {
        self.atom_position.move_key(delta)
    }
}

impl<R: PathRole, P: HasRootChildIndex<R>, S: CursorState> HasRootChildIndex<R>
    for PathCursor<P, S>
{
    fn root_child_index(&self) -> usize {
        HasRootChildIndex::<R>::root_child_index(&self.path)
    }
}
