//! Dissolution plan D3 and D4 (§4.2, §4.3, §5.1): new atoms committed
//! through `OverlayHandle::atom`, removal of staged vertices (a hole, no
//! vertex moves), `GraphEdit::dissolve` / `discard` /
//! `settle` with their `Candidates`, the commit check and
//! `validate_retention`.

use pretty_assertions::assert_eq;

use super::overlay::{
    Names,
    base,
    lift,
    occ,
};
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
    graph::{
        vertex::{
            StagedIndex,
            StageIndex,
            VertexId as _,
        },
    },
    positioned::{
        Candidates,
        CompositionRead,
        diff,
        GraphEdit,
        fixtures::fixture_graphs,
        Generation,
        GraphRead,
        interchange::{
            PositionedGraphData,
            store_interchange,
        },
        maximal,
        Occurrence,
        PositionedError,
        PositionedGraph,
        PositionedGraphRef,
        raw::{
            Remove as _,
            SEAL,
            VertexWrite as _,
            Write as _,
        },
        Removal,
        Settled,
        Stage,
        validate_store,
        VertexEdit,
        VertexRead,
        VertexState,
    },
    Token,
    ToToken,
};

#[test]
fn new_atom_is_committed_at_once() {
    let fixture = fixture_graphs()
        .into_iter()
        .find(|fixture| fixture.name == "linear_abc")
        .unwrap();
    let count = fixture.graph.vertex_count();
    let graph = PositionedGraphRef::new(fixture.graph);
    let opened = graph.read().generation().base;
    let mut handle = OverlayHandle::open(&graph);
    let x = handle.atom('x').unwrap();
    assert_eq!(
        x,
        Token::new(StageIndex::from(context_trace::VertexIndex(count)), 1)
    );
    assert_eq!(graph.read().generation().base, opened + 1);
    assert_eq!(
        graph.read().atom(&'x').map(|v| v.to_token()),
        Some(Token::new(context_trace::VertexIndex(count), 1))
    );
    let view = handle.stage();
    assert_eq!(view.generation().base, opened + 1);
    assert_eq!(view.generation().stage, 0, "the stage is unchanged");
    assert_eq!(view.vertex(x).unwrap().state(), VertexState::Committed);
    assert_eq!(view.atom(&'x').unwrap().to_token(), x);
    drop(view);
    assert_eq!(handle.staged_count(), 0);
    graph.read().validate().unwrap();
}

#[test]
fn existing_atom_is_returned() {
    let (graph, n) = base();
    let generation = graph.generation().base;
    let graph = PositionedGraphRef::new(graph);
    let mut handle = OverlayHandle::open(&graph);
    assert_eq!(handle.atom('a'), Ok(lift(n.a)));
    assert_eq!(handle.atom('b'), Ok(lift(n.b)));
    let c = handle.atom('c').unwrap();
    assert_eq!(handle.atom('c'), Ok(c));
    assert_eq!(graph.read().generation().base, generation + 1);
}

#[test]
fn atom_rejects_a_changed_base() {
    let (graph, _) = base();
    let graph = PositionedGraphRef::new(graph);
    let opened = graph.read().generation().base;
    let mut handle = OverlayHandle::open(&graph);
    graph.write().edit().atom('z');
    assert_eq!(
        handle.atom('c'),
        Err(CommitError::BaseChanged {
            opened,
            now: opened + 1,
        })
    );
    assert!(graph.read().atom(&'c').is_none());
    assert_eq!(
        handle.atom('z').map(|z| z.index.is_staged()),
        Ok(false),
        "existing atoms need no base write"
    );
}

#[test]
fn commit_detects_a_foreign_edit_after_an_own_atom() {
    let (graph, n) = base();
    let graph = PositionedGraphRef::new(graph);
    let mut handle = OverlayHandle::open(&graph);
    let c = handle.atom('c').unwrap();
    let after_atom = graph.read().generation().base;
    let root = {
        let mut view = handle.stage();
        let mut edit = GraphEdit::new(&mut view);
        let bc = edit.create(vec![occ(0, lift(n.b)), occ(1, c)]).unwrap();
        edit.finish();
        bc
    };
    graph.write().edit().atom('z');
    assert_eq!(
        handle.commit(root),
        Err(CommitError::BaseChanged {
            opened: after_atom,
            now: after_atom + 1,
        })
    );
}

/// The stage of [`staged_bab_last`].
pub(super) struct Slots {
    pub(super) ba: Token<StageIndex>,
    pub(super) aba: Token<StageIndex>,
    pub(super) baba: Token<StageIndex>,
    pub(super) bc: Token<StageIndex>,
    pub(super) bab: Token<StageIndex>,
}

