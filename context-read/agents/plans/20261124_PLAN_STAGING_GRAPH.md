---
tags: `#plan` `#context-read` `#context-trace` `#context-insert` `#context-search` `#positioned-composition` `#reader` `#staging`
summary: Staging graph for the positioned reader, behavior-neutral infrastructure. Tokens created during a read are staged (content may change through the owned-root `extend`); everything else is committed (content immutable forever, only composition shape may change). The read commits its stage at input end. Replaces the root manager's implicit ownership flag by an explicit vertex state. The stores are abstracted by traits with typed vertex indices (CommittedIndex, StagedIndex, StageIndex) and vertex objects (GraphRead hands out VertexRead objects; Navigate on vertices; sealed GraphWrite/VertexWrite behind a generic GraphEdit; Stage; legacy-style HasGraph/HasGraphMut access traits whose guards are GraphRead/GraphWrite views; StageHandle with commit and Translation; StagingStrategy; shared HasVertexIndex/Wide/ToToken accessors), so search, insertion and the reader are written once. Two implementations are built side by side and benchmarked: (a) a physical overlay graph next to the committed graph; (b) a stage tag on vertices of the one graph. The reader's output does not change; dissolution of provisional tokens is the follow-up plan 20261124_PLAN_DISSOLUTION.
status: 📋 draft revision 5 — behavior-neutral (dissolution moved to 20261124_PLAN_DISSOLUTION); typed indices and vertex-object traits (§3); legacy trait patterns adopted (§3.10: HasGraph/HasGraphMut replace GraphHandle/FindLocked, index-like lookup, Wide/ToToken supertraits, located prefix/postfix chains); (a) overlay is the default and is implemented first (D-S1), (b) tag follows for comparison
date: 2026-11-24
parent: 20261122_PLAN_POSITIONED_READER.md (phase R4)
depends_on: 20261122_PLAN_POSITIONED_READER.md (R1–R3 implemented), 20261122_PLAN_POSITIONED_INSERT.md (revision 3, GraphEdit D8)
---

# Plan: Staging Graph

**Crates:** `context-trace` (graph, `GraphEdit`), `context-read` (reader),
`context-insert` and `context-search` (graph parameter made generic, §3).
**Reader:** [20261122_PLAN_POSITIONED_READER](20261122_PLAN_POSITIONED_READER.md)
(phase R4).
**Follow-up:** [20261124_PLAN_DISSOLUTION](20261124_PLAN_DISSOLUTION.md)
(phase R5): G6, `dissolve`, `settle`; the first change of the reader's
output, built on this plan.
**Insertion:** [20261122_PLAN_POSITIONED_INSERT](20261122_PLAN_POSITIONED_INSERT.md)
(invariants G1–G5 §3.1, `GraphEdit` D8, generation D9, no rollback D10).

## 0. Motivation

- **Implicit staging today.** The root manager is already a one-vertex
  staging area: an **owned** root (created by this read, parentless) is
  edited in place through `GraphEdit::extend`. Ownership is a flag in
  `RootManager`; the graph does not know it, and every other token the read
  creates is indistinguishable from corpus tokens.
- **Dissolution needs a boundary.** Provisional-retention deviations (10
  cases) are fixed by dissolving tokens the running read created
  ([20261124_PLAN_DISSOLUTION](20261124_PLAN_DISSOLUTION.md)). That is only
  sound if such tokens are identifiable and every other token is immutable.
- **Several stores.** Combining several vertex stores into one coherent
  graph (a stage next to the committed graph; later several concurrent
  readers) needs a store abstraction that search, insertion and the reader
  are written against.

This plan provides the boundary and the abstraction **without changing any
result**: every read produces the same graph as the current reader,
including the 10 known deviations, for both implementations. Behavior
changes are reserved for the dissolution plan.

## 1. Goals and non-goals

Goals:

- A strict boundary between **committed** tokens (string content immutable
  forever) and **staged** tokens (created by the running read; content may
  change while parentless, as the owned root today).
- Search and insertion see staged tokens through ordinary parent links, so
  the reader loop (reader plan §3.1) stays unchanged.
- Both implementations behind common traits (§3), compared by tests
  and benchmarks (§7); the trait layer is the basis for several stores or
  concurrent readers later.
- Extension points the dissolution plan builds on without changing the
  traits' signatures (removal, input mark, G6 at commit).
- A structure that a later "overhang" (a periodic pattern that grows during
  the search) can build on.

Non-goals:

- **No behavior change:** reader output, insertion results and search
  results stay identical; the expected deviations (reader plan §5,
  `READ_DEVIATIONS`, the 9 ignored scenarios) stay as they are.
- No removal of vertices, no G6, no `input` mark (dissolution plan).
- No change to the reader loop, the search algorithm or the insertion
  algorithm (only their graph parameter becomes generic, §3).
- No rollback (insert plan D10): a stage is always committed.
- No concurrent reads in this revision (one open stage per graph); the
  overlay implementation keeps the door open (§4.4).

## 2. Model (common to both options)

### 2.1 Vertex states

| | Committed | Staged |
|---|---|---|
| String content | **immutable forever** | may change: `extend` while parentless (as the owned root today) |
| Existence | permanent | until `commit` (removal: dissolution plan) |
| Composition shape | may change (insertion adds or replaces occurrences); content constant | may change |
| Invariants | G1–G5 | G1–G5 |
| Created by | `StageHandle::commit`; atoms in (b) at once; `insert` outside a read (insert gate only, no stage, §2.2) | `create` during an open stage (insertion, root commits); atoms in (a) |

Every committed vertex keeps its string: no edit removes it or changes its
width. This is what other reads and later inputs may rely on.

### 2.2 Stage lifecycle

```text
read(graph, chars):
  handle := S::open(graph)            # one per read (StagingStrategy, §3.7)
  ... reader loop (reader plan §3.1), unchanged
  handle.commit(root)                 # StageHandle::commit: all staged → committed; root translated
```

- Atoms are never extended (`extend` requires a composite). In (b) they are
  committed at once. In (a) the base is read-only for the whole read, so
  new atoms live in the stage and are translated at commit; atom lookup
  checks the stage first, then the base.
