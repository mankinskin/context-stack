//! Interchange import by table construction (context-trace) equals the
//! import through one `GraphEdit` it replaced (crate ownership plan P0.2).

use context_trace::{
    HasVertexIndex,
    graph::vertex::VertexIndex,
    positioned::{
        GraphRead,
        VertexNode,
        interchange::PositionedGraphSnapshot,
    },
};

/// The old interchange import through one `GraphEdit` (replaced by table
/// construction, crate ownership plan P0.2); kept as the reference.
fn build_with_edit(
    data: &context_trace::positioned::interchange::PositionedGraphData
) -> (
    context_trace::positioned::PositionedGraph,
    std::collections::BTreeMap<String, context_trace::Token>,
) {
    use context_trace::positioned::{
        Occurrence,
        PositionedGraph,
    };
    let (atoms, mut composites): (Vec<_>, Vec<_>) =
        data.vertices.iter().partition(|(_, cover)| cover.is_empty());
    composites.sort_by_key(|(content, _)| (content.chars().count(), *content));
    let mut graph = PositionedGraph::default();
    let mut tokens = std::collections::BTreeMap::new();
    let mut edit = crate::edit::EditExt::edit(&mut graph);
    for (content, _) in atoms {
        tokens.insert(content.clone(), edit.atom(content.chars().next().unwrap()));
    }
    for (content, cover) in composites {
        let occurrences = cover
            .iter()
            .map(|(offset, child)| Occurrence::new(*offset, tokens[child]))
            .collect();
        tokens.insert(content.clone(), edit.create(occurrences).unwrap());
    }
    edit.finish();
    (graph, tokens)
}

/// Table construction reproduces the `GraphEdit` import exactly: slots,
/// nodes, widths, parent links, labels, atom map, generation and names, on
/// every graph of the v3, insert and read lanes.
#[test]
fn table_import_equals_edit_import() {
    use context_trace::positioned::{
        PositionedGraph,
        fixtures::{
            fixture_snapshots,
            insert_dir,
            manifest,
            read_dir,
        },
    };
    let lane = |dir: std::path::PathBuf| {
        manifest(&dir).into_iter().map(move |scenario| {
            let path = dir.join(format!("{}.json", scenario.name));
            let snapshot: PositionedGraphSnapshot =
                serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
            (scenario.name, snapshot)
        })
    };
    let graphs: Vec<_> = fixture_snapshots()
        .into_iter()
        .chain(lane(insert_dir()))
        .chain(lane(read_dir()))
        .collect();
    assert!(graphs.len() > 1000, "{} graphs", graphs.len());
    for (name, snapshot) in graphs {
        let (expected, expected_names) = build_with_edit(&snapshot.graph);
        let (actual, actual_names) =
            PositionedGraph::from_interchange_named(&snapshot.graph).unwrap();
        assert_eq!(actual.generation(), expected.generation(), "{name}: generation");
        // Vertex keys are random per vertex on both paths.
        let fields = |graph: &PositionedGraph| {
            (0..graph.vertex_count())
                .map(|slot| {
                    let v = graph.vertex_data(VertexIndex(slot)).unwrap();
                    #[cfg(debug_assertions)]
                    let label = v.label().to_string();
                    #[cfg(not(debug_assertions))]
                    let label = ();
                    format!("{:?}", (v.index(), v.width(), v.node(), v.parents(), label))
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(fields(&actual), fields(&expected), "{name}: vertices");
        let atoms = |graph: &PositionedGraph| {
            (0..graph.vertex_count())
                .filter_map(|slot| match graph.vertex_data(VertexIndex(slot)).unwrap().node() {
                    VertexNode::Atom(atom) => Some(*atom),
                    VertexNode::Composite(_) => None,
                })
                .map(|atom| (atom, graph.atom(&atom).map(|v| v.vertex_index())))
                .collect::<Vec<_>>()
        };
        assert_eq!(atoms(&actual), atoms(&expected), "{name}: atoms");
        assert_eq!(actual_names, expected_names, "{name}: names");
    }
}
