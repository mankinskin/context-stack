//! Corpus-to-graph pipeline and shared progress reporting.
//!
//! Construction enumerates substrings, selects frequency tokens and overlap
//! wrappers, then materializes selected decompositions with additional gap labels.

use std::sync::{
    Arc,
    MappedRwLockReadGuard,
    MappedRwLockWriteGuard,
    RwLock,
    RwLockReadGuard,
    RwLockWriteGuard,
};

#[cfg(not(target_arch = "wasm32"))]
use std::path::{
    absolute,
    PathBuf,
};

use derive_getters::Getters;
use derive_more::{
    Deref,
    DerefMut,
};
use derive_new::new;
use itertools::Itertools;
use ngram::NGram;
use pretty_assertions::assert_eq;

use context_trace::{
    graph::{
        getters,
        vertex::key::VertexKey,
        Hypergraph,
    },
    HashSet,
};
use serde::{
    Deserialize,
    Serialize,
};

use crate::{
    cancellation::Cancellation,
    graph::{
        labelling::{
            LabellingCtx,
            LabellingImage,
        },
        traversal::pass::RunResult,
        vocabulary::{
            entry::HasVertexEntries,
            ProcessStatus,
            Vocabulary,
        },
    },
    tests::TestCorpus,
};

pub(crate) mod containment;
pub(crate) mod labelling;
pub(crate) mod partitions;
pub(crate) mod trace;
pub mod traversal;
pub(crate) mod utils;
pub mod vocabulary;

#[cfg(not(target_arch = "wasm32"))]
lazy_static::lazy_static! {
    /// Legacy native corpus-cache directory, resolved relative to the process.
    pub(crate) static ref CORPUS_DIR: PathBuf = absolute(PathBuf::from_iter([".", "test", "cache"])).unwrap();
}

