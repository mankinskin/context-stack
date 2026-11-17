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
use context_trace::{
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
    /// Vertices marked as full texts when first inserted.
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

    /// Enumerate lengths globally from short to long, then each text at that length.
    ///
    /// Rejects an empty corpus or a zero maximum byte length. The maximum uses
    /// bytes while individual windows use characters; see the containment module.
    pub(crate) fn containment_pass(
        &mut self,
        ctx: &CorpusCtx<'_>,
    ) -> Result<(), super::traversal::pass::CancelReason> {
        let N: usize = ctx
            .corpus
            .iter()
            .map(|t| t.len())
            .max()
            .ok_or(super::traversal::pass::CancelReason::EmptyVocabulary)?;
        if N == 0 {
            return Err(super::traversal::pass::CancelReason::EmptyVocabulary);
        }
        ctx.status.next_pass(
            super::vocabulary::ProcessStatus::Containment,
            0,
            N * (N - 1),
        );
        // Length is the outer loop so all proper child strings exist first.
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
