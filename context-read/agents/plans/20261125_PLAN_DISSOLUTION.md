---
tags: `#plan` `#context-read` `#context-trace` `#positioned-composition` `#reader` `#retention` `#dissolution` `#staging`
summary: Implementation plan for the dissolution of provisional tokens (reader plan phase R5, roadmap step 5). The positioned reader keeps tokens that a prefix of the input made frequent but the rest of the input made redundant (`abcabca` keeps `abc`). Fix - retention invariant G6 (every committed vertex that is neither an atom nor an input has ≥ 2 parent occurrences), restored after every reader step by an eager, local `settle` that dissolves or discards staged tokens with ≤ 1 parent occurrence. Candidates come from the step's edit records; removal is immediate (swap-remove with renaming); new atoms are committed at once; committing an invalid vertex set panics. Tagged staging and the strategy abstraction are removed first. Exact code changes per task (D2–D7), unit and integration tests, per-step oracle lane and gate. Formal proofs (Appendix C) - the oracle retains exactly atoms, inputs and maximal repeats (M1); one reader step plus `settle` maps the oracle graph of the consumed prefix to that of the next (M4, relative to the insert contract); exact early-commit rule (Theorem E). Early commit uses stability marks (E-I decided).
status: ✅ D2–D6 implemented (2026-11-25, §6); D7 superseded by 20261126_PLAN_CHEAP_STABILIZATION (2026-11-26; Theorem E is no longer used); D1b done (Appendix C). Supersedes 20261124_PLAN_DISSOLUTION_OLD.md (revisions 1–3.2).
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
| D7 | Early commit by stability marks (§4.6) | no (adds marks) |
| D1b | Proofs M1–M5, RM, C1, Theorem E (§3.6, Appendix C) — **done** | — |

Order: D2 → D3 → D4 → D5 → D6 → D1b → D7 (all done). Every task ends
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
- > **Superseded (2026-11-26)** by
  > [20261126_PLAN_CHEAP_STABILIZATION](20261126_PLAN_CHEAP_STABILIZATION.md)
  > §4.5: a removal leaves a hole; no vertex moves and nothing is renamed.
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

### 3.6 Maximal-repeat characterization (task D1b, proven)

> `oracle(C) = G(atoms(C) ∪ inputs(C) ∪ MR(C))`

`MR(C)`: strings of ≥ 2 atoms with ≥ 2 distinct left contexts and ≥ 2
distinct right contexts over their occurrences in the inputs (input start
and end count as contexts distinct from every atom and from each other);
`G(W)` is the graph on the vertex set `W` with maximal compositions (G1,
G2). Proven in Appendix C (first observed on 199 oracle graphs of
scenario prefixes and 3544 random corpora, 0 differences):

