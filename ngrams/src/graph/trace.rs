use context_trace_legacy::VertexKey;
use tracing::{Level, Span};

use super::{
    partitions::container::PartitionContainer,
    vocabulary::{entry::HasVertexEntries, NGramId, Vocabulary},
};

pub(crate) fn partition_span(token: &str) -> Span {
    tracing::debug_span!("partition_node", token = %token)
}

pub(crate) fn partition_patterns(
    vocab: &Vocabulary,
    token: &str,
    container: &PartitionContainer,
) {
    if !tracing::enabled!(Level::DEBUG) {
        return;
    }
    let patterns: Vec<Vec<_>> = container
        .iter()
        .map(|pattern| {
            pattern
                .iter()
                .map(|child| vocab.containment.index_string(*child))
                .collect()
        })
        .collect();
    tracing::debug!(token = %token, ?patterns, "built partition patterns");
}

pub(crate) fn selected_child(vocab: &Vocabulary, child: NGramId, offset: usize) {
    tracing::trace!(
        child = %vocab.expect_vertex(&child).ngram,
        offset,
        "selected labelled child"
    );
}

pub(crate) fn descend_through_child(child: &str, offset: usize) {
    tracing::trace!(child = %child, offset, "descending through unlabelled child");
}

pub(crate) fn labelled_gap(label: &str) {
    tracing::debug!(label = %label, "labelled partition gap");
}

pub(crate) fn labelled_shared_descendant(
    vocab: &Vocabulary,
    node: NGramId,
    gap: VertexKey,
    offset: usize,
) {
    tracing::debug!(
        label = %vocab.expect_vertex(&node).ngram,
        gap = %vocab.expect_vertex(&gap).ngram,
        offset,
        "labelled shared gap descendant"
    );
}

#[cfg(test)]
pub(crate) fn scheduled_roots(vocab: &Vocabulary, roots: &[NGramId]) {
    tracing::debug!(
        roots = ?roots
            .iter()
            .map(|root| vocab.containment.vertex_key_string(&root.key))
            .collect::<Vec<_>>(),
        "scheduled partition roots"
    );
}
