pub(crate) mod has_read_context;
pub(crate) mod root;

use context_insert::*;
use context_trace::*;
use std::io::Read;
use tracing::debug;

use crate::{
    context::root::RootManager,
    expansion::block::BlockExpansionCtx,
    segment::{
        lazy_atoms_from_reader,
        ErasedSegmentIter,
        LazyAtomIter,
        NextSegment,
        SegmentIter,
        ToNewAtomIndices,
    },
};

// ---------------------------------------------------------------------------
// SegmentResult — structured output from the read pipeline
// ---------------------------------------------------------------------------

/// Result of processing one segment in the read pipeline.
///
/// Each call to the read pipeline's internal iterator yields one of these,
/// describing what happened during that segment's processing.
#[derive(Debug)]
pub enum SegmentResult {
    /// Unknown atoms were appended directly to the root.
    Unknown {
        /// The unknown atom pattern that was appended.
        atoms: Pattern,
        /// Current root token after appending.
        root: Option<Token>,
    },
    /// Known atoms were processed through the expansion pipeline.
    Known {
        /// The known pattern that was expanded.
        pattern: Pattern,
        /// Current root token after expansion + commit.
        root: Option<Token>,
    },
    /// A segment contained both unknown and known atoms.
    /// The unknown atoms were appended first, then the known atoms
    /// were expanded.
    Mixed {
        /// The unknown atom pattern that was appended.
        unknown_atoms: Pattern,
        /// The known pattern that was expanded.
        known_pattern: Pattern,
        /// Current root token after both operations.
        root: Option<Token>,
    },
}

impl SegmentResult {
    /// Get the root token from this result.
    pub fn root(&self) -> Option<Token> {
        match self {
            SegmentResult::Unknown { root, .. } => *root,
            SegmentResult::Known { root, .. } => *root,
            SegmentResult::Mixed { root, .. } => *root,
        }
    }
}

// ---------------------------------------------------------------------------
// ReadCtx — core read pipeline orchestrator
// ---------------------------------------------------------------------------

/// Context for reading sequences and building the hypergraph.
///
/// `ReadCtx` is the top-level orchestrator of the read pipeline. It owns a
/// `RootManager` (which tracks the running root token) and a `SegmentIter`
/// (which partitions the input atom stream into alternating unknown/known
/// chunks).
///
/// ## Eager vs Lazy
///
/// - **Eager** (`ReadCtx::new`): All characters are resolved to atoms upfront
///   into a `Vec<NewAtomIndex>`. This is the original path.
/// - **Lazy** (`ReadCtx::from_chars`, `ReadCtx::from_reader`): Characters are
///   resolved to atoms **on demand** as segments are consumed. This enables
///   streaming input and bounded memory usage.
///
/// Both paths produce identical results — the only difference is when atom
/// resolution occurs.
#[derive(Debug)]
pub struct ReadCtx {
    /// The root manager (Option to allow taking it for BlockExpansionCtx)
    root: Option<RootManager>,
    /// Iterator over segments of unknown/known atoms.
    ///
    /// This is an `ErasedSegmentIter` (type-erased via `Box<dyn Iterator>`)
    /// to keep the public API simple regardless of whether the backing
    /// iterator is eager or lazy.
    segments: ErasedSegmentIter,
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
    /// Create a `ReadCtx` from an eagerly-resolved atom sequence.
    ///
    /// This is the **original constructor** — all characters are resolved to
    /// `NewAtomIndex` values upfront before iteration begins.
    pub fn new(
        graph: HypergraphRef,
        seq: impl ToNewAtomIndices,
    ) -> Self {
        debug!("New ReadCtx (eager)");
        let new_indices = seq.to_new_atom_indices(&graph);
        let boxed: Box<dyn Iterator<Item = _>> =
            Box::new(new_indices.into_iter());
        Self {
            segments: SegmentIter::from_iter(boxed),
            root: Some(RootManager::new(graph)),
        }
    }

