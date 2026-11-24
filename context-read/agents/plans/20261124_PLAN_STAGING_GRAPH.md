---
tags: `#plan` `#context-read` `#context-trace` `#context-insert` `#context-search` `#positioned-composition` `#reader` `#retention` `#staging`
summary: Staging graph for the positioned reader. Tokens created during a read are staged (content may change, may be dissolved); everything else is committed (content immutable forever, only composition shape may change). The read commits its stage at input end. Dissolution of staged tokens with a single parent occurrence fixes the provisional-retention deviations (9 scenarios, 1 read-lane case). The stores are abstracted by traits (GraphRead/Navigate, sealed GraphWrite behind a generic GraphEdit, Stage with commit and Translation, GraphHandle, StagingStrategy), so search, insertion, settle and the reader are written once. Two implementations are built side by side and benchmarked: (a) a physical overlay graph next to the committed graph; (b) a stage tag on vertices of the one graph. The default strategy is decided after the evaluation.
status: 📋 draft revision 2 — both implementations behind traits; default strategy open (D-S1)
date: 2026-11-24
parent: 20261122_PLAN_POSITIONED_READER.md (§5 provisional retention, phase R4)
depends_on: 20261122_PLAN_POSITIONED_READER.md (R1–R3 implemented), 20261122_PLAN_POSITIONED_INSERT.md (revision 3, GraphEdit D8)
---

# Plan: Staging Graph

**Crates:** `context-trace` (graph, `GraphEdit`), `context-read` (reader),
`context-insert` and `context-search` (graph parameter made generic, §3).
**Reader:** [20261122_PLAN_POSITIONED_READER](20261122_PLAN_POSITIONED_READER.md)
(§5 retention, §8 phase R4).
**Insertion:** [20261122_PLAN_POSITIONED_INSERT](20261122_PLAN_POSITIONED_INSERT.md)
(invariants G1–G5 §3.1, `GraphEdit` D8, generation D9, no rollback D10).

## 0. Problem

The reader inserts every accepted match at once (legacy rule). A token that
a prefix of the input makes frequent can lose that status later in the same
input: all its occurrences end up inside occurrences of one larger token,
and the oracle does not retain it.

Example `abcabca`: after `abcabc`, `abc` is a border of the input read so
far and correctly a token (root `[abc@0, abc@3]`). The final `a` shifts
the border chain to `abca`, `abcabca`. Every occurrence of `abc` now lies
inside `abca`; the oracle has `abca = a@0, b@1, c@2, a@3` and no `abc`.

In general, the oracle tokens of `p^n t` (`t` a proper prefix of the
pattern `p`) are the chain `p^k t`; whether `p` is a token is decided by
the first atom of `t`, `(n − 1)·|p|` positions after `p` was committed. No
bounded window decides it (reader plan, analysis of the overhang variant
with full-period denial: fixes `n = 2` only).

**Evidence (2026-11-23):**

| Check | Result |
|---|---|
| Scenarios deviating (ignored, "provisional retention") | 9: `repeat_abcabca`, `repeat_ababa`, `insert_periodic`, `period_ab_x3_a`, `period_abc_x3_a`, `period_abc_x3_ab`, `period_abcd_x2_a`, `period_abcd_x2_abc`, `period_abcd_x3_ab` |
| Read-lane cases deviating | 1: `insert_deep_borders__1` (`READ_DEVIATIONS`) |
| Whole repetitions `p^n` (17 `period_*` scenarios) | all equal the oracle |
| Kind of deviation | only extra tokens, always created by the current read |
| R6 (§2.2) on all oracle graphs (v3, insert and read lanes) | holds on 1630 / 1630 |
| R6 on the reader output | violated in exactly the 10 deviating graphs |
| Offline dissolution (§2.4) of the reader output | 94 / 94 scenarios and 140 / 140 read-lane cases equal the oracle, in either order |

The simulation script is not in the repository yet; task S0 adds it as a
prototype next to `read_legacy_port.py`.

