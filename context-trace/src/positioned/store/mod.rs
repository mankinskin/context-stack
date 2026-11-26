//! Store abstraction (staging plan §3.3–§3.6): read traits over vertex
//! objects, the sealed write primitives behind `GraphEdit`, graph access
//! and store-generic navigation.
//!
//! - [`GraphRead`] hands out [`VertexRead`] objects (`graph.vertex(i)`);
//!   lookups accept anything carrying an index of the store
//!   ([`HasVertexIndex`](crate::graph::vertex::has_vertex_index::HasVertexIndex),
//!   legacy `VertexSet<I: GetVertexIndex>`) (`read.rs`).
//! - [`CompositionRead`] is what navigation, insertion and the reader need
//!   from a composition (`composition.rs`); [`NodeRef`] is either an atom
//!   value or one (`read.rs`).
//! - [`GraphWrite`] adds the raw write primitives of [`raw::Write`], which
//!   cannot be named outside `context-trace` without the hidden feature
//!   `edit` and whose methods take a [`raw::Seal`]: only `GraphEdit` and
//!   the stages mutate (insert plan D8, crate ownership plan E1)
//!   (`write.rs`).
//! - [`HasGraph`] / [`HasGraphMut`] abstract how a store is reached
//!   (`has_graph.rs`); [`Navigate`] is local navigation on vertex objects
//!   (`navigate.rs`).

mod composition;
mod has_graph;
mod navigate;
mod read;
mod write;

pub use self::{
    composition::CompositionRead,
    has_graph::{
        GraphAtom,
        HasGraph,
        HasGraphMut,
        IndexOf,
        KindOf,
    },
    navigate::{
        Located,
        Navigate,
        string_repr,
    },
    read::{
        Generation,
        GraphRead,
        NodeRef,
        VertexRead,
        VertexState,
    },
    write::GraphWrite,
};
pub(crate) use self::has_graph::{
    impl_has_graph,
    impl_has_graph_mut,
};
#[cfg(feature = "edit")]
#[doc(hidden)]
pub use self::write::raw;
#[cfg(not(feature = "edit"))]
pub(crate) use self::write::raw;
