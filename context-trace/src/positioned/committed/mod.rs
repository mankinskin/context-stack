//! The committed store: [`PositionedGraph`] and its shared handle
//! [`PositionedGraphRef`] (`graph.rs`), and their vertex object
//! [`PositionedVertexRef`] and store-trait impls, including the lock guards
//! (`access.rs`, staging plan §3.8). Feature `serde`: validated
//! (de)serialization of [`PositionedGraph`] (`serde.rs`).

mod access;
mod graph;
#[cfg(feature = "serde")]
mod serde;

pub use self::{
    access::PositionedVertexRef,
    graph::{
        PositionedGraph,
        PositionedGraphRef,
    },
};
