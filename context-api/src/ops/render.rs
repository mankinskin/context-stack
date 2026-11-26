//! Render (ascii, dot, mermaid, json) with the size gate (plan 01 §1.6).

use std::{
    collections::{
        BTreeMap,
        BTreeSet,
        VecDeque,
    },
    fmt::Write,
};

use context_trace::{
    HasVertexIndex,
    VertexIndex,
    positioned::{
        CompositionRead,
        GraphRead,
        NodeRef,
        PositionedGraph,
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
    ascii_graph::{
        AsciiOwnedRule,
        AsciiRenderMode,
        render_layered_ascii_owned_with_mode,
    },
    error::ApiError,
    types::{
        Gate,
        Limits,
        RenderEdge,
        RenderFormat,
        RenderGraph,
        RenderOutput,
        VertexRef,
    },
};

pub(crate) const DEFAULT_MAX_VERTICES: usize = 64;
pub(crate) const DEFAULT_MAX_LABEL_CHARS: usize = 4096;

/// Vertices and composition edges selected for rendering.
pub(crate) struct Subgraph {
    pub vertices: BTreeSet<usize>,
    /// Sorted by `(parent, offset, child)`.
    pub edges: Vec<RenderEdge>,
}

impl Subgraph {
    /// Σ widths of the vertices: the label size, known without expansion.
    pub fn label_chars(
        &self,
        graph: &PositionedGraph,
    ) -> usize {
        self.vertices
            .iter()
            .map(|&index| text::width(graph, VertexIndex(index)))
            .sum()
    }
}

fn children(
    graph: &PositionedGraph,
    index: usize,
) -> Vec<(usize, usize)> {
    match graph.vertex(VertexIndex(index)).expect("vertex of this graph").node() {
        NodeRef::Atom(_) => Vec::new(),
        NodeRef::Composite(composition) => (0..composition.len())
            .filter_map(|k| composition.get(k))
            .map(|occurrence| (occurrence.offset.into(), occurrence.child.index.0))
            .collect(),
    }
}

/// The whole graph, or `root` and its descendants down to `depth` child
/// steps (`None`: unlimited).
pub(crate) fn subgraph(
    graph: &PositionedGraph,
    root: Option<VertexIndex>,
    depth: Option<usize>,
) -> Subgraph {
    let vertices: BTreeSet<usize> = match root {
        None => graph.vertices().map(|v| v.vertex_index().0).collect(),
        Some(root) => {
            let mut seen = BTreeSet::from([root.0]);
            let mut queue = VecDeque::from([(root.0, 0usize)]);
            while let Some((index, level)) = queue.pop_front() {
                if depth.is_some_and(|depth| level >= depth) {
                    continue;
                }
                for (_, child) in children(graph, index) {
                    if seen.insert(child) {
                        queue.push_back((child, level + 1));
                    }
                }
            }
            seen
        },
    };
    let mut edges: Vec<RenderEdge> = vertices
        .iter()
        .flat_map(|&parent| {
            children(graph, parent)
                .into_iter()
                .map(move |(offset, child)| RenderEdge { parent, child, offset })
        })
        .filter(|edge| vertices.contains(&edge.child))
        .collect();
    edges.sort_by_key(|e| (e.parent, e.offset, e.child));
    Subgraph { vertices, edges }
}

/// The size gate (D7). `force` lifts the limits except the session's hard
/// `render_cap` on vertices.
pub(crate) fn check_gate(
    vertices: usize,
    label_chars: usize,
    gate: &Gate,
    render_cap: Option<usize>,
) -> Result<(), ApiError> {
    let (max_vertices, max_label_chars) = if gate.force {
        (usize::MAX, usize::MAX)
    } else {
        (
            gate.max_vertices.unwrap_or(DEFAULT_MAX_VERTICES),
            gate.max_label_chars.unwrap_or(DEFAULT_MAX_LABEL_CHARS),
        )
    };
    let max_vertices = render_cap.map_or(max_vertices, |cap| cap.min(max_vertices));
    if vertices > max_vertices || label_chars > max_label_chars {
        return Err(ApiError::TooLarge {
            vertices,
            label_chars,
            max_vertices,
            max_label_chars,
        });
    }
    Ok(())
}

/// [`text::resolve_vertex`], plus single-char texts (atoms), which the
/// ancestor search does not resolve.
pub(crate) fn resolve_vertex(
    graph: &PositionedGraph,
    vertex: &VertexRef,
) -> Result<VertexIndex, ApiError> {
    if let VertexRef::Text(text) = vertex {
        let mut chars = text.chars();
        if let (Some(c), None) = (chars.next(), chars.next()) {
            return graph.atom(&c).map(|atom| atom.vertex_index()).ok_or_else(|| {
                ApiError::NotAVertex {
                    text: text.clone(),
                    host: None,
                }
            });
        }
    }
    text::resolve_vertex(graph, vertex)
}

/// `render_cap`: the session's hard vertex cap that `gate.force` cannot lift.
pub(crate) fn render(
    view: &View<'_>,
    format: RenderFormat,
    root: Option<&VertexRef>,
    depth: Option<usize>,
    gate: &Gate,
    limits: &Limits,
    render_cap: Option<usize>,
) -> Result<RenderOutput, ApiError> {
    let graph = view.graph;
    let root = root.map(|root| resolve_vertex(graph, root)).transpose()?;
    let sub = subgraph(graph, root, depth);
    check_gate(sub.vertices.len(), sub.label_chars(graph), gate, render_cap)?;

    let width = label_width(limits);
    let (text, json) = match format {
        RenderFormat::Json => (None, Some(json_graph(graph, &sub, width))),
        RenderFormat::Dot => (Some(dot(view.name, graph, &sub, width)), None),
        RenderFormat::Mermaid => (Some(mermaid(graph, &sub, width)), None),
        RenderFormat::Ascii => (Some(ascii(graph, &sub, width, AsciiRenderMode::Grammar)?), None),
        RenderFormat::AsciiDag => (Some(ascii(graph, &sub, width, AsciiRenderMode::Dag)?), None),
    };
    Ok(RenderOutput {
        format,
        text,
        graph: json,
        vertices: sub.vertices.len(),
        edges: sub.edges.len(),
    })
}

fn json_graph(
    graph: &PositionedGraph,
    sub: &Subgraph,
    width: usize,
) -> RenderGraph {
    RenderGraph {
        vertices: sub
            .vertices
            .iter()
            .map(|&index| text::token_info(graph, VertexIndex(index), width))
            .collect(),
        edges: sub.edges.clone(),
    }
}

/// Labels truncated to `width`, with `…` appended when truncated.
fn labels(
    graph: &PositionedGraph,
    sub: &Subgraph,
    width: usize,
) -> BTreeMap<usize, String> {
    sub.vertices
        .iter()
        .map(|&index| {
            let info = text::token_info(graph, VertexIndex(index), width);
            let mut label = info.label;
            if info.label_truncated {
                label.push('…');
            }
            (index, label)
        })
        .collect()
}

fn is_atom(
    graph: &PositionedGraph,
    index: usize,
) -> bool {
    matches!(
        graph.vertex(VertexIndex(index)).expect("vertex of this graph").node(),
        NodeRef::Atom(_)
    )
}

pub(crate) fn escape_dot(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out
}

fn dot(
    name: &str,
    graph: &PositionedGraph,
    sub: &Subgraph,
    width: usize,
) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "digraph \"{}\" {{", escape_dot(name));
    out.push_str("  rankdir=TB;\n  node [fontname=\"monospace\"];\n");
    for (index, label) in labels(graph, sub, width) {
        let shape = if is_atom(graph, index) { ", shape=box" } else { "" };
        let _ = writeln!(out, "  v{index} [label=\"{}\"{shape}];", escape_dot(&label));
    }
    for edge in &sub.edges {
        let _ = writeln!(out, "  v{} -> v{} [label=\"@{}\"];", edge.parent, edge.child, edge.offset);
    }
    out.push_str("}\n");
    out
}