/// Cloneable access to progress state shared with callers observing a parse.
#[derive(Debug, Clone, Default, Deref, DerefMut)]
pub struct StatusHandle {
    /// Synchronization for progress only; traversal itself remains sequential.
    data: Arc<RwLock<Status>>,
}
impl From<Status> for StatusHandle {
    /// Move progress into a shared lock, preserving the supplied counters.
    fn from(value: Status) -> Self {
        Self {
            data: Arc::new(RwLock::new(value)),
        }
    }
}
impl StatusHandle {
    /// Replace the pass marker and counters under a write lock.
    pub(crate) fn next_pass(
        &self,
        pass: ProcessStatus,
        steps: usize,
        steps_total: usize,
    ) {
        self.data
            .write()
            .unwrap()
            .next_pass(pass, steps, steps_total);
    }
    /// Read the current stage while holding a mapped read lock.
    pub(crate) fn pass(&self) -> MappedRwLockReadGuard<'_, ProcessStatus> {
        RwLockReadGuard::<'_, Status>::map(self.data.read().unwrap(), |s| {
            &s.pass
        })
    }
    /// Borrow the stage for in-place mutation without resetting counters.
    pub(crate) fn pass_mut(&self) -> MappedRwLockWriteGuard<'_, ProcessStatus> {
        RwLockWriteGuard::<'_, Status>::map(self.data.write().unwrap(), |s| {
            &mut s.pass
        })
    }
    /// Read the active pass's completed-step counter.
    pub(crate) fn steps(&self) -> MappedRwLockReadGuard<'_, usize> {
        RwLockReadGuard::<'_, Status>::map(self.data.read().unwrap(), |s| {
            &s.steps
        })
    }
    /// Borrow the active pass's counter for an increment.
    pub(crate) fn steps_mut(&self) -> MappedRwLockWriteGuard<'_, usize> {
        RwLockWriteGuard::<'_, Status>::map(self.data.write().unwrap(), |s| {
            &mut s.steps
        })
    }
    /// Read the estimated work total; actual traversal work can exceed it.
    pub(crate) fn steps_total(&self) -> MappedRwLockReadGuard<'_, usize> {
        RwLockReadGuard::<'_, Status>::map(self.data.read().unwrap(), |s| {
            &s.steps_total
        })
    }
}
/// Observable pipeline stage and approximate work counters.
#[derive(Debug, Getters)]
pub struct Status {
    /// Input texts retained for progress consumers.
    #[getter(skip)]
    pub insert_texts: Vec<String>,
    /// Latest stage recorded by a pass.
    pass: ProcessStatus,
    /// Work units completed since the most recent counter reset.
    steps: usize,
    /// Nonzero work estimate supplied at a stage transition.
    steps_total: usize,
}
impl Default for Status {
    /// Initialize an empty stage with a nonzero placeholder work total.
    fn default() -> Self {
        Self {
            insert_texts: Default::default(),
            pass: Default::default(),
            steps: 0,
            steps_total: 1,
        }
    }
}
impl Status {
    /// Create empty-stage progress state carrying the supplied input texts.
    pub fn new(insert_texts: impl IntoIterator<Item = impl ToString>) -> Self {
        Self {
            insert_texts: insert_texts
                .into_iter()
                .map(|s| s.to_string())
                .collect(),
            ..Default::default()
        }
    }
    /// Set a stage and counters, rejecting a zero total and raising it to `steps`.
    pub(crate) fn next_pass(
        &mut self,
        pass: ProcessStatus,
        steps: usize,
        steps_total: usize,
    ) {
        assert!(steps_total > 0);
        self.pass = pass;
        self.steps = steps;
        self.steps_total = steps_total.max(steps);
    }
}
/// Named collection of texts whose substrings share one vocabulary.
#[derive(Debug, Default, Deref, Serialize, Deserialize)]
pub struct Corpus {
    /// Logical name used for optional cache lookup.
    pub(crate) name: String,
    /// Input order defines occurrence text indices.
    #[deref]
    pub(crate) texts: Vec<String>,
}
impl Corpus {
    /// Collect a corpus without validating its inputs or running any passes.
    pub fn new(
        name: impl ToString,
        texts: impl IntoIterator<Item = impl ToString>,
    ) -> Self {
        let mut s = Status::new(Vec::<String>::new());
        s.pass = ProcessStatus::Frequency;
        Self {
            name: name.to_string(),
            texts: texts.into_iter().map(|s| s.to_string()).collect(),
        }
    }

    /// Get the storage key for this corpus
    pub(crate) fn storage_key(&self) -> &str {
        &self.name
    }
}
/// Legacy channel sender for an abort signal; not used by the current pipeline.
pub(crate) type AbortSender = std::sync::mpsc::Sender<()>;
/// Receiving end of the legacy abort channel.
pub(crate) type AbortReceiver = std::sync::mpsc::Receiver<()>;
/// Selected output plus the exhaustive graph and final label identities.
#[derive(Debug)]
pub struct ParseResult {
    /// Reachable selected tokens and their constructed alternative patterns.
    pub graph: Hypergraph,
    /// All discovered substrings and elementary decompositions.
    pub containment: Hypergraph,
    /// Selection after all passes, including gap-token discovery.
    pub labels: HashSet<VertexKey>,
}
/// Build containment, select frequency tokens and wrappers, then construct patterns.
///
/// Cancellation is cooperative in traversal passes, not in initial substring
/// enumeration. Output is currently order-dependent because partitioning can
/// add labels after earlier nodes have already been decomposed.
pub fn parse_corpus(
    corpus: Corpus,
    mut status: StatusHandle,
    cancellation: impl Into<Cancellation>,
) -> RunResult<ParseResult> {
    let image = LabellingImage::from_corpus(&corpus, &mut status)?;
    let test_corpus = TestCorpus::new(image, corpus);
    let mut ctx = LabellingCtx::new(test_corpus, status, cancellation.into());

    ctx.label_freq()?;

    ctx.label_wrap()?;

    let graph = ctx.label_part()?;

    let LabellingImage { vocab, labels } = ctx.corpus.image;
    Ok(ParseResult {
        graph,
        labels,
        containment: vocab.containment,
    })
}
