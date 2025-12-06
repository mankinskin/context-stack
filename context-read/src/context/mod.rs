pub mod has_read_context;
pub mod root;
use std::sync::RwLockWriteGuard;

use context_insert::*;
use context_trace::*;
use derive_more::{
    Deref,
    DerefMut,
};
use tracing::debug;

use crate::{
    context::root::RootManager,
    sequence::{
        block_iter::{
            BlockIter,
            NextBlock,
        },
        ToNewAtomIndices,
    },
};
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct ReadCtx {
    #[deref]
    #[deref_mut]
    pub root: RootManager,
    pub blocks: BlockIter,
}
pub enum ReadState {
    Continue(Token, PatternEndPath),
    Stop(PatternEndPath),
}
impl Iterator for ReadCtx {
    type Item = ();
    fn next(&mut self) -> Option<Self::Item> {
        self.blocks.next().map(|block| self.read_block(block))
    }
}
impl ReadCtx {
    pub fn new(
        mut graph: HypergraphRef,
        seq: impl ToNewAtomIndices,
    ) -> Self {
        debug!("New ReadCtx");
        let new_indices = seq.to_new_Atom_indices(&mut graph.graph_mut());
        Self {
            blocks: BlockIter::new(new_indices),
            root: RootManager::new(graph),
        }
    }
    pub fn read_sequence(&mut self) -> Option<Token> {
        self.find_map(|_| None as Option<()>);
        self.root.root
    }
    pub fn read_known(
        &mut self,
        known: Pattern,
    ) {
        // TODO: This needs to be reimplemented with the current path construction API
        // The old new_directed API no longer exists
        // For now, just append the pattern directly
        let minified = known.into();
        self.append_pattern(minified);
    }
    fn read_block(
        &mut self,
        block: NextBlock,
    ) {
        let NextBlock { unknown, known } = block;
        self.append_pattern(unknown);
        self.read_known(known);
    }
    //pub fn read_next(&mut self) -> Option<Token> {
    //    match ToInsertCtx::<IndexWithPath>::insert_or_get_complete(
    //        &self.graph,
    //        self.sequence.clone(),
    //    ) {
    //        Ok(IndexWithPath {
    //            index,
    //            path: advanced,
    //        }) => {
    //            self.sequence = advanced;
    //            Some(index)
    //        },
    //        Err(ErrorReason::SingleIndex(index)) => {
    //            self.sequence.advance(&self.graph);
    //            Some(index)
    //        },
    //        Err(_) => {
    //            self.sequence.advance(&self.graph);
    //            None
    //        },
    //    }
    //}
    //pub fn read_pattern(
    //    &mut self,
    //    known: impl IntoPattern,
    //) -> Option<Token> {
    //    self.read_known(known.into_pattern());
    //    self.root
    //}
    //fn append_next(&mut self, end_bound: usize, index: Token) -> usize {
    //    self.append_index(index);
    //    0
    //}
}

impl_has_graph! {
    impl for ReadCtx,
    self => self.graph.write().unwrap();
    <'a> RwLockWriteGuard<'a, Hypergraph>
}
impl<R: InsertResult> ToInsertCtx<R> for ReadCtx {
    fn insert_context(&self) -> InsertCtx<R> {
        InsertCtx::from(self.graph.clone())
    }
}
impl_has_graph_mut! {
    impl for ReadCtx,
    self => self.graph.write().unwrap();
    <'a> RwLockWriteGuard<'a, Hypergraph>
}