- Insertion outside a read uses no stage (Q3, decided). `read` is the only
  public interface; insertion is a building block of the reader, and the
  only use outside a read is the insert gate (tests). There it runs on the
  committed store (`PositionedGraphRef` as `HasGraphMut`) and edits it
  directly, as today; insertion never reworks a vertex, so nothing needs to
  stay mutable. Inside a read, the same generic `insert_match` runs on the
  read's `StageHandle`.
- Commit boundary: **input end**.

### 2.3 What changes for the reader

| Today | With a stage |
|---|---|
| `RootManager { root, owned }` | `ReadStage { handle, root }`; "owned" = editable = `state() == Staged`, parentless and composite |
| `extend` precondition `HasParents` | `NotStaged` or `HasParents` (same set of accepted calls: an owned root is exactly a staged parentless composite root, see below) |
| tokens created by insertion are committed at once | staged until `commit`; visible to search and insertion through the stage |
| `read()` returns a `Token` | `read()` returns the translated `Token<CommittedIndex>`; same vertex content, same graph |

Nothing else changes: same searches, same insertions, same commits of the
root (reader plan §3.4), same final graph.

**Why "editable" reproduces `owned` exactly.** `RootManager::editable` is
`owned && parentless` today; `owned` is set only by `Commit::InPlace` and
`Commit::NewRoot` (a composite created by this read) and cleared on
`Adopt`. With a stage:

- *Composite is required.* In (a) atoms new in this read are staged and
  parentless; an adopted first atom would count as editable and `extend`
  would fail with `PositionedError::Atom`. Today it is not owned, and
  `Append` creates a new root. The `composite` condition restores that.
- *Adopted composites are never editable.* Every staged composite except
  the current root has a parent: insertion creates tokens inside a host,
  and a replaced root becomes the prefix child of its successor
  (`NewRoot`) or lies inside the adopted token (`ReplaceRoot`). So an
  adopted staged composite has parents, and committed ones are not staged,
  as `owned = false` today.

The reader keeps an `owned` debug assertion until S4's gate is green
(`debug_assert_eq!(editable, owned)` on every commit), then drops the flag.

### 2.4 Walkthrough `abcabca`

| Step | Graph (`S` = staged) |
|---|---|
| segments | `a`, `b`, `c` committed |
| unknown `a`, `b`, `c` | root S1 `ab`, extended in place to `abc` |
| forward `abc` | finds S1 itself (no edit), `t = R`; new root S2 `[S1@0, S1@3]` |
| probe `[S1, a]` | matches `S1@0, a@3` in S2; insertion creates S3 `abca = [S1@0, a@3]`; S2 `[S3@0, S1@3]` |
| commit `Expand` | `extend(S2, S3@3)`: S2 `[S3@0, S3@3]` |
| `handle.commit(S2)` | S1, S2, S3 committed; S1 `abc` keeps a single parent occurrence: today's deviation, unchanged |

The [dissolution plan](20261124_PLAN_DISSOLUTION.md) adds a `settle` step
before the commit that dissolves `abc` (S1; its §2.5).

## 3. Store abstraction (traits)