/// Escape a label for a quoted mermaid node text (entity codes).
pub(crate) fn escape_mermaid(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '"' => out.push_str("#quot;"),
            '#' => out.push_str("#35;"),
            '&' => out.push_str("#amp;"),
            '<' => out.push_str("#lt;"),
            '>' => out.push_str("#gt;"),
            '[' => out.push_str("#91;"),
            ']' => out.push_str("#93;"),
            '`' => out.push_str("#96;"),
            '\n' => out.push_str("<br/>"),
            '\r' => {},
            c => out.push(c),
        }
    }
    out
}

fn mermaid(
    graph: &PositionedGraph,
    sub: &Subgraph,
    width: usize,
) -> String {
    let mut out = String::from("graph TD\n");
    for (index, label) in labels(graph, sub, width) {
        let _ = writeln!(out, "  v{index}[\"{}\"]", escape_mermaid(&label));
    }
    for edge in &sub.edges {
        let _ = writeln!(out, "  v{} -->|{}| v{}", edge.parent, edge.offset, edge.child);
    }
    out
}

fn ascii(
    graph: &PositionedGraph,
    sub: &Subgraph,
    width: usize,
    mode: AsciiRenderMode,
) -> Result<String, ApiError> {
    if sub.vertices.is_empty() {
        return Ok("(empty graph)".into());
    }
    let mut labels = labels(graph, sub, width);
    for label in labels.values_mut() {
        *label = label
            .replace('\\', "\\\\")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t");
    }
    // The renderer identifies vertices by label: disambiguate collisions
    // (truncated labels) with the vertex index.
    let mut counts = BTreeMap::<String, usize>::new();
    for label in labels.values() {
        *counts.entry(label.clone()).or_default() += 1;
    }
    for (index, label) in labels.iter_mut() {
        if counts[label.as_str()] > 1 {
            *label = format!("{label}#{index}");
        }
    }

    let mut by_parent = BTreeMap::<usize, Vec<&RenderEdge>>::new();
    for edge in &sub.edges {
        by_parent.entry(edge.parent).or_default().push(edge);
    }
    let rules: Vec<AsciiOwnedRule> = by_parent
        .iter()
        .map(|(&parent, edges)| {
            let covered: usize =
                edges.iter().map(|e| text::width(graph, VertexIndex(e.child))).sum();
            let overlapping = covered > text::width(graph, VertexIndex(parent));
            let pattern = edges
                .iter()
                .map(|e| {
                    let label = &labels[&e.child];
                    if overlapping {
                        format!("{label}@{}", e.offset)
                    } else {
                        label.clone()
                    }
                })
                .collect();
            AsciiOwnedRule {
                parent: labels[&parent].clone(),
                patterns: vec![pattern],
            }
        })
        .collect();

    let connected: BTreeSet<usize> =
        sub.edges.iter().flat_map(|e| [e.parent, e.child]).collect();
    let isolated: Vec<&str> = labels
        .iter()
        .filter(|(index, _)| !connected.contains(index))
        .map(|(_, label)| label.as_str())
        .collect();

    let mut out = if rules.is_empty() {
        String::new()
    } else {
        render_layered_ascii_owned_with_mode(&rules, mode)
            .map_err(|e| ApiError::Internal(format!("ascii render: {e}")))?
    };
    if !isolated.is_empty() {
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        let _ = writeln!(out, "Unconnected: {}", isolated.join(", "));
    }
    Ok(out)
}

