pub(crate) mod has_read_context;
pub(crate) mod root;

use context_insert::*;
use context_trace::*;
use tracing::debug;

use crate::{
    context::root::RootManager,
    expansion::block::BlockExpansionCtx,
    segment::{
        NextSegment,
        SegmentIter,
        ToNewAtomIndices,
    },
};

/// Context for reading sequences and building the hypergraph.
#[derive(Debug)]
pub struct ReadCtx {
    /// The root manager (Option to allow taking it for BlockExpansionCtx)
    root: Option<RootManager>,
    /// Iterator over segments of unknown/known atoms
    pub(crate) segments: SegmentIter,
}

//pub(crate) enum ReadState {
//    Continue(Token, PatternEndPath),
//    Stop(PatternEndPath),
//}

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
            root: Some(RootManager::new(graph)),
        }
    }

    /// Get the graph reference.
    pub(crate) fn graph(&self) -> &HypergraphRef {
        &self.root.as_ref().expect("RootManager taken").graph
    }

    /// Get the current root token.
    pub(crate) fn root_token(&self) -> Option<Token> {
        self.root.as_ref().and_then(|r| r.root)
    }

    pub(crate) fn read_sequence(&mut self) -> Option<Token> {
        self.find_map(|_| None as Option<()>);
        self.root.as_ref().and_then(|r| r.root)
    }

    fn read_segment(
        &mut self,
        segment: NextSegment,
    ) {
        let NextSegment { unknown, known } = segment;
        debug!(
            unknown_len = unknown.len(),
            known_len = known.len(),
            unknown = ?unknown,
            known = ?known,
            "read_segment"
        );

        // Take RootManager to pass to BlockExpansionCtx
        let mut root = self.root.take().expect("RootManager was taken");

        // Append unknown pattern first
        root.append_pattern(unknown);

        if !known.is_empty() {
            // Process known pattern through BlockExpansionCtx
            // process() commits the chain to the root manager internally
            let mut block_ctx = BlockExpansionCtx::new(root, known);
            block_ctx.process();
            root = block_ctx.finish();
        }

        // Put RootManager back
        self.root = Some(root);
    }
}

// ReadCtx provides graph access through the root manager
impl_has_graph! {
    impl for ReadCtx,
    self => self.root.as_ref().expect("RootManager taken").graph.as_ref();
    <'a> &'a Hypergraph
}

impl<R: InsertResult> ToInsertCtx<R> for ReadCtx {
    fn insert_context(&self) -> InsertCtx<R> {
        InsertCtx::from(
            self.root.as_ref().expect("RootManager taken").graph.clone(),
        )
    }
}