Both implementations (§4, §5) are built side by side behind the same traits,
so search, insertion, the edit layer and the reader are written
once (and the dissolution plan's `settle`) and the benchmarks compare only the stores (§7).

Design rules (revision 3, extended in revision 5):

- **Legacy patterns first.** Where the legacy model already has a trait for
  the same concern (graph access, index-like lookup, width, token
  conversion, prefix/postfix chains), the new trait follows it and deviates
  only where the cover model or the overlay requires it (§3.10).
- **Typed indices.** Every store has its own index type. A committed index
  cannot address a stage's own vertices, and a staged index cannot address
  the committed graph (compile error). Stages accept both through one index
  type (`StageIndex`).
- **Vertex objects.** Single-vertex operations are not graph-level methods:
  the graph hands out a vertex object (`graph.vertex(i)`), and reads,
  navigation and (inside `context-trace`) writes are called on it.
- **Sealed writes.** Raw write primitives live in a trait that cannot be
  named outside `context-trace`; only `GraphEdit` calls them (D8).

### 3.1 Coupling today

- Search and paths: about 60 signatures in `context-search` and
  `context-trace/src/positioned/path/*` take `&PositionedGraph<G>`;
  `impl Find for PositionedGraph<G>` and `impl Find for PositionedGraphRef<G>`
  (`context-search/src/search/mod.rs`).
- Navigation (`token`, `start_at`, `end_at`, `prefix_child`,
  `parent_occurrences`, `atom_token`, `advance_end_path`) is inherent on
  `PositionedGraph` (`positioned/navigate.rs`); it only uses `vertex()`
  (width, composition, parents) and the atom index. `interval`,
  `contains_path` and `read_interval` (`graph.rs`) are used by the edit
  checks.
- `GraphEdit<'g, G>` (`positioned/edit.rs`) holds `&mut PositionedGraph<G>`
  and uses few raw primitives: push a vertex, set a node or width, link or
  unlink a parent, the atom index, the generation.
- `Token`, `Occurrence`, `Composition`, `OccurrenceLocation`, `Response`,
  `TraceCache` and the split graph store the untyped `VertexIndex`.
- `insert_match`/`insert_next_match` and the reader take
  `&PositionedGraphRef<G>` and lock per step (`find_ancestor` reads,
  `insert_match` writes).

### 3.2 Index types (context-trace)

```rust
/// Implemented only in context-trace (sealed).
pub trait VertexId: Copy + Eq + Ord + Hash + Debug + Send + Sync + sealed::Id {
    fn committed(self) -> Option<CommittedIndex>;
}

/// A vertex of the committed graph (today's `VertexIndex`).
pub struct CommittedIndex(usize);

/// A vertex created in the current stage; valid in no other stage and not
/// in the committed graph.
pub struct StagedIndex(usize);

/// Index space of a stage: either kind. Packed into one `usize` (top bit =
/// staged): same size as today, `state()` is a bit test.
pub struct StageIndex(usize);
impl StageIndex { pub fn split(self) -> Either<CommittedIndex, StagedIndex>; }
impl From<CommittedIndex> for StageIndex { … }      // always possible
impl From<StagedIndex> for StageIndex { … }
impl TryFrom<StageIndex> for CommittedIndex { … }   // committed vertices only
```

Index-carrying types become generic with the committed index as default:
`Token<I = CommittedIndex>`, `Occurrence<I>`, `Composition<I>`,
`VertexNode<A, I>`, `OccurrenceLocation<I>`, `Response<I>`,
`TraceCache<I>`. Search and insertion take `I` from the graph
(`V::Index`). Fixtures, interchange and `diff` stay on `CommittedIndex`.
The only way from a staged to a committed index is `Translation` (§3.6).

The shared accessors in `graph/vertex` (`HasVertexIndex`, `ToToken`) gain
the index type, following legacy `GetVertexIndex` (lookup by anything that
carries an index):

```rust
pub trait HasVertexIndex {
    type Index: VertexId;
    fn vertex_index(&self) -> Self::Index;
}
// for I: VertexId (itself), Token<I>, Occurrence<I>, vertex objects, &T, &mut T
pub trait ToToken: HasVertexIndex + Wide {
    fn to_token(&self) -> Token<Self::Index> { Token::new(self.vertex_index(), self.width()) }
}
impl<T: HasVertexIndex + Wide> ToToken for T {}
```

The legacy row model keeps its own untyped `VertexIndex` impls (behind the
`legacy` feature); only the positioned types use the associated index.

### 3.3 `GraphRead`, `VertexRead`, `CompositionRead` (context-trace)

```rust
pub trait GraphRead {
    type Kind: GraphKind;
    type Index: VertexId;
    type Vertex<'a>: VertexRead<'a, Kind = Self::Kind, Index = Self::Index> where Self: 'a;

    /// Lookup by anything carrying an index of this graph (index, token,
    /// occurrence, vertex object), as legacy `VertexSet<I: GetVertexIndex>`.
    /// A committed token is lifted with `.into()` before a stage lookup.
    fn vertex(&self, i: impl HasVertexIndex<Index = Self::Index>) -> Option<Self::Vertex<'_>>;
    fn atom(&self, value: &AtomOf<Self::Kind>) -> Option<Self::Vertex<'_>>;
    fn vertices(&self) -> impl Iterator<Item = Self::Vertex<'_>> + '_;
    fn generation(&self) -> Generation;
}
impl<T: GraphRead + ?Sized> GraphRead for &T { … }   // as legacy `HasGraph for &T`

/// A cheap `Copy` reference to one vertex. Carries the graph reference, so
/// it can navigate. Index, width and token come from the shared accessors
/// (`HasVertexIndex`, `Wide`, `ToToken`).
pub trait VertexRead<'a>: Copy + HasVertexIndex + Wide {
    type Kind: GraphKind;
    type Composition: CompositionRead<Index = Self::Index>;
    type Parents: Iterator<Item = (Token<Self::Index>, AtomPosition)>;

    /// Exactly one of atom value or composition.
    fn node(self) -> NodeRef<'a, AtomOf<Self::Kind>, Self::Composition>;
    fn parents(self) -> Self::Parents;                   // unordered
    fn parent_count(self) -> usize { self.parents().count() }
    fn state(self) -> VertexState;                       // Committed | Staged
    /// Another vertex of the same graph (child or parent).
    fn resolve(self, i: Self::Index) -> Option<Self>;
    #[cfg(debug_assertions)]
    fn label(self) -> &'a str;
}

/// Borrowed view of a vertex's node (read-side counterpart of the owned
/// `VertexNode<A, I>` used by `VertexWrite::set_node`).
#[derive(Clone, Copy, Debug)]
pub enum NodeRef<'a, A, C> {
    Atom(&'a A),
    Composite(C),
}
impl<'a, A, C: CompositionRead> NodeRef<'a, A, C> {
    /// Projections, like `Result::ok`.
    pub fn composite(self) -> Option<C>;
    pub fn atom(self) -> Option<&'a A>;
}

/// What navigation, insertion and the reader need from a composition
/// (today's `Composition` methods).
/// `Wide::width` is the end of the last occurrence (the owner's width).
pub trait CompositionRead: Copy + Wide {
    type Index: VertexId;
    fn len(self) -> usize;
    fn get(self, k: usize) -> Option<Occurrence<Self::Index>>;
    /// The unique occurrence at offset 0.
    fn prefix(self) -> Occurrence<Self::Index>;
    /// The unique occurrence ending at `width()` (the last one).
    fn postfix(self) -> Occurrence<Self::Index>;
    fn covering(self, p: AtomPosition) -> Option<Occurrence<Self::Index>>;
    fn ending_at(self, e: AtomPosition) -> Option<Occurrence<Self::Index>>;
    /// Occurrences strictly containing `p` (`offset < p < end`), a
    /// contiguous range; used by the split graph (insert plan rev 3).
    fn crossing(self, p: AtomPosition)
        -> impl ExactSizeIterator<Item = Occurrence<Self::Index>>;
    fn occurrences(self)
        -> impl DoubleEndedIterator<Item = Occurrence<Self::Index>> + ExactSizeIterator;
}
impl<I: VertexId> CompositionRead for &Composition<I> { … }  // direct slice access

/// Overlay view of an unchanged committed host: binary search on the base
/// slice, child indices lifted with `.into()`.
pub struct Lifted<'a>(&'a Composition<CommittedIndex>);
impl CompositionRead for Lifted<'_> { type Index = StageIndex; … }

pub enum VertexState { Committed, Staged }
/// D9 staleness. Committed store: `stage == 0`; overlay: base generation
/// at open plus the stage counter.
pub struct Generation { pub base: u64, pub stage: u64 }
```

- An overlay cannot return `&Composition<StageIndex>` for an unchanged
  committed host (its stored children are `CommittedIndex`), so
  compositions are read through `CompositionRead`. For `Lifted` every
  method is a binary search (or direct access) on the base slice, followed
  by `.into()` on the child indices.
- **No exclusive `Option`s:** atom value and composition are one
  `NodeRef`. The only `Option` left is `GraphRead::vertex(i)` (unknown
  index). Code handling both kinds (labels, interchange, validation,
  `GraphEdit` checks) matches on `node()`; navigation, which only descends
  through composites, uses `node().composite()?` (reaching an atom ends the
  descent).
- **`postfix`:** occurrence ends increase strictly with offsets
  (`Composition::new`), so the last occurrence is the unique one ending at
  the width, symmetric to `prefix`. `Composition` gains the inherent
  `postfix(&self) -> &Occurrence` (`self.0.last()`; never empty, a
  composition has ≥ 2 occurrences). It replaces the reader's
  `ending_at(width)` lookup.
- **`string_repr(v: impl VertexRead)`** (free function, legacy
  `HasVertexStringRepr`): the vertex's string, computed by expanding the
  composition down to atoms. Works for staged vertices and in release
  builds; no content is stored (expanded content stays debug-only, as
  `label()`). The overlay provides its own `TokenNameResolver` for
  `StagedIndex` labels (they change at `commit`).

### 3.4 `Navigate` (context-trace)

Navigation is called on the vertex object
(`graph.vertex(root)?.start_at(p)`); the bodies are today's
`navigate.rs` (`descend` becomes a private free function).

```rust
pub trait Navigate<'a>: VertexRead<'a> {
    fn start_at(self, p: AtomPosition)
        -> Option<(Token<Self::Index>, Vec<OccurrenceLocation<Self::Index>>)>;
    fn end_at(self, e: AtomPosition)
        -> Option<(Token<Self::Index>, Vec<OccurrenceLocation<Self::Index>>)>;
    /// `None` for atoms (end of the chain).
    fn prefix_child(self) -> Option<Occurrence<Self::Index>>;
    /// `None` for atoms (end of the chain).
    fn postfix_child(self) -> Option<Occurrence<Self::Index>>;
    /// Prefix chain largest first, down to an atom; `self` not included
    /// (search's prefix descent). Each item is the occurrence location in
    /// its predecessor on the chain and the prefix token, as legacy
    /// `HasTokenRoleIters::prefix_iter` yields `(ChildLocation, Token)`;
    /// folding the locations gives the path from `self` (legacy
    /// `prefix_path`).
    fn prefixes(self)
        -> impl Iterator<Item = (OccurrenceLocation<Self::Index>, Token<Self::Index>)>;
    /// Postfix chain largest first, down to an atom; `self` not included.
    /// Every postfix token of `self` lies on this chain (reader plan §3.2);
    /// the reader's expansion probes iterate it. Items as in `prefixes`
    /// (legacy `postfix_iter`).
    fn postfixes(self)
        -> impl Iterator<Item = (OccurrenceLocation<Self::Index>, Token<Self::Index>)>;
    /// Sorted by (parent width, parent index, offset), as today.
    fn parent_occurrences(self) -> impl Iterator<Item = (Token<Self::Index>, AtomPosition)>;
    fn advance_end_path(self, steps: Vec<OccurrenceLocation<Self::Index>>, p: AtomPosition)
        -> Option<(Token<Self::Index>, Vec<OccurrenceLocation<Self::Index>>)>;
    fn interval(self, …) -> …;
    fn contains_path(self, …) -> …;
}
impl<'a, V: VertexRead<'a>> Navigate<'a> for V {}
```

### 3.5 `GraphWrite` (sealed) and the generic `GraphEdit` (context-trace)

```rust
pub trait GraphWrite: GraphRead + raw::Write {}
impl<T: raw::Write> GraphWrite for T {}

mod raw {   // private module: not nameable, importable or implementable outside context-trace
    pub trait Write: super::GraphRead {
        type VertexMut<'a>: VertexWrite<Kind = Self::Kind, Index = Self::Index> where Self: 'a;
        fn vertex_mut(&mut self, i: Self::Index) -> Option<Self::VertexMut<'_>>;
        fn push(&mut self, node: VertexNode<AtomOf<Self::Kind>, Self::Index>, width: TokenWidth)
            -> Self::Index;                          // Staged inside a stage
        fn register_atom(&mut self, value: AtomOf<Self::Kind>, i: Self::Index);
        fn bump_generation(&mut self);
    }
    pub trait VertexWrite {
        type Kind: GraphKind;
        type Index: VertexId;
        fn set_node(&mut self, node: VertexNode<AtomOf<Self::Kind>, Self::Index>);
        fn set_width(&mut self, w: TokenWidth);
        fn link_parent(&mut self, parent: Self::Index, offset: AtomPosition);
        fn unlink_parent(&mut self, parent: Self::Index, offset: AtomPosition);
        #[cfg(debug_assertions)]
        fn set_label(&mut self, label: String);
    }
}
```

- Sealing alone would only prevent other crates from *implementing* the
  trait; putting the primitives into an unnameable supertrait also prevents
  them from *calling* them. D8 holds: only `GraphEdit` mutates.
- `GraphEdit` accesses one vertex at a time (set the host's node, drop the
  handle, then link each child).

```rust
pub struct GraphEdit<'g, S: GraphWrite + ?Sized> {
    graph: &'g mut S,
    created: Vec<Token<S::Index>>,
    edits: BTreeMap<S::Index, VertexEdit<S::Index>>,
    touched: BTreeSet<S::Index>,
    bumped: bool,
}

impl<'g, S: GraphWrite + ?Sized> GraphEdit<'g, S> {
    pub fn graph(&self) -> &S;
    pub fn atom(&mut self, value: AtomOf<S::Kind>) -> Token<S::Index>;
    pub fn create(&mut self, occurrences: Vec<Occurrence<S::Index>>)
        -> Result<Token<S::Index>, PositionedError>;
    pub fn add_occurrences(&mut self, v: S::Index, added: Vec<Occurrence<S::Index>>)
        -> Result<(), PositionedError>;
    /// Precondition: staged and parentless (`NotStaged` | `HasParents`);
    /// today's "owned root".
    pub fn extend(&mut self, v: S::Index, occurrence: Occurrence<S::Index>)
        -> Result<Token<S::Index>, PositionedError>;
    pub fn finish(self) -> Publication<S::Index>;
}
// new PositionedError variants: NotStaged,
// Atom (composite-only operation on an atom; replaces today's
// `Composition { error: TooFew }` from `GraphEdit::composite`)
```

`extend` and `add_occurrences` match `NodeRef::Composite` and return
`PositionedError::Atom` otherwise.

The state-dependent precondition of `extend` uses `VertexRead::state` and is
written once for both implementations. On the committed store (`state()` is
always `Committed`) `extend` is therefore always rejected; today's
`RootManager` only calls it on owned roots, which are staged in a stage.

Removal of vertices (`dissolve`, `discard`, a `remove` primitive) is not
part of this plan; the [dissolution plan](20261124_PLAN_DISSOLUTION.md) (§3 there) adds it to
these traits.

### 3.6 `HasGraph`, `HasGraphMut`, `Stage`, `StageHandle`, `Translation` (context-trace)

Access follows legacy `HasGraph`/`HasGraphMut`
(`context-trace/src/legacy/trace/has_graph.rs`): one trait abstracts *how*
a graph is reached (a plain reference or a lock), so search, tracing,
insertion and the reader take `G: HasGraph` (`G: HasGraphMut`) and work on
views and handles alike. The only deviation from legacy: the guards do not
`Deref` to a concrete store (an overlay view holds two guards, base read
lock and stage, and is no `PositionedGraph`); they *are* `GraphRead` /
`GraphWrite` views. Legacy `HasGraphMut` derefs to `&mut Hypergraph`; here
the write guard only exposes the sealed `GraphWrite` (§3.5), so D8 holds.
The new traits live in `context_trace::positioned`; the legacy traits of
the same name stay untouched in `context_trace::legacy` (no root
re-export, so the names do not collide).

```rust
pub trait HasGraph {
    type Kind: GraphKind;
    type Index: VertexId;
    type Guard<'a>: GraphRead<Kind = Self::Kind, Index = Self::Index> where Self: 'a;
    fn graph(&self) -> Self::Guard<'_>;
}
pub trait HasGraphMut: HasGraph {
    type GuardMut<'a>: GraphWrite<Kind = Self::Kind, Index = Self::Index> where Self: 'a;
    fn graph_mut(&mut self) -> Self::GuardMut<'_>;
}
impl<T: HasGraph + ?Sized> HasGraph for &T { … }          // as legacy
impl<T: HasGraph + ?Sized> HasGraph for &mut T { … }
impl<T: HasGraphMut + ?Sized> HasGraphMut for &mut T { … }

pub type KindOf<G> = <G as HasGraph>::Kind;              // legacy TravKind
pub type IndexOf<G> = <G as HasGraph>::Index;
pub type GraphAtom<G> = AtomOf<KindOf<G>>;               // legacy TravAtom

/// Explicit impls per implementer (no blanket over `GraphRead`, which would
/// overlap with the handle impls), generated by a macro as legacy
/// `impl_has_graph!`/`impl_has_graph_mut!`:
/// views (`PositionedGraph`, `OverlayView`, `TaggedView`): `Guard<'a> = &'a Self`,
///   `GuardMut<'a> = &'a mut Self`;
/// handles (`PositionedGraphRef`, `OverlayHandle`, `TaggedHandle`): guards are
///   lock views taken per call, as `PositionedGraphRef` locks per step today.

pub trait Stage: GraphWrite<Index = StageIndex> {
    fn staged(&self) -> impl Iterator<Item = StagedIndex> + '_;
}

/// Staged → committed, indexed by `StagedIndex`.
pub struct Translation(Vec<CommittedIndex>);
impl Translation {
    pub fn index(&self, i: StageIndex) -> CommittedIndex;  // committed: unchanged
    pub fn token(&self, t: Token<StageIndex>) -> Token<CommittedIndex>;
}

/// A handle with an open stage; the only way vertices cross the boundary.
pub trait StageHandle: HasGraphMut<Index = StageIndex> {
    /// The write guard as a `Stage`, named so callers avoid `for<'a>` bounds.
    type View<'a>: Stage<Kind = Self::Kind> where Self: 'a;
    fn stage(&mut self) -> Self::View<'_>;
    /// Takes the base write lock; staged → committed; for (a) asserts an
    /// unchanged base generation.
    fn commit(self, root: Token<StageIndex>)
        -> Result<(Token<CommittedIndex>, Translation), CommitError>;
}