/// On [`base`] plus the committed atom `c`: the insertion of `baba` with
/// `bab` created last (staged children `ba`, committed child `ab`, staged
/// parent `baba`, committed parent `abab`), and the parentless `bc` in a
/// middle slot.
pub(super) fn staged_bab_last(
    view: &mut OverlayView<'_>,
    n: Names,
    c: Token<StageIndex>,
) -> Slots {
    let (a, b, ab) = (lift(n.a), lift(n.b), lift(n.ab));
    let mut edit = GraphEdit::new(view);
    let ba = edit.create(vec![occ(0, b), occ(1, a)]).unwrap();
    let aba = edit.create(vec![occ(0, ab), occ(1, ba)]).unwrap();
    let baba = edit.create(vec![occ(0, ba), occ(2, ba)]).unwrap();
    let bc = edit.create(vec![occ(0, b), occ(1, c)]).unwrap();
    let bab = edit.create(vec![occ(0, ba), occ(1, ab)]).unwrap();
    edit.add_occurrences(baba.index, vec![occ(0, bab), occ(1, aba)])
        .unwrap();
    edit.add_occurrences(n.abab.index.into(), vec![occ(0, aba), occ(1, bab)])
        .unwrap();
    edit.add_occurrences(n.ababab.index.into(), vec![occ(1, baba)])
        .unwrap();
    edit.finish();
    Slots {
        ba,
        aba,
        baba,
        bc,
        bab,
    }
}

/// Unlink the children of the parentless staged `vertex` (what
/// `GraphEdit` does before a removal).
pub(super) fn unlink_children(
    view: &mut OverlayView<'_>,
    vertex: StageIndex,
) {
    assert_eq!(view.vertex(vertex).unwrap().parent_count(), 0);
    let children = composition(view, vertex);
    for child in children {
        view.vertex_mut(SEAL, child.child.index)
            .unwrap()
            .unlink_parent(vertex, child.offset);
    }
}

pub(super) fn composition(
    view: &OverlayView<'_>,
    vertex: StageIndex,
) -> Vec<Occurrence<StageIndex>> {
    view.vertex(vertex)
        .unwrap()
        .node()
        .composite()
        .unwrap()
        .occurrences()
        .collect()
}

pub(super) fn parents(
    view: &OverlayView<'_>,
    vertex: StageIndex,
) -> Vec<(StageIndex, usize)> {
    let mut parents: Vec<_> = view
        .vertex(vertex)
        .unwrap()
        .parents()
        .map(|(parent, offset)| (parent.index, offset.0))
        .collect();
    parents.sort();
    parents
}