The root manager is already a one-vertex staging area: an **owned** root
(created by this read, parentless) is edited in place. This plan makes that
explicit for every token the read creates and adds the one missing edit,
dissolution.

## 1. Goals and non-goals

Goals:

- A strict boundary between **committed** tokens (string content immutable
  forever) and **staged** tokens (created by the running read; content may
  change, may be removed).
- Dissolution of staged tokens that violate R6, so every read yields
  `oracle(C + [I])` with no expected deviations.
- Search and insertion see staged tokens through ordinary parent links, so
  the reader loop (reader plan §3.1) stays unchanged.
- Both implementations behind common traits (§3), compared by tests
  and benchmarks (§7); the trait layer is the basis for several stores or
  concurrent readers later.
- A structure that a later "overhang" (a periodic pattern that grows during
  the search) can build on.

Non-goals:

- No change to the reader loop, the search algorithm or the insertion
  algorithm (only their graph parameter becomes generic, §3).
- No rollback (insert plan D10): a stage is always committed; dissolution
  is a forward edit, not an undo.
- No concurrent reads in this revision (one open stage per graph); the
  overlay implementation keeps the door open (§4.4).

## 2. Model (common to both options)

### 2.1 Vertex states

| | Committed | Staged |
|---|---|---|
| String content | **immutable forever** | may change: `extend` while parentless (as the owned root today) |
| Existence | permanent | may be removed (`dissolve`) |
| Composition shape | may change (insertion adds or replaces occurrences; dissolution of a staged child replaces it by its children); content constant | may change |
| Invariants | G1–G5 and **R6** | G1–G5; R6 may be violated (provisional) |
| Created by | atoms (always committed), `Stage::commit` | `create` during an open stage (insertion, root commits) |

Every committed vertex keeps its string: no edit removes it or changes its
width. This is what other reads and later inputs may rely on.

### 2.2 Retention invariant R6

> **R6.** Every committed vertex that is neither an atom nor a corpus input
> has at least two parent occurrences (entries `(parent, offset)`).

A vertex with a single parent occurrence `(U, o)` occurs only inside `U`
at offset `o`, so its string has no context of its own; the oracle does not
retain it. R6 holds on every oracle graph (§0). R6 needs an `input` mark on
vertices (the final root of each read).

### 2.3 Stage lifecycle

```text
read(graph, chars):
  stage := S::open(graph)             # one per read (StagingStrategy, §3.5)
  ... reader loop (reader plan §3.1); every root commit is followed by settle(stage)
  S::commit(stage, root)              # Stage::commit: root marked input; all staged → committed; R6 asserted
```

- Atoms are committed at once (they are never dissolved).
- A standalone `insert(graph, query)` opens and commits its own stage; the
  query is the input, consistent with `insert_gate` (`oracle(C + [query])`).
- Commit boundary in revision 1: **input end**. An early commit (a staged
  token whose occurrences are not all inside one larger token can never
  violate R6 again) is open question Q2.

### 2.4 Dissolution

```text
dissolve(t):   precondition: t staged, not an atom, not the stage root,
               exactly one parent occurrence (U, o)
  comp(U) := maximal(comp(U) − t@o ∪ (comp(t) + o))
  links of t's children: (t, a) → (U, o + a), dropped where filtered by maximal
  remove t

settle(stage): while some staged t ≠ root has ≤ 1 parent occurrence: dissolve(t)
```

- **G2 after dissolution.** `comp(t)` is complete inside `t`; occurrences
  crossing `t@o` are already in `comp(U)`; `maximal` removes occurrences
  that are now inside another one. Proof to be written (task S1).
- **G4** by the intersection lemma (reader plan §3.4).
- **Termination:** each step removes one vertex. Cascades (a child loses an
  occurrence through `maximal`) are handled by the worklist.
- **Zero parent occurrences** (a former root that was replaced): removed
  without substitution.

### 2.5 Decisions never depend on dissolved tokens

**Lemma (redundancy).** A staged token `t` with a single parent occurrence
`(U, o)` occurs only inside `U` at offset `o`. Dissolving it does not change
any reader decision:

| Decision | Depends on | Effect of dissolving `t` |
|---|---|---|
| forward `insert_next_match(rest)` | the string of the longest occurring prefix | none; at most `Complete` becomes `Created` |
| probe `[P] ++ rest`, `P = t` | `t` is a postfix of the root | `U` ends where `t` ends and is probed first; `[t] ++ rest` matches only if `[U] ++ rest` does |
| `find_complete([R, t'])` | existence of `Y` | if `Y` was dissolved, a new root with the same string is created |
| root, `P`, `E` within one step | identities | `settle` runs only between steps; the root is exempt; `E` has ≥ 2 parent occurrences after the commit |

So the reader may hold only the root across `settle`, and decisions rely on
strings, never on staged identities. Proof to be written (task S1).

### 2.6 Walkthrough `abcabca`

| Step | Graph (`S` = staged) |
|---|---|
| segments | `a`, `b`, `c` committed |
| unknown `a`, `b`, `c` | root S1 `ab`, extended in place to `abc` |
| forward `abc` | finds S1 itself (no edit), `t = R`; new root S2 `[S1@0, S1@3]` |
| probe `[S1, a]` | matches `S1@0, a@3` in S2; insertion creates S3 `abca = [S1@0, a@3]`; S2 `[S3@0, S1@3]` |
| commit `Expand` | `extend(S2, S3@3)`: S2 `[S3@0, S3@3]` |
| `settle` | S1 has one parent occurrence `(S3, 0)`: dissolve, S3 `[a@0, b@1, c@2, a@3]` |
| `Stage::commit(S2)` | S2 input; S3 committed (2 parent occurrences); equals the oracle |


## 3. Store abstraction (traits)

Both implementations (§4, §5) are built side by side behind the same traits,
so search, insertion, the edit layer, `settle` and the reader are written
once and the benchmarks compare only the stores (§7).

### 3.1 Coupling today

- Search and paths: about 60 signatures in `context-search` and
  `context-trace/src/positioned/path/*` take `&PositionedGraph<G>`;
  `impl Find for PositionedGraph<G>` and `impl Find for PositionedGraphRef<G>`
  (`context-search/src/search/mod.rs`).
- Navigation (`token`, `start_at`, `end_at`, `prefix_child`,
  `parent_occurrences`, `atom_token`, `advance_end_path`) is inherent on
  `PositionedGraph` (`positioned/navigate.rs`); it only uses `vertex()`
  (width, composition, parents) and the atom index.
- `GraphEdit<'g, G>` (`positioned/edit.rs`) holds `&mut PositionedGraph<G>`
  and uses few raw primitives: push a vertex, set a composition or width,
  link or unlink a parent, the atom index, the generation.
- `insert_match`/`insert_next_match` and the reader take
  `&PositionedGraphRef<G>` and lock per step (`find_ancestor` reads,
  `insert_match` writes).

### 3.2 `GraphRead` and `Navigate` (context-trace)

```rust
pub trait GraphRead {
    type Kind: GraphKind;
    type Parents<'a>: Iterator<Item = (Token, AtomPosition)> where Self: 'a;

    fn width(&self, v: VertexIndex) -> Option<TokenWidth>;
    fn composition(&self, v: VertexIndex) -> Option<&Composition>; // None for atoms
    fn parents(&self, v: VertexIndex) -> Self::Parents<'_>;
    fn atom_token(&self, atom: &AtomOf<Self::Kind>) -> Option<Token>;
    fn state(&self, v: VertexIndex) -> VertexState;                 // Committed | Staged
    fn is_input(&self, v: VertexIndex) -> bool;
    fn generation(&self) -> Generation;                             // D9 staleness
}

/// Today's navigate.rs bodies as provided methods.
pub trait Navigate: GraphRead { … }
impl<T: GraphRead + ?Sized> Navigate for T {}
```

The associated `Parents` iterator lets the overlay merge base and staged
links without allocation. `Generation` is `(base, stage)` for the overlay.