#[cfg(test)]
pub(crate) mod tests {
    use context_trace::positioned::PositionedGraphRef;

    use super::*;
    use crate::store::file::GraphMeta;

    pub(crate) fn graph(inputs: &[&str]) -> (PositionedGraph, Vec<VertexIndex>) {
        let graph = PositionedGraphRef::new(PositionedGraph::default());
        let roots = inputs
            .iter()
            .map(|input| context_read::read(&graph, input.chars()).unwrap().index)
            .collect();
        let graph = graph.read().clone();
        (graph, roots)
    }

    fn gate(
        max_vertices: Option<usize>,
        max_label_chars: Option<usize>,
        force: bool,
    ) -> Gate {
        Gate {
            max_vertices,
            max_label_chars,
            force,
        }
    }

    #[test]
    fn gate_limits() {
        let ok = |v, l, g: Gate, cap| check_gate(v, l, &g, cap).is_ok();
        assert!(ok(64, 4096, Gate::default(), None));
        assert!(!ok(65, 1, Gate::default(), None));
        assert!(!ok(1, 4097, Gate::default(), None));
        assert!(ok(100, 1, gate(Some(100), None, false), None));
        assert!(!ok(11, 1, gate(Some(10), None, false), None));
        assert!(!ok(1, 11, gate(None, Some(10), false), None));
        assert!(ok(1_000_000, usize::MAX, gate(Some(1), Some(1), true), None));
        assert!(ok(10, 1_000_000, gate(None, None, true), Some(10)));
        assert!(!ok(10, 1, gate(Some(100), None, false), Some(9)));
        assert!(matches!(
            check_gate(11, 5, &gate(None, None, true), Some(10)),
            Err(ApiError::TooLarge { vertices: 11, label_chars: 5, max_vertices: 10, .. })
        ));
    }