#[test]
fn remove_leaves_a_hole() {
    let (base, n) = base();
    let graph = PositionedGraphRef::new(base);
    let mut handle = OverlayHandle::open(&graph);
    let c = handle.atom('c').unwrap();
    let mut view = handle.stage();
    let Slots {
        ba,
        aba,
        baba,
        bc,
        bab,
    } = staged_bab_last(&mut view, n, c);
    assert_eq!(bab.index, StageIndex::from(StagedIndex::new(4)));
    let (ab, abab) = (lift::<StageIndex>(n.ab), lift::<StageIndex>(n.abab));
    assert_eq!(
        parents(&view, ba.index),
        vec![(aba.index, 1), (bab.index, 0)]
    );
    validate_store(&view).unwrap();

    unlink_children(&mut view, bc.index);
    let generation = view.generation().stage;
    view.remove(SEAL, bc.index);
    assert_eq!(view.generation().stage, generation + 1);
    // Nothing moves: `bc`'s slot is a hole, `bab` keeps its index.
    assert!(view.vertex(bc.index).is_none());
    assert_eq!(bab.index, StageIndex::from(StagedIndex::new(4)));
    assert_eq!(view.staged().count(), 4);
    assert!(
        view.staged()
            .all(|staged| StageIndex::from(staged) != bc.index)
    );
    assert_eq!(composition(&view, bab.index), vec![occ(0, ba), occ(1, ab)]);
    #[cfg(debug_assertions)]
    {
        use context_trace::graph::vertex::name_resolver::TokenNameResolver;
        assert_eq!(view.vertex(bab).unwrap().label(), "bab");
        let resolver = view.name_resolver();
        let slot =
            |token: Token<StageIndex>| token.index.staged().unwrap().slot();
        assert_eq!(resolver.staged_token_name(slot(bc)), None);
        assert_eq!(
            resolver.staged_token_name(slot(bab)).as_deref(),
            Some("bab")
        );
    }
    // Children: staged (`ba`) and committed (`ab`, through its delta).
    assert_eq!(
        parents(&view, ba.index),
        vec![(aba.index, 1), (bab.index, 0)]
    );
    assert_eq!(
        parents(&view, ab.index),
        vec![(aba.index, 0), (bab.index, 1)]
    );
    // Parents: staged (`baba`) and committed (`abab`, through its delta).
    assert_eq!(
        parents(&view, bab.index),
        vec![(abab.index, 1), (baba.index, 0)]
    );
    assert_eq!(
        composition(&view, baba.index),
        vec![occ(0, bab), occ(1, aba)]
    );
    assert_eq!(
        composition(&view, abab.index),
        vec![occ(0, aba), occ(1, bab)]
    );
    // `b` and `c` lost their links to the removed `bc`.
    assert!(
        !parents(&view, lift::<StageIndex>(n.b).index)
            .iter()
            .any(|&(p, _)| p == bc.index)
    );
    assert_eq!(parents(&view, c.index), vec![]);
    validate_store(&view).unwrap();

    // Slots are not reused: a new vertex gets a fresh slot.
    let ca = {
        let mut edit = GraphEdit::new(&mut view);
        let ca = edit.create(vec![occ(0, c), occ(1, lift(n.a))]).unwrap();
        edit.finish();
        ca
    };
    assert_eq!(ca.index, StageIndex::from(StagedIndex::new(5)));
    unlink_children(&mut view, ca.index);
    view.remove(SEAL, ca.index);
    assert_eq!(view.staged().count(), 4);
    assert_eq!(parents(&view, c.index), vec![]);
    validate_store(&view).unwrap();
    drop(view);

    // `baba` has one parent occurrence (`ababab@1`): only the root may.
    let (_, translation) = handle.commit(baba).unwrap();
    assert_eq!(translation.len(), 4);
    let graph = graph.read();
    graph.validate().unwrap();
    validate_store(&*graph).unwrap();
}

fn staged(slot: usize) -> StageIndex {
    StagedIndex::new(slot).into()
}

/// The fixture graph `name` as interchange data.
fn fixture(name: &str) -> PositionedGraphData {
    fixture_graphs()
        .into_iter()
        .find(|fixture| fixture.name == name)
        .unwrap_or_else(|| panic!("no fixture {name}"))
        .graph
        .to_interchange()
}

fn assert_same(
    expected: &PositionedGraphData,
    actual: &PositionedGraphData,
) {
    let difference = diff(expected, actual);
    assert!(difference.is_empty(), "{difference}");
}

/// One `settle` edit, as the reader runs it after every step; its counts
/// (the gained candidates are checked in `stable.rs`).
fn settle(
    view: &mut OverlayView<'_>,
    candidates: Candidates,
    root: Token<StageIndex>,
) -> Settled {
    let mut edit = GraphEdit::new(view);
    let settled = edit.settle(candidates, root.index);
    edit.finish();
    counts(settled)
}

fn counts(settled: Settled) -> Settled {
    Settled {
        gained: Vec::new(),
        ..settled
    }
}

/// The tokens of [`abcabca`] before the last `settle`.
struct Walk {
    a: Token<StageIndex>,
    b: Token<StageIndex>,
    c: Token<StageIndex>,
    /// `s0`, one parent occurrence (`abca@0`).
    abc: Token<StageIndex>,
    /// `s1`, parentless.
    root: Token<StageIndex>,
    /// `s2`, two parent occurrences (`root@0`, `root@3`).
    abca: Token<StageIndex>,
    /// The candidates of the last step.
    candidates: Candidates,
}

