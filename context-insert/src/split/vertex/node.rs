use context_trace::*;
use derive_new::new;

//impl VertexData {
//    pub(crate) fn offset_children(
//        &self,
//        offset: Offset,
//    ) -> Vec<SubToken> {
//        self.selected_children(|_, pattern| {
//            TraceFront::trace_child_pos(pattern, offset).map(|p| p.sub_index)
//        })
//    }
//}
/// for insert
#[derive(Debug, Clone, Copy, new)]
pub struct NodeTraceCtx<'p> {
    pub(crate) patterns: &'p ChildPatterns,
    pub(crate) index: Token,
}

impl<'p> NodeTraceCtx<'p> {
    pub fn from_index<K: GraphKind>(
        graph: &'p Hypergraph<K>,
        index: Token,
    ) -> Self {
        Self {
            patterns: graph.expect_vertex(index).child_patterns(),
            index,
        }
    }
}

pub trait AsNodeTraceCtx {
    fn as_trace_context<'a>(&'a self) -> NodeTraceCtx<'a>
    where
        Self: 'a;
}

impl AsNodeTraceCtx for NodeTraceCtx<'_> {
    fn as_trace_context<'b>(&'b self) -> NodeTraceCtx<'b>
    where
        Self: 'b,
    {
        *self
    }
}
