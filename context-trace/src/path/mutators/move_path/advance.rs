use std::ops::ControlFlow;

use crate::path::{
    mutators::move_path::path::MovePath,
    structs::rooted::PathNode,
};

use crate::{
    direction::Right,
    path::accessors::role::End,
    trace::has_graph::HasGraph,
};

pub trait CanAdvance: Advance + Clone {
    fn can_advance<G: HasGraph>(
        &self,
        trav: &G,
    ) -> bool {
        self.clone().move_path(trav).is_continue()
    }
}

impl<T: Advance + Clone> CanAdvance for T {}

pub trait Advance: MovePath<Right, End> {
    fn advance<G: HasGraph>(
        &mut self,
        trav: &G,
    ) -> ControlFlow<()> {
        self.move_path(trav)
    }
}

// Blanket implementation for types that implement MovePath with any Node type
impl<T> Advance for T where T: MovePath<Right, End> {}
