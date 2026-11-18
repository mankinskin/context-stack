//! Maximal positioned child covers for a frozen retained set (v3 oracle).
//!
//! The composer reads an immutable [`Vocabulary`] and a read-only retained set
//! and computes, for every retained vertex, every maximal **proper** retained
//! occurrence as `(atom_offset, child)`. It never interns content, mutates
//! labels, or writes to any graph.
//!
//! Vertices are processed in increasing width. An atom exposes itself at
//! offset 0. A composite `V` of width `n` combines the occurrences exposed by
//! its width-`n - 1` prefix (at their offsets) and suffix (shifted by one) and
//! normalizes them: sort by start ascending and end descending, then keep an
//! interval only if its end exceeds every end seen so far. Within one owner a
//! span determines its content, so this drops duplicates and contained
//! intervals while keeping partial overlaps. A retained `V` stores the result
//! as its cover and exposes only itself; an unretained `V` exposes its cover.

use std::collections::BTreeMap;

use context_trace::{
    graph::vertex::key::VertexKey,
    HashMap,
    HashSet,
};
use itertools::Itertools;

use crate::{
    cancellation::{
        Cancellable,
        Cancellation,
    },
    graph::{
        traversal::pass::{
            CancelReason,
            RunResult,
        },
        vocabulary::{
            NGramId,
            ProcessStatus,
            Vocabulary,
        },
        StatusHandle,
    },
};

/// One positioned child occurrence: `(atom offset in the owner, child)`.
pub(crate) type Occurrence = (usize, NGramId);

/// Canonical cover sorted by `(offset, child width)`.
pub(crate) type Cover = Vec<Occurrence>;

/// Positioned covers of every retained vertex, keyed by its vocabulary identity.
///
/// Atoms have empty covers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PositionedCovers {
    pub(crate) covers: HashMap<VertexKey, Cover>,
}

impl PositionedCovers {
    /// Convert identities to content strings, ordered by owner content.
    pub(crate) fn by_content(
        &self,
        vocab: &Vocabulary,
    ) -> BTreeMap<String, Vec<(usize, String)>> {
        let content = |key: &VertexKey| vocab.entries[key].ngram.clone();
        self.covers
            .iter()
            .map(|(owner, cover)| {
                (
                    content(owner),
                    cover
                        .iter()
                        .map(|(offset, child)| (*offset, content(&child.key)))
                        .collect(),
                )
            })
            .collect()
    }
}

/// Processing order of vertices that share a width.
///
/// The result must not depend on it; `Reversed` exists to test that.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum TieOrder {
    /// Ascending content order.
    #[default]
    Forward,
    /// Descending content order.
    Reversed,
}

/// Compute positioned covers for `retained` using ascending content tie order.
pub(crate) fn compose(
    vocab: &Vocabulary,
    retained: &HashSet<VertexKey>,
    status: &StatusHandle,
    cancellation: &Cancellation,
) -> RunResult<PositionedCovers> {
    compose_with_order(vocab, retained, status, cancellation, TieOrder::Forward)
}

/// Compute positioned covers, processing same-width vertices in `tie_order`.
///
/// Fails with [`CancelReason::Cancelled`] if `cancellation` is signalled
/// before or during the work, and with [`CancelReason::Error`] if `retained`
/// names a vertex outside the vocabulary or the vocabulary lacks a prefix or
/// suffix of some vertex. No partial result is returned.
pub(crate) fn compose_with_order(
    vocab: &Vocabulary,
    retained: &HashSet<VertexKey>,
    status: &StatusHandle,
    cancellation: &Cancellation,
    tie_order: TieOrder,
) -> RunResult<PositionedCovers> {
    let check = || {
        if cancellation.is_cancelled() {
            Err(CancelReason::Cancelled)
        } else {
            Ok(())
        }
    };
    check()?;
    if retained.iter().any(|k| !vocab.entries.contains_key(k)) {
        return Err(CancelReason::Error);
    }
    status.next_pass(ProcessStatus::Compositions, 0, vocab.ids.len().max(1));

    let mut levels: BTreeMap<usize, Vec<(&str, NGramId)>> = BTreeMap::new();
    for (content, id) in vocab.ids.iter() {
        levels
            .entry(id.width)
            .or_default()
            .push((content.as_str(), *id));
    }

    let mut covers = PositionedCovers::default();
    // Occurrences exposed by every vertex of the previous width only.
    let mut exposed_prev: HashMap<VertexKey, Cover> = HashMap::default();
    let mut prev_width = 0;
    for (width, mut level) in levels {
        check()?;
        if width != prev_width + 1 {
            return Err(CancelReason::Error);
        }
        level.sort_unstable_by(|a, b| a.0.cmp(b.0));
        if tie_order == TieOrder::Reversed {
            level.reverse();
        }
        let mut exposed: HashMap<VertexKey, Cover> = HashMap::default();
        for &(content, id) in &level {
            check()?;
            let is_retained = retained.contains(&id.key);
            if width == 1 {
                if is_retained {
                    covers.covers.insert(id.key, Vec::new());
                }
                exposed.insert(id.key, vec![(0, id)]);
                continue;
            }
            let lookup = |part: &str| -> RunResult<&Cover> {
                vocab
                    .ids
                    .get(part)
                    .filter(|p| p.width == width - 1)
                    .and_then(|p| exposed_prev.get(&p.key))
                    .ok_or(CancelReason::Error)
            };
            let (last, _) = content.char_indices().last().unwrap();
            let first = content.chars().next().unwrap().len_utf8();
            let prefix = lookup(&content[..last])?;
            let suffix = lookup(&content[first..])?;
            let cover = normalize(
                prefix
                    .iter()
                    .copied()
                    .chain(suffix.iter().map(|&(o, c)| (o + 1, c))),
            );
            if is_retained {
                covers.covers.insert(id.key, cover);
                exposed.insert(id.key, vec![(0, id)]);
            } else {
                exposed.insert(id.key, cover);
            }
        }
        *status.steps_mut() += level.len();
        exposed_prev = exposed;
        prev_width = width;
    }
    Ok(covers)
}

/// Sort by start ascending and end descending; keep strictly growing ends.
fn normalize(occurrences: impl IntoIterator<Item = Occurrence>) -> Cover {
    let mut max_end = 0;
    occurrences
        .into_iter()
        .sorted_unstable_by_key(|&(start, child)| {
            (start, std::cmp::Reverse(start + child.width))
        })
        .filter(|&(start, child)| {
            let end = start + child.width;
            let keep = end > max_end;
            max_end = max_end.max(end);
            keep
        })
        .collect()
}