/// The reader's edits of `abcabca` (§3.5) on an empty graph, each step
/// followed by a `settle`, except the last step (the expansion by `abca`).
fn abcabca(handle: &mut OverlayHandle<'_>) -> Walk {
    let a = handle.atom('a').unwrap();
    let b = handle.atom('b').unwrap();
    let c = handle.atom('c').unwrap();
    let mut view = handle.stage();
    // unknown `a`: Append(a), adopted
    let mut root = a;
    // unknown `b`: NewRoot `ab`
    let mut edit = GraphEdit::new(&mut view);
    let ab = edit.create(vec![occ(0, a), occ(1, b)]).unwrap();
    let mut candidates = Candidates::default();
    candidates.record(&edit.finish());
    candidates.push(root.index);
    root = ab;
    assert_eq!(
        settle(&mut view, candidates, root),
        Settled {
            candidates: 2,
            ..Settled::default()
        }
    );
    // unknown `c`: InPlace `abc`
    let mut edit = GraphEdit::new(&mut view);
    let abc = edit.extend(ab.index, occ(2, c)).unwrap();
    let mut candidates = Candidates::default();
    candidates.record(&edit.finish());
    root = abc;
    assert_eq!(settle(&mut view, candidates, root), Settled::default());
    // forward `abc` (complete): NewRoot `abcabc`
    let mut edit = GraphEdit::new(&mut view);
    let abcabc = edit.create(vec![occ(0, abc), occ(3, abc)]).unwrap();
    let mut candidates = Candidates::default();
    candidates.record(&edit.finish());
    candidates.push(root.index);
    root = abcabc;
    assert_eq!(
        settle(&mut view, candidates, root),
        Settled {
            candidates: 2,
            ..Settled::default()
        }
    );
    // probe `[abc] ++ "a"`: the insertion creates `abca` inside the root
    let mut edit = GraphEdit::new(&mut view);
    let abca = edit.create(vec![occ(0, abc), occ(3, a)]).unwrap();
    edit.add_occurrences(root.index, vec![occ(0, abca)])
        .unwrap();
    let inserted = edit.finish();
    // commit Expand: InPlace
    let mut edit = GraphEdit::new(&mut view);
    root = edit.extend(root.index, occ(3, abca)).unwrap();
    let committed = edit.finish();
    let mut candidates = Candidates::default();
    candidates.record(&inserted);
    candidates.record(&committed);
    assert_eq!(
        (abc.index, root.index, abca.index),
        (staged(0), staged(1), staged(2))
    );
    assert_eq!(
        composition(&view, root.index),
        vec![occ(0, abca), occ(3, abca)]
    );
    Walk {
        a,
        b,
        c,
        abc,
        root,
        abca,
        candidates,
    }
}

/// The parts of a stage an edit may change.
fn stage_state(
    view: &OverlayView<'_>
) -> (PositionedGraphData, Generation, usize) {
    (
        store_interchange(view),
        view.generation(),
        view.staged().count(),
    )
}

#[test]
fn dissolve_and_discard_check_preconditions() {
    let mut base: PositionedGraph = PositionedGraph::default();
    let mut edit = base.edit();
    let x = edit.atom('x');
    let y = edit.atom('y');
    let xy = edit.create(vec![occ(0, x), occ(1, y)]).unwrap();
    edit.finish();
    let graph = PositionedGraphRef::new(base);
    let mut handle = OverlayHandle::open(&graph);
    let walk = abcabca(&mut handle);
    let xy = lift::<StageIndex>(xy).index;
    let (root, a, abc, abca) = (
        walk.root.index,
        walk.a.index,
        walk.abc.index,
        walk.abca.index,
    );
    let mut view = handle.stage();
    let before = stage_state(&view);
    let base_generation = graph.read().generation().base;

    let mut edit = GraphEdit::new(&mut view);
    for (result, expected) in [
        (edit.dissolve(a, root), PositionedError::Atom(a)),
        (edit.discard(a, root), PositionedError::Atom(a)),
        (edit.dissolve(xy, root), PositionedError::NotStaged(xy)),
        (edit.discard(xy, root), PositionedError::NotStaged(xy)),
        (edit.dissolve(root, root), PositionedError::IsRoot(root)),
        (edit.discard(root, root), PositionedError::IsRoot(root)),
        (
            edit.dissolve(abca, root),
            PositionedError::ParentCount {
                vertex: abca,
                count: 2,
            },
        ),
        (
            edit.discard(abc, root),
            PositionedError::ParentCount {
                vertex: abc,
                count: 1,
            },
        ),
        (
            edit.dissolve(staged(9), root),
            PositionedError::UnknownVertex(staged(9)),
        ),
    ] {
        assert_eq!(result, Err(expected));
    }
    assert_eq!(edit.finish(), Default::default(), "nothing recorded");
    assert_eq!(stage_state(&view), before, "the stage is unchanged");
    validate_store(&view).unwrap();
    assert_eq!(graph.read().generation().base, base_generation);
}

