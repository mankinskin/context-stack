//! Conversion between [`PositionedGraph`] and the interchange format
//! (`test-api`: fixture loading and test comparison).

use std::collections::BTreeMap;

use super::{
    PositionedCover,
    PositionedGraphData,
};
use std::fmt::Display;

use crate::positioned::{
    CompositionRead,
    GraphRead,
    NodeRef,
    Occurrence,
    PositionedGraph,
    VertexRead,
    string_repr,
};
use crate::{
    AtomOf,
    GraphKind,
    Token,
};

impl<G: GraphKind<Atom = char>> PositionedGraph<G> {
    /// Build a graph from structurally valid interchange data through one
    /// `GraphEdit` (atoms, then composites by increasing width) and validate
    /// it.
    pub fn from_interchange(data: &PositionedGraphData) -> Result<Self, String> {
        Self::build_from_interchange(data).map(|(graph, _)| graph)
    }

    /// [`Self::from_interchange`] plus the fixture key → token map.
    ///
    /// Tests translate string expectations into tokens only through this
    /// map; it is returned to the caller and never stored in the graph.
    pub fn from_interchange_named(
        data: &PositionedGraphData,
    ) -> Result<(Self, BTreeMap<String, Token>), String> {
        Self::build_from_interchange(data).map(|(graph, tokens)| {
            let names = tokens
                .into_iter()
                .map(|(content, token)| (content.to_string(), token))
                .collect();
            (graph, names)
        })
    }

    fn build_from_interchange(
        data: &PositionedGraphData,
    ) -> Result<(Self, BTreeMap<&str, Token>), String> {
        data.validate()?;
        let (atoms, mut composites): (Vec<_>, Vec<_>) = data
            .vertices
            .iter()
            .partition(|(_, cover): &(&String, &PositionedCover)| cover.is_empty());
        composites.sort_by_key(|(content, _)| (content.chars().count(), *content));
        let mut graph = Self::default();
        let mut tokens: BTreeMap<&str, Token> = BTreeMap::new();
        let mut edit = graph.edit();
        for (content, _) in atoms {
            let atom = content.chars().next().expect("validated nonempty");
            tokens.insert(content, edit.atom(atom));
        }
        for (content, cover) in composites {
            let occurrences = cover
                .iter()
                .map(|(offset, child)| Occurrence::new(*offset, tokens[child.as_str()]))
                .collect();
            let token = edit
                .create(occurrences)
                .map_err(|error| format!("{content:?}: {error}"))?;
            tokens.insert(content, token);
        }
        edit.finish();
        graph.validate().map_err(|error| error.to_string())?;
        Ok((graph, tokens))
    }

    /// ID-free content-keyed covers of every vertex ([`store_interchange`]).
    ///
    /// Export only: the interchange format is keyed by content, so names are
    /// read by traversal here and never stored in the graph.
    pub fn to_interchange(&self) -> PositionedGraphData {
        store_interchange(self)
    }
}

/// ID-free content-keyed covers of every vertex of any store (also an open
/// stage), named by [`string_repr`].
pub fn store_interchange<S: GraphRead + ?Sized>(store: &S) -> PositionedGraphData
where
    AtomOf<S::Kind>: Display,
{
    let name = |index: S::Index| -> String {
        string_repr(store.vertex(index).expect("vertex of this store"))
    };
    let vertices = store
        .vertices()
        .map(|vertex| {
            let cover = match vertex.node() {
                NodeRef::Atom(_) => Vec::new(),
                NodeRef::Composite(composition) => composition
                    .occurrences()
                    .map(|o| (o.offset.0, name(o.child.index)))
                    .collect(),
            };
            (string_repr(vertex), cover)
        })
        .collect();
    PositionedGraphData { vertices }
}
