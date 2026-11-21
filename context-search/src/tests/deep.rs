//! Deep regression pins on the `deep_*` fixtures: response paths with up
//! to 7 steps per side, checked step by step together with the traced
//! cache edges. Vertices are addressed through the fixture name map.

use context_trace::{
    positioned::SpanCoverage,
    AtomPosition,
    DirectedKey,
    DownPosition,
    Token,
    UpPosition,
};
use pretty_assertions::assert_eq;

use super::{
    search::{
        check,
        edges,
        names,
        start_steps,
    },
    util::{
        fixture,
        pos,
        span,
        step,
    },
};
use crate::{
    Find,
    Response,
};

type Edge = (DirectedKey, DirectedKey, AtomPosition);

fn up(
    t: Token,
    p: usize,
) -> DirectedKey {
    DirectedKey::up(t, UpPosition(pos(p)))
}

fn down(
    t: Token,
    p: usize,
) -> DirectedKey {
    DirectedKey::down(t, DownPosition(pos(p)))
}

/// Edges in a deterministic order (ties of [`edges`] may come in any order).
fn sorted(mut edges: Vec<Edge>) -> Vec<Edge> {
    edges.sort_by_cached_key(|edge| format!("{edge:?}"));
    edges
}

fn end_steps(response: &Response) -> Vec<context_trace::positioned::OccurrenceLocation> {
    response.path().end_path().unwrap().steps().to_vec()
}

/// `fg` straddles the two halves of `abcdefghijkl`: the start path climbs
/// the suffix chain `f < ef < … < abcdef`, the end path descends the prefix
/// chain `ghijkl > … > gh > g` (6 steps each).
#[test]
fn deep_chains_fg() {
    let f = fixture("deep_chains");
    let [f_, g, ef, def, cdef, bcdef, abcdef] =
        names(&f, ["f", "g", "ef", "def", "cdef", "bcdef", "abcdef"]);
    let [gh, ghi, ghij, ghijk, ghijkl, root] =
        names(&f, ["gh", "ghi", "ghij", "ghijk", "ghijkl", "abcdefghijkl"]);
    let response = check(&f, "fg");
    assert_eq!(response.root_token(), root);
    assert_eq!(response.span(), span(5, 7));
    assert_eq!(response.path().coverage(), SpanCoverage::Range);
    assert!(response.query_exhausted());
    assert_eq!(
        start_steps(&response),
        vec![
            step(root, 0, abcdef),
            step(abcdef, 1, bcdef),
            step(bcdef, 1, cdef),
            step(cdef, 1, def),
            step(def, 1, ef),
            step(ef, 1, f_),
        ]
    );
    assert_eq!(
        end_steps(&response),
        vec![
            step(root, 6, ghijkl),
            step(ghijkl, 0, ghijk),
            step(ghijk, 0, ghij),
            step(ghij, 0, ghi),
            step(ghi, 0, gh),
            step(gh, 0, g),
        ]
    );
    // Up keys at the token end (query position 1 after `f`), down keys at
    // the token start clamped to the match start (query position 1).
    assert_eq!(
        sorted(edges(&response)),
        sorted(vec![
            (up(ef, 1), up(f_, 1), pos(1)),
            (up(def, 1), up(ef, 1), pos(1)),
            (up(cdef, 1), up(def, 1), pos(1)),
            (up(bcdef, 1), up(cdef, 1), pos(1)),
            (up(abcdef, 1), up(bcdef, 1), pos(1)),
            (up(root, 1), up(abcdef, 1), pos(0)),
            (down(root, 1), down(ghijkl, 1), pos(6)),
            (down(ghijkl, 1), down(ghijk, 1), pos(0)),
            (down(ghijk, 1), down(ghij, 1), pos(0)),
            (down(ghij, 1), down(ghi, 1), pos(0)),
            (down(ghi, 1), down(gh, 1), pos(0)),
            (down(gh, 1), down(g, 1), pos(0)),
        ])
    );
}

