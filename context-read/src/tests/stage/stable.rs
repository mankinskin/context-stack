//! Stability marks of staged vertices (cheap stabilization plan 20261126
//! §5.3): a flag on the vertex (no index moves), no removal of stable
//! vertices, `GraphEdit::stabilize` with Rule S′ and the worklist
//! `GraphEdit::stabilize_candidates` (events E1 and E2).

use context_insert::edit::EditExt as _;
use pretty_assertions::assert_eq;

use super::{
    dissolve::{
        Slots,
        composition,
        parents,
        staged_bab_last,
        unlink_children,
    },
    overlay::{
        base,
        lift,
        occ,
    },
};
use crate::{
    stage::{
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
            has_vertex_index::HasVertexIndex as _,
            StagedIndex,
            StageIndex,
        },
    },
    positioned::{
        diff,
        GraphRead,
        interchange::{
            PositionedGraphData,
            store_interchange,
        },
        PositionedError,
        PositionedGraph,
        PositionedGraphRef,
        raw::{
            Remove as _,
            SEAL,
            Stabilize as _,
        },
        string_repr,
        validate_store,
        VertexRead,
    },
    Token,
};
use context_insert::{
    edit::{
        Candidates,
        GraphEdit,
        Stage,
    },
};

fn staged(slot: usize) -> StageIndex {
    StagedIndex::new(slot).into()
}

fn sorted(mut parents: Vec<(StageIndex, usize)>) -> Vec<(StageIndex, usize)> {
    parents.sort();
    parents
}

fn assert_same(
    expected: &PositionedGraphData,
    actual: &PositionedGraphData,
) {
    let difference = diff(expected, actual);
    assert!(difference.is_empty(), "{difference}");
}

