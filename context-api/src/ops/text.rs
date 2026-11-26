//! Reading text out of the graph without expanding whole vertices.

use std::ops::Range;

use context_search::Find;
use context_trace::{
    HasVertexIndex,
    Wide,
    AtomPosition,
    VertexIndex,
    positioned::{
        CompositionRead,
        GraphRead,
        NodeRef,
        PositionedGraph,
        VertexRead,
    },
};

use crate::{
    error::ApiError,
    types::{
        Limits,
        Snippet,
        TokenInfo,
        VertexRef,
    },
};

pub(crate) const DEFAULT_LABEL_WIDTH: usize = 80;
pub(crate) const DEFAULT_CONTEXT_WIDTH: usize = 40;
pub(crate) const DEFAULT_LIMIT: usize = 50;
pub(crate) const DEFAULT_MAX_VISITED: usize = 100_000;

pub(crate) fn label_width(limits: &Limits) -> usize {
    limits.label_width.unwrap_or(DEFAULT_LABEL_WIDTH)
}

pub(crate) fn width(
    graph: &PositionedGraph,
    index: VertexIndex,
) -> usize {
    graph.vertex(index).expect("vertex of this graph").width().0
}

/// Append the chars `range` (atom offsets) of vertex `index` to `out`.
///
/// Descends through the covering occurrence of every position, so the
/// cost is proportional to the range and the nesting depth, not to the
/// vertex width.
pub(crate) fn read_range(
    graph: &PositionedGraph,
    index: VertexIndex,
    range: Range<usize>,
    out: &mut String,
) {
    // Explicit stack: inputs can nest deeply.
    let mut stack = vec![(index, range)];
    while let Some((index, range)) = stack.pop() {
        if range.is_empty() {
            continue;
        }
        let vertex = graph.vertex(index).expect("vertex of this graph");
        match vertex.node() {
            NodeRef::Atom(atom) => out.push(*atom),
            NodeRef::Composite(composition) => {
                let mut segments = Vec::new();
                let mut pos = range.start;
                while pos < range.end {
                    let occurrence = composition
                        .covering(AtomPosition::from(pos))
                        .expect("compositions cover every position");
                    let offset: usize = occurrence.offset.into();
                    let stop = range.end.min(offset + occurrence.child.width.0);
                    segments.push((occurrence.child.index, pos - offset..stop - offset));
                    pos = stop;
                }
                stack.extend(segments.into_iter().rev());
            },
        }
    }
}

pub(crate) fn text_range(
    graph: &PositionedGraph,
    index: VertexIndex,
    range: Range<usize>,
) -> String {
    let mut out = String::with_capacity(range.len());
    read_range(graph, index, range, &mut out);
    out
}

/// The whole text of a vertex.
pub(crate) fn text(
    graph: &PositionedGraph,
    index: VertexIndex,
) -> String {
    text_range(graph, index, 0..width(graph, index))
}

pub(crate) fn token_info(
    graph: &PositionedGraph,
    index: VertexIndex,
    label_width: usize,
) -> TokenInfo {
    let width = width(graph, index);
    let shown = width.min(label_width);
    TokenInfo {
        index: index.0,
        width,
        label: text_range(graph, index, 0..shown),
        label_truncated: shown < width,
    }
}

/// `context_width` chars around `[offset, offset + len)` inside `index`.
pub(crate) fn snippet(
    graph: &PositionedGraph,
    index: VertexIndex,
    offset: usize,
    len: usize,
    context_width: usize,
) -> Snippet {
    let width = width(graph, index);
    let left_start = offset.saturating_sub(context_width);
    let right_end = width.min(offset + len + context_width);
    Snippet {
        left: text_range(graph, index, left_start..offset),
        matched: text_range(graph, index, offset..offset + len),
        right: text_range(graph, index, offset + len..right_end),
        left_truncated: left_start > 0,
        right_truncated: right_end < width,
    }
}

/// Resolve a vertex reference; text must be a vertex itself.
pub(crate) fn resolve_vertex(
    graph: &PositionedGraph,
    vertex: &VertexRef,
) -> Result<VertexIndex, ApiError> {
    match vertex {
        VertexRef::Index(index) => {
            let index = VertexIndex(*index);
            graph
                .vertex(index)
                .map(|_| index)
                .ok_or_else(|| ApiError::VertexNotFound {
                    vertex: format!("#{}", index.0),
                })
        },
        VertexRef::Text(text) => {
            let not_a_vertex = |host: Option<String>| ApiError::NotAVertex {
                text: text.clone(),
                host,
            };
            let response = graph
                .find_ancestor(text.chars())
                .map_err(|_| not_a_vertex(None))?;
            let root = response.root_token();
            if response.query_exhausted() && response.is_entire_root() {
                Ok(root.index)
            } else if response.query_exhausted() {
                let host = token_info(graph, root.index, DEFAULT_LABEL_WIDTH);
                Err(not_a_vertex(Some(format!("#{} \"{}\"", host.index, host.label))))
            } else {
                Err(not_a_vertex(None))
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use context_trace::positioned::PositionedGraphRef;

    use super::*;

    fn graph(inputs: &[&str]) -> (PositionedGraph, Vec<VertexIndex>) {
        let graph = PositionedGraphRef::new(PositionedGraph::default());
        let roots = inputs
            .iter()
            .map(|input| context_read::read(&graph, input.chars()).unwrap().index)
            .collect();
        let graph = graph.read().clone();
        (graph, roots)
    }

    #[test]
    fn ranges_equal_string_slices() {
        let inputs = ["abcabca", "bcabcab", "aaaaaa", "the cat sat on the mat"];
        let (graph, roots) = graph(&inputs);
        for (input, root) in inputs.iter().zip(roots) {
            let chars: Vec<char> = input.chars().collect();
            assert_eq!(text(&graph, root), *input);
            for start in 0..chars.len() {
                for end in start..=chars.len() {
                    let expected: String = chars[start..end].iter().collect();
                    assert_eq!(text_range(&graph, root, start..end), expected);
                }
            }
        }
    }

    #[test]
    fn labels_and_snippets() {
        let (graph, roots) = graph(&["hello world"]);
        let info = token_info(&graph, roots[0], 5);
        assert_eq!((info.label.as_str(), info.label_truncated, info.width), ("hello", true, 11));
        let snippet = snippet(&graph, roots[0], 6, 3, 2);
        assert_eq!(
            (snippet.left.as_str(), snippet.matched.as_str(), snippet.right.as_str()),
            ("o ", "wor", "ld")
        );
        assert!(snippet.left_truncated && !snippet.right_truncated);
    }

    #[test]
    fn resolve_text_vertices() {
        let (graph, roots) = graph(&["abcabc"]);
        assert_eq!(resolve_vertex(&graph, &VertexRef::Text("abcabc".into())).unwrap(), roots[0]);
        let abc = resolve_vertex(&graph, &VertexRef::Text("abc".into())).unwrap();
        assert_eq!(text(&graph, abc), "abc");
        assert!(matches!(
            resolve_vertex(&graph, &VertexRef::Text("bca".into())),
            Err(ApiError::NotAVertex { host: Some(_), .. })
        ));
        assert!(matches!(
            resolve_vertex(&graph, &VertexRef::Text("xyz".into())),
            Err(ApiError::NotAVertex { host: None, .. })
        ));
        assert!(matches!(
            resolve_vertex(&graph, &VertexRef::Index(999)),
            Err(ApiError::VertexNotFound { .. })
        ));
    }
}
