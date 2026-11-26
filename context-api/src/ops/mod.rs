//! Command implementations over an open graph.

pub(crate) mod export;
pub(crate) mod info;
pub(crate) mod read;
pub(crate) mod render;
pub(crate) mod search;
pub(crate) mod text;

use context_trace::positioned::PositionedGraph;

use crate::store::file::GraphMeta;

/// Read-only view of an open graph for queries.
pub(crate) struct View<'a> {
    pub name: &'a str,
    pub graph: &'a PositionedGraph,
    pub meta: &'a GraphMeta,
}
