//! Exhaustive substring vocabulary, occurrence metadata, and pass status.

use crate::graph::{
    containment::{
        CorpusCtx,
        TextLevelCtx,
    },
    traversal::direction::{
        TopDown,
        TraversalDirection,
    },
    vocabulary::entry::{
        HasVertexEntries,
        VocabEntry,
    },
    Corpus,
};
use context_trace::legacy::{
    graph::{
        vertex::{
            has_vertex_index::{
                HasVertexIndex,
                ToToken,
            },
            has_vertex_key::HasVertexKey,
            key::VertexKey,
            token::{
                Token,
                TokenWidth,
            },
            wide::Wide,
            VertexIndex,
        },
        Hypergraph,
    },
    HashMap,
    HashSet,
};
use derive_more::{
    Deref,
    From,
};
use derive_new::new;
use itertools::Itertools;
use serde::{
    Deserialize,
    Serialize,
};
use std::{
    borrow::Borrow,
    cmp::Ordering,
    fmt::Display,
    ops::Range,
    path::{
        absolute,
        Path,
        PathBuf,
    },
    sync::{
        Arc,
        RwLock,
    },
};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use tap::Tap;

use super::{
    Status,
    StatusHandle,
};

pub(crate) mod entry;

/// Stable vertex key paired with the substring's character width.
#[derive(
    Debug,
    Clone,
    Copy,
    From,
    new,
    Default,
    Deref,
    Hash,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
)]
pub(crate) struct NGramId {
    /// Identity shared by the vocabulary and derived partition graph.
    #[deref]
    pub(crate) key: VertexKey,
    /// Number of characters represented by the vertex.
    pub(crate) width: usize,
}
impl From<NGramId> for VertexKey {
    /// Drop cached width and retain vertex identity.
    fn from(ngram_id: NGramId) -> Self {
        ngram_id.key
    }
}
impl HasVertexKey for NGramId {
    /// Expose the identity used by graph and label lookups.
    fn vertex_key(&self) -> VertexKey {
        self.key
    }
}
impl Wide for NGramId {
    /// Wrap the cached character width in the graph's width type.
    fn width(&self) -> TokenWidth {
        TokenWidth(self.width)
    }
}

/// Ordered milestones used for progress reporting and labelling guards.
#[derive(
    Default, Debug, PartialEq, Eq, Copy, Clone, Serialize, Deserialize, EnumIter,
)]
pub enum ProcessStatus {
    /// No construction stage has been recorded.
    #[default]
    Empty,
    /// Enumerating substrings and occurrences.
    Containment,
    /// Selecting tokens by occurrence frequency.
    Frequency,
    /// Building positioned child covers for the frozen retained set (v3).
    ///
    /// Ordered directly after `Frequency` so the v3 pipeline is monotonic
    /// (`Containment < Frequency < Compositions < Finished`). The legacy
    /// pipeline never enters this stage, so its `< Frequency` and
    /// `< Wrappers` guards are unaffected; the v3 path never calls them.
    Compositions,
    /// Selecting tokens that enclose overlapping labelled children.
    Wrappers,
    /// Constructing decompositions in the output graph.
    Partitions,
    /// Partition construction has completed.
    Finished,
}
impl PartialOrd for ProcessStatus {
    /// Use the same pipeline-stage ordering as the total comparison.
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for ProcessStatus {
    /// Compare declaration-order discriminants to order pipeline milestones.
    fn cmp(
        &self,
        other: &Self,
    ) -> Ordering {
        (*self as usize).cmp(&(*other as usize))
    }
}
/// All discovered substring vertices and their corpus positions.
#[derive(
    Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq, Deref,
)]
pub(crate) struct Vocabulary {
    //#[deref]
    /// Elementary decompositions and reciprocal containment links.
    pub(crate) containment: Hypergraph,
    /// Corpus name used as a storage key.
    pub(crate) name: String,
    /// Unique content-to-handle mapping, exposed through dereferencing.
    #[deref]
    pub(crate) ids: HashMap<String, NGramId>,
    /// Single-character vertices.
    pub(crate) leaves: HashSet<NGramId>,
    /// Vertices whose content equals at least one full input text.
    ///
    /// Iteration is unsorted; consumers must not assume corpus input order.
    pub(crate) roots: HashSet<NGramId>,
    /// Occurrence metadata keyed by containment vertex identity.
    pub(crate) entries: HashMap<VertexKey, VocabEntry>,
}

impl Vocabulary {
    /// Build a fresh vocabulary without consulting the labelling-image cache.
    pub(crate) fn from_corpus(
        corpus: &Corpus,
        status: &mut StatusHandle,
    ) -> Result<Self, super::traversal::pass::CancelReason> {
        let mut vocab: Vocabulary = Default::default();
        vocab.name.clone_from(&corpus.name);
        vocab.containment_pass(&CorpusCtx { corpus, status })?;
        Ok(vocab)
    }

    /// Enumerate widths globally from short to long, then each text at that width.
    ///
    /// Rejects invalid input (see [`Corpus::validate`]) before enumeration.
    /// Widths count atoms (`char`s). Progress counts enumerated windows. After
    /// enumeration every full input is resolved to its vertex in `roots`.
    pub(crate) fn containment_pass(
        &mut self,
        ctx: &CorpusCtx<'_>,
    ) -> Result<(), super::traversal::pass::CancelReason> {
        ctx.corpus.validate()?;
        let widths = ctx.corpus.iter().map(|t| t.chars().count()).collect_vec();
        let N: usize = widths.iter().copied().max().unwrap_or_default();
        let windows: usize = widths.iter().map(|n| n * (n + 1) / 2).sum();
        ctx.status.next_pass(
            super::vocabulary::ProcessStatus::Containment,
            0,
            windows,
        );
        // Width is the outer loop so all proper child strings exist first.
        Itertools::cartesian_product((1..=N), ctx.corpus.iter().enumerate())
            .for_each(|(n, (i, text))| {
                TextLevelCtx {
                    corpus_ctx: ctx,
                    texti: i,
                    text,
                    n,
                }
                .on_nlevel(self)
            });
        for text in ctx.corpus.iter() {
            let id = *self
                .ids
                .get(text)
                .expect("every validated input is enumerated as a window");
            self.roots.insert(id);
        }
        Ok(())
    }
    //pub(crate) fn clean(&mut self) -> HashSet<NGramId> {
    //    let drained: HashSet<_> = self.entries
    //        .extract_if(|_, e| !e.needs_node())
    //        .map(|(i, _)| i)
    //        .collect();
    //    self.ids.retain(|_, i| !drained.contains(i));
    //    drained
    //}
}