pub enum CommitError {
    BaseChanged { opened: u64, now: u64 },
}
```

- `commit` lives on the handle, not on the view: an overlay view holds the
  base only read-locked and cannot write it.
- The root is held by the reader (`ReadStage`) and passed to `commit`; the
  stage does not store it. `commit` takes the root so that it is translated
  and so that the dissolution plan can mark it as input without changing
  the signature.

### 3.7 Search, insertion and reader (context-search, context-insert, context-read)

```rust
// context-search: one trait for views and handles, as legacy `Find: HasGraph`
pub trait Find: HasGraph {
    fn find_ancestor(&self, q: impl Searchable<Self::Kind, AncestorSearchTraversal>)
        -> SearchResult<Self::Index> { q.search(&self.graph()) }
}
impl<G: HasGraph + ?Sized> Find for G {}
```

A single blanket impl over `HasGraph` replaces the planned `Find` /
`FindLocked` pair (two blanket impls over views and handles would overlap);
call sites stay `graph.find_ancestor(q)`. `TraceCtx` and the search context
take `G: HasGraph` as legacy `TraceCtx<G: HasGraph>`.

```rust
// context-insert
pub fn insert_match<G: HasGraphMut>(graph: &mut G, response: Response<G::Index>)
    -> Result<InsertResult<G::Index>, InsertError>;