#[test]
fn dissolve_into_a_staged_host() {
    let graph = PositionedGraphRef::default();
    let mut handle = OverlayHandle::open(&graph);
    let Walk {
        a,
        b,
        c,
        abc,
        root,
        abca,
        ..
    } = abcabca(&mut handle);
    let mut view = handle.stage();
    let union = vec![occ(0, abc), occ(3, a), occ(0, a), occ(1, b), occ(2, c)]
        .into_iter()
        .filter(|o| *o != occ(0, abc))
        .collect();
    let mut edit = GraphEdit::new(&mut view);
    let removal = edit.dissolve(abc.index, root.index).unwrap();
    assert_eq!(
        removal,
        Removal {
            removed: abc.index,
            host: Some(abca.index),
            children: vec![a.index, b.index, c.index],
        }
    );
    let publication = edit.finish();
    assert_eq!(publication.created, vec![]);
    assert_eq!(
        publication.edits,
        [VertexEdit {
            vertex: abca.index,
            added: vec![occ(0, a), occ(1, b), occ(2, c)],
            removed: vec![],
        }],
        "the removed `abc@0` is not recorded"
    );
    assert_eq!(composition(&view, abca.index), maximal(union));
    assert_eq!(
        composition(&view, abca.index),
        vec![occ(0, a), occ(1, b), occ(2, c), occ(3, a)]
    );
    assert_eq!(
        composition(&view, root.index),
        vec![occ(0, abca), occ(3, abca)]
    );
    assert_eq!(
        parents(&view, a.index),
        vec![(abca.index, 0), (abca.index, 3)]
    );
    assert_eq!(parents(&view, b.index), vec![(abca.index, 1)]);
    assert_eq!(parents(&view, c.index), vec![(abca.index, 2)]);
    assert_eq!(
        parents(&view, abca.index),
        vec![(root.index, 0), (root.index, 3)]
    );
    assert_eq!(view.staged().count(), 2);
    validate_store(&view).unwrap();
    assert_same(&fixture("repeat_abcabca"), &store_interchange(&view));
}

/// The insertion of `aba` and `bab` into the committed `abab` of [`base`]
/// (the oracle graph of `ababab`) within `edit`: `ba` (`s0`, parents
/// `aba@1`, `bab@0`), `aba` (`s1`, parent `abab@0`) and `bab` (`s2`, parent
/// `abab@1`); `abab = [aba@0, bab@1]`.
fn insert_aba_bab(
    edit: &mut GraphEdit<'_, OverlayView<'_>>,
    n: Names,
) -> [Token<StageIndex>; 3] {
    let (a, b, ab) = (lift(n.a), lift(n.b), lift(n.ab));
    let ba = edit.create(vec![occ(0, b), occ(1, a)]).unwrap();
    let aba = edit.create(vec![occ(0, ab), occ(1, ba)]).unwrap();
    let bab = edit.create(vec![occ(0, ba), occ(1, ab)]).unwrap();
    edit.add_occurrences(n.abab.index.into(), vec![occ(0, aba), occ(1, bab)])
        .unwrap();
    assert_eq!(
        [ba.index, aba.index, bab.index],
        [staged(0), staged(1), staged(2)]
    );
    [ba, aba, bab]
}

#[test]
fn base_is_an_oracle_graph() {
    assert_same(&fixture("repeat_ababab"), &base().0.to_interchange());
}

#[test]
fn dissolve_into_a_committed_host() {
    let (base, n) = base();
    let graph = PositionedGraphRef::new(base);
    let mut handle = OverlayHandle::open(&graph);
    let mut view = handle.stage();
    let mut edit = GraphEdit::new(&mut view);
    let [ba, aba, bab] = insert_aba_bab(&mut edit, n);
    edit.finish();
    let (ab, abab) = (lift::<StageIndex>(n.ab), lift::<StageIndex>(n.abab));
    let root = lift::<StageIndex>(n.ababab).index;

    let mut edit = GraphEdit::new(&mut view);
    let removal = edit.dissolve(aba.index, root).unwrap();
    assert_eq!(
        removal,
        Removal {
            removed: aba.index,
            host: Some(abab.index),
            children: vec![ab.index, ba.index],
        }
    );
    let publication = edit.finish();
    assert_eq!(
        publication.edits,
        [VertexEdit {
            vertex: abab.index,
            added: vec![occ(0, ab)],
            removed: vec![],
        }]
    );
    // maximal([bab@1] ∪ [ab@0, ba@1]): `ba@1` lies in `bab@1`.
    assert_eq!(
        composition(&view, abab.index),
        vec![occ(0, ab), occ(1, bab)]
    );
    assert_eq!(
        parents(&view, ab.index),
        vec![(abab.index, 0), (bab.index, 1)]
    );
    assert_eq!(parents(&view, ba.index), vec![(bab.index, 0)]);
    assert_eq!(parents(&view, bab.index), vec![(abab.index, 1)]);
    validate_store(&view).unwrap();
}

