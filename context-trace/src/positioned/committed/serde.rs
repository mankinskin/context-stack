//! Validated (de)serialization of [`PositionedGraph`] (feature `serde`):
//! the table of `table.rs` (only the nodes and the generation are stored).
//! Loading is the only way to build a graph outside insertion, reading and
//! interchange import, so it accepts exactly the graphs that pass
//! [`PositionedGraph::validate`] and `validate_store`.

use serde::{
    Deserialize,
    Deserializer,
    Serialize,
    Serializer,
    de::Error as _,
};

use super::{
    PositionedGraph,
    table::{
        StoredGraph,
        StoredNode,
    },
};
use crate::{
    AtomOf,
    GraphKind,
    positioned::model::VertexNode,
};

impl<G: GraphKind> From<&PositionedGraph<G>> for StoredGraph<AtomOf<G>> {
    fn from(graph: &PositionedGraph<G>) -> Self {
        let vertices = graph
            .vertices
            .iter()
            .map(|vertex| match &vertex.node {
                VertexNode::Atom(atom) => StoredNode::Atom(*atom),
                VertexNode::Composite(composition) => StoredNode::Composite(
                    composition
                        .occurrences()
                        .iter()
                        .map(|o| (o.offset.0, o.child.index.0))
                        .collect(),
                ),
            })
            .collect();
        Self {
            vertices,
            generation: graph.generation,
        }
    }
}

impl<G: GraphKind> Serialize for PositionedGraph<G> {
    fn serialize<S: Serializer>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        StoredGraph::from(self).serialize(serializer)
    }
}

impl<'de, G: GraphKind> Deserialize<'de> for PositionedGraph<G> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let stored = StoredGraph::<AtomOf<G>>::deserialize(deserializer)?;
        Self::try_from(stored).map_err(D::Error::custom)
    }
}
