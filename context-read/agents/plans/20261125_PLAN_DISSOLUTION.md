---
tags: `#plan` `#context-read` `#context-trace` `#positioned-composition` `#reader` `#retention` `#dissolution` `#staging`
summary: Implementation plan for the dissolution of provisional tokens (reader plan phase R5, roadmap step 5). The positioned reader keeps tokens that a prefix of the input made frequent but the rest of the input made redundant (`abcabca` keeps `abc`). Fix - retention invariant G6 (every committed vertex that is neither an atom nor an input has ≥ 2 parent occurrences), restored after every reader step by an eager, local `settle` that dissolves or discards staged tokens with ≤ 1 parent occurrence. Candidates come from the step's edit records; removal is immediate (swap-remove with renaming); new atoms are committed at once; committing an invalid vertex set panics. Tagged staging and the strategy abstraction are removed first. Exact code changes per task (D2–D7), unit and integration tests, per-step oracle lane and gate.
status: 📋 ready for implementation — all design decisions final except the early-commit interface (E-I, §4.6, task D7 last). Supersedes 20261124_PLAN_DISSOLUTION_OLD.md (revisions 1–3.2).
date: 2026-11-25
parent: 20261122_PLAN_POSITIONED_READER.md (§5 provisional retention, phase R5)
depends_on: 20261124_PLAN_STAGING_GRAPH.md (implemented S0–S6), 20261122_PLAN_POSITIONED_INSERT.md (invariants G1–G5 §3.1, GraphEdit D8, no rollback D10)
---

# Plan: Dissolution of provisional tokens

This file is self-contained: it states the problem, the model, every code
change and the validation. Background: reader plan
[20261122_PLAN_POSITIONED_READER](20261122_PLAN_POSITIONED_READER.md) (§3
reader loop, §5 provisional retention), insert plan
[20261122_PLAN_POSITIONED_INSERT](20261122_PLAN_POSITIONED_INSERT.md)
(§3.1 invariants G1–G5), staging plan
[20261124_PLAN_STAGING_GRAPH](20261124_PLAN_STAGING_GRAPH.md) (overlay
stage, `ReadStage`). The design history is in the superseded
[20261124_PLAN_DISSOLUTION_OLD](20261124_PLAN_DISSOLUTION_OLD.md); nothing in it
is needed to implement this plan.

## 0. Overview

| Task | Content | Changes behavior |
|---|---|---|
| D2 | Remove tagged staging and the `StagingStrategy` abstraction (§4.1) | no |
| D3 | Overlay: new atoms committed immediately; swap-remove of staged vertices (§4.2) | no |
| D4 | `GraphEdit::dissolve` / `discard` / `settle`, candidates, commit check, `validate_retention`, test helpers (§4.3) | no (not called yet) |
| D5 | Reader: collect edit records, `settle` after every step; un-ignore the 9 scenarios (§4.4) | **yes: fixes the bug** |
| D6 | Benchmarks recorded (§4.5) | no |
| D7 | Early commit (§4.6), after decision E-I and D1b | no (adds marks) |
| D1b | Proofs M1–M5, C1 and the maximal-repeat prototype (§3.6); parallel, blocks only D7 | — |

Order: D2 → D3 → D4 → D5 → D6; D1b in parallel; D7 last. Every task ends
with the gate of §5.4 green. Done when every scenario, every read-lane
case and every intermediate step equals its oracle graph.

## 1. Problem

The reader inserts every accepted match at once (legacy rule). A token
that a prefix of the input makes frequent can lose that status later in
the same input: all its occurrences end up inside occurrences of one
larger token, and the oracle does not retain it.

**Example `abcabca`.** After `abcabc`, `abc` occurs twice (root
`[abc@0, abc@3]`) and is correctly a token. The final `a` makes `abca` a
token at offsets 0 and 3; every occurrence of `abc` now lies inside an
`abca`. The oracle graph has `abca = [a@0, b@1, c@2, a@3]` and no `abc`.
Current output:

```sh
cargo test -p context-read --no-default-features --features test-api \
  --test reader_behavior overlay::repeat_abcabca -- --ignored
```

```text
repeat_abcabca, final corpus graph: graph differs from expected:
  extra vertices: "abc"
  "abca": missing ["a"@0, "b"@1, "c"@2] extra ["abc"@0]
```

No bounded look-ahead decides this: in `p^n t` (`t` a proper prefix of
`p`), whether `p` stays a token is decided by the first atom of `t`,
`(n − 1)·|p|` positions after `p` was created. The decision must be
revised backwards: a token created earlier in the read is removed again.
The staging graph makes exactly these tokens (created by the running
read, not committed yet) identifiable and mutable.

**Affected today** (`context-read/tests/reader_behavior.rs`):

- 9 scenarios `#[ignore = "provisional retention (reader plan §5)"]`:
  `repeat_abcabca`, `repeat_ababa`, `insert_periodic`, `period_ab_x3_a`,
  `period_abc_x3_a`, `period_abc_x3_ab`, `period_abcd_x2_a`,
  `period_abcd_x3_ab`, `period_abcd_x2_abc`;
- 1 read-lane case in `READ_DEVIATIONS`: `insert_deep_borders__1`.

All deviations are extra tokens created by the current read; all other
94 − 9 scenarios and 140 − 1 read cases equal the oracle.

## 2. Current implementation (what the plan builds on)

**Reader** (`context-read/src`). `read` → `read_steps`:

1. `segments(handle, input)` (`segment.rs`) splits the input into runs of
   unknown and known atoms and creates the unknown atoms up front through
   `GraphEdit::atom` on the stage view (today: staged atoms).
2. Unknown atom: `commit(Span::Append(atom))`.
3. Known run, per step: `expand` (`expand.rs`) probes the root's postfix
   chain largest first with `insert_next_match([P] ++ rest)` (no probe at
   cursor 0 of a run; rejected probes do not edit); the first match
   longer than `P` is inserted as the expansion `E` and yields
   `Span::Expand { postfix, expansion }` or, if `R[..o] + E` is a token,
   `Span::ReplaceRoot`. Otherwise `forward` (`forward.rs`):
   `insert_next_match(rest)` (longest occurring prefix `t`), then
   `ReplaceRoot([R, t])` if that is a token, else `Append(t)`.