/// The composite query `[ef, ghij]` gives the same paths and edges as the
/// atom query `efghij` (canonical paths depend on root and span only).
#[test]
fn deep_chains_composite_query() {
    let f = fixture("deep_chains");
    let [ef, def, cdef, bcdef, abcdef] = names(&f, ["ef", "def", "cdef", "bcdef", "abcdef"]);
    let [ghij, ghijk, ghijkl, root] = names(&f, ["ghij", "ghijk", "ghijkl", "abcdefghijkl"]);
    let atoms = check(&f, "efghij");
    assert_eq!(atoms.root_token(), root);
    assert_eq!(atoms.span(), span(4, 10));
    assert_eq!(atoms.path().coverage(), SpanCoverage::Range);
    assert!(atoms.query_exhausted());
    assert_eq!(
        start_steps(&atoms),
        vec![
            step(root, 0, abcdef),
            step(abcdef, 1, bcdef),
            step(bcdef, 1, cdef),
            step(cdef, 1, def),
            step(def, 1, ef),
        ]
    );
    assert_eq!(
        end_steps(&atoms),
        vec![
            step(root, 6, ghijkl),
            step(ghijkl, 0, ghijk),
            step(ghijk, 0, ghij),
        ]
    );
    assert_eq!(
        sorted(edges(&atoms)),
        sorted(vec![
            (up(def, 2), up(ef, 2), pos(1)),
            (up(cdef, 2), up(def, 2), pos(1)),
            (up(bcdef, 2), up(cdef, 2), pos(1)),
            (up(abcdef, 2), up(bcdef, 2), pos(1)),
            (up(root, 2), up(abcdef, 2), pos(0)),
            (down(root, 2), down(ghijkl, 2), pos(6)),
            (down(ghijkl, 2), down(ghijk, 2), pos(0)),
            (down(ghijk, 2), down(ghij, 2), pos(0)),
        ])
    );

    let composite = f.graph.find_ancestor(vec![ef, ghij]).unwrap();
    assert_eq!(composite.root_token(), root);
    assert_eq!(composite.span(), atoms.span());
    assert!(composite.query_exhausted());
    assert_eq!(composite.path(), atoms.path());
    assert_eq!(sorted(edges(&composite)), sorted(edges(&atoms)));
}

/// Postfix of the prefix chain: the start path climbs 7 steps from `b`.
#[test]
fn deep_prefix_chain_postfix() {
    let f = fixture("deep_prefix_chain");
    let [b, h, ab, abc, abcd, abcde, abcdef, abcdefg, root] = names(
        &f,
        ["b", "h", "ab", "abc", "abcd", "abcde", "abcdef", "abcdefg", "abcdefgh"],
    );
    let response = check(&f, "bcdefgh");
    assert_eq!(response.root_token(), root);
    assert_eq!(response.span(), span(1, 8));
    assert_eq!(response.path().coverage(), SpanCoverage::Postfix);
    assert!(response.query_exhausted());
    assert_eq!(
        start_steps(&response),
        vec![
            step(root, 0, abcdefg),
            step(abcdefg, 0, abcdef),
            step(abcdef, 0, abcde),
            step(abcde, 0, abcd),
            step(abcd, 0, abc),
            step(abc, 0, ab),
            step(ab, 1, b),
        ]
    );
    assert_eq!(end_steps(&response), vec![step(root, 7, h)]);
    // Up keys at each token's end in query positions (`b` ends at 1).
    assert_eq!(
        sorted(edges(&response)),
        sorted(vec![
            (up(ab, 1), up(b, 1), pos(1)),
            (up(abc, 2), up(ab, 1), pos(0)),
            (up(abcd, 3), up(abc, 2), pos(0)),
            (up(abcde, 4), up(abcd, 3), pos(0)),
            (up(abcdef, 5), up(abcde, 4), pos(0)),
            (up(abcdefg, 6), up(abcdef, 5), pos(0)),
            (up(root, 6), up(abcdefg, 6), pos(0)),
        ])
    );
}

/// Prefix of the suffix chain: the end path descends 7 steps to `g`.
#[test]
fn deep_suffix_chain_prefix() {
    let f = fixture("deep_suffix_chain");
    let [a, g, gh, fgh, efgh, defgh, cdefgh, bcdefgh, root] = names(
        &f,
        ["a", "g", "gh", "fgh", "efgh", "defgh", "cdefgh", "bcdefgh", "abcdefgh"],
    );
    let response = check(&f, "abcdefg");
    assert_eq!(response.root_token(), root);
    assert_eq!(response.span(), span(0, 7));
    assert_eq!(response.path().coverage(), SpanCoverage::Prefix);
    assert!(response.query_exhausted());
    assert_eq!(start_steps(&response), vec![step(root, 0, a)]);
    assert_eq!(
        end_steps(&response),
        vec![
            step(root, 1, bcdefgh),
            step(bcdefgh, 1, cdefgh),
            step(cdefgh, 1, defgh),
            step(defgh, 1, efgh),
            step(efgh, 1, fgh),
            step(fgh, 1, gh),
            step(gh, 0, g),
        ]
    );
    // Down keys at each token's start in query positions.
    assert_eq!(
        sorted(edges(&response)),
        sorted(vec![
            (down(root, 1), down(bcdefgh, 1), pos(1)),
            (down(bcdefgh, 1), down(cdefgh, 2), pos(1)),
            (down(cdefgh, 2), down(defgh, 3), pos(1)),
            (down(defgh, 3), down(efgh, 4), pos(1)),
            (down(efgh, 4), down(fgh, 5), pos(1)),
            (down(fgh, 5), down(gh, 6), pos(1)),
            (down(gh, 6), down(g, 6), pos(0)),
        ])
    );
}