/// [`staged_bab_last`] on a fresh stage.
fn with_slots<R>(f: impl FnOnce(&mut OverlayHandle<'_>, Slots) -> R) -> R {
    let (base, n) = base();
    let graph = PositionedGraphRef::new(base);
    let mut handle = OverlayHandle::open(&graph);
    let c = handle.atom('c').unwrap();
    let slots = staged_bab_last(&mut handle.stage(), n, c);
    f(&mut handle, slots)
}

/// Stabilizing sets a flag: no vertex moves, no reference changes and the
/// content is unchanged. Removing an unstable vertex leaves the stable ones
/// in place, and the stage commits.
#[test]
fn stabilize_sets_a_flag() {
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
    let ab = lift::<StageIndex>(n.ab);
    let before = store_interchange(&view);
    let generation = view.generation().stage;

    view.stabilize(SEAL, bab.index);
    assert_eq!(view.generation().stage, generation + 1);
    assert!(view.is_stable(bab.index) && !view.is_stable(ba.index));
    assert_eq!(bab.index, staged(4));
    assert_eq!(composition(&view, bab.index), vec![occ(0, ba), occ(1, ab)]);
    assert_eq!(
        parents(&view, ba.index),
        sorted(vec![(aba.index, 1), (bab.index, 0)])
    );
    validate_store(&view).unwrap();
    assert_same(&before, &store_interchange(&view));

    view.stabilize(SEAL, aba.index);
    assert_eq!(stable_strings(&view), ["aba", "bab"]);

    // Removing the unstable `bc` leaves a hole; the stable vertices stay.
    unlink_children(&mut view, bc.index);
    view.remove(SEAL, bc.index);
    assert!(view.vertex(bc.index).is_none());
    assert!(view.is_stable(bab.index) && view.is_stable(aba.index));
    assert!(!view.is_stable(bc.index));
    validate_store(&view).unwrap();
    drop(view);
    let (_, translation) = handle.commit(baba).unwrap();
    assert_eq!(translation.len(), 4);
    let graph = graph.read();
    graph.validate().unwrap();
}

/// Stable vertices are never removed (store-enforced).
#[test]
#[should_panic(expected = "cannot be removed")]
fn remove_of_a_stable_vertex_panics() {
    with_slots(|handle, Slots { bc, .. }| {
        let mut view = handle.stage();
        // The raw stabilization does not check the parent count.
        view.stabilize(SEAL, bc.index);
        unlink_children(&mut view, bc.index);
        view.remove(SEAL, bc.index);
    });
}

/// Precondition errors leave the stage unchanged, in particular
/// `FixedParentCount` (Rule S′): `bc` has no parent occurrence, `baba` only
/// `ababab@1` (committed). In [`staged_bab_last`], `bab` has `baba@0`
/// (strictly inside) and `abab@1` (committed): it is stabilized;
/// stabilizing twice is a no-op.
#[test]
fn graph_edit_stabilize_checks() {
    with_slots(|handle, Slots { baba, bc, bab, .. }| {
        let mut view = handle.stage();
        let before = store_interchange(&view);
        let generation = view.generation();
        let (a, ab) = (
            Token::<StageIndex>::new(view.atom(&'a').unwrap().vertex_index(), 1),
            Token::<StageIndex>::new(
                view.vertices()
                    .find(|v| string_repr(*v) == "ab")
                    .unwrap()
                    .vertex_index(),
                2,
            ),
        );
        let mut edit = GraphEdit::new(&mut view);
        assert_eq!(
            edit.stabilize(ab.index, baba.index),
            Err(PositionedError::NotStaged(ab.index))
        );
        assert_eq!(
            edit.stabilize(a.index, baba.index),
            Err(PositionedError::Atom(a.index))
        );
        assert_eq!(
            edit.stabilize(baba.index, baba.index),
            Err(PositionedError::IsRoot(baba.index))
        );
        assert_eq!(
            edit.stabilize(bc.index, baba.index),
            Err(PositionedError::FixedParentCount {
                vertex: bc.index,
                count: 0,
            })
        );
        assert_eq!(
            edit.stabilize(baba.index, bc.index),
            Err(PositionedError::FixedParentCount {
                vertex: baba.index,
                count: 1,
            })
        );
        edit.finish();
        assert_eq!(view.generation(), generation);
        assert_eq!(stable_strings(&view), Vec::<String>::new());
        assert_same(&before, &store_interchange(&view));

        let mut edit = GraphEdit::new(&mut view);
        assert_eq!(edit.stabilize(bab.index, baba.index), Ok(true));
        assert_eq!(edit.stabilize(bab.index, baba.index), Ok(false));
        edit.finish();
        assert_eq!(stable_strings(&view), ["bab"]);
        assert_same(&before, &store_interchange(&view));
    });
}

/// End occurrences count only in committed or stable parents: `ab` has
/// `abab@0` (inside) and `abab@2` (at the end of the unstable `abab`), so
/// it is rejected; `abab` has only end occurrences, both in committed hosts,
/// and is stabilized; then `ab` is accepted.
#[test]
fn end_occurrences_count_in_fixed_parents_only() {
    with_hosts(|view, Hosts { ab, abab, root }| {
        let before = store_interchange(view);
        let mut edit = GraphEdit::new(view);
        assert_eq!(
            edit.stabilize(ab.index, root.index),
            Err(PositionedError::FixedParentCount {
                vertex: ab.index,
                count: 1,
            })
        );
        assert_eq!(edit.stabilize(abab.index, root.index), Ok(true));
        assert_eq!(edit.stabilize(abab.index, root.index), Ok(false));
        assert_eq!(edit.stabilize(ab.index, root.index), Ok(true));
        edit.finish();
        assert_eq!(stable_strings(view), ["ab", "abab"]);
        validate_store(&*view).unwrap();
        assert_same(&before, &store_interchange(view));
    });
}

fn view_parents(
    edit: &GraphEdit<'_, OverlayView<'_>>,
    index: StageIndex,
) -> usize {
    edit.graph().vertex(index).unwrap().parent_count()
}

/// The stage of [`with_hosts`].
struct Hosts {
    /// `a@0, b@1`, slot 0; parents `abab@0` (inside), `abab@2` (end).
    ab: Token<StageIndex>,
    /// `ab@0, ab@2`, slot 1.
    abab: Token<StageIndex>,
    /// Parentless, slot 2.
    root: Token<StageIndex>,
}

/// The committed `xabab = x@0, a@1, b@2, a@3, b@4` and `yabab` (same with
/// `y`), and the atoms.
fn hosts() -> PositionedGraph {
    let mut graph = PositionedGraph::default();
    let mut edit = graph.edit();
    let a = edit.atom('a');
    let b = edit.atom('b');
    for first in ['x', 'y'] {
        let first = edit.atom(first);
        edit.create(vec![occ(0, first), occ(1, a), occ(2, b), occ(3, a), occ(4, b)])
            .unwrap();
    }
    edit.finish();
    graph
}

fn committed(
    view: &OverlayView<'_>,
    string: &str,
) -> Token<StageIndex> {
    let vertex = view
        .vertices()
        .find(|v| string_repr(*v) == string)
        .unwrap();
    Token::new(vertex.vertex_index(), string.chars().count())
}

/// On [`hosts`]: the staged `ab` and `abab`, `abab` inserted at the end of
/// both committed hosts (`xabab@1`, `yabab@1`), and an unrelated parentless
/// staged root `xa`.
fn with_hosts<R>(f: impl FnOnce(&mut OverlayView<'_>, Hosts) -> R) -> R {
    let graph = PositionedGraphRef::new(hosts());
    let mut handle = OverlayHandle::open(&graph);
    let mut view = handle.stage();
    let (a, b, x) = (committed(&view, "a"), committed(&view, "b"), committed(&view, "x"));
    let (xabab, yabab) = (committed(&view, "xabab"), committed(&view, "yabab"));
    let mut edit = GraphEdit::new(&mut view);
    let ab = edit.create(vec![occ(0, a), occ(1, b)]).unwrap();
    let abab = edit.create(vec![occ(0, ab), occ(2, ab)]).unwrap();
    let root = edit.create(vec![occ(0, x), occ(1, a)]).unwrap();
    for host in [xabab, yabab] {
        edit.add_occurrences(host.index, vec![occ(1, abab)]).unwrap();
    }
    edit.finish();
    validate_store(&view).unwrap();
    f(&mut view, Hosts { ab, abab, root })
}

/// The strings of the stable staged vertices, in slot order.
fn stable_strings(view: &OverlayView<'_>) -> Vec<String> {
    view.staged()
        .map(StageIndex::from)
        .filter(|&index| view.is_stable(index))
        .map(|index| string_repr(view.vertex(index).unwrap()))
        .collect()
}

/// E2: `abab` (two end occurrences in committed hosts) is stabilized, then
/// its child `ab` (`abab@0` inside, `abab@2` now in a stable parent)
/// through the cascade. The root is not touched.
#[test]
fn stabilize_candidates_cascades() {
    with_hosts(|view, Hosts { abab, ab, root }| {
        let before = store_interchange(view);
        let mut edit = GraphEdit::new(view);
        assert_eq!(view_parents(&edit, ab.index), 2);
        let stabilized = edit.stabilize_candidates([abab.index], root.index);
        edit.finish();
        assert_eq!(stabilized, 2);
        assert_eq!(stable_strings(view), ["ab", "abab"]);
        assert!(!view.is_stable(root.index));
        assert_eq!(string_repr(view.vertex(root).unwrap()), "xa");
        validate_store(&*view).unwrap();
        assert_same(&before, &store_interchange(view));
    });
}

/// The staged `ab` and `abab` in the root `abab·x·abab` only: `abab` has
/// `root@0` (inside, fixed) and `root@5` (at the root's end, not fixed);
/// `ab` has `abab@0` (inside) and `abab@2` (end of the unstable `abab`).
/// One fixed occurrence each: nothing is stabilized.
#[test]
fn stabilize_candidates_skips_end_occurrences_in_unstable_parents() {
    let graph = PositionedGraphRef::new(hosts());
    let mut handle = OverlayHandle::open(&graph);
    let mut view = handle.stage();
    let (a, b, x) = (committed(&view, "a"), committed(&view, "b"), committed(&view, "x"));
    let mut edit = GraphEdit::new(&mut view);
    let ab = edit.create(vec![occ(0, a), occ(1, b)]).unwrap();
    let abab = edit.create(vec![occ(0, ab), occ(2, ab)]).unwrap();
    let root = edit
        .create(vec![occ(0, abab), occ(4, x), occ(5, abab)])
        .unwrap();
    edit.finish();
    let mut edit = GraphEdit::new(&mut view);
    let stabilized =
        edit.stabilize_candidates([abab.index, ab.index, root.index], root.index);
    assert_eq!(
        edit.stabilize(abab.index, root.index),
        Err(PositionedError::FixedParentCount {
            vertex: abab.index,
            count: 1,
        })
    );
    edit.finish();
    assert_eq!(stabilized, 0);
    assert_eq!(stable_strings(&view), Vec::<String>::new());
}

/// Case (i): two occurrences strictly inside the (unstable) root count.
/// In the root `ab·x·ab·y` (`abxaby`, as after reading `abxaby`), `ab` is
/// stabilized.
#[test]
fn occurrences_inside_the_root_are_fixed() {
    let graph = PositionedGraphRef::new(hosts());
    let mut handle = OverlayHandle::open(&graph);
    let mut view = handle.stage();
    let (a, b, x, y) = (
        committed(&view, "a"),
        committed(&view, "b"),
        committed(&view, "x"),
        committed(&view, "y"),
    );
    let mut edit = GraphEdit::new(&mut view);
    let ab = edit.create(vec![occ(0, a), occ(1, b)]).unwrap();
    let root = edit
        .create(vec![occ(0, ab), occ(2, x), occ(3, ab), occ(5, y)])
        .unwrap();
    edit.finish();
    let mut edit = GraphEdit::new(&mut view);
    assert_eq!(edit.stabilize_candidates([ab.index], root.index), 1);
    edit.finish();
    assert_eq!(stable_strings(&view), ["ab"]);
    assert!(!view.is_stable(root.index));
    validate_store(&view).unwrap();
}

/// `settle` then `stabilize_candidates` in one edit (as the reader runs
/// them): the discard of the parentless `ba` (slot 2) leaves a hole; the
/// gained candidate `abab` (E1: added to both committed hosts) keeps slot 3
/// and is stabilized, then `ab`.
#[test]
fn settle_keeps_gained_candidates() {
    let graph = PositionedGraphRef::new(hosts());
    let mut handle = OverlayHandle::open(&graph);
    let mut view = handle.stage();
    let (a, b, x) = (committed(&view, "a"), committed(&view, "b"), committed(&view, "x"));
    let (xabab, yabab) = (committed(&view, "xabab"), committed(&view, "yabab"));
    let mut edit = GraphEdit::new(&mut view);
    let ab = edit.create(vec![occ(0, a), occ(1, b)]).unwrap();
    let root = edit.create(vec![occ(0, x), occ(1, a)]).unwrap();
    let ba = edit.create(vec![occ(0, b), occ(1, a)]).unwrap();
    let abab = edit.create(vec![occ(0, ab), occ(2, ab)]).unwrap();
    for host in [xabab, yabab] {
        edit.add_occurrences(host.index, vec![occ(1, abab)]).unwrap();
    }
    let mut candidates = Candidates::default();
    candidates.record(&edit.finish());
    assert!(candidates.gained().contains(&abab.index));
    assert_eq!((ab.index, root.index, ba.index, abab.index), (staged(0), staged(1), staged(2), staged(3)));

    let mut edit = GraphEdit::new(&mut view);
    let mut settled = edit.settle(candidates, root.index);
    assert_eq!(settled.discarded, 1);
    // Created `ab`, the root and `abab`; the discarded `ba` is dropped.
    assert_eq!(settled.gained, vec![staged(0), staged(1), staged(3)]);
    let stabilized =
        edit.stabilize_candidates(std::mem::take(&mut settled.gained), root.index);
    edit.finish();
    assert_eq!(stabilized, 2);
    assert!(view.vertex(ba.index).is_none());
    assert_eq!(stable_strings(&view), ["ab", "abab"]);
    assert_eq!(string_repr(view.vertex(root).unwrap()), "xa");
    validate_store(&view).unwrap();
}

/// E1 inside `settle`: `aab = a@0, ab@1` has one parent occurrence
/// (`root@0` in the root `aab·x`) and is dissolved into the root. Its child
/// `ab` ended at the end of `aab` (not fixed) and now ends inside the root
/// (`root@1`); with the committed `xabab@3` it has two fixed parent
/// occurrences and is stabilized.
#[test]
fn settle_dissolution_gains() {
    let graph = PositionedGraphRef::new(hosts());
    let mut handle = OverlayHandle::open(&graph);
    let mut view = handle.stage();
    let (a, b, x) = (committed(&view, "a"), committed(&view, "b"), committed(&view, "x"));
    let xabab = committed(&view, "xabab");
    let mut edit = GraphEdit::new(&mut view);
    let ab = edit.create(vec![occ(0, a), occ(1, b)]).unwrap();
    edit.add_occurrences(xabab.index, vec![occ(3, ab)]).unwrap();
    let aab = edit.create(vec![occ(0, a), occ(1, ab)]).unwrap();
    let root = edit.create(vec![occ(0, aab), occ(3, x)]).unwrap();
    edit.finish();
    let mut edit = GraphEdit::new(&mut view);
    assert_eq!(
        edit.stabilize(ab.index, root.index),
        Err(PositionedError::FixedParentCount {
            vertex: ab.index,
            count: 1,
        })
    );
    edit.finish();

    let mut candidates = Candidates::default();
    candidates.push(aab.index);
    let mut edit = GraphEdit::new(&mut view);
    let mut settled = edit.settle(candidates, root.index);
    assert_eq!(settled.dissolved, 1);
    // `aab` (slot 1) was removed; the root keeps slot 2.
    assert_eq!(root.index, staged(2));
    assert_eq!(settled.gained, vec![ab.index]);
    let stabilized =
        edit.stabilize_candidates(std::mem::take(&mut settled.gained), root.index);
    edit.finish();
    assert_eq!(stabilized, 1);
    assert_eq!(stable_strings(&view), ["ab"]);
    assert_eq!(
        composition(&view, root.index),
        vec![occ(0, a), occ(1, ab), occ(3, x)]
    );
    validate_store(&view).unwrap();
}