#[test]
fn discard_removes_a_parentless_vertex() {
    let (base, n) = base();
    let graph = PositionedGraphRef::new(base);
    let mut handle = OverlayHandle::open(&graph);
    let mut view = handle.stage();
    let (a, b) = (lift::<StageIndex>(n.a), lift::<StageIndex>(n.b));
    let mut edit = GraphEdit::new(&mut view);
    let ba = edit.create(vec![occ(0, b), occ(1, a)]).unwrap();
    let bab = edit.create(vec![occ(0, ba), occ(1, lift(n.ab))]).unwrap();
    let removal = edit.discard(ba.index, bab.index);
    assert_eq!(
        removal,
        Err(PositionedError::ParentCount {
            vertex: ba.index,
            count: 1,
        })
    );
    let removal = edit.discard(bab.index, ba.index).unwrap();
    assert_eq!(
        removal,
        Removal {
            removed: bab.index,
            host: None,
            children: vec![ba.index, lift::<StageIndex>(n.ab).index],
        }
    );
    // `bab` was created in this edit: no record of it is left.
    assert_eq!(edit.finish().created, vec![ba]);
    assert_eq!(parents(&view, ba.index), vec![]);
    assert_eq!(view.staged().count(), 1);
    validate_store(&view).unwrap();

    // Discarding a middle slot moves nothing.
    let mut edit = GraphEdit::new(&mut view);
    let ab = lift::<StageIndex>(n.ab);
    let bb = edit.create(vec![occ(0, b), occ(1, b)]).unwrap();
    let removal = edit.discard(ba.index, bb.index).unwrap();
    assert_eq!(
        removal,
        Removal {
            removed: ba.index,
            host: None,
            children: vec![b.index, a.index],
        }
    );
    assert_eq!(edit.finish().created, vec![bb]);
    assert_eq!(composition(&view, bb.index), vec![occ(0, b), occ(1, b)]);
    assert_eq!(parents(&view, ab.index).len(), 2, "base links only");
    validate_store(&view).unwrap();
}

#[test]
fn candidates_are_lost_links_and_created_tokens() {
    let (base, n) = base();
    let graph = PositionedGraphRef::new(base);
    let mut handle = OverlayHandle::open(&graph);
    let mut view = handle.stage();
    let ab = lift::<StageIndex>(n.ab);
    let root = lift::<StageIndex>(n.ababab).index;

    let mut edit = GraphEdit::new(&mut view);
    let [ba, aba, bab] = insert_aba_bab(&mut edit, n);
    let mut candidates = Candidates::default();
    candidates.record(&edit.finish());
    assert_eq!(
        candidates.vertices(),
        [ab.index, ab.index, ba.index, aba.index, bab.index]
    );

    drop(view);

    // `ab@0` is removed from `abab` and re-added (by dissolving `aba`)
    // within one edit: not a candidate (`ab@2` is).
    let graph = PositionedGraphRef::new(super::overlay::base().0);
    let mut handle = OverlayHandle::open(&graph);
    let mut view = handle.stage();
    let mut edit = GraphEdit::new(&mut view);
    let [ba, aba, _] = insert_aba_bab(&mut edit, n);
    edit.dissolve(aba.index, root).unwrap();
    let publication = edit.finish();
    assert_eq!(publication.created, vec![ba, bab]);
    assert_eq!(
        publication.edits,
        [VertexEdit {
            vertex: lift::<StageIndex>(n.abab).index,
            added: vec![occ(1, bab)],
            removed: vec![occ(2, ab)],
        }]
    );
    let mut candidates = Candidates::default();
    candidates.record(&publication);
    candidates.push(root);
    assert_eq!(candidates.vertices(), [ab.index, ba.index, bab.index, root]);
}

#[test]
fn settle_reproduces_the_abcabca_walkthrough() {
    let graph = PositionedGraphRef::default();
    let mut handle = OverlayHandle::open(&graph);
    let Walk {
        abc,
        root,
        abca,
        candidates,
        ..
    } = abcabca(&mut handle);
    // `K = {abca, abc}`: `abc` lost both links to the root.
    assert_eq!(candidates.vertices(), [abc.index, abca.index, abc.index]);
    let mut view = handle.stage();
    assert_eq!(
        settle(&mut view, candidates, root),
        Settled {
            candidates: 2,
            dissolved: 1,
            discarded: 0,
            into_root: 0,
            gained: Vec::new(),
        }
    );
    assert!(view.vertex(abc.index).is_none());
    assert_eq!(
        composition(&view, root.index),
        vec![occ(0, abca), occ(3, abca)]
    );
    validate_store(&view).unwrap();
    let expected = fixture("repeat_abcabca");
    assert_same(&expected, &store_interchange(&view));
    drop(view);

    let (root, _) = handle.commit(root).unwrap();
    let graph = graph.read();
    graph.validate().unwrap();
    graph.validate_retention([root.index]).unwrap();
    assert_same(&expected, &graph.to_interchange());
}

