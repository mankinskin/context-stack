//! Shared label state, optional persistence, and entry points for the three passes.
//!
//! All passes use one growing label set. Frequency and wrapper entry points are
//! guarded by progress status; partition construction always starts a new graph.

use derive_more::derive::{
    Deref,
    DerefMut,
};
use derive_new::new;
use itertools::Itertools;
use serde::{
    Deserialize,
    Serialize,
};
use std::sync::{
    Arc,
    RwLock,
};
use tap::Tap;

use crate::{
    cancellation::{
        Cancellable,
        Cancellation,
    },
    config::CacheConfig,
    graph::{
        partitions::PartitionsCtx,
        traversal::pass::{
            CancelReason,
            RunResult,
            TraversalPass,
        },
        vocabulary::{
            ProcessStatus,
            Vocabulary,
        },
        Corpus,
        Status,
    },
    storage::{
        self,
        Storage,
        StorageError,
    },
    tests::TestCorpus,
};
use context_trace::{
    graph::{
        vertex::{
            key::VertexKey,
            VertexIndex,
        },
        Hypergraph,
    },
    HashSet,
};

pub(crate) mod frequency;
use frequency::FrequencyCtx;

pub(crate) mod wrapper;
use wrapper::WrapperCtx;

use super::StatusHandle;

impl From<Vocabulary> for LabellingImage {
    /// Start with the supplied vocabulary and no selected vertices.
    fn from(vocab: Vocabulary) -> Self {
        Self {
            vocab,
            labels: Default::default(),
        }
    }
}
/// Persistable vocabulary and current selection, without output graph or status.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct LabellingImage {
    /// Exhaustive substring graph and occurrence metadata.
    pub(crate) vocab: Vocabulary,
    /// Selected vertex identities; selection reasons are not stored separately.
    pub(crate) labels: HashSet<VertexKey>,
}
impl LabellingImage {
    /// Get the storage key for this labelling image
    pub(crate) fn storage_key(&self) -> String {
        self.vocab.name.clone()
    }

    /// Write to storage using the platform-appropriate storage backend
    pub(crate) fn write_to_storage(&self) -> Result<(), StorageError> {
        storage::store(&self.storage_key(), self)
    }

    /// Read from storage using the platform-appropriate storage backend
    pub(crate) fn read_from_storage(key: &str) -> Result<Self, StorageError> {
        storage::load(key)
    }

    /// Check if this labelling image exists in storage
    pub(crate) fn exists_in_storage(key: &str) -> bool {
        storage::exists(key)
    }

    /// Load by corpus name on native platforms when caching is enabled, or rebuild.
    ///
    /// Cache read errors fall through to fresh construction. Loaded images are
    /// not checked against the supplied texts and do not restore progress status.
    pub(crate) fn from_corpus(
        corpus: &Corpus,
        status: &mut StatusHandle,
    ) -> RunResult<Self> {
        // On native, try to read from storage cache first (only if cache is enabled)
        #[cfg(not(target_arch = "wasm32"))]
        if CacheConfig::is_enabled() {
            let key = corpus.name.clone();
            if let Ok(image) = Self::read_from_storage(&key) {
                eprintln!("Containment Pass already processed.");
                return Ok(image);
            }
        }

        // Create fresh from corpus
        Ok(Self::from(Vocabulary::from_corpus(corpus, status)?))
    }

    /// Write to the target storage location for this image (only if cache is enabled)
    pub(crate) fn write_to_target_file(&self) -> Result<(), StorageError> {
        if CacheConfig::is_enabled() {
            self.write_to_storage()
        } else {
            Ok(())
        }
    }
}
/// Mutable execution context shared by labelling, cover searches, and partitions.
#[derive(Debug, Deref, DerefMut, new)]
pub(crate) struct LabellingCtx {
    /// Image and source corpus, held in the same wrapper used by test helpers.
    #[deref]
    #[deref_mut]
    pub(crate) corpus: TestCorpus,
    /// Shared pass marker and progress counters.
    pub(crate) status: StatusHandle,
    /// Cooperatively checked cancellation source.
    pub(crate) cancellation: Cancellation,
}
impl LabellingCtx {
    /// Initialize corpus state and a fresh progress handle without running labels.
    pub(crate) fn from_corpus(
        corpus: Corpus,
        cancellation: impl Into<Cancellation>,
    ) -> RunResult<Self> {
        let mut status = StatusHandle::default();
        Ok(Self {
            corpus: TestCorpus::new(
                LabellingImage::from_corpus(&corpus, &mut status)?,
                corpus,
            ),
            status,
            cancellation: cancellation.into(),
        })
    }
    /// Return a cancellation error when the supplied handle has been signalled.
    pub(crate) fn check_cancelled(&self) -> RunResult<()> {
        if self.cancellation.is_cancelled() {
            Err(CancelReason::Cancelled)
        } else {
            Ok(())
        }
    }
    /// Borrow the exhaustive vocabulary rather than the derived output graph.
    pub(crate) fn vocab(&self) -> &'_ Vocabulary {
        &self.corpus.image.vocab
    }
    /// Borrow the current selection shared by every pass.
    pub(crate) fn labels(&self) -> &'_ HashSet<VertexKey> {
        &self.corpus.image.labels
    }
    /// Mutate global selection; no automatic graph invalidation is performed.
    pub(crate) fn labels_mut(&mut self) -> &'_ mut HashSet<VertexKey> {
        &mut self.corpus.image.labels
    }
    /// Run frequency selection unless status has already reached that stage.
    ///
    /// Optional cache write errors are currently discarded.
    pub(crate) fn label_freq(&mut self) -> RunResult<()> {
        if *self.status.pass() < ProcessStatus::Frequency {
            FrequencyCtx::new(&mut *self).run()?;
            let _ = self.image.write_to_target_file();
        } else {
            eprintln!("Frequency Pass already processed.");
        }
        Ok(())
    }
    /// Run bottom-up wrapper selection unless status has reached that stage.
    ///
    /// Optional cache write errors are currently discarded.
    pub(crate) fn label_wrap(&mut self) -> RunResult<()> {
        if *self.status.pass() < ProcessStatus::Wrappers {
            WrapperCtx::new(&mut *self).run()?;
            let _ = self.image.write_to_target_file();
        } else {
            eprintln!("Wrapper Pass already processed.");
        }
        Ok(())
    }
    /// Construct a fresh output graph, potentially adding further global labels.
    pub(crate) fn label_part(&mut self) -> RunResult<Hypergraph> {
        let mut ctx = PartitionsCtx::from(&mut *self);
        ctx.run()?;
        Ok(ctx.graph)
    }
}
