use std::ops::ControlFlow;

use crate::{
    direction::Direction,
    trace::has_graph::HasGraph,
};

pub(crate) trait MoveLeaf<D: Direction> {
    fn move_leaf<G: HasGraph>(
        &mut self,
        trav: &G,
    ) -> ControlFlow<()>;
}

//pub(crate) trait AdvanceLeaf: MoveLeaf<Right> {
//    fn advance_leaf<G: HasGraph>(
//        &mut self,
//        trav: &G,
//    ) -> ControlFlow<()> {
//        self.move_leaf(trav)
//    }
//}
//
//impl<T: MoveLeaf<Right>> AdvanceLeaf for T {}
//
//pub(crate) trait RetractLeaf: MoveLeaf<Left> {
//    fn retract_leaf<G: HasGraph>(
//        &mut self,
//        trav: &G,
//    ) -> ControlFlow<()> {
//        self.move_leaf(trav)
//    }
//}
//
//impl<T: MoveLeaf<Left>> RetractLeaf for T {}
//
