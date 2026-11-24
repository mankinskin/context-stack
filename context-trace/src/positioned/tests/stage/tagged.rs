//! Tagged stage (staging plan §5, §7.2): `GraphEdit` preconditions on the
//! tagged stage, tag bits mirroring vertex states, commit by flipping tags,
//! and differential tests against the overlay stage and direct edits.

use pretty_assertions::assert_eq;

use super::overlay::{
    Names,
    assert_identical,
    base,
    insert_baba,
    lift,
    new_atom_and_recompose,
    occ,
};
use crate::{
    ToToken,
    Token,
    graph::vertex::{
        StageIndex,
        StagedIndex,
        VertexIndex,
    },
    positioned::{
        GraphRead,
        OverlayHandle,
        PositionedError,
        PositionedGraph,
        PositionedGraphRef,
        Stage,
        StageHandle,
        TaggedHandle,
        VertexEdit,
        VertexRead,
        VertexState,
        edit::GraphEdit,
    },
};

/// Run `$sequence` on a stage of `$handle`, check the staged count, commit.
macro_rules! run_staged {
    ($handle:expr, $names:expr, $sequence:ident, staged = $staged:expr) => {{
        let mut handle = $handle;
        let root = {
            let mut view = handle.stage();
            let root = $sequence(&mut view, $names);
            assert_eq!(view.staged().count(), $staged);
            root
        };
        handle.commit(root).unwrap()
    }};
}

/// The same `GraphEdit` sequence applied directly, on an overlay stage and
/// on a tagged stage gives exactly the same committed graph (indices,
/// compositions, parents, labels, atoms, generation) and root. New atoms
/// are staged in (a) and committed at once in (b); both assign indices in
/// creation order, so the committed indices coincide.
macro_rules! assert_strategies_agree {
    ($sequence:ident, new_atoms = $atoms:expr) => {{
        let (base, names): (PositionedGraph, Names) = base();
        let mut direct = base.clone();
        let expected = $sequence(&mut direct, names);
        direct.validate().unwrap();
        let created = direct.vertex_count() - base.vertex_count();

        let overlay = PositionedGraphRef::new(base.clone());
        let (root, translation) = run_staged!(
            OverlayHandle::open(&overlay),
            names,
            $sequence,
            staged = created
        );
        assert_eq!(root, expected);
        assert_eq!(translation.len(), created);

        let tagged = PositionedGraphRef::new(base.clone());
        let (root, translation) = run_staged!(
            TaggedHandle::open(&tagged),
            names,
            $sequence,
            staged = created - $atoms
        );
        assert_eq!(root, expected);
        assert_eq!(translation.len(), created - $atoms);

        let (overlay, tagged) = (overlay.read(), tagged.read());
        tagged.validate().unwrap();
        assert_identical(&direct, &overlay);
        assert_identical(&direct, &tagged);
    }};
}

#[test]
fn tagged_commit_equals_overlay_and_direct_edits() {
    assert_strategies_agree!(insert_baba, new_atoms = 0);
    assert_strategies_agree!(new_atom_and_recompose, new_atoms = 1);
}

#[test]
fn tagged_extend_preconditions() {
    let (base, n) = base();
    let graph = PositionedGraphRef::new(base);
    let mut handle = TaggedHandle::open(&graph);
    let (a, b, ab, abab, ababab) = (
        lift::<StageIndex>(n.a),
        lift(n.b),
        lift(n.ab),
        lift(n.abab),
        lift(n.ababab),
    );
    let mut view = handle.stage();
    let mut edit = GraphEdit::new(&mut view);
    assert_eq!(
        edit.extend(ababab.index, occ(4, abab)),
        Err(PositionedError::NotStaged(ababab.index))
    );
    assert_eq!(
        edit.extend(abab.index, occ(2, ab)),
        Err(PositionedError::NotStaged(abab.index))
    );
    assert_eq!(
        edit.extend(a.index, occ(1, b)),
        Err(PositionedError::Atom(a.index))
    );
    let c = edit.atom('c');
    assert_eq!(
        edit.extend(c.index, occ(1, a)),
        Err(PositionedError::Atom(c.index)),
        "a new atom is committed at once"
    );
    let bc = edit.create(vec![occ(0, b), occ(1, c)]).unwrap();
    let root = edit.create(vec![occ(0, ab), occ(2, c)]).unwrap();
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
    let outer = edit.create(vec![occ(0, abcabc), occ(6, a)]).unwrap();
    assert_eq!(
        edit.extend(root.index, occ(6, a)),
        Err(PositionedError::HasParents(root.index))
    );
    let publication = edit.finish();
    assert_eq!(publication.edits, Vec::<VertexEdit<StageIndex>>::new());
    #[cfg(debug_assertions)]
    assert_eq!(view.vertex(root).unwrap().label(), "abcabc");
    drop(view);

    let (outer, translation) = handle.commit(outer).unwrap();
    assert_eq!(translation.index(root.index), VertexIndex(root.index.slot()));
    graph.read().validate().unwrap();
    let mut handle = TaggedHandle::open(&graph);
    let mut view = handle.stage();
    let outer = lift::<StageIndex>(outer);
    assert_eq!(
        GraphEdit::new(&mut view).extend(outer.index, occ(7, a)),
        Err(PositionedError::NotStaged(outer.index)),
        "committed by the previous stage"
    );
}

