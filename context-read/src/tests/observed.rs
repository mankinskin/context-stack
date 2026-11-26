//! [`read_observed`] reports the steps of [`read_traced`], and read graphs
//! survive a serde round trip slot for slot.

use context_trace::{
    VertexIndex,
    positioned::{
        GraphRead,
        PositionedGraph,
        PositionedGraphRef,
        VertexNode,
    },
};
use pretty_assertions::assert_eq;

use crate::{
    read,
    read_observed,
    read_traced,
};

const INPUTS: [&str; 6] = ["abab", "aaaa", "abc", "bcab", "abcabca", "ababcabab"];

#[test]
fn observed_steps_are_the_traced_steps() {
    let traced = PositionedGraphRef::default();
    let observed = PositionedGraphRef::default();
    for input in INPUTS {
        let (root, expected) = read_traced(&traced, input.chars());
        let mut steps = Vec::new();
        let observed_root =
            read_observed(&observed, input.chars(), |step| steps.push(*step));
        assert_eq!(steps, expected, "{input}");
        assert_eq!(observed_root, root, "{input}");
    }
}

fn assert_identical(
    expected: &PositionedGraph,
    actual: &PositionedGraph,
) {
    assert_eq!(actual.generation(), expected.generation());
    assert_eq!(actual.vertex_count(), expected.vertex_count());
    for slot in 0..expected.vertex_count() {
        let (e, a) = (
            expected.vertex_data(VertexIndex(slot)).unwrap(),
            actual.vertex_data(VertexIndex(slot)).unwrap(),
        );
        assert_eq!(a.index(), e.index());
        assert_eq!(a.width(), e.width(), "{e}");
        assert_eq!(a.node(), e.node(), "{e}");
        assert_eq!(a.parents(), e.parents(), "{e}");
        #[cfg(debug_assertions)]
        assert_eq!(a.label(), e.label(), "{e}");
    }
    assert_eq!(actual.to_interchange(), expected.to_interchange());
}

#[test]
fn read_graphs_round_trip_slot_for_slot() {
    let graph = PositionedGraphRef::default();
    for input in INPUTS {
        read(&graph, input.chars()).unwrap();
        let graph = graph.read();
        let json = serde_json::to_string(&*graph).unwrap();
        let from_json: PositionedGraph = serde_json::from_str(&json).unwrap();
        assert_identical(&graph, &from_json);
        let bytes = bincode::serialize(&*graph).unwrap();
        let from_bincode: PositionedGraph = bincode::deserialize(&bytes).unwrap();
        assert_identical(&graph, &from_bincode);
    }
    // Some atom follows some composite (`abc` creates `c` after `ab`).
    let graph = graph.read();
    let is_atom =
        |slot: usize| matches!(graph.vertex_data(VertexIndex(slot)).unwrap().node(), VertexNode::Atom(_));
    let first_composite = (0..graph.vertex_count()).find(|&slot| !is_atom(slot)).unwrap();
    assert!((first_composite..graph.vertex_count()).any(is_atom));
}

/// `abab` first, so the atom `c` of `abc` gets a higher index than the
/// composites `ab` and `abab`.
#[test]
fn late_atoms_round_trip() {
    let graph = PositionedGraphRef::default();
    let abab = read(&graph, "abab".chars()).unwrap();
    read(&graph, "abc".chars()).unwrap();
    let graph = graph.read();
    let c = (0..graph.vertex_count())
        .find(|&slot| graph.vertex_data(VertexIndex(slot)).unwrap().node() == &VertexNode::Atom('c'))
        .unwrap();
    let ab = graph.vertex_data(abab.index).unwrap().composition().unwrap().occurrences()[0]
        .child;
    assert_eq!(ab.width.0, 2);
    assert!(c > ab.index.0 && c > abab.index.0, "c {} must follow ab and abab", c);
    let json = serde_json::to_string(&*graph).unwrap();
    assert_identical(&graph, &serde_json::from_str(&json).unwrap());
}
