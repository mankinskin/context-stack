//! Build the exhaustive substring vocabulary and its elementary decompositions.
//!
//! The caller processes increasing n-gram lengths so shorter strings already
//! exist when a new composite is linked to them. Repeated content reuses a
//! vertex and adds an occurrence rather than creating another vertex.
//!
//! Widths, windows, occurrence offsets, and child slices all count atoms
//! (Rust `char`s), never UTF-8 bytes. Roots are resolved by the caller after
//! enumeration, independent of where the content was first interned.

use std::sync::{
    Arc,
    RwLock,
};

use derive_more::{
    Deref,
    From,
};
use derive_new::new;
use itertools::Itertools;
use ngram::NGram;
use serde::{
    Deserialize,
    Serialize,
};

use crate::graph::{
    vocabulary::{
        entry::{
            HasVertexEntries,
            VocabEntry,
        },
        NGramId,
        Vocabulary,
    },
    Corpus,
};
use context_trace::{
    graph::vertex::{
        data::VertexDataBuilder,
        has_vertex_index::{
            HasVertexIndex,
            ToToken,
        },
        has_vertex_key::HasVertexKey,
        location::child::ChildLocation,
        pattern::id::PatternId,
        token::Token,
        wide::Wide,
        ChildPatterns,
    },
    HashMap,
};

use super::{
    Status,
    StatusHandle,
};

/// The start of one substring occurrence within the input corpus.
#[derive(
    Debug,
    Clone,
    Copy,
    From,
    new,
    Default,
    Hash,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
)]
pub(crate) struct TextLocation {
    /// Zero-based index of the input text in the corpus.
    pub(crate) texti: usize,
    /// Zero-based character offset of the substring within that text.
    pub(crate) x: usize,
}

/// Corpus-wide inputs and progress reporting shared by containment construction.
#[derive(Debug, From)]
pub(crate) struct CorpusCtx<'a> {
    /// Texts whose substrings are being added to the vocabulary.
    pub(crate) corpus: &'a Corpus,
    /// Shared progress state, advanced once per enumerated occurrence.
    pub(crate) status: &'a mut StatusHandle,
}
/// One input text at one n-gram length in the containment pass.
#[derive(Debug, Clone, Copy, From, Deref)]
pub(crate) struct TextLevelCtx<'a> {
    /// Corpus-wide context, also exposed through dereferencing.
    #[deref]
    pub(crate) corpus_ctx: &'a CorpusCtx<'a>,
    /// Index of this text, used to identify its occurrences.
    pub(crate) texti: usize,
    /// Input text over which the character window slides.
    pub(crate) text: &'a String,
    /// Number of characters in each window.
    pub(crate) n: usize,
}

impl TextLevelCtx<'_> {
    /// Record every length-`n` character window, including overlapping windows.
    ///
    /// The caller must populate shorter substrings first because creating a
    /// composite looks up its children in the existing vocabulary.
    pub(crate) fn on_nlevel(
        &self,
        vocab: &mut Vocabulary,
    ) {
        self.text
            .chars()
            .ngrams(self.n)
            .enumerate()
            .for_each(|(xi, ngrami)| {
                // Window indices are character offsets, not UTF-8 byte offsets.
                NGramFrequencyCtx {
                    level_ctx: *self,
                    ngram: &String::from_iter(ngrami),
                    occurrence: TextLocation::new(self.texti, xi),
                }
                .on_ngram(vocab)
            })
    }
}

/// A single substring occurrence to merge into the containment vocabulary.
#[derive(Debug, Clone, Copy, From, Deref)]
pub(crate) struct NGramFrequencyCtx<'a> {
    /// Text and window-length context for this occurrence.
    #[deref]
    pub(crate) level_ctx: TextLevelCtx<'a>,
    /// Content used to look up or create the shared vocabulary vertex.
    pub(crate) ngram: &'a String,
    /// Position of this particular occurrence in the corpus.
    pub(crate) occurrence: TextLocation,
}

impl NGramFrequencyCtx<'_> {
    /// Count this occurrence, creating its vertex only for previously unseen content.
    pub(crate) fn on_ngram(
        &self,
        vocab: &mut Vocabulary,
    ) {
        *self.status.steps_mut() += 1;
        if let Some(ctx) = vocab.get_vertex_mut(self.ngram) {
            // Occurrence identity includes both the text index and its offset.
            ctx.entry.occurrences.insert(self.occurrence);
        } else {
            self.on_first_ngram(vocab)
        }
    }
    /// Insert unseen content, its first occurrence, and reciprocal containment links.
    ///
    /// Single characters become atoms and leaves; composites receive elementary
    /// child patterns. Root membership is resolved after enumeration.
    ///
    /// # Panics
    ///
    /// Panics if required child strings have not already been inserted.
    pub(crate) fn on_first_ngram(
        &self,
        vocab: &mut Vocabulary,
    ) {
        let children = self.find_children(vocab);

        let entry = VocabEntry {
            occurrences: FromIterator::from_iter([self.occurrence]),
            ngram: self.ngram.clone(),
        };
        let builder = if self.n != 1 {
            VertexDataBuilder::default()
                .width(self.n)
                .children(children.clone())
        } else {
            VertexDataBuilder::default().width(self.n)
        };
        let data = vocab.containment.finish_vertex_builder(builder);
        let id = NGramId::new(data.vertex_key(), self.n);
        vocab.ids.insert(self.ngram.clone(), id);
        vocab.entries.insert(id.vertex_key(), entry);
        if self.n == 1 {
            // Atom data preserves the character used to reconstruct graph strings.
            let atom = self.ngram.chars().next().unwrap().into();
            vocab.containment.insert_atom_data(atom, data);
            vocab.leaves.insert(id);
        } else {
            vocab.containment.insert_vertex_data(data);
        }
        for (pid, pat) in children {
            // Despite the local name, this token is the new parent of pat's nodes.
            let child = Token::new(
                vocab.containment.expect_index_for_key(&id),
                id.width(),
            );
            let pat_vec: Vec<Token> = pat.into();
            vocab
                .containment
                .add_parents_to_pattern_nodes(pat_vec, child, pid);
        }
    }
    /// Build elementary prefix/suffix decompositions from existing vocabulary nodes.
    ///
    /// For composites wider than two atoms, returns `[prefix(n - 1), last]`
    /// and `[first, suffix(n - 1)]`. A bigram has just one `[first, last]`
    /// pattern; an atom has none. All slicing happens at `char` boundaries.
    ///
    /// # Panics
    ///
    /// Panics when a required child is absent.
    pub(crate) fn find_children(
        &self,
        vocab: &mut Vocabulary,
    ) -> ChildPatterns {
        let chars: Vec<char> = self.ngram.chars().collect();
        let w = chars.len();
        let token = |vocab: &Vocabulary, s: &[char]| {
            let id = vocab.ids.get(&String::from_iter(s)).unwrap();
            Token::new(vocab.containment.expect_index_for_key(id), id.width())
        };
        let patterns: Vec<Vec<Token>> = match w {
            0 | 1 => vec![],
            2 => vec![vec![token(vocab, &chars[..1]), token(vocab, &chars[1..])]],
            _ => vec![
                vec![token(vocab, &chars[..w - 1]), token(vocab, &chars[w - 1..])],
                vec![token(vocab, &chars[..1]), token(vocab, &chars[1..])],
            ],
        };
        patterns
            .into_iter()
            .map(|p| (PatternId::default(), p.into_iter().collect()))
            .collect()
    }
}