    /// Create a `ReadCtx` from a lazy character iterator.
    ///
    /// Each character is resolved to a `NewAtomIndex` **on demand** as
    /// segments are consumed. Unknown characters are inserted into the
    /// graph immediately when first encountered.
    ///
    /// This enables bounded memory usage for large inputs.
    pub fn from_chars<C: Iterator<Item = char> + 'static>(
        graph: HypergraphRef,
        chars: C,
    ) -> Self {
        debug!("ReadCtx::from_chars (lazy)");
        let lazy = LazyAtomIter::new(chars, graph.clone());
        let boxed: Box<dyn Iterator<Item = _>> = Box::new(lazy);
        Self {
            segments: SegmentIter::from_iter(boxed),
            root: Some(RootManager::new(graph)),
        }
    }

    /// Create a `ReadCtx` from a byte stream reader.
    ///
    /// Characters are lazily resolved to atoms as they are consumed
    /// by the segmentation iterator. Unknown characters are inserted
    /// into the graph on demand.
    ///
    /// The reader is decoded as UTF-8; invalid byte sequences are replaced
    /// with U+FFFD (REPLACEMENT CHARACTER).
    pub fn from_reader(
        graph: HypergraphRef,
        reader: impl Read + 'static,
    ) -> Self {
        debug!("ReadCtx::from_reader (lazy/streaming)");
        let lazy = lazy_atoms_from_reader(reader, graph.clone());
        let boxed: Box<dyn Iterator<Item = _>> = Box::new(lazy);
        Self {
            segments: SegmentIter::from_iter(boxed),
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

    /// Read the full sequence, consuming all segments.
    ///
    /// Returns the final root token after all segments have been processed,
    /// or `None` if the input was empty.
    pub fn read_sequence(&mut self) -> Option<Token> {
        // Consume all segments via the Iterator impl (which calls read_segment)
        self.find_map(|_| None as Option<()>);
        self.root.as_ref().and_then(|r| r.root)
    }

    /// Read the next segment and return a structured `SegmentResult`.
    ///
    /// Returns `None` when all segments have been consumed.
    pub fn read_next_segment(&mut self) -> Option<SegmentResult> {
        self.segments.next().map(|segment| {
            let NextSegment { unknown, known } = segment;
            debug!(
                unknown_len = unknown.len(),
                known_len = known.len(),
                unknown = ?unknown,
                known = ?known,
                "read_next_segment"
            );

            let has_unknown = !unknown.is_empty();
            let has_known = !known.is_empty();

            // Take RootManager to pass to BlockExpansionCtx
            let mut root = self.root.take().expect("RootManager was taken");

            // Append unknown pattern first
            let unknown_clone = unknown.clone();
            root.append_pattern(unknown);

            if has_known {
                let known_clone = known.clone();
                let mut block_ctx = BlockExpansionCtx::new(root, known);
                block_ctx.process();
                root = block_ctx.finish();

                // Put RootManager back
                let current_root = root.root;
                self.root = Some(root);

                if has_unknown {
                    SegmentResult::Mixed {
                        unknown_atoms: unknown_clone,
                        known_pattern: known_clone,
                        root: current_root,
                    }
                } else {
                    SegmentResult::Known {
                        pattern: known_clone,
                        root: current_root,
                    }
                }
            } else {
                let current_root = root.root;
                self.root = Some(root);

                SegmentResult::Unknown {
                    atoms: unknown_clone,
                    root: current_root,
                }
            }
        })
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

// ---------------------------------------------------------------------------
// ReadSequenceIter — public iterator wrapper
// ---------------------------------------------------------------------------

/// Public iterator over the read pipeline.
///
/// Yields one [`SegmentResult`] per segment (unknown, known, or mixed block).
/// This provides segment-by-segment control over the read pipeline, which is
/// useful for:
/// - Streaming progress reporting
/// - Incremental graph building
/// - Debugging and inspection
///
/// # Example
/// ```rust,ignore
/// let iter = ReadSequenceIter::new(graph, "hello world".chars());
/// for result in iter {
///     match result {
///         SegmentResult::Unknown { atoms, root } => { /* ... */ }
///         SegmentResult::Known { pattern, root } => { /* ... */ }
///         SegmentResult::Mixed { .. } => { /* ... */ }
///     }
/// }
/// ```
pub struct ReadSequenceIter {
    ctx: ReadCtx,
}

impl ReadSequenceIter {
    /// Create a new `ReadSequenceIter` from an eager atom source.
    pub fn new(
        graph: HypergraphRef,
        seq: impl ToNewAtomIndices,
    ) -> Self {
        Self {
            ctx: ReadCtx::new(graph, seq),
        }
    }

    /// Create a new `ReadSequenceIter` from a lazy character iterator.
    pub fn from_chars<C: Iterator<Item = char> + 'static>(
        graph: HypergraphRef,
        chars: C,
    ) -> Self {
        Self {
            ctx: ReadCtx::from_chars(graph, chars),
        }
    }

    /// Create a new `ReadSequenceIter` from a byte stream reader.
    pub fn from_reader(
        graph: HypergraphRef,
        reader: impl Read + 'static,
    ) -> Self {
        Self {
            ctx: ReadCtx::from_reader(graph, reader),
        }
    }

    /// Get the current root token without consuming more segments.
    pub fn current_root(&self) -> Option<Token> {
        self.ctx.root_token()
    }

    /// Consume all remaining segments and return the final root token.
    pub fn finish(mut self) -> Option<Token> {
        // Drain all remaining segments
        while self.ctx.read_next_segment().is_some() {}
        self.ctx.root_token()
    }
}

impl Iterator for ReadSequenceIter {
    type Item = SegmentResult;

    fn next(&mut self) -> Option<Self::Item> {
        self.ctx.read_next_segment()
    }
}

impl std::fmt::Debug for ReadSequenceIter {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        f.debug_struct("ReadSequenceIter")
            .field("current_root", &self.ctx.root_token())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// HasGraph + ToInsertCtx impls (unchanged)
// ---------------------------------------------------------------------------

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