pub fn insert_next_match<G: HasGraphMut>(graph: &mut G, query: …) -> …;
// SplitGraph::build<V: GraphRead>, join<S: GraphWrite>(edit: &mut GraphEdit<'_, S>, …)

// context-read
pub trait StagingStrategy {
    type Handle<'g>: StageHandle<Kind = BaseGraphKind>;
    fn open<'g>(graph: &'g PositionedGraphRef) -> Self::Handle<'g>;
}
pub struct OverlayStaging;
pub struct TaggedStaging;
pub type DefaultStaging = OverlayStaging;   // D-S1

pub(crate) struct ReadStage<'g, S: StagingStrategy> {
    handle: S::Handle<'g>,
    root: Option<Token<StageIndex>>,
}
pub fn read_with<S: StagingStrategy>(graph: &PositionedGraphRef, input: impl IntoIterator<Item = char>)
    -> Option<Token<CommittedIndex>>;
pub fn read(graph: &PositionedGraphRef, input: impl IntoIterator<Item = char>)
    -> Option<Token<CommittedIndex>> { read_with::<DefaultStaging>(graph, input) }
```

### 3.8 Implementers

| Type | Crate | Implements | `Index` | Vertex object |
|---|---|---|---|---|
| `PositionedGraph<G>` and its `RwLock` guards | context-trace | `GraphRead`, `GraphWrite`, `HasGraph(Mut)` (guard `&Self`) | `CommittedIndex` | `&PositionedVertex` + graph reference |
| `PositionedGraphRef<G>` | context-trace | `HasGraph`, `HasGraphMut` (lock guards) | `CommittedIndex` | — |
| `StagingGraph<G>` | context-trace | storage only | — | — |
| `OverlayView<'a, G>` (a) | context-trace | `GraphRead`, `GraphWrite`, `Stage`, `HasGraph(Mut)` (guard `&Self`) | `StageIndex` | `enum { Base(&PositionedVertex, Option<&CowComposition>, &LinkDelta), Staged(&StagedVertex) }` |
| `OverlayHandle<'g, G>` (a) | context-trace | `HasGraph`, `HasGraphMut`, `StageHandle` | `StageIndex` | — |
| `TaggedView<'a, G>` (b) | context-trace | `GraphRead`, `GraphWrite`, `Stage`, `HasGraph(Mut)` (guard `&Self`) | `StageIndex` | `&PositionedVertex` + graph reference |
| `TaggedHandle<'g, G>` (b) | context-trace | `HasGraph`, `HasGraphMut`, `StageHandle` | `StageIndex` | — |
| vertex objects of all stores | context-trace | `VertexRead`, `Navigate`, `HasVertexIndex`, `Wide`, `ToToken` | the store's | — |
| `OverlayStaging`, `TaggedStaging` | context-read | `StagingStrategy` | — | — |
| `Find` for `G: HasGraph` | context-search | one blanket impl | `G::Index` | — |
| `SplitGraph`, `join`, `insert_match`, `insert_next_match` | context-insert | generic | from the graph | — |
| `ReadStage<S>`, `read_with` | context-read | uses `StageHandle` | `StageIndex` | — |

