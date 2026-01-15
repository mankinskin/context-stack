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
/// 
/// This context owns the patterns data since with interior mutability
/// we can't hold references across lock boundaries.
#[derive(Debug, Clone)]
pub struct NodeTraceCtx {
    pub(crate) patterns: ChildPatterns,
    pub(crate) index: Token,
}

impl NodeTraceCtx {
    pub fn new(patterns: ChildPatterns, index: Token) -> Self {
        Self { patterns, index }
    }
    
    pub fn from_index<K: GraphKind>(
        graph: &Hypergraph<K>,
        index: Token,
    ) -> Self {
        Self {
            patterns: graph.expect_child_patterns(index),
            index,
        }
    }
}

pub trait AsNodeTraceCtx {
    fn as_trace_context(&self) -> NodeTraceCtx;
}

impl AsNodeTraceCtx for NodeTraceCtx {
    fn as_trace_context(&self) -> NodeTraceCtx {
        self.clone()
    }
}