#[test]
fn tag_bits_mirror_vertex_states() {
    let (base, n) = base();
    let count = base.vertex_count();
    let opened = base.generation;
    let graph = PositionedGraphRef::new(base);
    let mut handle = TaggedHandle::open(&graph);
    let (c, ca) = {
        let mut view = handle.stage();
        let mut edit = GraphEdit::new(&mut view);
        let c = edit.atom('c');
        let ca = edit.create(vec![occ(0, c), occ(1, lift(n.a))]).unwrap();
        let abab = lift::<StageIndex>(n.abab);
        edit.finish();
        assert_eq!(c.index, StageIndex::from(VertexIndex(count)));
        assert_eq!(ca.index, StagedIndex::new(count + 1).into());
        assert_eq!(view.vertex(c).unwrap().state(), VertexState::Committed);
        assert_eq!(view.vertex(ca).unwrap().state(), VertexState::Staged);
        assert_eq!(view.atom(&'c').unwrap().to_token(), c);
        let a = view.vertex(lift::<StageIndex>(n.a)).unwrap();
        assert!(a.parents().any(|(parent, _)| parent == ca));
        let composition = view.vertex(ca).unwrap().node().composite().unwrap();
        assert_eq!(
            crate::positioned::CompositionRead::occurrences(composition)
                .collect::<Vec<_>>(),
            vec![occ(0, c), occ(1, lift(n.a))]
        );
        assert_eq!(view.vertex(abab).unwrap().state(), VertexState::Committed);
        assert_eq!(view.staged().collect::<Vec<_>>(), [StagedIndex::new(count + 1)]);
        assert_eq!(view.generation().base, opened);
        assert_eq!(view.generation().stage, 1);
        #[cfg(debug_assertions)]
        {
            use crate::graph::vertex::name_resolver::TokenNameResolver;
            assert_eq!(
                view.name_resolver().staged_token_name(count + 1).as_deref(),
                Some("ca")
            );
        }
        (c, ca)
    };
    assert_eq!(handle.staged_count(), 1);
    assert_eq!(
        graph.read().vertex_data(VertexIndex(count + 1)).unwrap().state(),
        VertexState::Staged,
        "other readers see staged vertices"
    );
    let (root, translation) = handle.commit(ca).unwrap();
    assert_eq!(root, Token::new(VertexIndex(count + 1), 2));
    assert_eq!(translation.index(c.index), VertexIndex(count));
    assert_eq!(translation.len(), 1);
    let graph = graph.read();
    graph.validate().unwrap();
    assert!(graph.vertices().all(|v| v.state() == VertexState::Committed));
    assert_eq!(graph.generation().base, opened + 1);
}

#[test]
fn dropping_a_tagged_handle_commits() {
    let (base, n) = base();
    let graph = PositionedGraphRef::new(base);
    let ba = {
        let mut handle = TaggedHandle::open(&graph);
        let mut view = handle.stage();
        let mut edit = GraphEdit::new(&mut view);
        let ba = edit.create(vec![occ(0, lift(n.b)), occ(1, lift(n.a))]).unwrap();
        edit.finish();
        ba
    };
    let index = VertexIndex(ba.index.slot());
    assert_eq!(
        graph.read().vertex_data(index).unwrap().state(),
        VertexState::Committed
    );
    drop(TaggedHandle::open(&graph));
    OverlayHandle::open(&graph);
}

#[test]
#[should_panic(expected = "a tagged stage is open")]
fn one_open_stage_per_graph() {
    let (base, _) = base();
    let graph = PositionedGraphRef::new(base);
    let _handle = TaggedHandle::open(&graph);
    OverlayHandle::open(&graph);
}
#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "disagrees with the vertex state")]
fn a_tag_bit_disagreeing_with_the_state_is_rejected() {
    let (base, n) = base();
    let graph = PositionedGraphRef::new(base);
    let mut handle = TaggedHandle::open(&graph);
    let mut view = handle.stage();
    let mut edit = GraphEdit::new(&mut view);
    let ba = edit.create(vec![occ(0, lift(n.b)), occ(1, lift(n.a))]).unwrap();
    edit.finish();
    view.vertex(StageIndex::from(VertexIndex(ba.index.slot())));
}