#[test]
fn settle_cascades_into_a_committed_host() {
    let (base, n) = base();
    let expected = base.to_interchange();
    let graph = PositionedGraphRef::new(base);
    let mut handle = OverlayHandle::open(&graph);
    let mut view = handle.stage();
    let mut edit = GraphEdit::new(&mut view);
    let [ba, aba, bab] = insert_aba_bab(&mut edit, n);
    edit.finish();
    assert_eq!(view.vertex(ba).unwrap().parent_count(), 2);
    let root = lift::<StageIndex>(n.ababab);
    // `bab` first (largest index): `ba` drops to one parent occurrence and
    // is dissolved into `aba`; then `aba`.
    let mut candidates = Candidates::default();
    candidates.push(aba.index);
    candidates.push(bab.index);
    let mut edit = GraphEdit::new(&mut view);
    let settled = counts(edit.settle(candidates, root.index));
    let publication = edit.finish();
    assert_eq!(
        settled,
        Settled {
            candidates: 2,
            dissolved: 3,
            discarded: 0,
            into_root: 0,
            gained: Vec::new(),
        }
    );
    assert_eq!(root, lift(n.ababab));
    assert_eq!(view.staged().count(), 0);
    // `abab` is back to `[ab@0, ab@2]`; the removed `aba@0`, `bab@1` are
    // not recorded (their children are gone).
    let ab = lift::<StageIndex>(n.ab);
    let mut edits = publication.edits;
    edits
        .iter_mut()
        .for_each(|edit| edit.added.sort_by_key(|o| o.offset));
    assert_eq!(publication.created, vec![]);
    assert_eq!(
        edits,
        [VertexEdit {
            vertex: lift::<StageIndex>(n.abab).index,
            added: vec![occ(0, ab), occ(2, ab)],
            removed: vec![],
        }]
    );
    validate_store(&view).unwrap();
    assert_same(&expected, &store_interchange(&view));
    drop(view);
    let (_, translation) = handle.commit(root).unwrap();
    assert!(translation.is_empty());
    graph.read().validate().unwrap();
}

#[test]
fn settle_keeps_the_root_index() {
    let (base, n) = base();
    let graph = PositionedGraphRef::new(base);
    let mut handle = OverlayHandle::open(&graph);
    let mut view = handle.stage();
    let (a, ab, abab, ababab) = (
        lift::<StageIndex>(n.a),
        lift::<StageIndex>(n.ab),
        lift::<StageIndex>(n.abab),
        lift::<StageIndex>(n.ababab),
    );
    // Reading `abababa` after `ababab`: the insertion of `aba` (with `ba`,
    // `bab`), then the root `abababa` in the last slot.
    let mut edit = GraphEdit::new(&mut view);
    let [_, aba, _] = insert_aba_bab(&mut edit, n);
    let inserted = edit.finish();
    let mut edit = GraphEdit::new(&mut view);
    let root = edit.create(vec![occ(0, ababab), occ(4, aba)]).unwrap();
    let committed = edit.finish();
    assert_eq!(root.index, staged(3));
    let mut candidates = Candidates::default();
    candidates.record(&inserted);
    candidates.record(&committed);
    // `bab` (s2) is dissolved into `abab`; `ba` (s0) drops to one parent
    // occurrence (`aba@1`) and is dissolved into `aba`. No vertex moves.
    // `aba` keeps `abab@0` and `root@4`.
    let settled = settle(&mut view, candidates, root);
    assert_eq!(
        settled,
        Settled {
            candidates: 5,
            dissolved: 2,
            discarded: 0,
            into_root: 0,
            gained: Vec::new(),
        }
    );
    assert_eq!(root, Token::new(staged(3), 7));
    assert_eq!(aba.index, staged(1));
    assert_eq!(
        view.staged().map(StageIndex::from).collect::<Vec<_>>(),
        [aba.index, root.index]
    );
    assert_eq!(
        composition(&view, root.index),
        vec![occ(0, ababab), occ(4, aba)]
    );
    assert_eq!(composition(&view, aba.index), vec![occ(0, ab), occ(2, a)]);
    assert_eq!(
        composition(&view, abab.index),
        vec![occ(0, aba), occ(2, ab)]
    );
    validate_store(&view).unwrap();
    drop(view);

    let (root, _) = handle.commit(root).unwrap();
    let graph = graph.read();
    graph.validate().unwrap();
    graph
        .validate_retention([n.ababab.index, root.index])
        .unwrap();
    // The oracle graph of `ababab, abababa`.
    let cover = |entries: &[(usize, &str)]| -> Vec<(usize, String)> {
        entries.iter().map(|&(o, c)| (o, c.to_string())).collect()
    };
    let expected = PositionedGraphData {
        vertices: [
            ("a", cover(&[])),
            ("b", cover(&[])),
            ("ab", cover(&[(0, "a"), (1, "b")])),
            ("aba", cover(&[(0, "ab"), (2, "a")])),
            ("abab", cover(&[(0, "aba"), (2, "ab")])),
            ("ababab", cover(&[(0, "abab"), (2, "abab")])),
            ("abababa", cover(&[(0, "ababab"), (4, "aba")])),
        ]
        .into_iter()
        .map(|(key, cover)| (key.to_string(), cover))
        .collect(),
    };
    assert_same(&expected, &graph.to_interchange());
    assert_eq!(root.width.0, 7);
}

