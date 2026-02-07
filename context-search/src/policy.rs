use itertools::Itertools;
use std::fmt::Debug;

use crate::container::{
    order::TraversalOrder,
    StateContainer,
};
use context_trace::{
    graph::vertex::{
        parent::{
            HasPatternId,
            Parent,
            PatternIndex,
        },
        VertexIndex,
    },
    path::mutators::raise::PathRaise,
    *,
};

pub trait SearchKind: TraceKind {
    type Container: StateContainer;
    type Policy: DirectedTraversalPolicy<Trav = Self::Trav>;
    type EndNode: PathNode;
}
impl<'a, K: SearchKind> SearchKind for &'a K {
    type Container = K::Container;
    type Policy = &'a K::Policy;
    type EndNode = K::EndNode;
}

pub trait DirectedTraversalPolicy: Sized + Debug {
    type Trav: HasGraph;

    /// nodes generated when an index ended
    /// (parent nodes)
    fn next_batch(
        trav: &Self::Trav,
        parent: &ParentState,
    ) -> Option<ParentBatch> {
        let batch = Self::gen_parent_batch(
            trav,
            parent.path.root_parent(),
            |trav, p| {
                let mut parent = parent.clone();
                parent.path_raise(trav, p);
                parent
            },
        );
        if batch.is_empty() {
            None
        } else {
            Some(batch)
        }
    }
    /// generates parent nodes
    fn gen_parent_batch<
        B: (Fn(&Self::Trav, ChildLocation) -> ParentState) + Copy,
    >(
        trav: &Self::Trav,
        index: Token,
        build_parent: B,
    ) -> ParentBatch {
        let vertex_data = trav.graph().expect_vertex_data(index);
        ParentBatch {
            parents: vertex_data
                .parents()
                .iter()
                .flat_map(|(i, parent): (&VertexIndex, &Parent)| {
                    let p = Token::new(*i, parent.width());
                    parent.pattern_indices().iter().map(
                        move |pi: &PatternIndex| {
                            ChildLocation::new(
                                p,
                                pi.pattern_id(),
                                pi.sub_index(),
                            )
                        },
                    )
                })
                .sorted_by(|a, b| TraversalOrder::cmp(a, b))
                .map(|p| build_parent(trav, p))
                .collect(),
        }
    }
}
impl<'a, P: DirectedTraversalPolicy> DirectedTraversalPolicy for &'a P {
    type Trav = &'a P::Trav;
}