Both stages use `StageIndex`, so the reader and the `GraphEdit`
differential tests have identical types for both strategies, and the boundary is a
compile-time property in (b) as well: a staged token leaves a read only
through `commit`.

### 3.9 Costs and risks

- **Churn:** the index parameter reaches about 60 search and path
  signatures, `Response`, `TraceCache`, the split graph and the edit layer.
  Split into S1 (index-generic types with `CommittedIndex` default, no
  behavior change) and S2 (vertex objects and traits).
- **Runtime:** none expected for the committed store (the vertex object is
  a reference and inlines); the stage check is one bit test in both
  stages. Verified against the S0 baseline.
- **Ergonomics:** generic code over handles needs `for<'a>` bounds; the
  named associated types (`StageHandle::View`), the `KindOf`/`IndexOf`
  aliases and the `impl_has_graph!` macro keep call sites readable.

### 3.10 Relation to the legacy traits

Revision 5 reviewed the legacy trait set for concerns the store
abstraction shares with the row model. Adopted (with the deviation each
needs) or deliberately not adopted:

| Legacy | Here | Decision |
|---|---|---|
| `HasGraph`/`HasGraphMut` (`legacy/trace/has_graph.rs`), `TravKind`/`TravAtom`, `impl_has_graph!` | `HasGraph`/`HasGraphMut`, `KindOf`/`IndexOf`/`GraphAtom`, same macro (§3.6) | adopted; guards are `GraphRead`/`GraphWrite` views instead of `Deref<Target = Hypergraph>`, write guard sealed (D8). Replaces `GraphHandle` and `FindLocked` of revision 4 |
| `Find: HasGraph` (`context-search/src/legacy/search/mod.rs`) | `Find: HasGraph`, one blanket impl (§3.7) | adopted |
| `VertexSet<I: GetVertexIndex>`, `HasVertexIndex` | `GraphRead::vertex(impl HasVertexIndex<Index = Self::Index>)`; `HasVertexIndex::Index` associated (§3.2, §3.3) | adopted with typed indices; `with_vertex(_mut)` callbacks and cloned `VertexData` dropped (DashMap artifact; `with_vertex_mut` would bypass `GraphEdit`) |
| `Wide`, `ToToken` (`graph/vertex`) | supertraits of `VertexRead` and `CompositionRead`; `token()` from `ToToken` (§3.2, §3.3) | adopted; removes duplicated `index`/`width`/`token` methods |
| `HasTokenRoleIters::prefix_iter`/`postfix_iter` → `(ChildLocation, Token)` | `Navigate::prefixes`/`postfixes` → `(OccurrenceLocation, Token)` (§3.4) | adopted; `BandIterator`/`BandExpandingPolicy` dropped (row choice; prefix and postfix are unique in a cover) |
| `HasVertexStringRepr` | `string_repr(impl VertexRead)`, computed (§3.3) | adopted; no stored content |
| `TokenNameResolver` | per store; overlay resolves `StagedIndex` labels (§3.3) | adopted |
| `HasVertexData` (token → cloned `VertexData`) | `graph.vertex(t)` | not adopted (clones; vertex objects replace it) |
| `GetVertexKey`/`HasVertexKey`/`VertexKey` | `Translation` | not adopted: a stable key would avoid translation but costs a key → index lookup per access; typed indices give the compile-time boundary |
| path accessors and mutators (`HasRootedPath`, `HasStartPath`/`HasEndPath`, `PathAppend`/`Pop`/`Raise`/`Lower`/`Simplify`, `MovePath`, `Advance`, …) | one `RolePath<R, Root>` with inherent methods | not adopted (one path type; position arithmetic stays in `path/role.rs`); S1 only makes them index-generic |
| insert traits (`ToInsertCtx`, `ResultExtraction`, `TryInitWith`, partition/mode traits) | insertion takes only a `Response` | not adopted |
| `IntoReadInput`/`ToNewAtomIndices` | `impl IntoIterator<Item = char>` | deferred until non-`char` atoms are needed |