### 3.3 `GraphWrite` (sealed) and the generic `GraphEdit` (context-trace)

```rust
pub trait GraphWrite: GraphRead + sealed::Sealed {
    fn push_vertex(&mut self, node: VertexNode<AtomOf<Self::Kind>>, width: TokenWidth) -> VertexIndex;
    fn set_composition(&mut self, v: VertexIndex, c: Composition);
    fn set_width(&mut self, v: VertexIndex, w: TokenWidth);
    fn link(&mut self, child: VertexIndex, parent: VertexIndex, offset: AtomPosition);
    fn unlink(&mut self, child: VertexIndex, parent: VertexIndex, offset: AtomPosition);
    fn remove_vertex(&mut self, v: VertexIndex);
    fn register_atom(&mut self, atom: AtomOf<Self::Kind>, v: VertexIndex);
    fn bump_generation(&mut self);
}
```

- Sealed: only stores in `context-trace` implement it, and only
  `GraphEdit` calls it. D8 holds: no other crate mutates unchecked.
- `GraphEdit<'g, S: GraphWrite>` keeps every precondition and `maximal`
  as today and gains `dissolve` (§2.4). State-dependent preconditions
  (`extend` and `dissolve` only on staged vertices) use `state()` and are
  written once for both implementations.

### 3.4 `Stage` (context-trace)

```rust
pub trait Stage: GraphWrite {
    fn root(&self) -> Option<Token>;
    fn staged(&self) -> impl Iterator<Item = VertexIndex> + '_;
    /// Every staged vertex becomes committed; `root` is marked input; R6 asserted.
    fn commit(self, root: Token) -> Translation; // staged index → committed index
}

pub fn settle<S: Stage>(edit: &mut GraphEdit<'_, S>, root: Token); // §2.4 worklist
```

`Translation` is the identity for (b). Committing is the "settling between
graphs": the only operation that moves vertices across the boundary.

### 3.5 `GraphHandle` and `StagingStrategy` (context-trace, context-read)

```rust
pub trait GraphHandle {
    type Read<'a>: Deref<Target: GraphRead> where Self: 'a;
    type Write<'a>: DerefMut<Target: GraphWrite> where Self: 'a;
    fn read(&self) -> Self::Read<'_>;
    fn write(&self) -> Self::Write<'_>;
}

pub trait StagingStrategy {           // context-read
    type Handle<'g>: GraphHandle;
    fn open<'g>(graph: &'g PositionedGraphRef) -> Self::Handle<'g>;
    fn commit(handle: Self::Handle<'_>, root: Token) -> Translation;
}
```

- The handle keeps the per-step locking: `insert_next_match<H:
  GraphHandle>(&H, query)` and `Find` for any handle.
- Generics, not `dyn`: monomorphization keeps (b) as fast as today and the
  benchmarks measure stores, not dispatch.

### 3.6 Implementers

| Type | Crate | Implements | Role |
|---|---|---|---|
| `PositionedGraph<G>` | context-trace | `GraphRead`, `GraphWrite` | committed store; for (b) also per-vertex `state`, `input` and tombstones |
| `PositionedGraphRef<G>` | context-trace | `GraphHandle` | committed-only handle (standalone use, tests) |
| `StagingGraph<G>` | context-trace | storage only | (a): staged vertices, copy-on-write compositions of committed hosts, extra and removed parent links of committed children |
| `Overlay<'g, G>` | context-trace | `GraphRead`, `GraphWrite`, `Stage` | (a): base `&PositionedGraph` + `&mut StagingGraph` |
| `OverlayHandle<'g>` | context-trace | `GraphHandle` | (a): base read lock per step plus the stage |
| `TaggedStage<'g, G>` | context-trace | `GraphRead`, `GraphWrite`, `Stage` | (b): wraps `&mut PositionedGraph` with an open stage |
| `TaggedHandle<'g>` | context-trace | `GraphHandle` | (b) |
| `OverlayStaging`, `TaggedStaging` | context-read | `StagingStrategy` | reader, tests and benchmark selection |
| `Find` for `V: GraphRead` and `H: GraphHandle` | context-search | blanket impls | search over any view |
| `SplitGraph`, `join`, `insert_match`, `insert_next_match` | context-insert | generic over `GraphRead`, `GraphEdit<S>`, `GraphHandle` | no algorithm change |
| `ReadStage<S: Stage>`, `read_with` | context-read | uses `Stage`, `settle` | replaces `RootManager` (§6) |

