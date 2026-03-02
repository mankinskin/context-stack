//! Diagnostic tests that dump raw event sequences to stderr.
//!
//! All tests are `#[ignore]`d — they never run in CI.  Run them on
//! demand to inspect the event trace of a particular search:
//!
//! ```bash
//! cargo test -p context-search dump_ -- --ignored --nocapture
//! ```

#[cfg(test)]
use {
    crate::search::Find,
    context_trace::{
        *,
        graph::vertex::token::Token,
    },
    itertools::Itertools,
};

fn dump(label: &str, events: &[context_trace::graph::visualization::GraphOpEvent]) {
    eprintln!("\n=== {label} ({} events) ===", events.len());
    for e in events {
        eprintln!("  [{:>2}] transition: {:?}", e.step, e.transition);
        let pg = &e.path_graph;
        eprintln!("        path_graph: start_node={:?} start_path={:?} root={:?} end_path={:?} end_edges={:?}",
            pg.start_node, pg.start_path, pg.root, pg.end_path, pg.end_edges);
    }
}

#[test]
#[ignore = "diagnostic: run with --ignored --nocapture"]
fn dump_long_pattern() {
    let Env1 {
        graph, a, b, c, d, e, f, g, h, i,
        ababababcdefghi, ..
    } = &*Env1::get();
    let _tracing = init_test_tracing!(graph);
    graph.emit_graph_snapshot();

    let query: Vec<_> = [a, b, a, b, a, b, a, b, c, d, e, f, g, h, i]
        .into_iter()
        .cloned()
        .collect();
    let response = graph.find_ancestor(&query).unwrap();
    dump("long_pattern", &response.events);
}

#[test]
#[ignore = "diagnostic: run with --ignored --nocapture"]
fn dump_ancestor2() {
    use context_trace::*;
    let graph = Hypergraph::<BaseGraphKind>::default();
    insert_atoms!(graph, {a, b, x, y, z});
    insert_patterns!(graph,
        ab => [a, b],
        by => [b, y],
        yz => [y, z],
        xa => [x, a],
    );
    insert_patterns!(graph,
        xab => [[x, ab],[xa, b]],
    );
    insert_patterns!(graph,
        (xaby, _xaby_ids) => [[xa, by],[xab,y]],
        (xabyz, _xabyz_ids) => [[xaby, z],[xab,yz]],
    );
    let _tracing = init_test_tracing!(&graph);
    graph.emit_graph_snapshot();
    let graph = HypergraphRef::from(graph);
    let query = vec![by, z];
    let response = graph.find_ancestor(&query).unwrap();
    dump("ancestor2_byz", &response.events);
}

#[test]
#[ignore = "diagnostic: run with --ignored --nocapture"]
fn dump_ancestor3() {
    let graph = Hypergraph::<BaseGraphKind>::default();
    let (a, b, _w, x, y, z) = graph
        .insert_atoms([
            Atom::Element('a'),
            Atom::Element('b'),
            Atom::Element('w'),
            Atom::Element('x'),
            Atom::Element('y'),
            Atom::Element('z'),
        ])
        .into_iter()
        .next_tuple()
        .unwrap();
    let ab = graph.insert_pattern(vec![a, b]);
    let _by = graph.insert_pattern(vec![b, y]);
    let yz = graph.insert_pattern(vec![y, z]);
    let xa = graph.insert_pattern(vec![x, a]);
    let (xab, _xab_ids) = graph.insert_patterns_with_ids([
        Pattern::from(vec![x, ab]),
        Pattern::from(vec![xa, b]),
    ]);
    let (xaby, _xaby_ids) = graph.insert_patterns_with_ids([
        Pattern::from(vec![xab, y]),
        Pattern::from(vec![xa, _by]),
    ]);
    let _xabyz = graph.insert_patterns([vec![xaby, z], vec![xab, yz]]);
    let _tracing = init_test_tracing!(&graph);
    graph.emit_graph_snapshot();
    let gr = HypergraphRef::from(graph);
    let query = vec![ab, y];
    let response = gr.find_ancestor(&query).unwrap();
    dump("ancestor3_aby", &response.events);
}

#[test]
#[ignore = "diagnostic: run with --ignored --nocapture"]
fn dump_parent_b_c() {
    let Env1 { graph, b, c, .. } = &*Env1::get();
    let _tracing = init_test_tracing!(graph);
    graph.emit_graph_snapshot();
    let query = vec![Token::new(b, 1), Token::new(c, 1)];
    let response = graph.find_parent(&query).unwrap();
    dump("parent_b_c", &response.events);
}

#[test]
#[ignore = "diagnostic: run with --ignored --nocapture"]
fn dump_parent_ab_c() {
    let Env1 { graph, ab, c, .. } = &*Env1::get();
    let _tracing = init_test_tracing!(graph);
    graph.emit_graph_snapshot();
    let query = [Token::new(ab, 2), Token::new(c, 1)];
    let response = graph.find_parent(&query).unwrap();
    dump("parent_ab_c", &response.events);
}

#[test]
#[ignore = "diagnostic: run with --ignored --nocapture"]
fn dump_consecutive1() {
    let Env1 {
        graph, a, b, c, g, h, i, ..
    } = &*Env1::get();
    let _tracing = init_test_tracing!(graph);
    graph.emit_graph_snapshot();
    let query = vec![
        Token::new(g, 1), Token::new(h, 1), Token::new(i, 1),
        Token::new(a, 1), Token::new(b, 1), Token::new(c, 1),
    ];
    let query = PatternPrefixPath::from(Pattern::from(query));
    let fin1 = graph.find_ancestor(&query).unwrap();
    dump("consecutive1_search1", &fin1.events);

    let query2 = fin1.end.cursor().clone();
    let fin2 = graph.find_ancestor(&query2).unwrap();
    dump("consecutive1_search2", &fin2.events);
}
