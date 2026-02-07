use context_trace::*;

use crate::{
    context::ReadCtx,
    segment::ToNewAtomIndices,
};

/// Trait for types that can create or provide a ReadCtx for reading sequences.
pub(crate) trait HasReadCtx {
    fn read_context(&'_ mut self) -> ReadCtx;
    fn read_sequence(&mut self) -> Option<Token> {
        self.read_context().read_sequence()
    }
}

impl<T: HasReadCtx> HasReadCtx for &'_ mut T {
    fn read_context(&mut self) -> ReadCtx {
        (**self).read_context()
    }
}
impl<S: ToNewAtomIndices + Clone> HasReadCtx for (HypergraphRef, S) {
    fn read_context(&mut self) -> ReadCtx {
        let (graph, seq) = self;
        ReadCtx::new(graph.clone(), seq.clone())
    }
}
impl<S: ToNewAtomIndices + Clone> HasReadCtx for (&mut HypergraphRef, S) {
    fn read_context(&mut self) -> ReadCtx {
        let (graph, seq) = self;
        ReadCtx::new(graph.clone(), seq.clone())
    }
}
