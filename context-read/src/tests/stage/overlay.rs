//! Overlay stage (staging plan §4, §7.2): `extend` preconditions on staged
//! and committed vertices, commit translation, the base generation check
//! and equivalence of staged and direct `GraphEdit` sequences.

use context_insert::edit::EditExt as _;
use pretty_assertions::assert_eq;

use crate::{
    stage::{
        CommitError,
        overlay::{
            OverlayHandle,
            OverlayView,
        },
        StageHandle,
    },
};
use context_trace::{
    BaseGraphKind,
    graph::{
        vertex::{
            CommittedIndex,
            StagedIndex,
            StageIndex,
            VertexId,
            VertexIndex,
        },
    },
    positioned::{
        GraphRead,
        GraphWrite,
        Occurrence,
        PositionedError,
        PositionedGraph,
        PositionedGraphRef,
        VertexRead,
        VertexState,
    },
    Token,
    ToToken,
};
use context_insert::{
    edit::{
        GraphEdit,
        Stage,
        VertexEdit,
    },
};

pub(super) fn occ<I: VertexId>(
    offset: usize,
    token: Token<I>,
) -> Occurrence<I> {
    Occurrence::new(offset, token)
}

pub(super) fn lift<I: VertexId + From<CommittedIndex>>(token: Token) -> Token<I> {
    Token::new(I::from(token.index), token.width)
}

/// The committed tokens of [`base`].
#[derive(Clone, Copy)]
pub(super) struct Names {
    pub(super) a: Token,
    pub(super) b: Token,
    pub(super) ab: Token,
    pub(super) abab: Token,
    pub(super) ababab: Token,
}

/// `ab = a@0, b@1`, `abab = ab@0, ab@2`, `ababab = abab@0, abab@2`.
pub(super) fn base() -> (PositionedGraph, Names) {
    let mut graph = PositionedGraph::default();
    let mut edit = graph.edit();
    let a = edit.atom('a');
    let b = edit.atom('b');
    let ab = edit.create(vec![occ(0, a), occ(1, b)]).unwrap();
    let abab = edit.create(vec![occ(0, ab), occ(2, ab)]).unwrap();
    let ababab = edit.create(vec![occ(0, abab), occ(2, abab)]).unwrap();
    edit.finish();
    (
        graph,
        Names {
            a,
            b,
            ab,
            abab,
            ababab,
        },
    )
}

