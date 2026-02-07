pub mod has_read_context;
pub mod root;

use context_insert::*;
use context_trace::*;
use derive_more::{
    Deref,
    DerefMut,
};
use tracing::debug;

use crate::{
    context::root::RootManager,
    expansion::block::BlockExpansionCtx,
    sequence::{
        segment_iter::{
            NextSegment,
            SegmentIter,
        },
        ToNewAtomIndices,
    },
};
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct ReadCtx {
    #[deref]
    #[deref_mut]
    pub root: RootManager,
    pub segments: SegmentIter,
}
pub enum ReadState {
    Continue(Token, PatternEndPath),
    Stop(PatternEndPath),
}
impl Iterator for ReadCtx {
    type Item = ();
    fn next(&mut self) -> Option<Self::Item> {
        self.segments.next().map(|block| self.read_segment(block))
    }
}
impl ReadCtx {
    pub fn new(
        graph: HypergraphRef,
        seq: impl ToNewAtomIndices,
    ) -> Self {
        debug!("New ReadCtx");
        let new_indices = seq.to_new_atom_indices(&graph);
        Self {
            segments: SegmentIter::new(new_indices),
            root: RootManager::new(graph),
        }
    }
    pub fn read_sequence(&mut self) -> Option<Token> {
        self.find_map(|_| None as Option<()>);
        self.root.root
    }
    fn read_segment(
        &mut self,
        segment: NextSegment,
    ) {
        let NextSegment { unknown, known } = segment;
        debug!(
            unknown_len = ?unknown.len(),
            known_len = ?known.len(),
            unknown = ?unknown,
            known = ?known,
            "read_segment"
        );
        self.append_pattern(unknown);
        if !known.is_empty() {
            let block = BlockExpansionCtx::new(self.clone(), known).process();
            self.append_block(block);
        }
    }
}

// ReadCtx derefs to RootManager which derefs to HypergraphRef
impl_has_graph! {
    impl for ReadCtx,
    self => &***self;  // ReadCtx -> RootManager -> HypergraphRef -> Hypergraph
    <'a> &'a Hypergraph
}
impl<R: InsertResult> ToInsertCtx<R> for ReadCtx {
    fn insert_context(&self) -> InsertCtx<R> {
        InsertCtx::from(self.graph.clone())
    }
}