    #[test]
    fn escaping() {
        assert_eq!(escape_dot("a\"b\\c\nd"), "a\\\"b\\\\c\\nd");
        assert_eq!(escape_mermaid("a\"[b]#<\n"), "a#quot;#91;b#93;#35;#lt;<br/>");
    }

    #[test]
    fn depth_limited_root_subgraph() {
        let (graph, roots) = graph(&["abcd", "abcdx", "abx"]);
        let root = roots[1];
        let all = subgraph(&graph, Some(root), None);
        let texts = |sub: &Subgraph| -> BTreeSet<String> {
            sub.vertices.iter().map(|&i| text::text(&graph, VertexIndex(i))).collect()
        };
        assert!(texts(&all).iter().all(|t| "abcdx".contains(t.as_str())));
        assert!(texts(&all).contains("a") && texts(&all).contains("abcdx"));
        let zero = subgraph(&graph, Some(root), Some(0));
        assert_eq!(texts(&zero), BTreeSet::from(["abcdx".to_string()]));
        assert!(zero.edges.is_empty());
        let one = subgraph(&graph, Some(root), Some(1));
        let direct: BTreeSet<usize> = children(&graph, root.0).into_iter().map(|(_, c)| c).collect();
        assert_eq!(one.vertices.len(), direct.len() + 1);
        assert!(one.edges.iter().all(|e| one.vertices.contains(&e.child)));
        assert!(one.vertices.len() < all.vertices.len());
    }

    #[test]
    fn formats() {
        let (graph, _) = graph(&["a\"b", "a\"bx"]);
        let meta = GraphMeta::new("g");
        let view = View { name: "g", graph: &graph, meta: &meta };
        let limits = Limits::default();
        let run = |format| render(&view, format, None, None, &Gate::default(), &limits, None).unwrap();
        let dot = run(RenderFormat::Dot).text.unwrap();
        assert!(dot.starts_with("digraph \"g\" {"), "{dot}");
        assert!(dot.contains("[label=\"a\\\"b\"]"), "{dot}");
        assert!(dot.contains("shape=box"), "{dot}");
        let mermaid = run(RenderFormat::Mermaid).text.unwrap();
        assert!(mermaid.starts_with("graph TD\n"));
        assert!(mermaid.contains("[\"a#quot;b\"]"), "{mermaid}");
        assert!(mermaid.contains("-->|0|"), "{mermaid}");
        let json = run(RenderFormat::Json);
        assert!(json.text.is_none());
        assert_eq!(json.graph.unwrap().vertices.len(), json.vertices);

        let truncated = Limits { label_width: Some(2), ..Limits::default() };
        let dot = render(&view, RenderFormat::Dot, None, None, &Gate::default(), &truncated, None)
            .unwrap()
            .text
            .unwrap();
        assert!(dot.contains("a\\\"…"), "{dot}");
        let ascii = render(&view, RenderFormat::Ascii, None, None, &Gate::default(), &truncated, None)
            .unwrap()
            .text
            .unwrap();
        assert!(ascii.contains('#'), "colliding truncated labels are disambiguated: {ascii}");

        let atom = render(&view, RenderFormat::Json, Some(&VertexRef::Text("x".into())), None, &Gate::default(), &limits, None);
        assert_eq!(atom.unwrap().graph.unwrap().vertices[0].label, "x");

        let (empty, _) = self::graph(&[]);
        let view = View { name: "e", graph: &empty, meta: &meta };
        let ascii = render(&view, RenderFormat::Ascii, None, None, &Gate::default(), &limits, None);
        assert_eq!(ascii.unwrap().text.unwrap(), "(empty graph)");
    }
}