4. `ReadStage::commit(span)` (`root.rs`): `Adopt` (first `Append`,
   `ReplaceRoot`; no edit), `InPlace` (editable root = staged,
   parentless, composite: `GraphEdit::extend`), `NewRoot`
   (`GraphEdit::create(maximal([R@0, x@o]))`). Its `GraphEdit::finish()`
   record is dropped.
5. `ReadStage::finish` → `StageHandle::commit(root)`.

Insertion (`context-insert/src/insert/mod.rs`) returns
`InsertOutcome::{Complete, NoExpansion, Created { token, response,
publication }}`; only `Created` edits (one `GraphEdit`). The reader calls
`.token()` and drops the `Publication`.

**Stage** (`context-trace/src/positioned/stage`). Two implementations
behind `StagingStrategy` (`context-read/src/staging.rs`): overlay
(`stage/overlay/`, default) and tagged (`stage/tagged/`). Overlay:
`OverlayHandle { base: &PositionedGraphRef, stage: RefCell<StagingGraph> }`;
`StagingGraph { vertices: Vec<StagedVertex>, atoms, deltas:
StageMap<CommittedIndex, BaseDelta>, base_generation, generation }`;
`StagedVertex { width, node, parents: Links<StageIndex>, label }`;
`BaseDelta { composition: Option<Composition<StageIndex>>, added:
Links<StageIndex>, removed: Links<CommittedIndex> }` (copy-on-write
changes of committed vertices; `added` holds only links to staged
parents). `commit_into` pushes the staged vertices in slot order
(`Translation` table) and applies the deltas.

**Edits** (`context-trace/src/positioned/edit`). `GraphEdit<'g, S:
GraphWrite> { graph, created, edits: BTreeMap<_, VertexEdit>, touched,
bumped }`; operations `atom`, `create`, `add_occurrences`, `extend`;
`recompose` updates a composition and the children's links and records
`VertexEdit { vertex, added, removed }` for vertices that existed before
the edit (occurrences removed and re-added cancel). `finish()` validates
the touched vertices (debug, `test-api`) and returns `Publication {
created, edits }`. Raw writes are sealed (`store/write.rs`, `raw::Write`
with `Seal`).

## 3. Model

### 3.1 Retention invariant G6, inputs and enforcement

> **G6.** Every committed vertex that is neither an atom nor a corpus
> input has at least two parent occurrences (entries `(parent, offset)`).

A vertex with a single parent occurrence `(U, o)` occurs only inside `U`
at offset `o` (L1, Appendix A): it has no context of its own and the
oracle does not retain it. G6 holds on all 2339 oracle graphs (v3, insert
and read lanes; test `every_oracle_graph_satisfies_retention` in
`context-read/tests/fixture_validation.rs`).

**Inputs.** An input is the whole string of one `read` call. `read` is
the only public interface, so only reads create inputs; insertion never
does, and the insert gate (insertion outside a read) does not check G6.
There is **no stored input mark** and no `VertexRead::is_input`: inside a
stage the only staged vertex that can be an input is the stage root
(known by identity to `settle` and `commit`); committed vertices are never
dissolved, so nothing at runtime needs to know whether a committed vertex
is an input. A committed composite without parents is an input; an input
may also keep exactly one parent occurrence (corpus `bc, abcd, abce`, then
reading `xabcd` creates `abc` and both occurrences of `bc` move into it).

**Enforcement:**

1. **Staged vertices:** `settle` (§3.3) after every reader step dissolves
   or discards every staged composite other than the root with ≤ 1 parent
   occurrence.
2. **Commit:** `StageHandle::commit(root)` panics if a staged composite
   other than `root` has < 2 parent occurrences (a bug of `settle`). This
   is the only runtime G6 check.
3. **Committed graph:** checked only in tests,
   `PositionedGraph::validate_retention(inputs)` after every read, with
   the inputs passed explicitly.

### 3.2 Dissolution and discard

```text
dissolve(t):  t staged, composite, ≠ root, exactly one parent occurrence (U, o)
  comp(U) := maximal(comp(U) − t@o ∪ (comp(t) + o))
  unlink every child link (t, a); the children kept in comp(U) are linked (U, o + a)
  remove t

discard(t):   t staged, composite, ≠ root, no parent occurrence
  unlink every child link (t, a); remove t
```

- `U` may be staged or committed; a committed `U` changes only its
  composition shape (copy-on-write in its `BaseDelta`), never its content.
- G1–G5 are preserved (T1); only occurrences from `comp(t) + o` can be
  filtered by `maximal`. `discard` changes no composition.
- **Removal is immediate (swap-remove).** The last staged slot `l` moves
  into the slot of `t`; every reference to `l` is renamed in the same
  call: parent links of its children (staged child: `StagedVertex::parents`;
  committed child: `BaseDelta::added`) and compositions of its parents
  (staged parent: its node; committed parent: `BaseDelta::composition`).
  Cost `O(|comp(l)| + p(l))`. Result `Moved { from: l, to: t }` (none if
  `t` was last). No tombstones.
- Removal happens only inside `settle`; every index recorded during a
  step is valid when `settle` starts. Across steps the reader holds only
  the root (renamed by `settle`) and committed atoms (never move).
- Staged slot order is no longer creation order. Nothing depends on it.

### 3.3 `settle`

**Invariant (S):** every staged composite other than the root has ≥ 2
parent occurrences. It holds at the start of every step; `settle`
restores it at the end of every step (all span kinds, also `Adopt`).

```text
settle(K, root):
  while let Some(t) = K.pop():
    if t is committed or t = root: continue      # atoms are committed (§4.2)
    match p(t):                                   # parent occurrences
      ≥ 2: keep
      1:   r := dissolve(t); K ∪= r.children
      0:   r := discard(t);  K ∪= r.children
    apply r.moved to K and root                   # drop t from K, rename l → t
```