#[test]
fn settle_counts_dissolutions_into_the_root_and_discards() {
    let (base, n) = base();
    let graph = PositionedGraphRef::new(base);
    let mut handle = OverlayHandle::open(&graph);
    let mut view = handle.stage();
    let (a, b, ab) = (
        lift::<StageIndex>(n.a),
        lift::<StageIndex>(n.b),
        lift::<StageIndex>(n.ab),
    );
    // root `bab = [ba@0, ab@1]` (`ba` has one parent occurrence) and the
    // parentless `bb`.
    let mut edit = GraphEdit::new(&mut view);
    let ba = edit.create(vec![occ(0, b), occ(1, a)]).unwrap();
    let root = edit.create(vec![occ(0, ba), occ(1, ab)]).unwrap();
    edit.create(vec![occ(0, b), occ(1, b)]).unwrap();
    let mut candidates = Candidates::default();
    candidates.record(&edit.finish());
    let settled = settle(&mut view, candidates, root);
    assert_eq!(
        settled,
        Settled {
            candidates: 3,
            dissolved: 1,
            discarded: 1,
            into_root: 1,
            gained: Vec::new(),
        }
    );
    // `bb` (s2) is discarded, `ba` (s0) dissolved; the root stays in s1.
    assert_eq!(root, Token::new(staged(1), 3));
    assert_eq!(composition(&view, root.index), vec![occ(0, b), occ(1, ab)]);
    assert_eq!(view.staged().count(), 1);
    validate_store(&view).unwrap();
}

#[test]
#[should_panic(
    expected = "invalid vertex set: staged s0 has 1 parent occurrences"
)]
fn commit_rejects_an_invalid_vertex_set() {
    let (base, n) = base();
    let graph = PositionedGraphRef::new(base);
    let mut handle = OverlayHandle::open(&graph);
    let root = {
        let mut view = handle.stage();
        let mut edit = GraphEdit::new(&mut view);
        let (a, b, ab) = (lift(n.a), lift(n.b), lift(n.ab));
        let ba = edit.create(vec![occ(0, b), occ(1, a)]).unwrap();
        let bab = edit.create(vec![occ(0, ba), occ(1, ab)]).unwrap();
        edit.finish();
        bab
    };
    let _ = handle.commit(root);
}

#[test]
fn every_fixture_graph_satisfies_retention() {
    for fixture in fixture_graphs() {
        fixture
            .graph
            .validate_retention(
                fixture
                    .corpus
                    .iter()
                    .map(|input| fixture.names[input].index),
            )
            .unwrap_or_else(|error| panic!("{}: {error}", fixture.name));
    }
}

#[test]
fn retention_rejects_a_composite_with_one_parent_occurrence() {
    let mut graph: PositionedGraph = PositionedGraph::default();
    let mut edit = graph.edit();
    let (a, b, c) = (edit.atom('a'), edit.atom('b'), edit.atom('c'));
    let ab = edit.create(vec![occ(0, a), occ(1, b)]).unwrap();
    let abc = edit.create(vec![occ(0, ab), occ(2, c)]).unwrap();
    edit.finish();
    graph.validate().unwrap();
    assert_eq!(
        graph.validate_retention([abc.index]),
        Err(PositionedError::ParentCount {
            vertex: ab.index,
            count: 1,
        })
    );
    assert_eq!(graph.validate_retention([abc.index, ab.index]), Ok(()));
}
