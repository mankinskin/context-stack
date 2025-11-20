use std::ops::ControlFlow;

use crate::{
    direction::Left,
    path::accessors::role::End,
    trace::has_graph::HasGraph,
};

use crate::path::mutators::move_path::path::MovePath;
pub(crate) trait Retract: MovePath<Left, End> {
    fn retract<G: HasGraph>(
        &mut self,
        trav: &G,
    ) -> ControlFlow<()> {
        self.move_path(trav)
    }
}

impl<T: MovePath<Left, End>> Retract for T {}
