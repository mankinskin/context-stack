//! Validated (de)serialization of [`PositionedGraph`] (feature `serde`).
//!
//! Only the nodes are stored (slot `i` is `VertexIndex(i)`) and the
//! generation; widths, reciprocal parent links, the atom map, vertex keys and
//! debug labels are derived on load. Loading is the only way to build a graph
//! outside insertion and reading, so it accepts exactly the graphs that pass
//! [`PositionedGraph::validate`] and `validate_store`.

use std::collections::{
    BTreeMap,
    HashMap,
};

use serde::{
    Deserialize,
    Deserializer,
    Serialize,
    Serializer,
    de::Error as _,
};

use super::PositionedGraph;
#[cfg(debug_assertions)]
use crate::positioned::edit::check::composed_label;
use crate::{
    AtomOf,
    GraphKind,
    Token,
    TokenWidth,
    VertexKey,
    graph::vertex::VertexIndex,
    positioned::{
        edit::check::validate_store,
        model::{
            Composition,
            Occurrence,
            PositionedVertex,
            VertexNode,
        },
    },
};

#[derive(Serialize, Deserialize)]
struct StoredGraph<A> {
    vertices: Vec<StoredNode<A>>,
    generation: u64,
}

#[derive(Serialize, Deserialize)]
enum StoredNode<A> {
    Atom(A),
    /// `(offset, child index)` in stored (sorted) order.
    Composite(Vec<(usize, usize)>),
}

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

#[derive(Clone, Copy, PartialEq, Eq)]
enum Visit {
    New,
    Open,
    Done,
}

/// Widths of all slots and an order in which every child precedes its
/// parents (children may have higher indices than their parents).
fn widths<A>(
    vertices: &[StoredNode<A>]
) -> Result<(Vec<usize>, Vec<usize>), String> {
    let count = vertices.len();
    let mut widths = vec![0; count];
    let mut state = vec![Visit::New; count];
    let mut order = Vec::with_capacity(count);
    let children = |vertex: usize| match &vertices[vertex] {
        StoredNode::Atom(_) => &[][..],
        StoredNode::Composite(occurrences) => &occurrences[..],
    };
    for root in 0..count {
        if state[root] != Visit::New {
            continue;
        }
        state[root] = Visit::Open;
        let mut stack = vec![(root, 0)];
        while let Some((vertex, next)) = stack.last_mut() {
            let vertex = *vertex;
            if let Some(&(_, child)) = children(vertex).get(*next) {
                *next += 1;
                match state.get(child) {
                    None => {
                        return Err(format!(
                            "vertex {vertex}: unknown child {child} (of {count} vertices)"
                        ));
                    },
                    Some(Visit::Open) => {
                        return Err(format!(
                            "vertex {vertex}: cycle through child {child}"
                        ));
                    },
                    Some(Visit::Done) => {},
                    Some(Visit::New) => {
                        state[child] = Visit::Open;
                        stack.push((child, 0));
                    },
                }
                continue;
            }
            widths[vertex] = match &vertices[vertex] {
                StoredNode::Atom(_) => 1,
                StoredNode::Composite(occurrences) => occurrences
                    .iter()
                    .try_fold(0usize, |width, &(offset, child)| {
                        offset
                            .checked_add(widths[child])
                            .map(|end| width.max(end))
                    })
                    .ok_or_else(|| format!("vertex {vertex}: offset out of range"))?,
            };
            state[vertex] = Visit::Done;
            order.push(vertex);
            stack.pop();
        }
    }
    Ok((widths, order))
}

impl<G: GraphKind> TryFrom<StoredGraph<AtomOf<G>>> for PositionedGraph<G> {
    type Error = String;

    fn try_from(stored: StoredGraph<AtomOf<G>>) -> Result<Self, String> {
        let (widths, order) = widths(&stored.vertices)?;
        let mut atoms = HashMap::new();
        let mut vertices = Vec::with_capacity(stored.vertices.len());
        for (slot, node) in stored.vertices.into_iter().enumerate() {
            let index = VertexIndex(slot);
            let node = match node {
                StoredNode::Atom(atom) => {
                    if let Some(first) = atoms.insert(atom, index) {
                        return Err(format!(
                            "vertex {slot}: duplicate atom {atom} (first at {})",
                            first.0
                        ));
                    }
                    VertexNode::Atom(atom)
                },
                StoredNode::Composite(stored) => {
                    let occurrences: Vec<Occurrence> = stored
                        .iter()
                        .map(|&(offset, child)| {
                            Occurrence::new(offset, Token::new(VertexIndex(child), widths[child]))
                        })
                        .collect();
                    let composition = Composition::new(occurrences.clone())
                        .map_err(|error| format!("vertex {slot}: {error}"))?;
                    // `Composition::new` sorts; a stored table must already be.
                    if composition.occurrences() != occurrences.as_slice() {
                        return Err(format!("vertex {slot}: occurrences are not sorted"));
                    }
                    VertexNode::Composite(composition)
                },
            };
            vertices.push(PositionedVertex {
                index,
                key: VertexKey::default(),
                width: TokenWidth(widths[slot]),
                node,
                parents: BTreeMap::new(),
                #[cfg(debug_assertions)]
                label: String::new(),
            });
        }
        for parent in 0..vertices.len() {
            if let VertexNode::Composite(composition) = &vertices[parent].node {
                for occurrence in composition.occurrences().to_vec() {
                    vertices[occurrence.child.index.0]
                        .parents
                        .entry(VertexIndex(parent))
                        .or_default()
                        .insert(occurrence.offset);
                }
            }
        }
        #[cfg_attr(not(debug_assertions), allow(unused_mut))]
        let mut graph = Self {
            vertices,
            atoms,
            generation: stored.generation,
        };
        #[cfg(debug_assertions)]
        for slot in order {
            let label = match &graph.vertices[slot].node {
                VertexNode::Atom(atom) => atom.to_string(),
                VertexNode::Composite(composition) => composed_label(
                    &graph,
                    composition.occurrences().iter().copied(),
                ),
            };
            graph.vertices[slot].label = label;
        }
        #[cfg(not(debug_assertions))]
        let _ = order;
        graph.validate().map_err(|error| error.to_string())?;
        validate_store(&graph).map_err(|error| error.to_string())?;
        Ok(graph)
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