| # | Statement | Appendix C | Used for |
|---|---|---|---|
| M1 | the characterization above, from the oracle's selection procedure | C.1 | reduces correctness to vertex sets |
| M2 | a maximal repeat that is no input has p ≥ 2 in every graph whose vertex set contains the inputs; in `G(V(C))` a non-atom, non-input vertex has p ≥ 2 iff it is in MR | C.2 | G6 ⇔ maximal repeats |
| F | every `settle` fixpoint of a vertex set between `V(D')` and the step's vertex set is exactly `V(D')` | C.3 | the value of `settle` |
| M3 | G4 / G5 for `G(V(C))` | C.4 | invariants without the oracle |
| M5 | `MR(C) ⊆ MR(C + [X])` for every string `X`; adding an existing vertex as an input changes no vertex set | C.5 | committed non-inputs keep p ≥ 2; `Complete` insertions |
| M4 | one step plus `settle` maps `G(V(C + [I[..c]]))` to `G(V(C + [I[..c']]))` — relative to the insert contract A1 | C.6 | per-step oracle equality (§5.3) |
| RM, S | right-maximal root; strings straddling the step's seam occur once | C.6 | M4 |
| RR | `ReplaceRoot` never happens after the first step (0 of 45297 steps, Appendix B) | C.6 | — (observation; code unchanged) |
| E | exact early-commit rule (corrects the earlier E2, §4.6) | C.7 | D7 |
| C1 | locality of losses (§3.3) | Appendix A | cost bound |

The only unproven premise is A1, the insert contract (insertion of an
occurring string `N` into `G(V(D))` gives `G(V(D + [N]))`); it is the
insert plan's gate property and the per-step lane checks it on every step.

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

### 4.6 D7 — early commit by stability marks

> **Superseded (2026-11-26)** by
> [20261126_PLAN_CHEAP_STABILIZATION](20261126_PLAN_CHEAP_STABILIZATION.md)
> (Rule S, Theorem S). Theorem E, the right-context walk (Lemma W) and the
> uniformity test are no longer used by the implementation or the tests;
> this section is kept as design history.

Decided (E-I): stability marks. A *stable* staged vertex is never removed
again; nothing is written to the base before `StageHandle::commit`.

**Rule (exact, Theorem E in C.7; corrects the earlier rule).** After
`settle` at cursor `c` (root string `P`), every staged composite `t ≠ R`
is a maximal repeat of `D = C + [P]` with ≥ 2 left contexts (M4, M2), so
only right contexts matter. Let `F(t)` be its *fixed* right contexts: the
right contexts of all its occurrences in the committed inputs and in `P`
except the occurrence ending at the cursor. `t` is never dissolved by any
continuation (*permanent*) iff

- (a) `t` is not a suffix of `P` (off the chain), or
- (b) `|F(t)| ≥ 2`, or
- (c) `F(t) = {end of a committed input}`, or
- (d) `F(t) = {a}` for an atom `a` and `t = aⁿ`.

Otherwise `F(t) = {a}` with `t ≠ aⁿ`, and the continuation `a`, followed
by the end of the input, dissolves `t`. The earlier rule (`≥ 2` fixed
right contexts on the chain) missed (c) and (d): it was sound but not
exact. A brute-force check (Appendix B) found 750 of 4655 permanent
tokens in cases (c) and (d).

**Graph-local form.**

- *Chain*: the root's postfix chain, which is exactly the set of vertices
  that are proper suffixes of `P` (Lemma Ch, C.6). (a) means not on the
  chain.
- *Walk* (Lemma W, C.7): `F(t)` = for every parent occurrence `(U, o)` of
  `t`: the atom at `o + |t|` in `U` if `o + |t| < |U|`; else the contexts
  of `U`, recursively, plus `End(U)` if `U` is a committed input. The
  root's own end is never added (it is the cursor). Stop at 2. Only
  parentless committed vertices are recognized as inputs (no input mark),
  so the walk may miss the end of a committed input that has parents:
  sound, possibly late for (b)/(c). It is exact when every committed input
  is parentless.
- (d): `t` is *uniform* iff every child of `comp(t)` is the same atom or
  a uniform vertex of that atom (atoms are uniform).
- Invariant (U): after `stabilize`, every unstable staged vertex other
  than the root is on the chain. So the vertices to check at a step are
  exactly the unstable slots `[stable(), len)`: the previous chain plus
  the step's new tokens. No extra bookkeeping is needed.
- A stable vertex keeps p ≥ 2 in every later state (Theorem E, M2), so
  `settle` skips it. Its composition can still change (dissolution into
  it, insertion); only removal is excluded.

**Code changes.**

context-trace:

```rust
// store/write.rs, inside `pub(crate) mod raw`
pub trait Stabilize: Remove {
    /// Mark the staged vertex `index` stable: swap it into slot `stable()`
    /// (renaming every reference of both swapped vertices) and grow the
    /// stable prefix by one. `None` if no swap was needed.
    fn stabilize(&mut self, seal: Seal, index: StageIndex) -> Option<Swap>;
}
// `Remove::remove` panics on a stable slot.

// stage/mod.rs
pub trait Stage: GraphWrite<Index = StageIndex> + raw::Stabilize {
    /// The staged vertices in slot order; the first `stable()` are stable.
    fn staged(&self) -> impl Iterator<Item = StagedIndex> + '_;
    fn stable(&self) -> usize;
    fn is_stable(&self, index: StageIndex) -> bool {
        index.staged().is_some_and(|s| s.slot() < self.stable())
    }
}

/// Two staged vertices exchanged slots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Swap { pub a: StageIndex, pub b: StageIndex }
impl Swap {
    pub fn apply(self, index: StageIndex) -> StageIndex; // a ↔ b, else unchanged
}
```

- `stage/overlay/staging.rs`: `StagingGraph::stable: usize`; `commit_into`
  unchanged (every staged vertex is committed).
- `stage/overlay/view.rs`: `Stabilize for OverlayView`:
  `debug_assert!(slot >= stable)`; if `slot != stable`, swap
  `vertices[slot]` and `vertices[stable]` and rename references with the
  permutation `σ = (a b)`, applied once to every site: parent keys in
  the links of both vertices' children (staged: `parents`; committed:
  `deltas[child].added`) and the compositions of both vertices' parents
  (staged node; committed `deltas[p].composition`). The two vertices may
  reference each other, so collect the sites first and rename them with
  `σ` in one pass. Factor the D3 renaming into a helper over a
  permutation of indices that `remove` (a single move) and `stabilize`
  (a swap) share. `stable += 1`; bump `generation`. `remove` panics
  (`assert!`) on `slot < stable`.
- New `positioned/edit/stable.rs`:

```rust
impl<S: Stage + ?Sized> GraphEdit<'_, S> {
    /// Errors (graph unchanged): NotStaged, Atom, IsRoot, ParentCount
    /// { count } for p < 2. Ok(None) if already stable or no swap needed.
    /// Applies the swap to `touched`, `edits` and `created`.
    pub fn stabilize(&mut self, t: StageIndex, root: StageIndex)
        -> Result<Option<Swap>, PositionedError<StageIndex>>;
}

/// A fixed right context found by the walk of Lemma W.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RightContext<A, I> { Atom(A), End(I) }

/// At most `limit` distinct fixed right contexts of `t` (Lemma W); a
/// committed parentless vertex is an input and adds `End`; staged vertices
/// (also the root, whose end is the cursor) add nothing.
pub fn fixed_right_contexts<G: GraphRead>(graph: &G, t: G::Index, limit: usize)
    -> Vec<RightContext<AtomOf<G::Kind>, G::Index>>;

/// `Some(a)` iff `t`'s content is `a` repeated (atoms: themselves).
pub fn uniform_atom<G: GraphRead>(graph: &G, t: G::Index) -> Option<AtomOf<G::Kind>>;
```

  The atom at offset `o + |t|` of `U` is read by a descent through
  `CompositionRead::covering` (not a content index; `read_interval` exists
  only in debug and `test-api` builds).
  The walk recurses into parents of parents (strictly wider, so it
  terminates) and deduplicates contexts.
  *As implemented:* `fixed_right_contexts(graph, t, limit)` has no `root`
  parameter. The root needs no special case: it is staged (never an
  input) unless adopted, and an adopted committed parentless root is a
  committed input whose end is a genuine fixed context; the cursor
  occurrence of a staged root contributes nothing because staged vertices
  never add `End`.
- `edit/dissolve.rs`, `settle`: skip stable vertices
  (`debug_assert!(p >= 2)`).
- `positioned/mod.rs`: re-export `Swap`, `RightContext`,
  `fixed_right_contexts`, `uniform_atom`.

context-read:

- `root.rs`: `ReadStage::stabilize(&mut self) -> usize`. Collect the
  unstable staged composites (slots `stable()..len`) other than the root
  and the chain (`postfixes` of the root, as a set). Then, in one
  `GraphEdit`, for each `t`: `permanent = !chain.contains(t) || match
  fixed_right_contexts(.., t, root, 2)[..] { [_, _] => true, [End(_)] =>
  true, [Atom(a)] => uniform_atom(.., t) == Some(a), [] => unreachable }`.
  If permanent, `edit.stabilize(t, root)` and apply the returned swap to
  the remaining list, the chain set and the root. Return the count.
- `read.rs`: after `settle` in every step, `let stabilized =
  root.stabilize();`. `Step` gains `pub stabilized: usize`, and
  `StepView` exposes it. Update `step(..)` in `reader_behavior.rs`.

**Tests.**

- Unit (context-trace, new `positioned/tests/stage/stable.rs`):
  - a swap renames staged and committed children's links and staged and
    committed parents' compositions; two vertices that reference each
    other; `validate_store` passes; the root is renamed when swapped;
  - `remove` of a stable slot panics (`#[should_panic]`);
  - `GraphEdit::stabilize` errors leave the graph unchanged; it is
    idempotent;
  - `fixed_right_contexts` on every fixture graph equals the
    definitional contexts computed from the fixture corpus by string
    search (test code only), up to `limit`, when no committed input has
    parents; otherwise a subset;
  - `uniform_atom`.
- Per-step lane (`context-read/src/tests/steps.rs`), after `stabilize`
  on every step:
  - (U) holds;
  - a vertex once stable is present in every later step and in the
    committed graph;
  - exactness: the definitional class of Theorem E, computed from
    strings, satisfies "stable ⇒ permanent", and "permanent ⇒ stable"
    unless the witness is the end of a committed input with parents
    (counted, not failed);
  - E2 witness: every unstable `t ≠ R` with `F(t) = {a}` is absent from
    `oracle(C + [P·a])`.
- `tests/fixture_validation.rs`: `oracle_vertex_sets_are_maximal_repeats`
  (M1). For every scenario and read-lane oracle graph, its vertex strings
  equal atoms ∪ inputs ∪ MR of its corpus, computed by brute force.
- Trace tests: expected `stabilized` per step.

**Exit.** The gate (§5.4) is unchanged; the lane and unit tests above
pass; no stable vertex is removed (store-enforced); after the last step
of every input, every unstable staged vertex but the root is on the
chain; stability latency (steps from creation to stabilization; mean,
max) and the conservative misses are recorded in §6.

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

- `grammar` (only `context_trace::legacy`; unchanged by this plan) panicked
  during setup (`legacy/graph/vertex/data/core.rs:111`: pattern width
  mismatch in `T0w1`) and aborted `cargo bench` before the other benches;
  they were run with `--bench <name>`. Fixed separately in `9d783fa`
  (vertex index allocation in the grammar builder); it runs again (D7
  table).
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

### D7 results (2026-11-26)

> **Note.** These numbers are of the superseded D7 implementation
> (Theorem E). The current implementation and its numbers are in
> [20261126_PLAN_CHEAP_STABILIZATION](20261126_PLAN_CHEAP_STABILIZATION.md) §6.

Gate §5.4 green (also `cargo test --release -p context-read`).

- **Lane** (`src/tests/steps.rs`, after `settle` and `stabilize` on every
  step): 701 / 701 steps of 94 scenarios equal their oracle graph; (S),
  (U) and `validate_store` hold on every step; no stable vertex is removed
  later in its read, and every stable vertex is in the committed graph.
- **Exactness** against Theorem E computed from strings, on every staged
  composite of every step: stable ⇒ permanent and permanent ⇒ stable both
  hold, with **0 late** (no committed input with parents was ever the
  missing context on the v3 corpora).
- **E2 witnesses:** 215 unstable tokens with `F(t) = {a}`; each is absent
  from `oracle(C + [P·a])`.
- **Stabilized:** 88 in total. Latency (steps from first appearance in the
  stage to stabilization): mean 0.26, max 1.
- **Trace tests:** only `repeat_aaa` stabilizes, 1 at its last step (`aa`,
  case (d)).
- **Sensitivity** (mutations, reverted): dropping case (d) fails 7
  scenarios; dropping case (c) fails 3 (e.g. `seam_bcd`: `bc` with
  `F = {End(abc)}`); stabilizing everything trips the stable-vertex
  assertion in `settle`.
- **New tests:**
  - context-trace unit tests `positioned/tests/stage/stable.rs`: swap
    renaming with mutually referencing vertices, `remove` of a stable slot
    panics, `GraphEdit::stabilize` errors and idempotence, the walk equals
    the corpus contexts on every fixture graph, `uniform_atom`;
  - `fixture_validation::oracle_vertex_sets_are_maximal_repeats` (M1):
    holds on every oracle graph of the v3, insert and read lanes.

**Implementation notes** (deviations from §4.6):

- `fixed_right_contexts` has no `root` parameter (see §4.6).
- The atom lookup descends with `covering`.
- The reader decides permanence on the unchanged stage first, then applies
  the swaps in one `GraphEdit`.
- `RightContexts` memoizes the capped contexts of every ancestor within one
  `stabilize`.
- `uniform_atom` first rejects vertices whose first and last atoms differ.

Without the memo, `stabilize` was quadratic per step on periodic input
(`abc^256 x`: 3.5 s instead of 42 ms).

**Cost** (criterion medians, release; D6 numbers for comparison):

| Benchmark id | D6 | D7 |
|---|---|---|
| `read/corpus/all` | 1.892 ms | 2.139 ms [2.100, 2.174] |
| `read/periodic/abc^n_x/64` | 1.010 ms | 2.764 ms [2.754, 2.780] |
| `read/periodic/abc^n_x/256` | 41.77 ms | 129.9 ms [127.9, 132.4] |
| `read/periodic/abc^n_x/512` | 364.3 ms | 1.112 s [1.107, 1.118] |
| `find_ancestor/corpus_inputs` | 861.5 µs | 855.7 µs (unchanged, no read) |
| `commit/overlay/abc^n_x/{64,256,512}` | 5.55 / 18.27 / 36.69 µs | 5.76 / 18.37 / 36.47 µs (unchanged) |
| `grammar_worst_case` | — (panicked) | 6.27 ms [6.23, 6.31] |

`staging_memory`: retained memory is unchanged. Peak growth rises slightly
from the walk's memo: 207.7 KiB instead of 199.4 KiB (`abc^256 x`) and
413.0 KiB instead of 393.4 KiB (`abc^512 x`).

At `abc^256 x`, about 58 ms of the added time is the context walk and
about 27 ms the uniformity check (measured by disabling each). Optimizing
this is left to the profiling plan; one option is to cache the contexts of
unchanged chain members across steps.

## 7. Decisions

| | Decision |
|---|---|
| Staging | Overlay only; tagged staging, `StagingStrategy`, `read_with`, the strategy comparison test and the `pre-staging` benchmark baseline are removed (D2). |
| Removal | Only staged vertices are removed, immediately (swap-remove with renaming); no tombstones (§3.2). Superseded 2026-11-26: a removal leaves a hole, no renaming (cheap stabilization plan §4.5). |
| `settle` | Eager (end of every step, all span kinds) and local; candidates from the step's edit records plus the previous root; exact by (S) (§3.3). |
| G6 | Staged vertices: `settle`. Commit: panic on an invalid vertex set (only runtime check). Committed graph: `validate_retention` in tests (§3.1). |
| Inputs | No stored input mark, no `VertexRead::is_input`; the stage root is the only staged input (§3.1). |
| Root | Dissolution into the root is allowed (Lemma RM, §3.4). |
| Atoms | New atoms are committed immediately and never move; the stage holds only composites (§4.2). |
| Performance | Not optimized here; recorded in D6; profiling and a `Sync` handle later. |
| Early commit | Stability marks (E-I decided 2026-11-25): stable vertices occupy a slot prefix of the stage and are never removed; no base write during a read. Exact rule of Theorem E (§4.6, C.7), implemented conservatively only for committed inputs with parents (no input mark). |

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

**E1 / E2.** Superseded by Theorem E (C.7). The earlier E2 ("a settled
token that is not safe can be dissolved by some continuation") is false
when the single fixed right context is the end of a committed input, or
when `t = aⁿ`; §4.6 uses the exact rule.

**Note on RM.** The proof above treats `ReplaceRoot` roots (b) and (c) as
possible. By C.6 (Corollary RR) they never occur after the first step;
C.6 proves RM independently of this case analysis.

## Appendix B. Evidence and prototypes

Prototypes in `context-insert/agents/prototypes` (Python, brute-force
search stand-ins; see its README):

| Prototype | Checks | Result |
|---|---|---|
| `dissolve.py` (`python3 read_legacy_port.py --dissolve`) | reader port with eager `settle` | 199 / 199 reads and 701 / 701 steps equal the oracle (without `settle` 189 / 199); 0 G6 violations; order-independent; decisions identical with and without `settle`; 16 dissolutions, 0 into the root |
| one-off random run | 4744 reads on all `{a,b}` strings of length 2–10, 1500 random `{a,b,c}` strings, 600 pairs | all equal the oracle, decisions identical, 548 dissolutions |
| `early_commit.py` | RM, early-commit rule, base views | 5269 reads, 28630 steps: RM 0 violations in 21185 checks (1348 steps with a root with parents); 0 of 13845 off-chain tokens unsafe; walk = definition on 15060 chain tokens; 0 of 7168 safe tokens dissolved later; physical base views: G4/G5 0 / 67 violations, G6 violated in 67 / 67 |
| Corollary RR check (temporary test, 2026-11-25, deleted) | `read_traced` over all scenarios, all read-lane cases and every `{a,b}` string of length 2–11, read forward and reversed into one graph | 45297 steps, 0 `Adopt` after the first step of a read |
| Theorem E check (`/tmp` brute force, 2026-11-25) | 1500 random corpora over `{a,b,c}`: every staged maximal repeat of every prefix, classified by the exact rule; permanent ⇒ in `V` of every prefix of every continuation up to length 3; not permanent ⇒ absent from `V(C + [P·a])` | 6041 tokens: 4655 permanent (750 of them by cases (c)/(d), missed by the earlier rule), 1386 not permanent; 0 failures |

## Appendix C. Formal proofs (task D1b)

### C.0 Definitions and basic facts

- **Corpus.** A corpus `D = (T₁, …, Tₙ)` is a list of nonempty strings
  (*texts*, the inputs) over the atoms `Σ`. `|s|` is the width of `s`.
  `sub(D)` is the set of strings that occur in some text. `texts(D)` and
  `atoms(D)` are the texts and the atoms occurring in `D`. `D + [X]`
  appends the text `X`.
- **Occurrences and contexts.** An occurrence of `s` is a pair `(k, x)`
  with `T_k[x..x+|s|] = s`; `count_D(s)` is their number (overlaps
  count). The boundary symbols `^k` and `$k` (one pair per text index
  `k`) are pairwise distinct and distinct from every atom.
  - The left context of `(k, x)` is `T_k[x−1]` if `x > 0`, else `^k`.
  - The right context is `T_k[x+|s|]` if `x + |s| < |T_k|`, else `$k`.
  - `L_D(s)` and `R_D(s)` are the sets of left and right contexts over
    all occurrences.
  - `s` is *left-branching* if `|L_D(s)| ≥ 2`, *right-branching* if
    `|R_D(s)| ≥ 2`.
- **Maximal repeats and the vertex set.** `MR(D)` = `{ s : |s| ≥ 2, s`
  left- and right-branching `}`, and `V(D) = atoms(D) ∪ texts(D) ∪ MR(D)`.
- **Graphs.** For a finite set of strings `W`, `G(W)` is the graph with
  one vertex per string of `W`. `comp(u)` is the maximal set, under
  interval containment, of occurrences `(o, w)` of strings
  `w ∈ W \ {u}` in `u`; atoms have empty compositions.
  - `(u, o)` is a *parent occurrence* of `t` iff `(o, t) ∈ comp(u)`;
    `p(t)` is the number of parent occurrences.
  - G1 (distinct content) and G2 (maximal compositions) say exactly that
    a positioned graph is `G(W)` for its set of vertex strings. T2
    (Appendix A) uses this.
  - A stage also holds the atoms of the whole input, which `segments`
    commits up front. An atom that does not occur in `D` lies in no other
    vertex and hosts nothing, so it is ignored below. "Up to unread
    atoms" means: apart from such atoms.

**F1 (injection).** If `u` contains `t` at offset `o`, then
`(k, y) ↦ (k, y + o)` maps the occurrences of `u` injectively to
occurrences of `t`. So `count(u) ≤ count(t)`. If `o + |t| < |u|`, every
image has right context `u[o+|t|]`. If `o + |t| = |u|`, the image has the
right context of the occurrence of `u`. Left contexts behave
symmetrically. ∎

**F2 (boundary contexts).** If `R_D(s) = {$k}`, every occurrence of `s`
ends at the end of `T_k`; there is only one such position, so
`count(s) = 1`. The same holds for `^k`. Hence if `count(s) ≥ 2`, a
unique context is an atom, and every maximal repeat has `count ≥ 2`. ∎

**F3 (closure).** Let `s ∈ sub(D)`. Let `w` be the longest string such
that every occurrence of `s` is followed by `w` inside its text, and `v`
the longest string such that every occurrence is preceded by `v`. Let
`X(s) = v·s·w`. The occurrences of `s`, `vs`, `sw` and `X(s)` are in
bijection (shift by `|v|`). Then:

1. If `count(s) ≥ 2`, then `X(s) ∈ MR(D)`.
2. If `count(s) = 1`, then `X(s)` is the text containing the occurrence.
3. If `u ∈ texts(D) ∪ MR(D)` contains `s` at offset `q`, then `u`
   contains `X(s)` at offset `q − |v|`.
4. If `|s| ≥ 2` and `s ∉ MR(D) ∪ texts(D)`, then `X(s) ≠ s`.

*Proof.*

1. If `L(vs)` were a single atom `c`, `v` could be extended by `c`; if
   it were `{^k}`, then `count = 1` (F2). So `vs` is left-branching, and
   `L(X) = L(vs)` because the occurrences correspond with the same left
   contexts. The right side is symmetric, and `|X| ≥ 2`.
2. With a single occurrence every extension has a unique context, which
   is an atom until the text boundary is reached.
3. Suppose `u` does not reach the end of `X(s)`: `j = |u| − q − |s| <
   |w|`. Every occurrence of `u` contains an occurrence of `s` at `q`,
   which is followed by `w`, so every occurrence of `u` is followed by
   `w[j]`. Then `R(u) = {w[j]}`, so `u ∉ MR`. And `u` is no text: the
   text `T_m = u` is an occurrence of `u` with right context `$m`. This
   contradicts the assumption on `u`. The left side is symmetric.
4. Some context of `s` is unique. If `count(s) = 1`, `X(s)` is the text
   (2), which is not `s`. Otherwise the unique context is an atom (F2),
   so `v` or `w` is nonempty. ∎

### C.1 M1: the oracle retains exactly atoms, inputs and maximal repeats

**The oracle** (`ngrams::graph::parse_corpus`):

- *Vocabulary*: `sub(D)` with occurrence sets. The containment parents of
  `s` (`|s| ≥ 2`) are the one-atom extensions `x·s` (with `s` at offset
  1) and `s·y` (offset 0) that occur ([find_children](../../../ngrams/src/graph/containment.rs)).
- *Labels* (`labelling/frequency.rs`): seeded with `texts(D) ∪
  atoms(D)`. Every other string is tested in order of decreasing width
  and labelled iff some `u ∈ cover(s)` has `count(u) < count(s)`.
- *`cover(s)`* (`utils/cover/frequency.rs`): breadth-first over states
  `(u, off)`, where `u` contains `s` at `off`, layer by layer in depth
  `|u| − |s|`, starting from the parents of `s`, each state visited once.
  A labelled state *contributes* the not yet claimed positions
  `{(k, y + off)}` of the occurrences of `u`. A contributing state joins
  the cover and is not expanded; every other state is expanded to its
  parents.
- *Compose*: every labelled string gets its maximal cover by labelled
  strings, so `oracle(D) = G(labels)`.

**Theorem M1.** `labels = V(D)`, hence `oracle(D) = G(V(D))`.

*Proof.* By induction on decreasing width; for strings wider than `s`,
assume: labelled ⇔ in `V(D)`. Seeds are atoms and texts. Let `|s| ≥ 2`
and `s ∉ texts(D)`.

*(O) `cover(s) ≠ ∅`.* Take an occurrence `(k, x)`; `T_k ≠ s`. The
one-atom extensions of `s` inside `T_k` form a chain of states up to
`(T_k, x)`. Each state of the chain either contributed (then the cover is
nonempty) or was expanded. If all were expanded, `T_k` is reached. It is
labelled, and it contributes unless position `(k, x)` was already
claimed, which again means the cover is nonempty.

*(⇐) `s ∈ MR(D)` ⇒ labelled.* Every cover member `u` is a proper
ancestor and extends `s` to the right or to the left. By F1 its images
share one right (resp. left) context. `s` has two, so some occurrence of
`s` is not an image: `count(u) < count(s)`. The cover is nonempty (O),
so `s` is labelled.

*(⇒) `s ∉ MR(D)` ⇒ not labelled.*

- If `count(s) = 1`, no string has a count below 1.
- Otherwise let `X = X(s)`. Then `X ∈ MR(D)` (F3.1), `X ≠ s` (F3.4), and
  `X` is labelled (induction hypothesis).
- Every labelled proper ancestor state `(u, off)` has
  `u ∈ texts ∪ MR` (induction hypothesis), so it contains `X` at
  `off − |v|` (F3.3). Hence `|u| ≥ |X|`, with equality only for
  `(X, |v|)`.
- So the layers of depth below `|X| − |s|` contain no labelled state and
  claim nothing. Every string on the extension chain from `s` to `X`
  inside `X` is unlabelled, so the state `(X, |v|)` is reached at depth
  `|X| − |s|`. It is the only labelled state of that layer and claims all
  `count(s)` positions (bijection).
- Later states claim nothing, so `cover(s) = {X}` with
  `count(X) = count(s)`, and `s` is not labelled. ∎

### C.2 Hosts and M2

Throughout C.2–C.4: `texts(D) ⊆ W ⊆ sub(D)`, up to unread atoms.

**Hosts.** Let `(k, x)` be an occurrence of `t ∈ W` that is not a whole
text. A *host* is an occurrence `(k, y)` of some `u ∈ W \ {t}` whose
interval contains `[x, x+|t|)`; then `|u| > |t|`. A host is *minimal* if
no other host's interval lies strictly inside its interval.

**Lemma H.**

- (a) Every occurrence of `t` that is not a whole text has a host (its
  text).
- (b) A minimal host `(k, y)` of `u` gives the parent occurrence
  `(u, x − y)`.
- (c) Every parent occurrence `(u, o)` arises by (b) from some occurrence
  of `t`.

*Proof.*

- (b) If `(x − y, t) ∉ comp(u)`, an occurrence of some `w ∈ W \ {u, t}`
  in `u` strictly contains it. Since `|w| < |u|`, it is a host strictly
  inside `(k, y)`.
- (c) Take an occurrence `(k, y)` of `u` (which exists since
  `u ∈ sub(D)`); `t` occurs at `(k, y + o)`. A host strictly inside
  `(k, y)` would be a vertex occurrence in `u` strictly containing `t@o`,
  contradicting `(o, t) ∈ comp(u)`. ∎

**Theorem M2.** If `t ∈ W ∩ MR(D)` and `t ∉ texts(D)`, then `p(t) ≥ 2`
in `G(W)`.

*Proof.* `p(t) ≥ 1` by H(a, b). Suppose `p(t) = 1` with `(U, o)`. By
H(b), every minimal host of every occurrence of `t` is an occurrence of
`U` containing `t` at offset `o`. Since `|U| > |t|`, `U` extends `t` on
one side, so by F1 `L_D(t)` or `R_D(t)` is a single context: `t ∉ MR`.
Contradiction. ∎

**Corollary (G6 on `V`).** In `G(V(D))` a vertex that is neither an atom
nor a text is in `MR(D)` (by definition) and has p ≥ 2 (M2).

### C.3 F: the fixpoint of removal

**Theorem F.** Let `D'` be a corpus and `E ⊆ W` a set of *exempt*
vertices (in the reader: atoms, committed vertices and the root), such
that:

- `texts(D') ⊆ E ⊆ V(D')`, up to unread atoms;
- `MR(D') ⊆ W ⊆ sub(D')`, up to unread atoms.

A *removal* deletes a vertex `t ∈ W \ E` with `|t| ≥ 2` and `p(t) ≤ 1`
in `G(W)`. Every set `F` reached by removals in which nothing is
removable equals `V(D')`, up to unread atoms.

*Proof.*

*(⊇)* Texts are exempt. A non-text `t ∈ MR(D')` has `p(t) ≥ 2` in every
intermediate set, because those contain the texts (M2). So it is never
removed.

*(⊆)* Let `t ∈ F` with `|t| ≥ 2` and `t ∉ texts(D') ∪ MR(D')` be of
maximal width. Then `t ∉ E`, so `p(t) ≥ 2`. Every parent occurrence
`(u, o)` has `u ∈ F` and `|u| > |t|`, so `u ∈ texts(D') ∪ MR(D')` (by
maximality, or because `u ∈ E`).

- *`count(t) = 1`.* Then `count(u) ≤ 1` (F1), so `u ∉ MR` (F2) and `u`
  is a text. The text `u` is an occurrence of `u` and contains `t` at
  `o`, so `u` is the text of the unique occurrence `(k, x)`, and
  `o = x`. At most one parent occurrence exists: contradiction.
- *`count(t) ≥ 2`.* `X(t) ∈ MR(D') ⊆ F` (F3.1, ⊇) and `X(t) ≠ t`
  (F3.4). By F3.3, `u` contains `X(t)` at `o − |v|`. If `u ≠ X(t)`, that
  is a vertex occurrence in `u` strictly containing `t@o`, so
  `(o, t) ∉ comp(u)`. Hence `(u, o) = (X(t), |v|)` is the only parent
  occurrence: contradiction. ∎

Together with exactness (§3.3, which uses (S) before the step) and
confluence (T2), `settle` computes exactly this fixpoint.

### C.4 M3: G4 and G5 on `G(V(D))`

**G4 (overlap closure).** Let `x@a` and `y@b` (`a < b`) be consecutive
elements of `comp(U)` in `G(V(D))` with overlap `z = U[b .. a+|x|]`.
Then `z ∈ V(D)`.

*Proof.* `y` ends after `x` (maximality), so `x@a` is followed in `U` by
an atom.

- *Right contexts.* `x ∈ MR` has two right contexts. If `x` is a text,
  it has `$` and that atom. `z` is a suffix of `x`, so
  `R(z) ⊇ R(x)` (F1 with `o + |z| = |x|`).
- *Left contexts.* `b ≥ 1`, so `y` is preceded in `U` by an atom, and
  `L(z) ⊇ L(y)` has ≥ 2 elements.
- Hence `z` is an atom or in `MR(D)`. ∎

**G5 (unique minimal host).** Let `N ∈ sub(D) \ V(D)` with `|N| ≥ 2`.
The occurrences `(K, q)` of `N` in vertices with `N@q` inside no element
of `comp(K)` are exactly `{(X(N), |v|)}`.

*Proof.*

- *Containment.* Every vertex `K ⊇ N` with `|K| ≥ 2` is a text or in MR,
  so it contains `X(N)` at `q − |v|` (F3.3).
- *Only `X(N)`.* If `K ≠ X(N)`, that occurrence lies inside an element of
  `comp(K)` together with `N@q`, so `(K, q)` is not a minimal host.
- *Unique offset.* `X(N)` contains `N` only at `|v|`: another offset
  `q'` would give `X(N)` at `q' − |v|` inside `X(N)`.
- *Existence.* No child of `X(N)` contains `X(N)`, and `X(N) ∈ V(D)`
  (F3.1, F3.2). ∎

### C.5 M5 and adding an existing vertex as an input

**M5.** `L_D(s) ⊆ L_{D+[X]}(s)` and `R_D(s) ⊆ R_{D+[X]}(s)` for every
string `X`: occurrences are only added, and the contexts of existing
occurrences are unchanged because boundaries are indexed by text. Hence
`MR(D) ⊆ MR(D + [X])` and `V(D) ⊆ V(D + [X])`. ∎

**Lemma AV.** If `N ∈ V(D)`, then `V(D + [N]) = V(D)`.

*Proof.* Only substrings `s ≠ N` of `N` gain occurrences: those in the
new text `N`. `N` occurs in `D`, so an inner occurrence (neither prefix
nor suffix of `N`) has the contexts of the same position inside an
occurrence of `N` in `D`.

- *`s` a prefix, not a suffix, of `N`.* `s` gains only `^N` on the left;
  its right contexts are unchanged. Suppose `s` becomes MR. Then `s` was
  right-branching and `|L_D(s)| = 1`. The prefix occurrences inside the
  occurrences of `N` in `D` give `L_D(N) ⊆ L_D(s)`, so `N ∉ MR`, so `N`
  is a text `T_k`. Then `L_D(s) = {^k}`, so `count_D(s) = 1` (F2), and
  `s` was not right-branching: contradiction.
- *`s` a suffix, not a prefix, of `N`.* Symmetric.
- *`s` a border (prefix and suffix).* `s` gains `^N` and `$N`. Suppose
  `|R_D(s)| = 1`. The prefix occurrence inside each occurrence of `N`
  has right context `N[|s|]`, and the suffix occurrence has the right
  context of that occurrence of `N`. So `R_D(N) = {N[|s|]}`, an atom.
  Then `N ∉ MR`, and `N` is no text (a text has `$`): contradiction with
  `N ∈ V(D)`. Symmetrically `|L_D(s)| ≥ 2`, so `s ∈ MR(D)` already. ∎

So an insertion that finds an existing vertex (`Complete`,
`NoExpansion`, a single atom) leaves `G(V(D + [N])) = G(V(D))`.

### C.6 The reader step and M4

**Model of one step** (`read.rs`, `expand.rs`, `forward.rs`,
`root.rs`):

- *Before the step.* Committed corpus `C` (finished inputs, committed
  graph `G(V(C))`), input `I`, cursor `c`, root string `P = I[..c]`
  (`ε` before the first step), `D = C + [P]`.
- *The step* consumes `m ≠ ε`; `P' = P·m` and `D' = C + [P']`. It
  inserts `N` (none for an unknown atom); then `P' = P[..o]·N`, where:
  - **(U) unknown atom:** `m = x ∉ sub(D)`, no insertion, `o = |P|`.
  - **(E) expansion** (cursor within the run > 0): the chain (Lemma Ch)
    is probed largest first.
    - The accepted postfix `S` gives `N = E = S·m`, the longest prefix of
      `S·rest` in `sub(D)`, with `|E| > |S|`; `o = |P| − |S|`.
    - Every larger chain member `Y` was rejected: `Y·m[0] ∉ sub(D)`.
      Rejected probes do not edit.
  - **(F) forward.** Either all chain members were rejected, or there was
    no probe at cursor 0 of a run. `N = m = t` is the longest prefix of
    the rest in `sub(D)` (a single atom if nothing longer occurs);
    `o = |P|`.
- *Root commit:* `Adopt`, `InPlace` (`extend` of an editable root) or
  `NewRoot` (`create(maximal([R@0, N@o]))`), then `settle`.

**Assumption A1 (insert contract).** Inserting an occurring string `N`
into `G(V(D))` yields `G(V(D + [N]))`. For `N ∈ V(D)` this is the
identity (Lemma AV), so A1 concerns only `Created`. It is the insert
plan's gate property (`insert_gate`); the per-step lane (§5.3) checks it
on every step. It is not proven here.

**Lemma Ch (chain).** In `G(W)`, the postfix iteration of the root `R`
yields exactly the vertices that are proper suffixes of `P`, largest
first.

*Proof.* Two occurrences ending at `|P|` are nested, so the last element
of `comp(R)` contains every vertex occurrence ending at `|P|`: it is the
largest proper vertex suffix `L`. Every smaller vertex suffix of `P` is
a vertex suffix of `L`; recurse. ∎

**Lemma RM (right-maximal root).** At every step with `P ≠ ε`:
`P·m[0] ∉ sub(D)`.

**Lemma S (seam).** At every step and for every `k < o`:
`P[k..]·m[0] ∉ sub(D)`. Consequently every string `s` with an occurrence
at some `k < o` in `P'` and `k + |s| > |P|` (it *straddles* the seam) is
not in `sub(D)` and occurs exactly once in `D'`; in particular
`s ∉ MR(D')`.

*Proof of RM and S, by joint induction over the steps.* This is part of
the induction of M4: before step `n` the stage is `G(V(D))`, so the
chain consists of the vertices of `V(D)` that are proper suffixes of `P`
(Ch). Probes return the longest prefix of `[Y] ++ rest` in `sub(D)`
(search contract).

*RM at step `n`.* Let `P = P₋·m₋`, where `P₋` and `m₋` are the root and
the consumed atoms of step `n − 1`.

- If `P₋ = ε` (first step): `P = x`, an unknown atom occurring once in
  `D`; or `P = t`, the longest prefix of the run in `sub(C)`. In the
  second case `P·m[0]` is a longer prefix of the input (or `m[0]` is an
  unknown atom); it is not in `sub(C)`, and not in `sub(P)` because it
  is longer.
- Otherwise `o₋ ≥ 1` (`S₋` is a proper suffix, or `o₋ = |P₋|`), so `P`
  straddles the seam of step `n − 1` at `k = 0`. By S(n − 1),
  `P ∉ sub(D₋) ⊇ sub(C)`. `P·m[0]` contains `P`, so it is not in
  `sub(C)`, and it is longer than `P`. So `P·m[0] ∉ sub(D)`.

*S at step `n`.*

- **(U):** `m[0] = x ∉ sub(D)`.
- **(F) at cursor 0 of a run with a root:** the previous step was (U), so
  `P` ends with an atom that occurs in `D` only there. `P[k..]·m[0]`
  contains it followed by `m[0]`.
- **(E), and (F) at cursor > 0:** let `k < o` and `Z = P[k..]`, so
  `|Z| > |S|`, where `|S| = 0` for (F). Suppose `Z·m[0] ∈ sub(D)`.
  - Let `Y = P[j..]` (`j ≤ k`) be the shortest suffix of `P` with
    `|Y| ≥ |Z|` that is left-branching in `D` or equals `P`.
  - Each `P[i..]` with `j < i ≤ k` has the single left context
    `P[i−1]`, so every occurrence of `Z` extends to an occurrence of
    `Y`, and `Y·m[0] ∈ sub(D)`.
  - `Y` has the right contexts `$P` and `m[0]`.
  - If `Y = P`, this contradicts RM.
  - Otherwise `Y ∈ MR(D)` is a vertex and a proper suffix of `P`, on the
    chain (Ch) and larger than `S`. Its probe finds at least `Y·m[0]` and
    is accepted, contrary to the probe order and the rejections.

*Consequence.* A straddling `s` at `k` contains `P[k..]·m[0] ∉ sub(D)`,
so it occurs neither in `C` nor in `P` nor in `P'[o..] = N`, which are
all in `sub(D)`. A second occurrence would also straddle, at some
`k' < o` with `k' ≠ k`; let `k < k'` and `d = k' − k`.

- Both occurrences give `P'[i] = P'[i+d]` for `k ≤ i < k + |s|`.
- Because `k' < o ≤ |P|` and `k + |s| > |P|`, this covers `i = |P| − d`,
  so `m[0] = P'[|P|] = P[|P| − d]`.
- It also covers `i ∈ [k, |P| + 1 − d)`, so
  `P[k'..]·m[0] = P'[k'..|P|+1] = P'[k..|P|+1−d]`, which lies inside
  `P`.
- So `P[k'..]·m[0] ∈ sub(D)`, contradicting S for `k'`. ∎

**Corollary RR.** After the first step, `ReplaceRoot` never happens.
Because `o ≥ 1`, `P'` straddles at `0`, so
`P' ∉ sub(D) = sub(D + [N])`, and `find_complete` finds no vertex. The
only `Adopt` is the first `Append`. The code is unchanged; the case is
dead after the first step (0 of 45297 steps, Appendix B).

**Lemma RC (root commit).** After the insertion, the stage is
`G(V(D + [N]))` (A1; for (U), `G(V(D))`). The root commit yields `G(W)`
with:

- `InPlace`: `W = V(D + [N]) \ {P} ∪ {P'}`;
- `NewRoot`: `W = V(D + [N]) ∪ {P'}`;
- first step: `W = V(D + [N])` with `P' = N` adopted.

*Proof.* `P'` is a new string (RR). Every occurrence of a string of
`W \ {P'} ⊆ sub(D)` in `P'` is non-straddling (S; an atom cannot
straddle because `o ≤ |P|`). So it lies in `P` or in `N@o`, and is
contained in `R@0`, in an element of `comp(R)` (`InPlace`), or in
`N@o`. Hence `maximal(comp(R) ∪ {N@o})` and `maximal([R@0, N@o])` are
the G2 compositions of `P'`.

- `P'` occurs in no other vertex.
- In `InPlace`, `P` was parentless, so removing it as a string changes
  no other composition. ∎

**Lemma IP (in-place root).** `InPlace` ⇒ `P ∉ MR(D')`.

*Proof.* The root is editable: `p(P) = 0` in `G(V(D + [N]))`. By H,
`P` has no occurrence in `D + [N]` other than the text `P`:

- a committed text equal to `P` would be the same vertex as the staged
  root (G1);
- `N ≠ P` (`E ≠ R`, `t ≠ R`), so an occurrence inside `N` would be a
  proper one with a host, hence a parent.

In `D'`, `P` cannot straddle: straddling strings occur once (S), but `P`
also occurs at `0`. A non-straddling occurrence other than at `0` would
lie in `C` or `N`, which was just excluded. So `count_{D'}(P) = 1`. ∎

**Lemma MC (coverage).** `MR(D') ⊆ V(D + [N])` (for (U): `⊆ V(D)`).

*Proof.* Let `t ∈ MR(D')`. By S, `t` has no straddling occurrence. Map
each occurrence of `t` in `D'` to one in `D + [N]`:

- an occurrence in `C` maps to itself;
- one at `k` with `k + |t| ≤ |P|` maps to `(P, k)`;
- otherwise `k ≥ o`, and it maps to `(N, k − o)`.

The image's context is either the original atom or a boundary of the
image's text, at its start or end. Two occurrences with different right
contexts keep different right contexts:

- two atoms are the originals, which differ;
- an atom and a boundary differ;
- two boundaries `$` of the same text would be the same image, but the
  map is injective (different texts, or the same offset shift).

Left contexts are symmetric. So `t` is left- and right-branching in
`D + [N]`, or it is the text `P` or `N`; either way `t ∈ V(D + [N])`. ∎

**Theorem M4.** Suppose that before a step:

- the committed graph is `G(V(C))`;
- the stage is `G(V(D))` up to unread atoms;
- (S) holds.

Then after insertion, root commit and `settle`, the stage is
`G(V(D'))` up to unread atoms, with root `P'`.

*Proof.* `W` is given by RC. Apply Theorem F with
`E = atoms ∪ V(C) ∪ {P'}`:

- `E ⊆ V(D')` by M5.
- `texts(D') = texts(C) ∪ {P'} ⊆ E`.
- `W ⊆ sub(D')` because `sub(D) ⊆ sub(D')`.
- `MR(D') ⊆ W` by MC; in the `InPlace` case `P` is removed from `W`, but
  `P ∉ MR(D')` by IP.

(S) before the step holds by the corollary of M2. By exactness (§3.3)
`settle` reaches a fixpoint, and by Theorem F its vertex set is
`V(D')`. ∎

**Corollary (reads).** By induction over the steps and reads (base: the
empty graph), the committed graph after reading `I` is
`G(V(C + [I])) = oracle(C + [I])` (M1). G6 holds on it (M2), so the
commit check never panics. This holds relative to A1.

### C.7 Theorem E: the exact early-commit rule

> **Superseded (2026-11-26)** by
> [20261126_PLAN_CHEAP_STABILIZATION](20261126_PLAN_CHEAP_STABILIZATION.md)
> (Rule S, Theorem S). Theorem E, the right-context walk (Lemma W) and the
> uniformity test are no longer used by the implementation or the tests;
> this section is kept as design history.

Setting: after `settle` at cursor `c`, the stage is `G(V(D))` (M4). A
staged composite `t ≠ R` is in `MR(D) \ texts(D)`.

- `F(t) = R_D(t) \ {$P}`: the right contexts of all occurrences except
  the one ending at the cursor.
- `t` is *permanent* if `t ∈ V(C + [I[..c']])` for every continuation of
  the input and every `c' ≥ c`.

By M4 and M2, a permanent vertex is never removed by any `settle`: it
has p ≥ 2 in every intermediate set. Insertion and root commits remove
no other vertex. By M5 it is never removed in later reads either.

**Theorem E.** `t` is permanent iff:

- (a) `t` is not a suffix of `P`, or
- (b) `|F(t)| ≥ 2`, or
- (c) `F(t) = {$k}` for a committed input `k`, or
- (d) `F(t) = {a}` and `t = aⁿ`.

*Proof.* Left contexts only grow, and `|L_D(t)| ≥ 2`, so `t ∈
MR(C + [I[..c']])` iff it has two right contexts there. Moreover
`R_{C+[I[..c']]}(t) ⊇ F(t)`.

- (a) No occurrence ends at the cursor, so `F(t) = R_D(t)`, which has
  ≥ 2 elements.
- (b) Immediate.
- Otherwise `t` is a suffix of `P`, and `R_D(t) = F(t) ∪ {$P}` with
  `|R_D(t)| ≥ 2`, so `|F(t)| = 1`.
- (c) The cursor occurrence later has the context `I[c]` (an atom) or
  the end of the current prefix, both `≠ $k`.
- (d) Let `e ≥ c` be the end, within `I[..c']`, of the maximal run of
  `a`s containing position `c − 1`. The occurrence of `aⁿ` ending at `e`
  has a right context other than `a`.
- Not (c) or (d): `F(t) = {a}` with `t ≠ aⁿ`. Continue with `a` and end
  the input.
  - The occurrences of `t` in `P·a` are the fixed ones (context `a`), the
    cursor occurrence (now followed by `a`), and those ending at `c + 1`.
  - An occurrence ending at `c + 1` would make `t` a suffix of both `P`
    and `P·a`. Then `t[i] = P[c−n+1+i] = t[i+1]` for `i < n − 1` and
    `t[n−1] = a`, so `t = aⁿ`: excluded.
  - So `R(t) = {a}`, `t ∉ V(C + [P·a])`, and by M4 the last step's
    `settle` removes `t`. ∎

**Lemma W (walk).** In `G(W)` with `texts(D) ⊆ W ⊆ sub(D)`:

`R_D(t) = ⋃_{(U,o) parent} ctx(U, o) ∪ {$k : T_k = t}`

where `ctx(U, o) = {U[o+|t|]}` if `o + |t| < |U|`, else `R_D(U)`. The
recursion terminates because `|U| > |t|`.

*Proof.*

- By H, every occurrence of `t` that is not a whole text lies at
  `o = x − y` in a minimal host `(k, y)` of `U` with parent occurrence
  `(U, o)`. Its right context is `U[o+|t|]`, or that of the occurrence of
  `U` when `t` ends at `U`'s end.
- Conversely, every parent occurrence combined with every occurrence of
  `U` yields an occurrence of `t`. ∎

For `F(t)`, the walk drops `$P`: the root is not treated as an input.
Recognizing only parentless committed vertices as inputs drops `$k` for
committed inputs with parents. So the implemented walk returns a subset
of `F(t)`: stabilization is sound and may be late only in that case.
Uniformity (d) is local: `t = aⁿ` iff every child in `comp(t)` is `a` or
uniform in `a`.

**Invariant (U).** After `stabilize`, every unstable staged vertex other
than the root is a proper suffix of `P'` (by (a), every other vertex is
stable). So the vertices to check at the next step are the unstable slot
range: at most the chain plus the step's new tokens.