/// Exact equality: indices, widths, compositions, parent links, labels,
/// atom index and generation (not the random vertex keys).
pub(super) fn assert_identical(
    left: &PositionedGraph,
    right: &PositionedGraph,
) {
    let strip = |graph: &PositionedGraph| {
        (0..graph.vertex_count())
            .map(|slot| graph.vertex_data(VertexIndex(slot)).unwrap())
            .map(|v| format!("{} {:?} {:?}", v, v.node(), v.parents()))
            .collect::<Vec<_>>()
    };
    assert_eq!(strip(left), strip(right));
    // The atom lookup of both graphs agrees on every atom.
    let atoms = |graph: &PositionedGraph| {
        (0..graph.vertex_count())
            .filter_map(|slot| match graph.vertex_data(VertexIndex(slot)).unwrap().node() {
                context_trace::positioned::VertexNode::Atom(atom) => Some((*atom, slot)),
                context_trace::positioned::VertexNode::Composite(_) => None,
            })
            .map(|(atom, slot)| {
                let found = GraphRead::atom(graph, &atom).map(|v| context_trace::HasVertexIndex::vertex_index(&v));
                (atom, slot, found)
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(atoms(left), atoms(right));
    assert_eq!(left.generation().base, right.generation().base);
}

/// Insertion of `baba` into `ababab` (insert plan §9.3) by hand, in two
/// edits: new vertices in committed hosts, base links removed.
pub(super) fn insert_baba<S: GraphWrite<Kind = BaseGraphKind>>(
    graph: &mut S,
    n: Names,
) -> Token<S::Index>
where
    S::Index: From<CommittedIndex>,
{
    let (a, b, ab, abab, ababab) = (
        lift(n.a),
        lift(n.b),
        lift(n.ab),
        lift(n.abab),
        lift(n.ababab),
    );
    let mut edit = GraphEdit::new(graph);
    let ba = edit.create(vec![occ(0, b), occ(1, a)]).unwrap();
    let aba = edit.create(vec![occ(0, ab), occ(1, ba)]).unwrap();
    let bab = edit.create(vec![occ(0, ba), occ(1, ab)]).unwrap();
    edit.add_occurrences(abab.index, vec![occ(0, aba), occ(1, bab)])
        .unwrap();
    edit.finish();
    let mut edit = GraphEdit::new(graph);
    let baba = edit.create(vec![occ(0, bab), occ(1, aba)]).unwrap();
    edit.add_occurrences(ababab.index, vec![occ(1, baba)])
        .unwrap();
    edit.finish();
    baba
}

/// Atom lookup (`c` is created beforehand: a stage commits new atoms through
/// `OverlayHandle::atom`), new composites over committed children, and a
/// staged host dropping a link it added to a committed child (`a@0` replaced
/// by `ab@0`). The root `bcabcabc` gives `bc` and `abc` two parent
/// occurrences each (a valid vertex set for the commit).
pub(super) fn new_atom_and_recompose<S: GraphWrite<Kind = BaseGraphKind>>(
    graph: &mut S,
    n: Names,
) -> Token<S::Index>
where
    S::Index: From<CommittedIndex>,
{
    let (a, b, ab) = (lift(n.a), lift(n.b), lift(n.ab));
    let mut edit = GraphEdit::new(graph);
    let c = edit.atom('c');
    assert_eq!(edit.atom('c'), c);
    let bc = edit.create(vec![occ(0, b), occ(1, c)]).unwrap();
    let abc = edit.create(vec![occ(0, a), occ(1, bc)]).unwrap();
    edit.finish();
    let mut edit = GraphEdit::new(graph);
    assert!(edit.create(vec![occ(0, ab)]).is_err());
    edit.add_occurrences(abc.index, vec![occ(0, ab)]).unwrap();
    let root = edit
        .create(vec![occ(0, bc), occ(2, abc), occ(5, abc)])
        .unwrap();
    edit.finish();
    let mut edit = GraphEdit::new(graph);
    assert_eq!(edit.atom('a'), a);
    assert_eq!(edit.finish().created, vec![]);
    root
}

/// Creating `atoms` and then applying `sequence` on a stage and committing
/// gives exactly the graph of applying both directly.
fn assert_overlay_equals_direct(
    atoms: &[char],
    sequence: impl FnOnce(&mut PositionedGraph, Names) -> Token,
    staged: impl FnOnce(&mut OverlayView<'_>, Names) -> Token<StageIndex>,
) {
    let (initial, names) = base();
    let mut direct = initial.clone();
    let mut edit = direct.edit();
    for &atom in atoms {
        edit.atom(atom);
    }
    edit.finish();
    let base = direct.clone();
    let expected = sequence(&mut direct, names);
    direct.validate().unwrap();

    let graph = PositionedGraphRef::new(initial);
    let mut handle = OverlayHandle::open(&graph);
    for &atom in atoms {
        handle.atom(atom).unwrap();
    }
    let root = {
        let mut view = handle.stage();
        let root = staged(&mut view, names);
        assert_eq!(
            view.staged().count(),
            direct.vertex_count() - base.vertex_count()
        );
        root
    };
    assert_eq!(
        graph.read().generation(),
        base.generation(),
        "base untouched"
    );
    let (committed, translation) = handle.commit(root).unwrap();
    assert_eq!(committed, expected);
    assert_eq!(
        translation.len(),
        direct.vertex_count() - base.vertex_count()
    );
    let graph = graph.read();
    graph.validate().unwrap();
    assert_identical(&direct, &graph);
}

#[test]
fn overlay_commit_equals_direct_edits() {
    assert_overlay_equals_direct(&[], insert_baba, |view, n| {
        insert_baba(view, n)
    });
    assert_overlay_equals_direct(&['c'], new_atom_and_recompose, |view, n| {
        new_atom_and_recompose(view, n)
    });
}

#[test]
fn extend_rejects_committed_vertices() {
    let (base, n) = base();
    let graph = PositionedGraphRef::new(base);
    let mut handle = OverlayHandle::open(&graph);
    let mut view = handle.stage();
    let mut edit = GraphEdit::new(&mut view);
    let (ab, abab) = (lift::<StageIndex>(n.ab), lift::<StageIndex>(n.abab));
    let parentless = lift::<StageIndex>(n.ababab);
    assert_eq!(
        edit.extend(parentless.index, occ(4, abab)),
        Err(PositionedError::NotStaged(parentless.index))
    );
    assert_eq!(
        edit.extend(abab.index, occ(2, ab)),
        Err(PositionedError::NotStaged(abab.index))
    );
    assert_eq!(edit.finish().created, vec![]);
    assert_eq!(view.generation().stage, 0, "rejections do not mutate");
}

#[test]
fn extend_grows_a_staged_parentless_vertex_only() {
    let (base, n) = base();
    let graph = PositionedGraphRef::new(base);
    let mut handle = OverlayHandle::open(&graph);
    let (a, b, ab) = (lift(n.a), lift(n.b), lift(n.ab));
    let c = handle.atom('c').unwrap();
    let (root, bc) = {
        let mut view = handle.stage();
        let mut edit = GraphEdit::new(&mut view);
        let bc = edit.create(vec![occ(0, b), occ(1, c)]).unwrap();
        let root = edit.create(vec![occ(0, ab), occ(2, c)]).unwrap();
        edit.finish();
        (root, bc)
    };
    let mut view = handle.stage();
    let mut edit = GraphEdit::new(&mut view);
    assert_eq!(
        edit.extend(bc.index, occ(3, a)),
        Err(PositionedError::OutsideVertex {
            vertex: bc.index,
            occurrence: occ(3, a),
        })
    );
    assert_eq!(
        edit.extend(root.index, occ(2, c)),
        Err(PositionedError::OutsideVertex {
            vertex: root.index,
            occurrence: occ(2, c),
        })
    );
    let abcab = edit.extend(root.index, occ(3, ab)).unwrap();
    assert_eq!(abcab, Token::new(root.index, 5));
    let abcabc = edit.extend(root.index, occ(4, bc)).unwrap();
    assert_eq!(abcabc, Token::new(root.index, 6));
    let outer = edit.create(vec![occ(0, abcabc), occ(6, a)]).unwrap();
    assert_eq!(
        edit.extend(root.index, occ(6, a)),
        Err(PositionedError::HasParents(root.index))
    );
    let publication = edit.finish();
    assert_eq!(
        publication.edits,
        [VertexEdit {
            vertex: root.index,
            added: vec![occ(3, ab), occ(4, bc)],
            removed: vec![],
        }]
    );
    #[cfg(debug_assertions)]
    assert_eq!(view.vertex(root).unwrap().label(), "abcabc");
    // A valid vertex set for the commit: `outer` discarded, `bc@1` added
    // (`bc` gets two parent occurrences, `abcabc` is the root).
    let mut edit = GraphEdit::new(&mut view);
    edit.discard(outer.index, abcabc.index).unwrap();
    edit.add_occurrences(root.index, vec![occ(1, bc)]).unwrap();
    edit.finish();
    drop(view);

    let (_, translation) = handle.commit(abcabc).unwrap();
    let graph = graph.read();
    graph.validate().unwrap();
    let root = translation.token(abcabc);
    let links =
        |token: Token| graph.vertex_data(token).unwrap().parents().clone();
    assert_eq!(
        links(n.ab),
        [
            (n.abab.index, [0.into(), 2.into()].into()),
            (root.index, [0.into(), 3.into()].into())
        ]
        .into()
    );
    assert_eq!(
        links(translation.token(bc)),
        [(root.index, [1.into(), 4.into()].into())].into()
    );
    assert_eq!(links(root), [].into());
    #[cfg(debug_assertions)]
    assert_eq!(graph.vertex_data(root).unwrap().label(), "abcabc");
}

#[test]
fn commit_translates_staged_indices_in_slot_order() {
    let (base, n) = base();
    let count = base.vertex_count();
    let graph = PositionedGraphRef::new(base);
    let mut handle = OverlayHandle::open(&graph);
    let c = handle.atom('c').unwrap();
    assert_eq!(c, Token::new(StageIndex::lift(VertexIndex(count)), 1));
    assert_eq!(
        graph.read().atom(&'c').unwrap().to_token(),
        Token::new(VertexIndex(count), 1),
        "new atoms are committed at once"
    );
    let (ca, caca, abab) = {
        let mut view = handle.stage();
        let mut edit = GraphEdit::new(&mut view);
        assert_eq!(edit.atom('c'), c);
        let ca = edit.create(vec![occ(0, c), occ(1, lift(n.a))]).unwrap();
        let caca = edit.create(vec![occ(0, ca), occ(2, ca)]).unwrap();
        edit.finish();
        assert_eq!(view.vertex(ca).unwrap().state(), VertexState::Staged);
        assert_eq!(view.vertex(c).unwrap().state(), VertexState::Committed);
        assert_eq!(view.atom(&'c').unwrap().to_token(), c);
        assert_eq!(view.atom(&'a').unwrap().to_token(), lift(n.a));
        (ca, caca, lift::<StageIndex>(n.abab))
    };
    assert_eq!(ca.index, StagedIndex::new(0).into());
    assert_eq!(handle.staged_count(), 2);
    let (root, translation) = handle.commit(caca).unwrap();
    assert_eq!(translation.index(c.index), VertexIndex(count));
    assert_eq!(translation.index(ca.index), VertexIndex(count + 1));
    assert_eq!(root, Token::new(VertexIndex(count + 2), 4));
    assert_eq!(
        translation.token(abab),
        n.abab,
        "committed indices are unchanged"
    );
    graph.read().validate().unwrap();
}

#[test]
fn commit_rejects_a_changed_base() {
    let (base, n) = base();
    let graph = PositionedGraphRef::new(base);
    let opened = graph.read().generation().base;
    let count = graph.read().vertex_count();
    let mut handle = OverlayHandle::open(&graph);
    let root = {
        let mut view = handle.stage();
        let mut edit = GraphEdit::new(&mut view);
        let root = edit
            .create(vec![occ(0, lift(n.b)), occ(1, lift(n.a))])
            .unwrap();
        edit.finish();
        root
    };
    graph.write().edit().atom('z');
    assert_eq!(
        handle.commit(root),
        Err(CommitError::BaseChanged {
            opened,
            now: opened + 1,
        })
    );
    assert_eq!(graph.read().vertex_count(), count + 1, "only `z` committed");
}

#[test]
fn stage_generation_counts_edits_and_commit_adds_them() {
    let (base, n) = base();
    let opened = base.generation().base;
    let graph = PositionedGraphRef::new(base);
    let mut handle = OverlayHandle::open(&graph);
    let root = {
        let mut view = handle.stage();
        let root = insert_baba(&mut view, n);
        assert_eq!(view.generation().base, opened);
        assert_eq!(view.generation().stage, 2);
        root
    };
    handle.commit(root).unwrap();
    assert_eq!(graph.read().generation().base, opened + 2);
}

#[cfg(debug_assertions)]
#[test]
fn staged_labels_resolve_through_the_overlay() {
    use context_trace::graph::vertex::name_resolver::TokenNameResolver;
    let (base, n) = base();
    let graph = PositionedGraphRef::new(base);
    let mut handle = OverlayHandle::open(&graph);
    let mut view = handle.stage();
    let mut edit = GraphEdit::new(&mut view);
    let ba = edit
        .create(vec![occ(0, lift(n.b)), occ(1, lift(n.a))])
        .unwrap();
    edit.finish();
    let labels = view.name_resolver();
    assert_eq!(
        labels
            .staged_token_name(ba.index.staged().unwrap().slot())
            .as_deref(),
        Some("ba")
    );
    assert_eq!(labels.token_name(n.ab.index.0).as_deref(), Some("ab"));
}
