//! Conversion between [`PositionedGraph`] and the v3 interchange.

use std::collections::BTreeMap;

use super::{
    Occurrence,
    PositionedGraph,
    VertexNode,
    interchange::{
        PositionedCover,
        PositionedGraphData,
    },
};
use crate::{
    GraphKind,
    Token,
    graph::vertex::VertexIndex,
};

impl<G: GraphKind<Atom = char>> PositionedGraph<G> {
    /// Build a graph from structurally valid interchange data, inserting
    /// vertices bottom-up by width.
    pub fn from_interchange(data: &PositionedGraphData) -> Result<Self, String> {
        data.validate()?;
        let mut order: Vec<(&String, &PositionedCover)> = data.vertices.iter().collect();
        order.sort_by_key(|(content, _)| (content.chars().count(), *content));
        let mut graph = Self::default();
        let mut tokens: BTreeMap<&str, Token> = BTreeMap::new();
        for (content, cover) in order {
            let token = if cover.is_empty() {
                let atom = content.chars().next().expect("validated nonempty");
                graph.insert_atom(atom)
            } else {
                let occurrences = cover
                    .iter()
                    .map(|(offset, child)| Occurrence::new(*offset, tokens[child.as_str()]))
                    .collect();
                graph
                    .insert(occurrences)
                    .map_err(|error| format!("{content:?}: {error}"))?
            };
            tokens.insert(content, token);
        }
        Ok(graph)
    }

    /// ID-free content-keyed covers of every vertex.
    pub fn to_interchange(&self) -> PositionedGraphData {
        let name = |index: VertexIndex| -> String {
            self.content(index)
                .expect("vertex of this graph")
                .into_iter()
                .collect()
        };
        let vertices = self
            .vertices()
            .map(|vertex| {
                let cover = match vertex.node() {
                    VertexNode::Atom(_) => Vec::new(),
                    VertexNode::Composite(composition) => composition
                        .occurrences()
                        .iter()
                        .map(|o| (o.offset.0, name(o.child.index)))
                        .collect(),
                };
                (name(vertex.index()), cover)
            })
            .collect();
        PositionedGraphData { vertices }
    }
}