Terminates (each removal removes a vertex); the result is independent of
the order (T2). Afterwards the stage equals the oracle graph of the
consumed prefix `oracle(C + [I[..cursor]])` (M4; checked per step, §5.3).

**Candidates `K` of a step** (from the step's edit records):

```text
K := { o.child | P ∈ records of the step, e ∈ P.edits, o ∈ e.removed }  # lost a link
   ∪ { t.index | P ∈ records of the step, t ∈ P.created }               # new
   ∪ { previous root, if the step changed the root index }               # no longer exempt
```

Records of a step: the `Publication` of the accepted insertion
(`InsertOutcome::Created` from `expand` or `forward`) and of the root
commit's `GraphEdit`.

**Exactness.** A parent count drops only by losing a link of a vertex
that existed before the edit (a link to a vertex created in the same edit
is gained first: net ≥ 0); these losses are exactly the `removed`
occurrences of the records. So after a step a violator of (S) is a child
in some `removed` list, a token created in the step, or the previous root;
every other staged composite had ≥ 2 after the previous `settle` and lost
nothing. Inside `settle`, a removal lowers only the counts of the removed
vertex's children (L2), which are added to `K`. Debug and `test-api`
builds assert (S) by a full scan after every `settle`.

**Locality (C1, cost only).** A link `x@k` filtered from `comp(U)` by an
added occurrence `y@j` belongs to a child of `y` or to a descendant of `y`
through vertices created in the same step. So `K` is bounded by the
step's new compositions, not by the stage size.

### 3.4 Reader decisions are unchanged

Dissolving a staged token with ≤ 1 parent occurrence changes no reader
decision (Appendix A: L3–L6, RM):

| Decision | Depends on | Effect of dissolving `t` |
|---|---|---|
| forward `insert_next_match(rest)` | string of the longest occurring prefix (`occ(G)` unchanged by L1) | none; at most `Complete` becomes `Created` |
| probe `[t] ++ rest`, `t` a postfix of `R`, only parent `U ≠ R` | `U` is a larger postfix, probed first | `[t] ++ rest` matches only if `[U] ++ rest` does (L4) |
| probe `[t] ++ rest`, only parent `R` | — | always rejected (L6, by Lemma RM); dissolution removes one rejected probe |
| `find_complete([R, t'])` | existence of a token by string | if dissolved, a new root with the same string is created |
| root, `P`, `E` within one step | identities | `settle` runs between steps; the root is exempt |

**Lemma RM (the root is right-maximal).** At the start of every probing
step, `R + rest[0]` occurs in no vertex: the root either has no parent
occurrence (`InPlace`, `NewRoot`), or it ends with a longest match of the
search that produced it (`Adopt` in its three forms). So dissolving into
the root is allowed for every root, with or without parents.

### 3.5 Walkthrough `abcabca` (after D3–D5)

Empty graph. `segments` commits `a`, `b`, `c` at once.

| Step | Graph (S = staged) | Records → `K` → `settle` |
|---|---|---|
| unknown `a` | `Append(a)`: adopt `a` | none |
| unknown `b` | `NewRoot`: S1 `ab = [a@0, b@1]` | created {S1} = root → skip |
| unknown `c` | `InPlace`: S1 `abc = [a, b, c]` | S1 is root → skip |
| forward `abc` | `insert_next_match` finds S1 (`Complete`); `[S1, S1]` no token: `NewRoot` S2 `[S1@0, S1@3]` | created {S2} (root), previous root S1: p = 2, keep |
| probe `[S1] ++ "a"` | insertion creates S3 `abca = [S1@0, a@3]` inside S2: S2 `[S3@0, S1@3]` | insert record: created {S3}, S2 removed {S1@0} |
| commit `Expand` | `InPlace`: S2 `[S3@0, S3@3]` | commit record: S2 removed {S1@3}. `K` = {S3, S1}: S3 p = 2 keep; S1 p = 1 (`S3@0`): dissolve into S3 → S3 `[a@0, b@1, c@2, a@3]`; swap-remove moves S3 into S1's slot, root S2 unchanged |
| `commit(S2)` | valid; equals the oracle | — |

Without `settle`, S1 `abc` is committed with one parent occurrence: the
current deviation.

### 3.6 Maximal-repeat characterization (task D1b)

Observed on 199 oracle graphs of scenario prefixes and 3544 random
corpora, 0 differences:

> `oracle(C) = atoms(C) ∪ inputs(C) ∪ MR(C)`

`MR(C)`: strings of ≥ 2 atoms with ≥ 2 distinct left contexts and ≥ 2
distinct right contexts over their occurrences in the inputs (input start
and end count as contexts distinct from every atom and from each other).
D1b proves (prototype `maximal_repeats.py` + written proofs):

| # | Statement | Used for |
|---|---|---|
| M1 | the characterization above | reduces correctness to vertex sets |
| M2 | in such a graph a non-atom, non-input vertex has p ≥ 2 iff it is in MR | G6 ⇔ maximal repeats |
| M3 | G4 / G5 for such vertex sets | invariants without the oracle |
| M4 | one step plus `settle` maps the vertex set of `C + [I[..c]]` to that of `C + [I[..c']]` | per-step oracle equality (§5.3) |
| M5 | `MR(C) ⊆ MR(C + [I])` for finished inputs | committed non-inputs keep p ≥ 2 |
| C1 | locality of losses (§3.3) | cost bound |

D3–D6 do not depend on D1b: the per-step oracle lane (§5.3) checks M4
empirically. D7 relies on M2, M4, M5.

## 4. Code changes

Paths are relative to the repository root. "Remove" means delete the code
and fix every compile error by deleting the dependent code, not by
replacing it.

### 4.1 D2 — remove tagged staging and `StagingStrategy` (behavior-neutral)

context-trace:

- Delete `context-trace/src/positioned/stage/tagged/` (`mod.rs`,
  `handle.rs`, `view.rs`, `vertex.rs`, `composition.rs`) and
  `context-trace/src/positioned/tests/stage/tagged.rs` (plus `mod tagged;`
  in `tests/stage/mod.rs`).
- `positioned/stage/mod.rs`: remove `pub mod tagged;` and the tagged
  docs; `Translation` becomes a table only (remove `Map::Untag`,
  `Translation::untag`; `Translation(Vec<CommittedIndex>)`, `slots`,
  `index`, `token`, `len`, `is_empty` keep their signatures).
- `positioned/mod.rs`: remove the tagged re-exports (`TaggedLabels`,
  `TaggedComposition`, `TaggedHandle`, `TaggedVertex`, `TaggedView`) and
  the module doc lines about the tagged stage.
- `positioned/committed/graph.rs`: remove the field
  `stage: Option<StageId>` and its import; `positioned/committed/access.rs`:
  remove the "committed edit while a tagged stage is open" assertion.
- `positioned/model/vertex.rs`: remove `PositionedVertex::state` (field and
  accessor); committed `VertexRead::state` (`committed/access.rs`) returns
  `VertexState::Committed`; drop the `state:` initializers
  (`committed/access.rs`, `stage/overlay/staging.rs`). `VertexState` and
  `VertexRead::state` stay (overlay vertices report `Staged`).
- `positioned/tests/guard.rs`: remove `("stage", "Option<StageId>")` and
  `("state", "VertexState")` from the allowlists.
- `positioned/stage/overlay/handle.rs`: remove the "a tagged stage is
  open" assertion in `open`.
- `graph/vertex/id.rs`: remove `StageIndex::tagged`, and `StageIndex::slot`
  if unused afterwards.

context-read:

- Delete `src/staging.rs`; `lib.rs`: export only `Commit`, `Step`,
  `StepKind`, `read`, `read_traced`.
- `read.rs`: remove `read_with`; `read` and `read_traced` call
  `read_steps(graph, input, observe)` (no type parameter).
- `root.rs`: `pub(crate) struct ReadStage<'g> { pub(crate) handle:
  OverlayHandle<'g>, root: Option<Token<StageIndex>> }`, `open` calls
  `OverlayHandle::open`; test alias `type Stage<'g> = ReadStage<'g>`.
- `tests/reader_behavior.rs`: remove the `strategies!` macro, `dump` and
  `strategies_commit_identical_graphs`; `with_scenarios!(scenarios)` at
  top level (one `#[test]` per scenario, names without `overlay::`);
  `assert_scenario`, `read_case`, `assert_read_gate` non-generic on
  `context_read::read`; one `#[test] fn read_gate()`; update the module
  doc.
- Benches: `benches/staging.rs` — remove the `_tagged` ids
  (`all_tagged`, `abc^n_x_tagged`, `corpus_inputs_tagged`),
  `find_ancestor_tagged`, the strategy generics and the `pre-staging`
  baseline doc (keep `corpus_inputs_overlay`: search through an empty
  overlay view); `benches/staging_commit.rs` — remove
  `commit_tagged` and the generic; `benches/staging_memory.rs` — remove the
  tagged rows and the generic. Use `context_read::read`.

Exit: gate §5.4 green; the same 9 scenarios ignored and the same
`READ_DEVIATIONS`.

### 4.2 D3 — overlay: immediate atoms and swap-remove (behavior-neutral)

**Immediate atoms** (`stage/overlay/handle.rs`, `view.rs`, `staging.rs`):

```rust
impl<'g, G: GraphKind> OverlayHandle<'g, G> {
    /// The atom for `value`; a new atom is created in the committed graph
    /// at once (no view may be alive). Err if the base changed since open.
    pub fn atom(&mut self, value: AtomOf<G>) -> Result<Token<StageIndex>, CommitError>;
}
```

- Existing atom (`base.atoms`): return it lifted.
- Else: `let mut base = self.base.write()`; if `base.generation !=
  stage.base_generation` return `Err(CommitError::BaseChanged {..})`;
  `let mut edit = base.edit(); let token = edit.atom(value);
  edit.finish();` then `stage.base_generation = base.generation`, so the
  commit check still detects foreign edits.
- Remove `StagingGraph::atoms`; `OverlayView::atom` looks up the base only;
  `raw::Write::push` with `VertexNode::Atom` and `register_atom` on the
  overlay view panic ("atoms are committed through
  `OverlayHandle::atom`"); `commit_into` handles only composites. So
  `GraphEdit::atom` on a stage returns existing atoms and panics on new
  ones (a bug).
- `context-read/src/segment.rs`: `segments(handle: &mut OverlayHandle<'_>,
  chars)` creates unknown atoms with `handle.atom(c)` (panic on `Err`, like
  `finish`), with no view alive; `Segment` holds committed atoms (still
  typed `Token<StageIndex>`). Update its tests (`generation().stage`
  stays 0, the base generation grows).
- Overlay unit tests (`positioned/tests/stage/overlay.rs`) that create
  atoms through `edit.atom` on a stage use `handle.atom`.

**Swap-remove** (`store/write.rs`, `stage/mod.rs`, overlay):

```rust
// store/write.rs, inside `pub(crate) mod raw`
pub trait Remove: Write {
    /// Remove a staged vertex without parent links whose children no longer
    /// link to it (checked by GraphEdit). Swap-remove: the last staged
    /// vertex moves into the freed slot and every reference to it is
    /// renamed before returning.
    fn remove(&mut self, seal: Seal, index: StageIndex) -> Option<Moved>;
}

// stage/mod.rs
pub trait Stage: GraphWrite<Index = StageIndex> + raw::Remove {
    /// The staged vertices in slot order.
    fn staged(&self) -> impl Iterator<Item = StagedIndex> + '_;
}

/// A staged vertex moved from slot `from` to slot `to` by a removal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Moved { pub from: StageIndex, pub to: StageIndex }
impl Moved {
    pub fn apply(self, index: StageIndex) -> StageIndex; // from → to, else unchanged
}
```

`OverlayView::remove(t)`: `debug_assert!` no parents; `let last =
vertices.len() - 1; vertices.swap_remove(t.slot())`; if `t.slot() !=
last`, rename `l = StagedIndex(last)` → `t`:

- for every child occurrence of the moved vertex: staged child → move the
  key `l` to `t` in its `parents`; committed child → the same in
  `deltas[child].added`;
- for every parent `(p, offset)` of the moved vertex: staged parent →
  `composition.map_index(l → t)`; committed parent → the same on
  `deltas[p].composition` (present: a committed vertex references a staged
  child only through its delta);
- bump `stage.generation`.

Docs: `Stage::staged`, `StagingGraph::vertices` say "slot order".

Exit: unit tests §5.1 (D3); gate green; reader output unchanged.

### 4.3 D4 — `GraphEdit` dissolution, `settle`, checks

New file `context-trace/src/positioned/edit/dissolve.rs` with
`impl<S: Stage + ?Sized> GraphEdit<'_, S>`; types re-exported from
`positioned` (`Candidates`, `Removal`, `Settled`, `Moved`).

```rust
impl<S: Stage + ?Sized> GraphEdit<'_, S> {
    /// §3.2. Errors (graph unchanged): NotStaged (committed), Atom,
    /// IsRoot, ParentCount { vertex, count } (count ≠ 1).
    pub fn dissolve(&mut self, t: StageIndex, root: StageIndex)
        -> Result<Removal, PositionedError<StageIndex>>;
    /// Same checks with count ≠ 0.
    pub fn discard(&mut self, t: StageIndex, root: StageIndex)
        -> Result<Removal, PositionedError<StageIndex>>;
    /// §3.3; renames `root` on a move.
    pub fn settle(&mut self, candidates: Candidates, root: &mut Token<StageIndex>)
        -> Settled;
}

#[derive(Clone, Debug, Default)]
pub struct Candidates(Vec<StageIndex>);
impl Candidates {
    /// Children of `removed` occurrences and the created tokens.
    pub fn record(&mut self, publication: &Publication<StageIndex>);
    /// A vertex that lost its exemption (the previous root).
    pub fn push(&mut self, vertex: StageIndex);
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Removal {
    pub removed: StageIndex,         // index at removal time
    pub host: Option<StageIndex>,    // dissolve: U (after renaming); discard: None
    pub children: Vec<StageIndex>,   // after renaming
    pub moved: Option<Moved>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Settled { pub candidates: usize, pub dissolved: usize, pub discarded: usize, pub into_root: usize }
```

- **`dissolve`:** checks; `(U, o)` = the only parent occurrence; new
  composition `maximal(comp(U) − t@o ∪ comp(t) + o)` through `recompose(U,
  …)` (unlinks `t@o`, links the added children); then `unlink(t, a)` for
  every child of `t`; then `self.graph.remove(SEAL, t)`; `mutate()` first.
- **`discard`:** checks; unlink every child; remove.
- **Bookkeeping after a removal** (both): remove `t` from `touched`,
  `edits` and `created`; apply `moved` to `touched`, the keys and the
  occurrence children of `edits`, and `created`, so `finish()` validates
  only existing vertices.
- **`settle`:** worklist from `candidates` (deduplicated); skip committed
  indices (`!is_staged()`) and `root.index`; `parent_count()` ≥ 2 keep,
  1 dissolve, 0 discard (an `Err` is a bug: panic); after a removal drop
  `t` from the worklist, apply `moved` to the worklist and `root.index`,
  push the renamed children; count `into_root` when the host is the root.
  At the end, under `cfg(any(debug_assertions, feature = "test-api"))`,
  assert (S) by a full scan of `self.graph.staged()`.
- **`Publication`** is unchanged: `dissolve` / `discard` return their
  `Removal`, `settle` renames the root itself and returns counts. The
  record of a `settle` edit is not used.
- **Errors** (`model/error.rs`): `ParentCount { vertex: I, count: usize }`,
  `IsRoot(I)`, with `Display` arms.
- **Commit check** (`stage/overlay/handle.rs`, `commit`): before
  `commit_into`, for every staged composite `≠ root.index` with
  `parents < 2`: `panic!("invalid vertex set: staged {index} has {count}
  parent occurrences")`.
- **`validate_retention`** (`committed/graph.rs`, `test-api`):
  `pub fn validate_retention(&self, inputs: impl IntoIterator<Item =
  CommittedIndex>) -> Result<(), PositionedError>`: every composite not in
  `inputs` has `parent_count() ≥ 2`, else `ParentCount`. Separate from
  `validate()`.
- **Store-generic test helpers** (`test-api`, for the per-step lane):
  - `positioned::interchange::store_interchange(store: &impl GraphRead)
    -> PositionedGraphData` (content-keyed covers through `string_repr`);
    `PositionedGraph::to_interchange` calls it.
  - `positioned::validate_store(store: &impl GraphRead) -> Result<(),
    PositionedError<Index>>`: `validate_vertex` for every vertex (links,
    widths) and `check_overlap_closure` for every composite (G4).

Exit: unit tests §5.1 (D4); gate green; reader output unchanged.

### 4.4 D5 — reader integration (fixes the bug)

- `forward.rs`: `forward(...) -> (Span, Option<Publication<StageIndex>>)`
  (the publication of `InsertOutcome::Created`).
- `expand.rs`: `Accepted` gains `publication: Option<Publication<StageIndex>>`;
  rejected probes never yield one (`debug_assert`).
- `root.rs`: `ReadStage::commit(span) -> (Commit, Option<Publication<StageIndex>>)`
  (`None` for `Adopt`); new
  `ReadStage::settle(&mut self, candidates: Candidates) -> Settled`: opens
  one `GraphEdit` on `self.handle.stage()`, calls `edit.settle(candidates,
  root)` on the current root, finishes the edit, stores the renamed root.
- `read.rs`, `read_steps`, per step (unknown atom, expansion, forward):
  `Candidates::default()`; `record` the insertion publication and the
  commit publication; `push(previous root)` if the root index changed
  (`Adopt`, `NewRoot`) and there was one; `root.settle(candidates)`;
  then `observe`.
- `Step` gains `pub dissolved: usize` and `pub discarded: usize` (from
  `Settled`); update the `step(..)` helper in `reader_behavior.rs`. The
  three trace tests keep their expected kinds and commits and expect
  `dissolved = discarded = 0` on every step (their repeated tokens are
  committed inputs, e.g. `abc` in `accepted_materialization`, or keep
  p ≥ 2).
- Internal observer for tests: `read_steps` passes `&StepView { step,
  handle: &OverlayHandle, root }` (`pub(crate)`); `read_traced` maps it to
  `Step`.
- `finish` unchanged (`handle.commit(root)`, now with the panic check).

Tests:

- `tests/reader_behavior.rs`: remove the 9 `#[ignore = "provisional
  retention (reader plan §5)"]` and `READ_DEVIATIONS` (the read gate
  expects 0 failures). `assert_scenario`: after every read,
  `validate_retention` with the tokens returned so far. `read_case`: build
  the base with `from_interchange_named`, inputs = the base corpus strings
  mapped through the returned names plus the returned root;
  `validate_retention` after the read.
- New unit test module `context-read/src/tests/steps.rs` (`mod steps;`
  inside `mod tests` in `lib.rs`; `ngrams` is a dev-dependency): the
  per-step lane of §5.3.

Exit: gate §5.4 green with 0 ignored for provisional retention and an
empty deviation list.

### 4.5 D6 — benchmarks

Run `cargo bench -p context-read --no-default-features --features
test-api` (benches `grammar`, `staging`, `staging_commit`,
`staging_memory`) and record medians plus, from `read_traced`, total
dissolutions, discards and candidates per read in §6 of this file. No
baseline comparison; no optimization in this plan (profiling is a later
plan, including a `Sync` `OverlayHandle` for parallel reads).

### 4.6 D7 — early commit (after D1b; interface pending decision E-I)

**Rule (final).** A staged composite `t ≠ R` is *safe* when it has ≥ 2
distinct left contexts and ≥ 2 distinct *fixed* right contexts over the
committed inputs and the consumed prefix (the occurrence ending at the
cursor has no right context yet). Safe tokens are never dissolved (E1);
a settled token that is not safe can be dissolved by some continuation
(E2). Graph-local form:

- the *chain* = the root's postfix chain (tokens with an occurrence ending
  at the cursor);
- a settled staged composite `≠ R` **off the chain** is safe;
- **on the chain**: safe iff a walk over parent occurrences finds ≥ 2
  fixed right contexts (stop at 2): `(U, o)` with `o + |t| < |U|` gives
  the atom at `o + |t|` in `U`; at the end of `U`, recurse into `U`'s
  parents; `U = R` adds nothing for the cursor; a parentless committed `U`
  is an input and adds its end. Committed inputs with parents are missed
  (no input mark): conservative.
- per step only the non-stable tokens need checking: the chain, the
  step's created tokens and the previous chain.

**Interface (proposed; decision E-I open).** Stability mark, no base write
during the read:

- stage: stable vertices occupy a slot prefix `[0, s)`;
  `raw::Stabilize::stabilize(seal, t) -> Option<Swap>` swaps `t` into slot
  `s`; `raw::Remove` panics on slots `< s`; `Stage::stable()`;
- `GraphEdit::stabilize(t, root)`: staged, composite, `≠ root`, p ≥ 2;
  bookkeeping renames as for `Moved`; `Candidates` skips stable vertices;
- reader: `ReadStage::stabilize()` after every `settle` applies the rule;
  `Step::stabilized: usize`; `read_traced` reports per committed token
  the step at which it became stable.

Alternative (not proposed): physical commit into the base during the
read. It needs a G6 notion for open reads (safe tokens usually have their
second parent occurrence in the uncommitted root: 67 of 67 prototype base
views violate G6), closure over staged descendants and projection of
committed hosts' deltas, and it only helps parallel readers.

Exit: gate unchanged; no stable vertex removed (store-enforced); after the
last step of an input every staged token but the root is stable or on
the chain; stability latency recorded.

## 5. Validation

### 5.1 Unit tests (context-trace, `positioned/tests/stage/`)

New file `dissolve.rs` (fixtures via
`context_trace::positioned::fixtures::fixture_graphs`; corpora must be
consistent with the oracle):

- D3 `OverlayHandle::atom`: new atom is committed (base generation + 1,
  stage generation unchanged), existing atom returned, `BaseChanged` after
  a foreign base edit, commit still detects a foreign edit after an own
  atom.
- D3 swap-remove: removing a middle slot moves the last vertex; its staged
  and committed children's parent links and its staged and committed
  parents' compositions are renamed; `validate_store` passes; removing the
  last slot returns `None`.
- D4 `dissolve` / `discard` preconditions: committed → `NotStaged`, atom →
  `Atom`, root → `IsRoot`, wrong count → `ParentCount`; the graph is
  unchanged after an error.
- D4 postconditions: host composition = `maximal(...)`; links moved;
  `validate_store` passes; host committed (delta) and staged; the
  returned `Removal` (`host`, `children`, `moved`).
- D4 `Candidates::record`: children of removed occurrences and created
  tokens; an occurrence removed and re-added in one edit is not a
  candidate.
- D4 `settle`: the `abcabca` walkthrough (§3.5) reproduced through
  `GraphEdit` calls; a cascade (dissolving `t` drops a child to p = 1);
  the root is renamed when it is the moved vertex; `Settled` counts.
- D4 commit panics on a staged composite with p < 2 (`#[should_panic]`).
- D4 `validate_retention`: passes on every fixture graph with its corpus
  as inputs; fails on a graph with a non-input composite of p = 1.

### 5.2 Integration tests (context-read)

- `tests/reader_behavior.rs`: all 94 scenarios (no ignore),
  `manifest_is_covered`, `read_gate` with 0 deviations over the 140 read
  cases, `steps_consume_disjoint_ranges`, the three trace tests;
  `validate()` and `validate_retention` after every read.
- `tests/fixture_validation.rs`: unchanged (fixtures in sync, G6 on all
  oracle graphs).

### 5.3 Per-step oracle lane (`context-read/src/tests/steps.rs`)

For every scenario of `fixtures/v3/scenarios.json`, read its inputs in
order into one graph; after every step (after `settle`), on the stage
view:

1. `validate_store(view)` (links, widths, G4);
2. (S): every staged composite `≠ root` has ≥ 2 parent occurrences;
3. `store_interchange(view)` equals the oracle graph of the earlier
   inputs plus the consumed prefix (`root.width` atoms), computed with
   `ngrams::graph::parse_corpus` as in `fixture_validation.rs`; atoms
   absent from the oracle (created by `segments` before they are
   consumed) are ignored.

Expected: every step equal (prototype D0: 701 / 701 steps of 199 reads).
A failure prints the scenario, the consumed prefix and the `diff`.

### 5.4 Gate (every task)

```sh
cargo check -p context-trace -p context-search -p context-insert -p context-read --no-default-features --features test-api
cargo test -p context-read --no-default-features --features test-api
cargo test -p context-trace -p context-search -p context-insert --no-default-features --features test-api
cargo test --release -p context-insert --no-default-features --features test-api insert_gate
cargo test -p context-read --features legacy-tests
cargo test -p context-trace -p context-search -p context-insert --features legacy-tests
cargo build --workspace --all-targets --exclude context-editor-kernel --exclude sandbox-app
```

(`context-editor-kernel` and `sandbox-app` need winit, which does not
build in the Linux codespace.) D6 additionally runs `cargo bench -p
context-read --no-default-features --features test-api`.

**Verifying the fix (after D5):**

```sh
cargo test -p context-read --no-default-features --features test-api --test reader_behavior repeat_abcabca
```

passes without `--ignored`; the committed graph has `abca = [a@0, b@1,
c@2, a@3]` and no `abc`. The same holds for the other 8 scenarios and
`insert_deep_borders__1`.

## 6. Results

**D5 (2026-11-25).** Gate §5.4 green. `reader_behavior`: 100 tests
(94 scenarios, none ignored; `read_gate` 140 / 140 cases with
`validate_retention`, 0 deviations). Per-step lane
(`src/tests/steps.rs`): 94 scenarios, 701 / 701 steps equal their oracle
graph, (S) and `validate_store` on every step; 16 dissolutions, 0 discards
in total (the prototype's numbers). The context-read suite also passes in
`--release`.

### D6 benchmarks (2026-11-25)

`cargo bench -p context-read --no-default-features --features test-api`,
release profile, Linux workstation; medians (criterion
`median.point_estimate`, 95 % CI in brackets). No baseline comparison.

| Benchmark id | Median |
|---|---|
| `read/corpus/all` | 1.892 ms [1.887, 1.897] |
| `read/periodic/abc^n_x/64` | 1.010 ms [0.999, 1.039] |
| `read/periodic/abc^n_x/256` | 41.77 ms [41.41, 41.95] |
| `read/periodic/abc^n_x/512` | 364.3 ms [362.2, 370.0] |
| `find_ancestor/corpus_inputs` | 861.5 µs [839.5, 865.2] |
| `find_ancestor/corpus_inputs_overlay` | 855.0 µs [854.0, 855.8] |
| `commit/overlay/abc^n_x/64` | 5.55 µs [5.45, 5.68] |
| `commit/overlay/abc^n_x/256` | 18.27 µs [18.03, 18.58] |
| `commit/overlay/abc^n_x/512` | 36.69 µs [35.44, 38.02] |
| `grammar_worst_case` | — (panics, see below) |

`staging_memory`:

| Input | Peak growth (KiB) | Retained (KiB) |
|---|---|---|
| corpus (max peak, total retained) | 53.9 | 446.1 |
| abc^64 x | 53.9 | 39.5 |
| abc^256 x | 199.4 | 152.0 |
| abc^512 x | 393.4 | 302.0 |

Notes:

- `grammar` (only `context_trace::legacy`; unchanged by this plan) panics
  during setup (`legacy/graph/vertex/data/core.rs:111`: pattern width
  mismatch in `T0w1`) and aborts `cargo bench` before the other benches;
  they were run with `--bench <name>`. Not addressed here.
- `staging_commit` panicked after D3 (its replay created atoms through
  `GraphEdit::atom` on the stage, which is no longer allowed); it now
  commits atoms via `OverlayHandle::atom` before replaying composites.

Settle statistics from `read_traced` over the v3 corpora
(`fixtures/v3/scenarios.json`; each scenario's inputs read in order into
one graph). `Step` does not report candidates, so they are not recorded.

| | Reads | Steps | Dissolved | Discarded |
|---|---|---|---|---|
| total (94 scenarios) | 199 | 701 | 16 | 0 |
| max per read | — | — | 3 (`insert_periodic`, `ababababa`) | 0 |

Scenarios with a dissolution (one read each unless noted):
`insert_periodic` 3; `period_ab_x3_a`, `period_abc_x3_a`,
`period_abc_x3_ab`, `period_abcd_x3_ab` 2 each; `repeat_abcabca`,
`repeat_ababa`, `period_abcd_x2_a`, `period_abcd_x2_abc`,
`insert_deep_borders` (3 reads) 1 each. All other reads: 0.

## 7. Decisions

| | Decision |
|---|---|
| Staging | Overlay only; tagged staging, `StagingStrategy`, `read_with`, the strategy comparison test and the `pre-staging` benchmark baseline are removed (D2). |
| Removal | Only staged vertices are removed, immediately (swap-remove with renaming); no tombstones (§3.2). |
| `settle` | Eager (end of every step, all span kinds) and local; candidates from the step's edit records plus the previous root; exact by (S) (§3.3). |
| G6 | Staged vertices: `settle`. Commit: panic on an invalid vertex set (only runtime check). Committed graph: `validate_retention` in tests (§3.1). |
| Inputs | No stored input mark, no `VertexRead::is_input`; the stage root is the only staged input (§3.1). |
| Root | Dissolution into the root is allowed (Lemma RM, §3.4). |
| Atoms | New atoms are committed immediately and never move; the stage holds only composites (§4.2). |
| Performance | Not optimized here; recorded in D6; profiling and a `Sync` handle later. |
| Early commit | Rule final (§4.6). **Open: E-I** — stability mark (proposed) vs physical commit. |

## Appendix A. Proofs

Notation: `occ(G)` = strings occurring in some vertex of `G`; `p(x)` =
number of parent occurrences of `x`. G1–G5 are assumed before every edit.

**L1 (locality).** If `t` has exactly one parent occurrence `(U, o)`,
every occurrence of `t`'s string in a vertex `w ≠ t` lies inside an
occurrence of `U` in `w`, at offset `o` within it; `t` occurs in `U` only
at `o`, and `t@o` lies inside no other element of `comp(U)`. *Proof:*
induction on `|w|`; by G2 the occurrence is a listed child (then `(w, x)`
is a parent occurrence, so `w = U`, `x = o`) or lies inside a listed
child `c` with `|c| < |w|`. ∎ Hence `occ(G) = occ(G − t)`.

**T1 (dissolution keeps G1–G5).** G1: a vertex is removed, no content
changes. G2 for `w ≠ U`: `comp(w)` does not list `t`; maximality over a
subset is preserved. G2 for `U`: every vertex occurrence in `U` lies inside
an element of `comp(U) − t@o ∪ comp(t) + o`, so `maximal` of that set is
the maximal set; no element of `comp(U) − t@o` is filtered (it would lie
inside `t@o`). G3: maintained by `GraphEdit`. G4: an overlap of two
elements is a vertex of `G` (intersection lemma) and not `t` (L1, or it
is shorter than `t`). G5: `t`'s string has the unique minimal host
`(U, o)` (L1); other strings keep or inherit their minimal hosts through
`U`. ∎ `discard` (`p = 0`) changes no composition.

**L2 (monotonicity).** Removing `t` with `p(t) ≤ 1` raises no other
count: only `U` and `t`'s children change links; a child loses `(t, a)`
and gains at most `(U, o + a)`. ∎

**T2 (confluence).** By G1/G2 the graph is a function of its vertex set;
`settle` terminates; two removable vertices stay removable after removing
either (L2) and both orders give the same vertex set; Newman's lemma. ∎

**L3 (forward match).** `occ` is unchanged by `settle` (L1), so the
longest occurring prefix is the same string; a dissolved token is at most
recreated by insertion.

**L4 (probe, host `U ≠ R`).** If postfix `t` has its only parent
occurrence in `U ≠ R`, the occurrence of `t` at `R`'s end lies inside a
`U` ending at `|R|` (L1), so `U` is a larger postfix, probed first; any
occurrence of `t + rest[..k]` lies inside `U + rest[..k]`. So the probe of
`t` is accepted only if that of `U` was. ∎

**RM (right-maximal root).** At the start of every probing step
`R + rest[0] ∉ occ(G)`. *Proof:* by the last root commit.
`InPlace`/`NewRoot`: `R` has no parent occurrence (editable roots are
parentless; `create` makes a new vertex), so `R`'s string occurs only as
`R`. `Adopt`: (a) first `Append`: `R = m`, the forward match; (b)
`ReplaceRoot` after forward: `R = R' + m`; (c) `ReplaceRoot` after a
probe: `R = R'[..o] + E`, `m = E` the longest occurring prefix of
`[P] ++ rest`. If `R + x` occurred, its suffix `m + x` would be a longer
occurring prefix: contradiction. `occ(G)` is unchanged until the next
probe: insertion of an occurring string, lookups, `Adopt`, `settle` (L1;
L2 gives `R` no parent) and rejected probes do not change it. A run after
an unknown atom has no probe at cursor 0. ∎

**L6 (probe, host `R`).** If postfix `t` has its only parent occurrence
in `R`, the probe `[t] ++ rest` is rejected: by L1 an occurrence of
`t + rest[0]` would lie at the end of an occurrence of `R`, giving
`R + rest[0] ∈ occ(G)`, contrary to RM. ∎ After dissolution the last child
of `t` becomes the last child of `R` (only `t@o` ended at `|R|`), so the
iterator continues exactly where it would have descended.

**L5 (`find_complete`, identities).** `R + t'` is looked up by string; a
dissolved `Y` is recreated as a new root of the same string. `settle` runs
between steps and the root is exempt.

**C1 (locality of losses).** If `maximal` filters `x@k` from `comp(U)`
because an added `y@j` contains it, `x` is a child of `y` or a descendant
through vertices created in the same edit: otherwise an existing child of
`y` containing `x@k` occurred in `U`, contradicting the maximality of
`x@k` before the edit. ∎ (Cost only.)

**E1 / E2 (early-commit rule; rest on M2, M4, M5).** E1: contexts of
existing occurrences never change and later input only adds occurrences
and fixes the cursor occurrence, so a safe `t` stays in MR of every later
prefix, of `C + [I]` and (M5) of every later corpus; by M4/M2 it keeps
p ≥ 2 and is never removed. E2: a settled `t` that is not safe is in MR
(M2) only through the cursor occurrence, so it has exactly one fixed right
context `a`; the continuation `a` followed by the input end leaves `a` as
its only right context, and `t` is dissolved. ∎

## Appendix B. Evidence and prototypes

Prototypes in `context-insert/agents/prototypes` (Python, brute-force
search stand-ins; see its README):

| Prototype | Checks | Result |
|---|---|---|
| `dissolve.py` (`python3 read_legacy_port.py --dissolve`) | reader port with eager `settle` | 199 / 199 reads and 701 / 701 steps equal the oracle (without `settle` 189 / 199); 0 G6 violations; order-independent; decisions identical with and without `settle`; 16 dissolutions, 0 into the root |
| one-off random run | 4744 reads on all `{a,b}` strings of length 2–10, 1500 random `{a,b,c}` strings, 600 pairs | all equal the oracle, decisions identical, 548 dissolutions |
| `early_commit.py` | RM, early-commit rule, base views | 5269 reads, 28630 steps: RM 0 violations in 21185 checks (1348 steps with a root with parents); 0 of 13845 off-chain tokens unsafe; walk = definition on 15060 chain tokens; 0 of 7168 safe tokens dissolved later; physical base views: G4/G5 0 / 67 violations, G6 violated in 67 / 67 |
