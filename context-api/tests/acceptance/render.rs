//! Render formats and the size gate (plan 1.6, 1.8 test 7).

use std::collections::BTreeMap;

use context_api::{
    types::*,
    ErrorKind,
};

use crate::common::*;

/// Every string over `{a, b, c, d}` of length 3: 64 inputs, 16 pairs and 4
/// atoms, so more than the default limit of 64 vertices.
fn large_corpus() -> Vec<String> {
    crate::corpora::strings(&['a', 'b', 'c', 'd'], 3)
        .into_iter()
        .filter(|s| s.len() == 3)
        .collect()
}

fn large_env(configure: impl FnOnce(&mut context_api::SessionConfig)) -> Env {
    let env = Env::with(configure);
    create(&env.session, "big", &large_corpus());
    let vertices = info(&env.session, "big").vertex_count;
    assert!(vertices > 64, "precondition: {vertices} vertices");
    env
}

#[test]
fn render_above_the_default_limit_needs_force() {
    let env = large_env(|_| {});
    let s = &env.session;
    let vertex_count = info(s, "big").vertex_count;
    for format in [
        RenderFormat::Ascii,
        RenderFormat::AsciiDag,
        RenderFormat::Dot,
        RenderFormat::Mermaid,
        RenderFormat::Json,
    ] {
        let error = render(s, "big", format, None, Gate::default()).unwrap_err();
        assert_eq!(error.kind, ErrorKind::TooLarge, "{format:?}: {error:?}");
    }

    let dot = render(s, "big", RenderFormat::Dot, None, force()).unwrap();
    assert!(dot.text.as_deref().unwrap_or("").starts_with("digraph"), "{dot:?}");
    assert_eq!(dot.vertices, vertex_count);

    let mermaid = render(s, "big", RenderFormat::Mermaid, None, force()).unwrap();
    assert!(mermaid.text.as_deref().unwrap_or("").starts_with("graph TD"), "{mermaid:?}");

    let ascii = render(s, "big", RenderFormat::Ascii, None, force()).unwrap();
    assert!(!ascii.text.as_deref().unwrap_or("").trim().is_empty());

    // An explicit limit replaces the default.
    let gate = Gate {
        max_vertices: Some(1000),
        max_label_chars: Some(1_000_000),
        force: false,
    };
    render(s, "big", RenderFormat::Dot, None, gate).unwrap();
}

#[test]
fn export_above_the_default_limit_needs_force() {
    let env = large_env(|_| {});
    let error = exec_err(
        &env.session,
        context_api::Command::Export {
            graph: "big".into(),
            format: ExportFormat::InterchangeV3,
            gate: Gate::default(),
        },
    );
    assert_eq!(error.kind, ErrorKind::TooLarge);
    export(&env.session, "big");
}

#[test]
fn a_small_root_renders_without_force() {
    let env = large_env(|_| {});
    let root = Some(VertexRef::Text("abc".into()));
    let dot = render(&env.session, "big", RenderFormat::Dot, root.clone(), Gate::default()).unwrap();
    assert!(dot.vertices <= 64 && dot.vertices >= 4, "{dot:?}");
    assert!(dot.text.as_deref().unwrap_or("").starts_with("digraph"));

    let json = render(&env.session, "big", RenderFormat::Json, root, Gate::default()).unwrap();
    let graph = json.graph.expect("json graph");
    let mut labels: Vec<&str> = graph.vertices.iter().map(|v| v.label.as_str()).collect();
    labels.sort();
    for expected in ["a", "abc", "b", "c"] {
        assert!(labels.contains(&expected), "{labels:?}");
    }
    assert!(labels.iter().all(|l| "abc".contains(l)), "only descendants of abc: {labels:?}");
}

#[test]
fn json_render_lists_edges_between_listed_vertices() {
    let env = large_env(|_| {});
    let output = render(&env.session, "big", RenderFormat::Json, None, force()).unwrap();
    assert_eq!(output.text, None);
    let graph = output.graph.expect("json graph");
    assert_eq!(output.vertices, graph.vertices.len());
    assert_eq!(output.edges, graph.edges.len());

    let vertices: BTreeMap<usize, &TokenInfo> = graph.vertices.iter().map(|v| (v.index, v)).collect();
    assert_eq!(vertices.len(), graph.vertices.len(), "duplicate vertices");
    for edge in &graph.edges {
        let (Some(parent), Some(child)) = (vertices.get(&edge.parent), vertices.get(&edge.child))
        else {
            panic!("edge {edge:?} references an unlisted vertex");
        };
        assert!(edge.offset + child.width <= parent.width, "{edge:?}: {parent:?} / {child:?}");
        let text: Vec<char> = parent.label.chars().collect();
        assert_eq!(char_slice(&text, edge.offset, child.width), child.label, "{edge:?}");
    }
    for vertex in &graph.vertices {
        let children = graph.edges.iter().filter(|e| e.parent == vertex.index).count();
        match vertex.width {
            1 => assert_eq!(children, 0, "atom {vertex:?} has edges"),
            _ => assert!(children >= 2, "composite {vertex:?} has {children} edges"),
        }
    }
    // The whole graph: every vertex of the interchange export.
    let exported = export(&env.session, "big").graph.vertices;
    let mut labels: Vec<String> = graph.vertices.iter().map(|v| v.label.clone()).collect();
    labels.sort();
    assert_eq!(labels, exported.into_keys().collect::<Vec<_>>());
}

#[test]
fn small_graphs_render_without_force() {
    let env = Env::new();
    create(&env.session, "small", &strings(&["abcab", "bca"]));
    for format in [
        RenderFormat::Ascii,
        RenderFormat::AsciiDag,
        RenderFormat::Dot,
        RenderFormat::Mermaid,
    ] {
        let output = render(&env.session, "small", format, None, Gate::default()).unwrap();
        assert!(!output.text.as_deref().unwrap_or("").is_empty(), "{format:?}");
    }
}

#[test]
fn force_does_not_lift_the_session_render_cap() {
    let env = large_env(|config| config.render_cap = Some(10));
    let error = render(&env.session, "big", RenderFormat::Dot, None, force()).unwrap_err();
    assert_eq!(error.kind, ErrorKind::TooLarge);
    let root = Some(VertexRef::Text("abc".into()));
    render(&env.session, "big", RenderFormat::Dot, root, force()).unwrap();
}
