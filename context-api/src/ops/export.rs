//! Export and import (plan 01 §1.7).

use std::path::Path;

use context_trace::positioned::{
    GraphRead,
    PositionedGraph,
    interchange::PositionedGraphSnapshot,
};

use super::{
    View,
    render::{
        check_gate,
        subgraph,
    },
    text,
};
use crate::{
    base64::{
        base64_decode,
        base64_encode,
    },
    error::ApiError,
    store::file::{
        self,
        GraphMeta,
    },
    types::{
        ExportFormat,
        ExportOutput,
        Gate,
        ImportData,
        InputRecord,
        VertexRef,
    },
};

pub(crate) fn export(
    view: &View<'_>,
    format: ExportFormat,
    gate: &Gate,
    render_cap: Option<usize>,
) -> Result<ExportOutput, ApiError> {
    match format {
        ExportFormat::InterchangeV3 => {
            let graph = view.graph;
            let vertices = graph.vertices().count();
            // Content keys spell out every vertex: Θ(Σ widths).
            let label_chars = subgraph(graph, None, None).label_chars(graph);
            check_gate(vertices, label_chars, gate, render_cap)?;
            // Not validated: a graph without inputs has an empty corpus.
            let corpus = view
                .meta
                .inputs
                .iter()
                .map(|input| text::text(graph, context_trace::VertexIndex(input.token)))
                .collect();
            let snapshot = PositionedGraphSnapshot::new(corpus, graph.to_interchange());
            let data = serde_json::to_string(&snapshot)
                .map_err(|e| ApiError::Internal(format!("encode interchange: {e}")))?;
            Ok(ExportOutput {
                format,
                bytes: data.len() as u64,
                data,
            })
        },
        ExportFormat::Native => {
            let bytes = file::encode(view.meta, view.graph)?;
            Ok(ExportOutput {
                format,
                bytes: bytes.len() as u64,
                data: base64_encode(&bytes),
            })
        },
    }
}

/// Decode import data into a validated graph and its metadata (input log;
/// `name` is the target graph name).
pub(crate) fn import(
    name: &str,
    data: ImportData,
) -> Result<(GraphMeta, PositionedGraph), ApiError> {
    let invalid = |what: &str, e: String| ApiError::InvalidInput(format!("{what}: {e}"));
    match data {
        ImportData::InterchangeV3 { document } => {
            let snapshot: PositionedGraphSnapshot = serde_json::from_str(&document)
                .map_err(|e| invalid("invalid interchange document", e.to_string()))?;
            snapshot
                .validate()
                .map_err(|e| invalid("invalid interchange document", e))?;
            let graph = PositionedGraph::from_interchange(&snapshot.graph)
                .map_err(|e| invalid("invalid interchange graph", e))?;
            let mut meta = GraphMeta::new(name);
            let read_at = file::now();
            for (i, corpus_text) in snapshot.corpus.iter().enumerate() {
                let token = text::resolve_vertex(&graph, &VertexRef::Text(corpus_text.clone()))
                    .map_err(|e| invalid(&format!("corpus text {i}"), e.to_string()))?;
                meta.inputs.push(InputRecord {
                    id: i as u32,
                    label: format!("corpus[{i}]"),
                    token: token.0,
                    width: corpus_text.chars().count(),
                    read_at: read_at.clone(),
                });
            }
            Ok((meta, graph))
        },
        ImportData::Native { base64 } => {
            let bytes = base64_decode(&base64).map_err(|e| invalid("invalid base64", e))?;
            let (stored, graph) = file::decode(Path::new("<import>"), &bytes)?;
            let mut meta = GraphMeta::new(name);
            meta.inputs = stored.inputs;
            Ok((meta, graph))
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::render::tests::graph;

    fn meta(
        graph: &PositionedGraph,
        roots: &[context_trace::VertexIndex],
    ) -> GraphMeta {
        let mut meta = GraphMeta::new("g");
        for (i, &root) in roots.iter().enumerate() {
            meta.inputs.push(InputRecord {
                id: i as u32,
                label: format!("in{i}"),
                token: root.0,
                width: text::width(graph, root),
                read_at: file::now(),
            });
        }
        meta
    }

    fn texts(
        graph: &PositionedGraph,
        meta: &GraphMeta,
    ) -> Vec<String> {
        meta.inputs
            .iter()
            .map(|i| text::text(graph, context_trace::VertexIndex(i.token)))
            .collect()
    }

    #[test]
    fn round_trips() {
        let (graph, roots) = graph(&["abcab", "bca", "a", "abcab"]);
        let meta = meta(&graph, &roots);
        let view = View { name: "g", graph: &graph, meta: &meta };
        let force = Gate { force: true, ..Gate::default() };

        let v3 = export(&view, ExportFormat::InterchangeV3, &force, None).unwrap();
        assert_eq!(v3.bytes, v3.data.len() as u64);
        let (imported_meta, imported) =
            import("h", ImportData::InterchangeV3 { document: v3.data }).unwrap();
        assert_eq!(imported.to_interchange(), graph.to_interchange());
        assert_eq!(imported_meta.name, "h");
        assert_eq!(imported_meta.inputs[1].label, "corpus[1]");
        assert_eq!(texts(&imported, &imported_meta), ["abcab", "bca", "a", "abcab"]);

        let native = export(&view, ExportFormat::Native, &Gate::default(), Some(0)).unwrap();
        let (native_meta, native_graph) =
            import("n", ImportData::Native { base64: native.data }).unwrap();
        assert_eq!(native_graph.to_interchange(), graph.to_interchange());
        assert_eq!(native_meta.name, "n");
        assert_eq!(native_meta.inputs, meta.inputs);
    }

    #[test]
    fn interchange_export_is_gated_and_import_validates() {
        let (graph, roots) = graph(&["abcab"]);
        let meta = meta(&graph, &roots);
        let view = View { name: "g", graph: &graph, meta: &meta };
        let tiny = Gate { max_vertices: Some(1), ..Gate::default() };
        assert!(matches!(
            export(&view, ExportFormat::InterchangeV3, &tiny, None),
            Err(ApiError::TooLarge { .. })
        ));
        let invalid = |data| matches!(import("x", data), Err(ApiError::InvalidInput(_)));
        assert!(invalid(ImportData::InterchangeV3 { document: "{".into() }));
        assert!(invalid(ImportData::InterchangeV3 {
            document: r#"{"format_version":3,"corpus":["ab"],"vertices":{"a":[],"b":[]}}"#.into()
        }));
        assert!(invalid(ImportData::Native { base64: "!!".into() }));
    }
}