## 4. Implementation (a): physical overlay graph

### 4.1 Structures

```text
StagingGraph {
  vertices: Slab<StagedVertex>,                       # removal frees the slot
  compositions: Map<VertexIndex, Composition>,        # copy-on-write shape of committed hosts
  added_parents: Map<VertexIndex, Set<(VertexIndex, AtomPosition)>>,   # committed children
  removed_parents: Map<VertexIndex, Set<(VertexIndex, AtomPosition)>>,
  base_generation: u64,                               # base generation when opened
  generation: u64,
}
Overlay<'g> { base: &'g PositionedGraph, stage: &'g mut StagingGraph }
```

### 4.2 Index space

Staged vertices get indices from a reserved range (high bit set), so
`Token` and `VertexIndex` stay plain integers, `state(v)` is one bit test,
and search needs no type change. `commit` maps them to base indices
(`Translation`); the reader translates the only token it holds, the root.
Alternative: `VertexIndex` as an enum `Committed(i) | Staged(i)` (more type
churn; Q4).

### 4.3 Operations

- Read: staged index → stage; committed index → stage override for the
  composition if present, else base; parents = base links − removed + added.
- Write: only into the stage; the base is never touched before `commit`.
- `commit` (takes the base write lock): assert the base generation equals
  `base_generation`; push staged vertices children first, rewriting
  occurrences through the translation; apply composition overrides and link
  changes to committed vertices; mark the root input; assert R6.

### 4.4 Properties

- Explicit interface: staged vertices live in another type; the base is
  read-only for the whole read; other readers never see a stage.
- Costs: one stage check per vertex access, merged parent iteration,
  commit with translation, memory of the stage until commit.
- Concurrency (out of scope): several overlays over one base; a conflicting
  commit (two stages create the same string, G1) needs a rebase rule.

## 5. Implementation (b): logical stage tag

### 5.1 Changes

- `PositionedVertex { state: VertexState, input: bool, … }`;
  `PositionedGraph { vertices: Vec<Option<PositionedVertex>>, stage:
  Option<StageId>, … }` (tombstones keep indices stable; `vertex()`,
  `vertices()`, interchange, `diff` and `validate()` skip removed slots).
- `TaggedStage` writes into the base; `create` tags new vertices `Staged`;
  `commit` flips the tags of the stage's vertices, marks the root input and
  asserts R6.

### 5.2 Properties

- No lookup overhead beyond the tag; commit is linear in the staged
  vertices without copying.
- The boundary is enforced by `GraphEdit` preconditions at runtime.
- Other readers would see staged vertices and shape changes of committed
  hosts (fine with one read at a time).
- Tombstones stay until a compaction (not planned).

## 6. Reader integration

- `RootManager { root, owned }` becomes `ReadStage<S: Stage> { root }`;
  "owned" becomes "staged and parentless". The commit table (reader plan
  §3.4) is unchanged.
- After each root commit: `settle`.
- `read_with::<S: StagingStrategy>(graph, input)`: `S::open`, the reader
  loop, `S::commit(handle, root)`, return the translated root.
- `read(graph, input)` keeps its signature and uses the default strategy
  (decided in task S7).
- A standalone `insert(graph, query)` opens and commits its own stage
  (Q3).
- `GraphEdit::extend` precondition: staged and parentless (replaces
  `HasParents`).

## 7. Comparison and evaluation

Both implementations are kept and tested; the evaluation decides the
default strategy and whether (b) stays as an alternative.

