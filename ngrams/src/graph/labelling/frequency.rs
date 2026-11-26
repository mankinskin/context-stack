//! Select substrings whose occurrence counts exceed a selected ancestor's count.
//!
//! This is a relative frequency criterion, not a configurable minimum count.

use derive_more::{
    Deref,
    DerefMut,
};
use derive_new::new;
use itertools::Itertools;

use crate::graph::{
    labelling::LabellingCtx,
    traversal::pass::RunResult,
    utils::cover::frequency::FrequencyCover,
    vocabulary::{
        entry::{
            HasVertexEntries,
            VertexCtx,
        },
        ProcessStatus,
    },
};
use context_trace_legacy::graph::vertex::has_vertex_key::HasVertexKey;

/// Deterministic frequency selection over the whole containment vocabulary.
#[derive(Debug, Deref, new, DerefMut)]
pub(crate) struct FrequencyCtx<'b> {
    /// Vocabulary and shared label set updated by this pass.
    #[deref]
    #[deref_mut]
    pub(crate) ctx: &'b mut LabellingCtx,
}

impl FrequencyCtx<'_> {
    /// Seed all roots and atoms, then test every other vertex exactly once.
    ///
    /// Candidates are visited in decreasing width with ties broken by content,
    /// so every strictly wider ancestor is decided before a candidate is tested.
    /// Same-width vertices never contain each other, so the tie order does not
    /// influence the result; it only makes the schedule explicit.
    pub(crate) fn run(&mut self) -> RunResult<()> {
        tracing::debug!("Frequency Pass");
        self.check_cancelled()?;
        let seeds = self
            .vocab()
            .roots
            .iter()
            .chain(self.vocab().leaves.iter())
            .map(HasVertexKey::vertex_key)
            .collect_vec();
        self.labels_mut().extend(seeds);

        let candidates = self
            .vocab()
            .ids
            .iter()
            .filter(|(_, id)| !self.labels().contains(&id.vertex_key()))
            .sorted_by(|(a, ia), (b, ib)| {
                ib.width.cmp(&ia.width).then_with(|| a.cmp(b))
            })
            .map(|(_, id)| id.vertex_key())
            .collect_vec();
        let total = self.vocab().entries.len();
        self.status.next_pass(
            ProcessStatus::Frequency,
            total - candidates.len(),
            total,
        );
        for key in candidates {
            self.check_cancelled()?;
            *self.status.steps_mut() += 1;
            let entry = self.vocab().get_vertex(&key).unwrap();
            if self.entry_is_frequent(&entry)? {
                self.labels_mut().insert(key);
            }
        }
        Ok(())
    }
    /// Whether any ancestor in the occurrence cover has fewer occurrences.
    ///
    /// Counts include overlapping positions across the entire corpus. A selected
    /// candidate is also printed to stderr for diagnostics.
    pub(crate) fn entry_is_frequent(
        &self,
        entry: &VertexCtx,
    ) -> RunResult<bool> {
        FrequencyCover::from_entry(self, entry).map(|cover| {
            cover
                .iter()
                .any(|p| {
                    self.vocab().get_vertex(p).unwrap().count() < entry.count()
                })
                .then(|| tracing::trace!(ngram = %entry.ngram, "frequency token"))
                .is_some()
        })
    }
}
