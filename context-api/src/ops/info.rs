//! Read-only graph queries: inputs, stats, vertex, validate.

use std::collections::{
    BTreeMap,
    BTreeSet,
};

use context_trace::{
    HasVertexIndex,
    Wide,
    VertexIndex,
    positioned::{
        CompositionRead,
        GraphRead,
        NodeRef,
        VertexRead,
    },
};

use super::{
    View,
    text::{
        self,
        label_width,
    },
};
use crate::{
    error::ApiError,
    types::{
        GraphStats,
        GraphSummary,
        InputList,
        Limits,
        OccurrenceInfo,
        ParentInfo,
        ValidationReport,
        VertexInfo,
        VertexRef,
    },
};

pub(crate) fn inputs(
    view: &View<'_>,
    limits: &Limits,
) -> InputList {
    let limit = limits.limit.unwrap_or(usize::MAX);
    InputList {
        total: view.meta.inputs.len(),
        items: view.meta.inputs.iter().take(limit).cloned().collect(),
    }
}

pub(crate) fn stats(
    view: &View<'_>,
    summary: GraphSummary,
) -> GraphStats {
    let mut width_histogram = BTreeMap::new();
    let mut parent_count_histogram = BTreeMap::new();
    let mut max_width = 0;
    let (mut composites, mut occurrences) = (0usize, 0usize);
    for vertex in view.graph.vertices() {
        let width = vertex.width().0;
        max_width = max_width.max(width);
        *width_histogram.entry(width).or_insert(0) += 1;
        *parent_count_histogram.entry(vertex.parent_count()).or_insert(0) += 1;
        if let NodeRef::Composite(composition) = vertex.node() {
            composites += 1;
            occurrences += composition.len();
        }
    }
    GraphStats {
        summary,
        max_width,
        width_histogram,
        parent_count_histogram,
        avg_composition_size: if composites == 0 {
            0.0
        } else {
            occurrences as f64 / composites as f64
        },
    }
}

/// Parents of a vertex with all offsets, ordered by parent width, index.
pub(crate) fn parents(
    view: &View<'_>,
    index: VertexIndex,
    label_width: usize,
) -> Vec<ParentInfo> {
    let vertex = view.graph.vertex(index).expect("vertex of this graph");
    let mut by_parent: BTreeMap<(usize, usize), BTreeSet<usize>> = BTreeMap::new();
    for (parent, offset) in vertex.parents() {
        by_parent
            .entry((parent.width.0, parent.index.0))
            .or_default()
            .insert(offset.into());
    }
    by_parent
        .into_iter()
        .map(|((_, parent), offsets)| ParentInfo {
            parent: text::token_info(view.graph, VertexIndex(parent), label_width),
            offsets: offsets.into_iter().collect(),
        })
        .collect()
}

pub(crate) fn vertex(
    view: &View<'_>,
    vertex: &VertexRef,
    limits: &Limits,
) -> Result<VertexInfo, ApiError> {
    let index = text::resolve_vertex(view.graph, vertex)?;
    let label_width = label_width(limits);
    let data = view.graph.vertex(index).expect("resolved vertex");
    let composition = match data.node() {
        NodeRef::Atom(_) => Vec::new(),
        NodeRef::Composite(composition) => (0..composition.len())
            .filter_map(|k| composition.get(k))
            .map(|occurrence| OccurrenceInfo {
                offset: occurrence.offset.into(),
                child: text::token_info(view.graph, occurrence.child.index, label_width),
            })
            .collect(),
    };
    Ok(VertexInfo {
        token: text::token_info(view.graph, index, label_width),
        is_atom: composition.is_empty(),
        composition,
        parents: parents(view, index, label_width),
        inputs: view
            .meta
            .inputs
            .iter()
            .filter(|input| input.token == index.0)
            .map(|input| input.id)
            .collect(),
    })
}

pub(crate) fn validate(view: &View<'_>) -> ValidationReport {
    let errors: Vec<String> = view
        .graph
        .validate()
        .err()
        .map(|err| format!("{err:?}"))
        .into_iter()
        .collect();
    let inputs: BTreeSet<usize> = view.meta.inputs.iter().map(|input| input.token).collect();
    let retention_violations: Vec<_> = view
        .graph
        .vertices()
        .filter(|vertex| matches!(vertex.node(), NodeRef::Composite(_)))
        .filter(|vertex| !inputs.contains(&vertex.vertex_index().0))
        .filter(|vertex| vertex.parent_count() < 2)
        .map(|vertex| {
            text::token_info(view.graph, vertex.vertex_index(), text::DEFAULT_LABEL_WIDTH)
        })
        .collect();
    ValidationReport {
        valid: errors.is_empty() && retention_violations.is_empty(),
        errors,
        retention_violations,
    }
}
