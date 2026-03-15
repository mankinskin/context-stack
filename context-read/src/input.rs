//! Input conversion for the read pipeline.
//!
//! `IntoReadInput` is the single conversion trait for everything the read
//! pipeline accepts as input.  Implement it for a type to make it passable
//! to [`crate::read`] and [`crate::pipeline::ReadCtx::new`].

use context_trace::{
    graph::vertex::atom::NewAtomIndices,
    trace::has_graph::HasGraph,
    BaseGraphKind,
    HypergraphRef,
    Token,
};

use crate::segment::ToNewAtomIndices;

/// Conversion trait for types that can be used as input to the read pipeline.
///
/// The pipeline always works on `NewAtomIndices` internally, so this trait
/// just converts `self` into that form given access to the graph (for atom
/// insertion).
pub trait IntoReadInput: std::fmt::Debug {
    fn into_read_input<G: HasGraph<Kind = BaseGraphKind>>(
        self,
        graph: &G,
    ) -> NewAtomIndices;
}

// --- Blanket: anything that already implements ToNewAtomIndices ---
impl<T: ToNewAtomIndices + std::fmt::Debug> IntoReadInput for T {
    fn into_read_input<G: HasGraph<Kind = BaseGraphKind>>(
        self,
        graph: &G,
    ) -> NewAtomIndices {
        self.to_new_atom_indices(graph)
    }
}

/// Trait for types that can create or provide a ReadCtx for reading sequences.
#[allow(dead_code)]
pub(crate) trait HasReadCtx {
    fn read_context(&'_ mut self) -> crate::pipeline::ReadCtx;
    fn read_sequence(&mut self) -> Option<Token> {
        self.read_context().read_sequence()
    }
}

impl<T: HasReadCtx> HasReadCtx for &'_ mut T {
    fn read_context(&mut self) -> crate::pipeline::ReadCtx {
        (**self).read_context()
    }
}
impl<S: ToNewAtomIndices + Clone> HasReadCtx for (HypergraphRef, S) {
    fn read_context(&mut self) -> crate::pipeline::ReadCtx {
        let (graph, seq) = self;
        crate::pipeline::ReadCtx::new(graph.clone(), seq.clone())
    }
}
impl<S: ToNewAtomIndices + Clone> HasReadCtx for (&mut HypergraphRef, S) {
    fn read_context(&mut self) -> crate::pipeline::ReadCtx {
        let (graph, seq) = self;
        crate::pipeline::ReadCtx::new(graph.clone(), seq.clone())
    }
}