## 4. Implementation (a): physical overlay graph

### 4.1 Structures

```text
StagingGraph {
  vertices: Vec<StagedVertex>,                        # by StagedIndex (Slab in the dissolution plan)
  atoms: Map<Atom, StagedIndex>,                      # atoms new in this read
  compositions: Map<CommittedIndex, Composition<StageIndex>>,  # copy-on-write shape of committed hosts
  added_parents: Map<CommittedIndex, Set<(StageIndex, AtomPosition)>>,  # committed children
  removed_parents: Map<CommittedIndex, Set<(StageIndex, AtomPosition)>>,
  base_generation: u64,                               # base generation when opened
  generation: u64,
}
OverlayHandle<'g> { base: &'g PositionedGraphRef, stage: Mutex<StagingGraph> }
OverlayView<'a>   { base: RwLockReadGuard<'a, PositionedGraph>, stage: MutexGuard<'a, StagingGraph> }
```

The view holds the base read-locked even for writes; only
`OverlayHandle::commit` takes the base write lock. `removed_parents` is
needed already without dissolution: insertion replaces occurrences in
committed hosts (unlinks children).

### 4.2 Index space

Typed indices (§3.2): the overlay's `Index` is `StageIndex` (top bit set =
staged), so `state()` is one bit test and `StageIndex` has the size of
today's `VertexIndex`. Base vertices are reached through
`CommittedIndex::into()`; an unchanged committed host's composition is
returned as `Lifted` (§3.3). `commit` builds the `Translation`; the reader
translates the only token it holds, the root.

### 4.3 Operations

- Read (`vertex(i)` → `OverlayVertex`): staged index → stage; committed
  index → stage override for the composition if present, else the base
  composition as `Lifted`; parents = base links − removed + added.
- Write (`vertex_mut(i)`): only into the stage; for a committed index the
  vertex object writes the copy-on-write composition and the link deltas.
  The base is never touched before `commit`.
- `commit` (takes the base write lock): assert the base generation equals
  `base_generation`; push staged vertices children first, rewriting
  occurrences through the translation; apply composition overrides and link
  changes to committed vertices.
- Index order: staged vertices are pushed in `StagedIndex` order, so
  committed indices keep the creation order, as the current reader assigns
  them. Children-first holds because a staged vertex's children are
  created before it.

### 4.4 Properties

- Explicit interface: staged vertices live in another type; the base is
  read-only for the whole read; other readers never see a stage.
- Costs: one stage check per vertex access, merged parent iteration,
  commit with translation, memory of the stage until commit.
- Concurrency (out of scope): several overlays over one base; a conflicting
  commit (two stages create the same string, G1) needs a rebase rule.

## 5. Implementation (b): logical stage tag

### 5.1 Changes

- `PositionedVertex { state: VertexState, … }`;
  `PositionedGraph { stage: Option<StageId>, … }`.
- `TaggedHandle` holds the `PositionedGraphRef` and the open `StageId`;
  `TaggedView` (a write guard plus the stage id) writes into the base;
  `push` tags new vertices `Staged`.
- Index: `StageIndex` as in (a). The top bit mirrors the vertex tag: a
  staged vertex in slot `i` is addressed as `StagedIndex(i)`, a committed
  one as `CommittedIndex(i)`. Tags change only at commit, when all indices
  are translated, so the bit never goes stale. The `Translation` only clears
  the bit.
- `commit` flips the tags of the stage's vertices.

### 5.2 Properties

- No lookup overhead beyond the tag; commit is linear in the staged
  vertices without copying.
- The boundary is enforced by the index types at compile time (staged
  tokens leave only through `commit`) and by `GraphEdit` preconditions on
  the vertex state at runtime.
- Other readers would see staged vertices and shape changes of committed
  hosts (fine with one read at a time).

## 6. Reader integration

- `RootManager { root, owned }` becomes `ReadStage<'g, S: StagingStrategy>
  { handle, root: Option<Token<StageIndex>> }`; "owned" becomes "staged,
  parentless and composite" (§2.3). The commit table (reader plan §3.4) is unchanged.
- `read_with::<S: StagingStrategy>(graph, input)`: `S::open`, the reader
  loop, `handle.commit(root)`, return the translated root
  (`Token<CommittedIndex>`).
- `read(graph, input)` keeps its signature and uses `DefaultStaging`
  (`OverlayStaging`, D-S1).
- `insert` outside a read (insert gate only) uses no stage (§2.2).
- `GraphEdit::extend` precondition: staged and parentless (replaces
  `HasParents`).
- Tests stay as they are: the 9 scenarios ignored for provisional
  retention stay ignored, `READ_DEVIATIONS` stays unchanged (both are
  removed by the dissolution plan).

## 7. Comparison and evaluation

Both implementations are kept and tested. (a) is the default; the
evaluation decides whether (b) stays as an alternative and gives the
performance numbers for a later revision of D-S1.

| | (a) overlay | (b) tag |
|---|---|---|
| Interface | explicit (separate store type; typed indices) | one store; typed indices, state preconditions at runtime |
| Lookup cost | stage check per access, merged parents | none |
| Commit cost | copy and translate | flip tags |
| Isolation | yes | no |
| Memory | stage freed at commit | none extra |
| Overhang (future) | stage is a natural scratch space | possible, less isolated |

**D-S1 (decided 2026-11-24):** (a) overlay, the separate vertex store, is
the default and is implemented first. (b) is implemented afterwards behind
the same traits for the benchmark comparison. The dissolution plan reruns
the comparison with `settle` (its §6).

### 7.1 Benchmarks (criterion, `context-read/benches/staging.rs`)