| | (a) overlay | (b) tag |
|---|---|---|
| Interface | explicit (types) | runtime preconditions |
| Lookup cost | stage check per access, merged parents | none |
| Commit cost | copy and translate | flip tags |
| Isolation | yes | no |
| Memory | stage freed at commit | tombstones |
| Overhang (future) | stage is a natural scratch space | possible, less isolated |

**D-S1 (open):** default strategy. Preference: (a) for the interface,
unless the cost on search and insertion is too high.

### 7.1 Benchmarks (criterion, `context-read/benches`)

- Baseline: the current reader, recorded before task S3.
- Per strategy: reader throughput (atoms per second) on the v3 corpus
  inputs and on long periodic inputs (`p^n t`, large `n`).
- `find_ancestor` on committed graphs through `PositionedGraph` and through
  an empty `Overlay` (cost of the indirection alone).
- Commit cost relative to read cost; `settle` dissolutions per read and
  cascade depth; peak memory.

### 7.2 Tests for both

- A macro instantiates every reader scenario and the read lane for
  `OverlayStaging` and `TaggedStaging`.
- Differential test: both strategies produce the same interchange graph for
  every scenario and read-lane case.
- `insert_gate` on the committed store and on both stage types.
- Unit tests of `GraphEdit` preconditions (`extend`, `dissolve` on
  committed vertices rejected), `dissolve` postconditions, `commit`
  (translation, R6 assertion, generation check for (a)).

## 8. Open questions

1. **D-S1:** default strategy (§7).
2. **Commit boundary:** input end (this revision) or early commit of tokens
   that can no longer violate R6.
3. **Standalone insertion:** confirm that `insert` outside a read opens and
   commits its own stage.
4. **Index space of (a):** reserved range (proposed) or `VertexIndex` enum.
5. **Eager or deferred `settle`:** eager is expected to give
   `oracle(C + [I[..cursor]])` after every step (checked in S0).
6. **Proofs:** G2/G4 after `dissolve` (§2.4), the redundancy lemma (§2.5),
   R6 against the oracle's frequency rule.

## 9. Tasks

| Task | Content | Exit |
|---|---|---|
| S0 | Prototype: dissolution simulation under `context-insert/agents/prototypes`; eager `settle` in `read_legacy_port.py`, per-step comparison with the oracle of the prefix | §0 numbers reproduced; Q5 answered |
| S1 | Proofs (§2.4, §2.5); R6 as a fixture check in `fixture_validation` (all lanes) | written; R6 test green |
| S2 | Benchmark baseline of the current reader (§7.1) | numbers recorded here |
| S3 | Traits `GraphRead`, `Navigate`, `GraphWrite`, `Stage`, `GraphHandle`; `PositionedGraph` and `PositionedGraphRef` implement them; search, paths and insertion made generic; `GraphEdit<S>` | all gates green; benchmarks within noise of S2 |
| S4 | (b): state and input tags, tombstones, `TaggedStage`, `GraphEdit::dissolve`, `settle`, R6 in `validate()` | unit tests (§7.2) |
| S5 | (a): `StagingGraph`, `Overlay`, `OverlayHandle`, commit with translation | unit tests; differential tests against (b) on `GraphEdit` sequences |
| S6 | Reader: `ReadStage`, `read_with`, both strategies | gate (§10) for both strategies |
| S7 | Benchmarks per strategy (§7.1); decide D-S1 | decision recorded here |

## 10. Gate

```sh
cargo check -p context-trace -p context-search -p context-insert -p context-read --no-default-features --features test-api
cargo test -p context-read --no-default-features --features test-api
cargo test -p context-trace -p context-search -p context-insert --no-default-features --features test-api
cargo test --release -p context-insert --no-default-features --features test-api insert_gate
cargo test -p context-read --features legacy-tests
cargo bench -p context-read
```

- Every scenario and read-lane case passes for both strategies, none ignored
  for provisional retention; `READ_DEVIATIONS` is empty; the differential
  test is green.
- `validate()` (with R6) after every read and every insert case.
- Insert gate unchanged (all cases).
- S3 is performance-neutral against the S2 baseline; the strategy costs are
  recorded with D-S1.
