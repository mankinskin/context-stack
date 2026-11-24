//! The committed store: [`PositionedGraph`] and its shared handle
//! [`PositionedGraphRef`] (`graph.rs`), and their vertex object
//! [`PositionedVertexRef`] and store-trait impls, including the lock guards
//! (`access.rs`, staging plan §3.8).

mod access;
mod graph;

pub use self::{
    access::PositionedVertexRef,
    graph::{
        PositionedGraph,
        PositionedGraphRef,
    },
};