- Baseline: the current reader, measured before task S1 and kept as a
  saved criterion baseline (`--save-baseline pre-staging`); the old code
  path is not kept alive for later comparisons.
- Per strategy: reader throughput (atoms per second) on the v3 corpus
  inputs and on long periodic inputs (`p^n t`, large `n`).
- `find_ancestor` on committed graphs through `PositionedGraph` and through
  an empty `OverlayView` (cost of the indirection alone).
- Commit cost relative to read cost; peak memory.

### 7.2 Tests for both

- **Behavior neutrality** is the existing test baseline: the same tests
  pass and fail against the same oracle fixtures as before (the 9 ignored
  scenarios stay ignored, `READ_DEVIATIONS` unchanged). No new fixtures, no
  recorded reader graphs.
- A macro instantiates every reader scenario and the read lane for
  `OverlayStaging` and `TaggedStaging` with these expectations.
- `insert_gate` unchanged on the committed store (insertion outside a
  read uses no stage).
- Unit tests of `GraphEdit` preconditions (`extend` on committed vertices
  rejected, `extend` on a staged vertex with parents rejected), `commit`
  (translation, generation check for (a)).

## 8. Open questions

1. ~~D-S1~~ — decided: (a) overlay is the default and is implemented
   first (§7).
2. ~~Standalone insertion~~ — decided: no stage; insertion is not public,
   only `read` is (§2.2).
3. ~~Index space of (a)~~ — decided (revision 3): typed indices
   `CommittedIndex`, `StagedIndex`, packed `StageIndex` for both stages
   (§3.2).

No open questions remain in this plan. Commit boundary, `settle` timing
and the proofs belong to the dissolution plan.

## 9. Tasks

| Task | Content | Exit |
|---|---|---|
| S0 | Benchmark baseline of the current reader (§7.1), saved as `pre-staging` | numbers recorded here |
| S1 | Typed indices (§3.2): `VertexId`, `CommittedIndex`, `StagedIndex`, `StageIndex`; `Token<I>`, `Occurrence<I>`, `Composition<I>`, `VertexNode<A, I>`, `OccurrenceLocation<I>`, `Response<I>`, `TraceCache<I>` with `CommittedIndex` default; `HasVertexIndex::Index` associated, `ToToken` returns `Token<Self::Index>`, `Wide` for the index-generic types; no behavior change | all gates green (incl. `--features legacy-tests`); benchmarks within noise of S0 |
| S2 | Traits (§3.3–§3.7, §3.10): `GraphRead` (index-like lookup) / `VertexRead: HasVertexIndex + Wide` with `NodeRef`, `CompositionRead: Wide` (incl. `postfix`, `crossing`) and `Composition::postfix`, `Navigate` on vertex objects (incl. `postfix_child`, located `prefixes`/`postfixes`; the reader's `expand::postfixes` and its fixture test move here), `string_repr`, `PositionedError::Atom`, sealed `raw::Write`/`VertexWrite`, generic `GraphEdit<S>`, `HasGraph`/`HasGraphMut` with `KindOf`/`IndexOf` and `impl_has_graph!`, `Find: HasGraph`; `PositionedGraph`, its guards and `PositionedGraphRef` implement them; search, tracing, paths and insertion take `G: HasGraph(Mut)` | all gates green; benchmarks within noise of S0 |
| S3 | (a): `StagingGraph`, `OverlayHandle`/`OverlayView`, `Lifted`, `Stage`, `StageHandle`, commit with `Translation`; `extend` precondition `NotStaged` | unit tests (§7.2) |
| S4 | Reader: `ReadStage`, `read_with`, `read` on `OverlayStaging` | gate (§10) for (a) |
| S5 | (b): state tag, `TaggedHandle`/`TaggedView`; `TaggedStaging` | unit tests; differential tests against (a) on `GraphEdit` sequences; gate (§10) for (b) |
| S6 | Benchmarks per strategy (§7.1) | numbers recorded here (input for a later revision of D-S1) |

## 10. Gate

```sh
cargo check -p context-trace -p context-search -p context-insert -p context-read --no-default-features --features test-api
cargo test -p context-read --no-default-features --features test-api
cargo test -p context-trace -p context-search -p context-insert --no-default-features --features test-api
cargo test --release -p context-insert --no-default-features --features test-api insert_gate
cargo test -p context-read --features legacy-tests
cargo bench -p context-read
```

- Every test that passes today passes for both strategies; the same tests
  are ignored and `READ_DEVIATIONS` is unchanged.
- `validate()` after every read and every insert case.
- Insert gate unchanged (all cases).
- S1 and S2 are performance-neutral against the S0 baseline; the strategy
  costs are recorded in S6.

## 11. Results

### S0 — pre-staging baseline (2026-11-24)

`context-read/benches/staging.rs` (criterion 0.5, `bench` profile,
warm-up 1 s, measurement 3 s, 30 samples; periodic group 10 samples),
saved as criterion baseline `pre-staging`:

```sh
cargo bench -p context-read --no-default-features --features test-api --bench staging -- --save-baseline pre-staging
```

| Benchmark | Input | Time (median) | Throughput |
|---|---|---|---|
| `read/corpus/all` | all 94 v3 scenarios, each read into a fresh graph (1171 atoms) | 1.316 ms | 890 Katoms/s |
| `read/periodic/abc^n_x/64` | `"abc"·64 + "x"` (193 atoms) | 828 µs | 233 Katoms/s |
| `read/periodic/abc^n_x/256` | `"abc"·256 + "x"` (769 atoms) | 36.4 ms | 21.2 Katoms/s |
| `read/periodic/abc^n_x/512` | `"abc"·512 + "x"` (1537 atoms) | 333 ms | 4.6 Katoms/s |
| `find_ancestor/corpus_inputs` | every corpus input searched in its committed v3 fixture graph (`PositionedGraph`, 199 queries) | 869 µs | 229 Kqueries/s |

- Periodic reads grow roughly cubically in `n` (×44 for 64 → 256, ×9.2
  for 256 → 512); `n = 1024` takes 2.76 s per read and was dropped from the
  bench to keep its runtime short.
- Run-to-run noise on this machine (two consecutive runs): below 1 %.
