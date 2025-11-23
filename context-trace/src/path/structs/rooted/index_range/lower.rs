//! PathLower implementation for IndexRangePath

use std::ops::ControlFlow;

use crate::{
    TokenWidth,
    graph::vertex::{
        location::pattern::IntoPatternLocation,
        wide::Wide,
    },
    path::mutators::{
        lower::PathLower,
        move_path::key::{
            AtomPosition,
            RetractKey,
        },
    },
    trace::has_graph::HasGraph,
};

use super::IndexRangePath;

impl PathLower for (&mut AtomPosition, &mut IndexRangePath) {
    fn path_lower<G: HasGraph>(
        &mut self,
        trav: &G,
    ) -> ControlFlow<()> {
        let (root_pos, range) = self;
        let (start, end, root) = (
            &mut range.start.sub_path,
            &mut range.end.sub_path,
            &mut range.root,
        );
        if let Some(prev) = start.path.pop() {
            let graph = trav.graph();
            let pattern = graph.expect_pattern_at(prev);
            root_pos.retract_key(
                pattern[prev.sub_index + 1..]
                    .iter()
                    .fold(TokenWidth(0), |a, c| a + c.width())
                    .0,
            );
            start.root_entry = prev.sub_index;
            end.root_entry = pattern.len() - 1;
            end.path.clear();
            root.location = prev.into_pattern_location();

            ControlFlow::Continue(())
        } else {
            ControlFlow::Break(())
        }
    }
}
