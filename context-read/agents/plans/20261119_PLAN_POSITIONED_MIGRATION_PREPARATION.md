---
tags: `#plan` `#context-read` `#context-insert` `#context-search` `#context-trace` `#ngrams` `#positioned-composition` `#legacy` `#refactor` `#testing`
summary: Prepare the crates for the positioned reader. Isolate the row-based implementation and its tests in `legacy` modules without behavior change, move the v3 interchange into context-trace, scaffold the positioned graph model with a v3 fixture round-trip loop, separate v2/v3 fixture manifests, add the missing reader scenarios, ignore legacy tests at the test-suite cutover (opt-in via `legacy-tests`), then promote reusable pieces out of legacy on demand and add read-only navigation. Algorithm work (reader plan Phase 3+) starts only after these gates.
status: ✅ complete (steps 1, 2, 4, 5 done; step 3 is pull-based and continuous) — open: editor-kernel build check (1.6), not runnable in the Linux codespace (winit)
revision: 2
date: 2026-11-19
parent: 20261118_PLAN_POSITIONED_COMPOSITION_READER.md (this plan is its preparatory "Phase −1" plus Phases 0-2)
related: ../../../ngrams/agents/plans/20261118_PLAN_POSITIONED_COMPOSITION_ORACLE_IMPLEMENTATION.md
---

# Plan: Positioned Migration Preparation

**Crates:** `context-trace`, `context-search`, `context-insert`, `context-read`,
`ngrams` (interchange types only), consumers `context-api`, `context-cli`,
`context-http`, `context-mcp`.
**Goal:** a workspace in which the row model is fenced off in `legacy`
namespaces, the positioned model exists with a green v3 fixture test loop,
and the new reader can be written in the main modules without touching legacy
code.
**Out of scope:** the positioned merge, seam discovery, accumulation and
streaming retention (reader plan Phases 3-5) and the legacy removal (Phase 6).

---

## 0. Baseline and research findings (2026-11-19)

Record these before changing anything; every step below is gated against them.

### 0.1 Test baseline

- `cargo test -p context-read`: `reader_behavior` has **6 known failures**:
  `accepted_materialization`, `flat_complement`, `infix`, `complex_overlap`,
  `nested_complements`, `multiple_overlaps`. All other context-read targets pass
  (lib 32, fixture_validation 4, reader_behavior 48/54, 3 ignored doc/other).
- Counted unit tests: `context-search` 60, `context-insert` 53,
  `context-read` src/tests/cursor.rs 16, request.rs 3.
- [x] Before Step 1, capture the full baseline into the session or a
  commit message: `cargo test --workspace --exclude context-editor-kernel --exclude sandbox-app --no-fail-fast 2>&1 | grep -E "^test .*(ok|FAILED|ignored)$" | sort`
  and keep the list for diffing after each move.

### 0.2 Row-model coupling (classification)

**A = model-agnostic, stays at crate root. B = row-specific, moves to legacy.
C = mixed, split.**

| Crate / module | Class | Notes |
|---|---|---|
| trace `direction/` | A | |
| trace `logging/` | A | |
| trace `graph/kind.rs` (`GraphKind`) | A | parameterizes atoms/direction, not storage |
| trace `graph/vertex/{vertex_index,token,key,atom…}` | A | `Token` string display currently looks up the row test graph (`token.rs:235`, `vertex_index.rs:43`) → needs a model-neutral name lookup |
| trace `graph/mod.rs` (`Hypergraph`, `HypergraphRef`) | C | storage container reusable in concept; vertex type is `VertexData` with `ChildPatterns` |
| trace `graph/vertex/{data,pattern,location,parent}` | B | `ChildPatterns`, `PatternId`, `ChildLocation{parent,pattern_id,sub_index}`, `Parent{PatternIndex}` |
| trace `graph/{insert,getters,snapshot,validation,search_path}` | B | snapshot edges are `pattern_idx/sub_index` |
| trace `graph/visualization.rs` | C | event plumbing reusable |
| trace `path/` | B | except `AtomPosition` (`path/mutators/move_path/key.rs`) → move to root |
| trace `trace/` | C/B | `has_graph` traits reusable in concept; cache/state/traceable row-based |
| trace `tests/test_case` | A | `TestCase`/`TestEnv` harness |
| trace `tests/{macros,env}` | B | `Env1/Env2`, pattern/path macros |
| search `container/` (BFT/DFT) | A | |
| search `policy.rs` | C | shell reusable, parent generation row-based |
| search `logging/` | A/C | |
| search `compare, match, cursor, state, search` | B/C | `Find`, `Response` are row-shaped |
| insert `insert/` (`InsertCtx`, `InsertOutcome`) | C | interface shape reusable |
| insert `visualization.rs` | A/C | |
| insert `interval, join, split, overlap` | B | incl. `scope_complement` (`insert/scoped.rs:417`), `Overlap::close` (`overlap/mod.rs:105`) |
| read `segment.rs`, `input.rs` | A | input → atom indices |
| read `pipeline/`, `expansion/`, `request.rs` | B | `RootManager` concept (ownership, commit boundary) reusable |

### 0.3 Dependency constraints

- `ngrams` depends on `context-trace` (its containment working graph is the
  row `Hypergraph`) → `context-trace` cannot depend on `ngrams`.
- `context-read` has `ngrams` only as a dev-dependency; `context-api`
  depends on `ngrams` (`parse_corpus_legacy` in `workspace/manager.rs`).
- Downstream imports mostly use root re-exports (`use context_trace::{…}`,
  ~147 sites); deep-path uses are few (`graph::vertex`, `graph::test_graph`,
  `graph::snapshot`, `trace::cache`, `context_search::response`,
  `context_insert::overlap::{prefix_frames, find_complete}`).

### 0.4 Fixture coupling

- `scenarios.json` is shared by both lanes; `reader_behavior::fixture_manifest_is_covered_and_round_trips`
  requires a v2 file for every manifest entry, and `fixture_validation`
  requires exactly one v3 file per entry. Adding v3-only scenarios breaks v2.
- Missing reader Phase-0 scenarios: standalone `abcabca`, `ababa`, seam case
  (`abc` / `cde` with retained `bcd`), late retention, duplicate observation
  of a composite.

### 0.5 Retention monotonicity probe (empirical, not a proof)

Probe: run `parse_corpus` on growing corpora and diff retained sets.

- **Across completed inputs:** 0 violations (51 manifest scenarios plus
  57 600 checks over 3-text sequences of words of width 1-4 over 2-3 letters).
- **Within one input (growing prefix):** 278 violations, e.g. prefix
  `abcab` retains `ab`, `abcabc` does not; `ababa` → `ab` lost at `ababab`.

Consequence for the reader: frequency-derived tokens must stay provisional
until an input completes; only input boundaries are publication points.
Must be written into the reader plan (Step 5) and turned into a permanent test
(Step 2.6).

---

## Step 1 — Isolate the row implementation in `legacy` (move only)

**Principle:** no behavior change, no logic edits beyond import paths and
visibility. One commit per crate, bottom-up. Modules stay inside their crates
(separate crates would break orphan rules for impls on shared `Token`/ID types
and lose `pub(crate)` access).

### 1.1 Feature flag

- [x] Add a default-on `legacy` feature to `context-trace`, `context-search`,
  `context-insert`, `context-read`, propagating downward
  (`context-read/legacy = ["context-insert/legacy", …]`).
- [x] `context-api`, `context-cli`, `ngrams` enable `legacy` explicitly.
- [x] CI-able check:
  `cargo check -p context-trace -p context-search -p context-insert -p context-read --no-default-features --features test-api`
  must compile (proves new code does not depend on legacy). This becomes
  meaningful once Step 2 adds code; in Step 1 it may compile nearly empty crates.

### 1.2 context-trace

- [x] Create `src/legacy/mod.rs`; move B modules: `graph/vertex/{data,pattern,location,parent}`,
  `graph/{insert,getters,snapshot,validation,search_path,test_graph}`, `path/`,
  row parts of `trace/`, `tests/{macros,env}` and row tests.
- [x] `Hypergraph`/`HypergraphRef` (row `VertexData`) move to
  `legacy::graph`; keep the name, so legacy code stays textually similar.
- [x] Keep at root: `direction`, `logging`, `GraphKind`, `VertexIndex`,
  `VertexKey`, `Token`/`TokenWidth`, atoms, `tests/test_case`, proc-macro
  re-exports, `HashMap/HashSet` aliases, `init_test_tracing`.
- [x] Move `AtomPosition` to a root module (e.g. `src/position.rs`) and
  re-export it from its old legacy path.
- [x] Replace the `Token`/`VertexIndex` → test-graph string lookup with a
  model-neutral hook (e.g. thread-local `TokenNameResolver` trait object that
  the legacy test graph registers). Behavior of existing debug output must
  not change.
- [x] Re-export: `pub use legacy::*`-style re-exports are **not** allowed at the
  crate root (they would hide the fence). Downstream imports change to
  `context_trace::legacy::{…}`.
- [x] `ngrams` imports switch to `context_trace::legacy::…`.

### 1.3 context-search

- [x] Move `compare, match, cursor, state, search` and row tests to
  `src/legacy/`. `Find`, `Searchable`, `Response` become
  `context_search::legacy::…`.
- [x] `container/` and `logging/` stay at root if they compile without legacy
  types; otherwise move now and promote later (Step 3).

### 1.4 context-insert

- [x] Move `insert, interval, join, split, overlap, visualization` and
  `tests/` under `src/legacy/`. Fix `context_read` deep paths to
  `context_insert::legacy::overlap::{prefix_frames, find_complete}`.

### 1.5 context-read

- [x] Move `pipeline/`, `expansion/`, `request.rs`, `src/tests/` under
  `src/legacy/`. Keep `segment.rs`/`input.rs` at root only if they compile
  without legacy types; otherwise move and promote in Step 3.
- [x] Public API becomes `context_read::legacy::{read, ReadCtx, ReadSequenceIter, SegmentResult, IntoReadInput}`.
  Do **not** keep an unqualified `context_read::read` alias; the name is
  reserved for the positioned reader.
- [x] Tests: `tests/reader_behavior.rs` → `tests/legacy_reader_behavior.rs`;
  `tests/support/mod.rs` split into `support/legacy.rs` (v2) and
  `support/positioned.rs` (v3) with a shared `mod.rs`.
- [x] Mark the 6 known failures `#[ignore = "legacy known failure: <ref>"]`
  and list them in the test file header and in the overlap provider plan,
  so the legacy gate is "all non-ignored pass" and regressions are visible.
  The list stays fixed; it is checked by review, not by a test. At the
  test-suite cutover (2.7) all legacy tests become ignored anyway.

### 1.6 Consumers

- [x] `context-api`: `commands/{insert,read,debug}.rs`, `types.rs`,
  `commands/compare.rs`, `workspace/manager.rs` → legacy imports.
- [x] `context-cli` incl. `tests/integration/ngrams_oracle_tests.rs`,
  `tests/common/graph_compare.rs`; `context-http`, `context-mcp` (via
  `context-api` only; verify).
- [ ] Context editor kernel: no direct dependency found; verify it still
  builds in an editor-capable environment (winit fails in the Linux codespace).

### 1.7 Docs

- [x] Update `FILE_INDEX.md`, `README.md` of each moved crate with the
  legacy/positioned split and the rule "no new code in `legacy`, only fixes
  needed to keep tests green".

**Step 1 done (2026-11-19).** Commits (bottom-up, each builds the workspace):
`5f33d5e` trace, `7dfc2ff` search, `840ffa2` insert, `ca3e5c8` read, plus the
docs/plan commit. Notes and deviations:

- `legacy` modules mirror the root (`pub use crate::*`, and
  `pub use crate::<module>::*` in split modules such as `legacy::graph`,
  `legacy::graph::vertex`, `legacy::direction`, `legacy::logging`,
  `legacy::tests`). Moved code only changed `crate::` → `crate::legacy::`;
  consumers changed `context_<crate>::` → `context_<crate>::legacy::`. The
  root never re-exports `legacy`.
- context-trace root impls for row types (e.g. `Wide for Pattern`,
  `HasVertexIndex for VertexData`, `SubToken`, `Token::down_key`,
  `GraphKind::Direction: PatternDirection`, `TestEnv::graph`, `AsGraphRef`
  impls) carry `#[cfg(feature = "legacy")]` instead of being split into new
  files.
- Name hook: `graph/vertex/name_resolver.rs` (thread-local
  `TokenNameResolver`); `legacy::graph::test_graph` registers the graph as the
  resolver; output is unchanged.
- search `container/`, `logging/`, `policy.rs` and read `segment.rs`,
  `input.rs` depend on row types → moved (promotion in Step 3). The roots of
  search/insert/read are empty apart from `pub mod legacy`.
- `context_read::legacy` additionally re-exports `ReadCtx` (listed in 1.5).
- Gate 1: passed 831 → 831, failed 7 → 1, ignored 79 → 85. The remaining
  failure is the baseline doctest `legacy::graph::test_graph (line 12)` (E0284,
  only in the workspace run). The 6 known failures print
  `ignored, legacy known failure: …`, which the baseline `awk` filter (anchored
  on `ignored$`) does not list.
- Editor kernel: no direct dependency on the four crates; not built here
  (winit).

**Gate 1:**
- `cargo build --workspace --all-targets --exclude context-editor-kernel --exclude sandbox-app`
- Full test list identical to the 0.1 baseline (modulo renamed test paths and
  the documented ignores).
- `git diff --stat` shows only moves (`git log --follow` works) and import
  edits; review with `git diff -M --stat`.

---

## Step 2 — Positioned model scaffold and v3 test loop (reader Phases 0-1)

### 2.1 Interchange types live in context-trace

- [x] Move `PositionedGraphData`, `PositionedGraphSnapshot`,
  `PositionedCover`, `POSITIONED_FORMAT_VERSION`, the structural validator and
  `unique_keys` from `ngrams/src/output.rs` to
  `context-trace/src/positioned/interchange.rs`.
- [x] `ngrams::output` re-exports them (no API break for ngrams users); move
  their tests (`ngrams/src/tests/output.rs` structural parts) along or keep
  them in ngrams calling the re-export — do not duplicate.
- [x] `ngrams::output::legacy` stays in ngrams.

### 2.2 Positioned model types (`context_trace::positioned`)

- [x] `Occurrence { offset: AtomPosition-like, child: VertexIndex }`.
- [x] `Composition` = sorted `Vec<Occurrence>` with constructor enforcing
  order `(offset, width)`, no duplicates, no contained intervals.
- [x] `PositionedVertex { index, key, width, composition, parents }` with
  reciprocal `parents: Vec<(VertexIndex /*parent*/, offset)>` (or a map
  parent → offsets).
- [x] `OccurrenceLocation { parent, offset, child }`; `OccurrencePath`
  (chain; absolute offset = sum).
- [x] `IntervalView { owner, range }` — explicitly not a token.
- [x] `PositionedGraph<G: GraphKind>` with its own storage, shared ID/atom
  types; insertion API that only accepts complete compositions (no gap,
  wrapper or slice vertices by construction).
- [x] `validate()`: oracle structural rules 1-8 plus reciprocal-link
  consistency and one vertex per content.
- [x] Decide and document: thread-safety (`DashMap` like legacy, or plain map
  behind `Arc<RwLock>`); keep it simple until the reader needs concurrency.
  **Decision (2026-11-19):** plain `Vec`/`HashMap` storage in
  `PositionedGraph` (not internally synchronized), shared as
  `PositionedGraphRef<G> = Arc<RwLock<PositionedGraph<G>>>` (newtype with
  `read()`/`write()`); readers share the lock, insertion is exclusive.
  Revisit only when the reader needs concurrent insertion.

### 2.3 Conversion and comparison

- [x] `PositionedGraph::from_interchange(&PositionedGraphData)` (bottom-up by
  width) and `to_interchange()`.
- [x] `fn diff(expected, actual) -> GraphDiff` listing missing/extra vertices
  and missing/extra occurrences per owner; readable failure output. This is
  the comparison the positioned reader tests will use.

### 2.4 Fixture manifests

- [x] Move v2 artifacts to `tests/fixtures/v2/` with a frozen
  `v2/scenarios.json` (copy of today's manifest). Update
  `legacy_reader_behavior` and v2 validation paths.
- [x] `tests/fixtures/v3/scenarios.json` becomes the authoritative manifest;
  regenerate command in README:
  `cargo run --release -p ngrams --bin ngrams -- --file context-read/tests/fixtures/v3/scenarios.json --out context-read/tests/fixtures/v3`.
- [x] `fixture_validation` checks each lane against its own manifest; v3
  still recomputes from `parse_corpus`.

### 2.5 New scenarios (v3 only)

Each expectation hand-checked against the oracle rules, never derived from
reader output. Add to v3 manifest and generate:

- [x] `abcabca` → `abcabca = [abca@0, abca@3]`, no `abc/bca/bc` vertices.
- [x] `ababa` → `ababa = [aba@0, aba@2]`.
- [x] Seam case: corpus where `abc` and `cde` are observed and `bcd` is
  retained (choose inputs so the oracle retains `bcd`; verify), combined
  vertex must list `bcd` crossing the seam.
- [x] Late retention: a token retained only after a later input
  (e.g. `vis` then a corpus making `vi` frequent); expected graph per prefix
  corpus — add one fixture per completed-input prefix
  (`<name>__1`, `<name>__2`, …).
- [x] Duplicate observation of a composite (`["abcabc", "abcabc"]`) and an
  input that is a substring of an earlier one.

### 2.6 Tests

- [x] `context-trace` unit tests: every v3 fixture → `PositionedGraph` →
  `validate()` → `to_interchange()` equals the fixture.
- [x] Validator rejection tests for model-only rules (broken reciprocal link,
  duplicate content) — structural rules are already tested in the
  interchange module; do not duplicate.
- [x] Content reconstruction of every vertex from its composition equals its
  key.
- [x] Retention monotonicity tests in `ngrams` (from 0.5): across completed
  inputs, retained sets are monotone for manifest scenarios plus a bounded
  generated set (normal test) and the larger sweep (ignored release test).
  Add one explicit counter-example test documenting within-input
  non-monotonicity (`abcab` → `abcabc` loses `ab`).
- [x] `context-read/tests/reader_behavior.rs` (new, positioned): one test per
  v3 scenario comparing `context_read::read` (positioned, not yet
  implemented) via `diff`; all `#[ignore = "positioned reader pending"]`.
  A meta-test asserts every v3 manifest entry has a test.
- [x] `context_read::read` positioned stub returning `unimplemented!()` or
  `Err(NotImplemented)` with the target signature
  (`&PositionedGraphRef, input) -> Option<Token>`), so test code compiles.

### 2.7 Test-suite cutover: legacy tests become ignored

Decision: when the new suite exists, **old tests are ignored**, not deleted
and not compiled out. They are still compiled and can be run on request.

- [x] Cutover point: immediately after the 2.6 tests pass and Gate 2 holds
  with the legacy lane still running. That is the last full legacy run.
- [x] Add a `legacy-tests` feature (off by default) to `context-trace`,
  `context-search`, `context-insert` and `context-read`, propagating like
  `legacy`. Mark every legacy test with
  `#[cfg_attr(not(feature = "legacy-tests"), ignore = "legacy suite")]`.
  This covers unit tests in moved modules, `tests/legacy_reader_behavior.rs`
  (including the `scenarios!` macro expansion), v2 fixture validation,
  `context-cli` legacy oracle comparisons and `context-api` tests that
  exercise row behavior.
- [x] The 6 known failures keep their unconditional
  `#[ignore = "legacy known failure: …"]`, so `--features legacy-tests`
  runs green.
- [x] Use the feature, not `cargo test -- --ignored`, to run legacy tests.
  `--ignored` also picks up the pending positioned reader lane and the slow
  ngrams sweeps.
- [x] Default `cargo test` from then on runs only the positioned suite, the
  oracle tests and model-agnostic tests. Document in each crate README:
  `cargo test -p <crate> --features legacy-tests` runs the legacy suite.
- [x] Run the legacy suite explicitly when a change touches `legacy` code or
  promotes something out of it (Step 3). Otherwise it is not a gate.

**Step 2 done (2026-11-19).** Commits (each builds the workspace):
`859a785` interchange move, `403a669` positioned types, `34d5885`
conversion + diff, `41a31e0` fixture split, `e487fe8` new v3 scenarios,
`991cfea` ngrams retention tests, `e5d450e` context-trace model tests,
`92c8537` positioned reader lane + `context_read::read` stub, `a037e18`
legacy doctest fix, `f5f8d68` test-suite cutover, plus this plan update.
Notes and deviations:

- Types (`context-trace/src/positioned/`): `interchange.rs`,
  `composition.rs` (`Occurrence { offset: AtomPosition, child: Token }` —
  `Token` instead of `VertexIndex` so the composition knows child widths;
  `Composition::new` sorts its input and rejects too few occurrences,
  duplicates, contained intervals and gaps), `location.rs`
  (`OccurrenceLocation`, `OccurrencePath`, `IntervalView`), `graph.rs`
  (`PositionedVertex` with `parents: BTreeMap<parent, BTreeSet<offset>>`,
  atoms/composites as `VertexNode`, `PositionedGraph<G>` with dense `Vec`
  storage + content index; `insert` additionally rejects unknown children,
  stale widths, disagreeing overlaps and duplicate content; `validate()`
  checks rules 1-7 — rule 8, corpus roots, stays on the snapshot — plus
  reciprocal links and one vertex per content), `convert.rs`, `diff.rs`.
  `ngrams::output` re-exports the interchange; structural rejection tests
  moved to context-trace, oracle-dependent output tests stay in ngrams.
- Fixtures: `tests/fixtures/v2/` (+ frozen `v2/scenarios.json`) and
  `tests/fixtures/v3/` (+ authoritative `v3/scenarios.json`); regeneration of
  the existing v3 fixtures produced no diff.
- New v3 scenarios, all matching the hand-checked expectation (no oracle
  discrepancy): `repeat_abcabca` (`abcabca = [abca@0, abca@3]`, vertices
  `a b c abca abcabca`), `repeat_ababa` (`ababa = [aba@0, aba@2]`),
  `seam_bcd` (`["abc","cde","xbcdy","abcde"]`; `bcd` retained by frequency,
  `abcde = [abc@0, bcd@1, cde@2]`), `late_retention__1` (`["vis"]`) /
  `late_retention__2` (`["vis","vim"]`, `vi` appears only now),
  `duplicate_observation` (`["abcabc","abcabc"]`), `substring_input`
  (`["abcabc","bca"]`, `abcabc = [abc@0, bca@1, abc@3]`).
- `context_read::read(&PositionedGraphRef, impl IntoIterator<Item = char>)
  -> Option<Token>` panics with `unimplemented!`. `tests/support::legacy`,
  `legacy_reader_behavior` and the v2 checks of `fixture_validation` are
  gated on context-read's `legacy` feature, so the positioned lane also runs
  with `--no-default-features --features test-api`.
- Retention: bounded sweep (196 two-text corpora over `{a,b}`, width ≤ 3)
  and all v3 scenarios run by default; the ignored release sweep checks
  54 000 transitions (three texts over `{a,b}`, width ≤ 4) plus 1 521
  (two texts over `{a,b,c}`, width ≤ 3): no violation.
- Gate 2: last full legacy run before the cutover 844 ok / 0 failed / 144
  ignored (Step 1: 831 / 1 / 85 by the old filter; differences are only new
  tests, the 3 moved interchange tests, the fixed doctest and reason-ignored
  tests the old filter did not list). After the cutover: 544 ok / 0 failed /
  444 ignored; exactly 300 tests changed `ok` → ignored (298 `legacy suite`
  + 2 legacy doctests). `cargo test -p context-trace -p context-search
  -p context-insert -p context-read -p context-cli --features legacy-tests`
  is green. The `--no-default-features` check compiles.
- Cutover scope: unit tests under `src/legacy` (247 compiled; 8 more in the
  uncompiled `context-trace/src/legacy/tests/state_advance.rs`),
  `legacy_reader_behavior` 48 (+6 known failures, unconditional),
  `fixture_validation` v2 checks 3, context-cli `graph_compare` 9 and oracle
  self-checks 4. Not marked: tests that already carry an ignore reason
  (search `dump_events`, context-cli `RC-…`), the 16 ts-rs generated
  `export_bindings_*` tests of legacy snapshot types (no attribute to
  attach), and all context-api/http/mcp and context-cli command tests (API
  behavior, row-backed internally). context-api therefore has no
  `legacy-tests` feature. Legacy doctests use
  `cfg_attr(not(feature = "legacy-tests"), doc = "```rust,ignore")`.

**Gate 2:** all new tests pass (except the ignored reader lane); the legacy
lane is unchanged at the last full run before the cutover; the
`--no-default-features` check from 1.1 compiles the positioned modules; after
the cutover, default `cargo test` reports legacy tests as ignored.

---

## Step 3 — Promote reusable pieces out of `legacy` (pull-based)

Only promote when positioned code needs it and new-model tests cover it.
Promotion = move to root module + legacy re-imports from root. Candidates:

- [ ] search `container/` (BFT/DFT worklists), `policy.rs` shell
- [ ] event/visualization plumbing (trace `graph/visualization.rs`,
  insert `visualization.rs`), with model-neutral payloads
- [ ] `InsertOutcome` / `InsertCtx` interface shape (as a new trait, not by
  making the row impl generic)
- [ ] read `segment.rs` / `input.rs` (input → atoms, unknown/known runs)
- [ ] `RootManager` ownership + commit-boundary concept (rewrite, not move)
- [ ] test harness pieces (`TestCase`, tracing setup)

Rule: never make a legacy type generic over both models to "share" it; copy
the concept instead. Track each promotion in this list with the commit.

**Status (2026-11-19, after Step 4):** no promotion so far. The navigation
APIs needed no legacy code: they use small explicit worklists (`Vec` stack +
visited set) instead of the row BFT/DFT `container/`, and the v3 fixture
loader was written fresh (`context_trace::positioned::fixtures`, `test-api`).

**Gate 3:** per promotion, `cargo test --features legacy-tests` on the
affected crates is unchanged, and the promoted item has a positioned-model
test.

---

## Step 4 — Read-only navigation (reader Phase 2)

Three separate APIs, separate tests, all on v3 fixtures:

- [x] **Boundary search** (`context-search`): retained occurrences starting /
  ending at a boundary of an owner, returning `OccurrencePath` evidence.
  Test: results equal oracle-retained prefixes/postfixes per vertex.
- [x] **Occurrence enumeration in an interval** (`context-trace`): descend
  through intersecting children incl. boundary-crossing ones, report whole
  retained occurrences inside `IntervalView`, dedup by
  `(token, owner-relative offset)`. Tests: interval cutting through a child
  finds smaller occurrences inside it; same occurrence via two paths reported
  once; compare against brute-force substring enumeration over the retained
  set.
- [x] **Content reading**: emit each atom position once; may cut through
  children. Test: equals owner content for all vertices and random intervals.

**Gate 4:** all navigation tests pass on every v3 fixture.

**Step 4 done (2026-11-19).** Commits (each builds the workspace):
`ddee2e1` content reading (+ shared fixture loader), `839beeb` occurrence
enumeration, `088971a` boundary search. No reader algorithm was added;
`context_read::read` stays a stub. Notes:

- APIs:
  - `context_trace::positioned::fixtures::{v3_fixtures, v3_graphs, v3_dir}`
    (`test-api`; replaces the private loader of the Step 2 model tests).
  - `PositionedGraph::read_interval(&self, &IntervalView) ->
    Option<Vec<AtomOf<G>>>` (`context-trace/src/positioned/read.rs`).
  - `PositionedGraph::occurrences_in(&self, &IntervalView) ->
    Option<Vec<Occurrence>>` (`context-trace/src/positioned/enumerate.rs`):
    owner-relative offsets, sorted by `(offset, width)`, deduplicated, owner
    excluded; descent memoized per `(child, absolute offset)`.
  - `context_search::boundary::{boundary_occurrences(graph, owner, position,
    Boundary::{Start, End}), prefixes(graph, owner), postfixes(graph, owner)}
    -> Option<Vec<BoundaryOccurrence { token, path: OccurrencePath }>>`
    (`context-search/src/boundary/mod.rs`): proper descendants only, longest
    first, one token once with one evidence path. Generalized from
    prefix/postfix to any position because the reader's seam discovery
    (Phase 3) needs boundaries inside owners too.
  - `None` for an unknown owner or a range/position beyond its width.
- Tests (all fixtures are small — max width 14, 416 vertices — so they are
  exhaustive instead of random): expected values come from string
  comparison over the fixture's vertex keys, never from the graph.
  - read: every interval of every vertex equals the key slice; views outside
    the owner are rejected.
  - enumerate: every interval of every vertex equals all substring matches of
    retained keys (owner excluded); `seam_bcd` `abcde[1..3)` cuts through
    `abc` and `bcd` and still finds `bc@1`; `bc@1` is reachable via both and
    reported once; for every vertex the result equals the deduplicated set of
    naive descent paths. A mutation that skips boundary-crossing children
    fails two of the three tests.
  - boundary: prefixes/postfixes per vertex, start/end searches at every
    position, and each evidence path is stored, rooted at the owner, ends at
    the token and has the right offset.
- Gate 4: default workspace run 552 passed / 0 failed / 444 ignored (Step 2:
  544 / 0 / 444; +8 new tests). `cargo test -p context-trace -p context-search
  --no-default-features --features test-api --lib --tests` passes (26 + 3).
  Without `--lib --tests` two pre-existing context-trace doctests fail to
  compile because they import `context_trace::legacy`
  (`logging::tracing_utils::init_test_tracing`,
  `TestTracing::init_with_graph`); not caused by Step 4.
- No promotion out of `legacy` (see Step 3), so no `legacy-tests` rerun was
  required.

---

## Step 5 — Plan and docs handoff

- [x] Reader plan → revision 3: reference this plan as Phase −1/0-2
  prerequisite; add the within-input provisional-retention rule and the
  across-input monotonicity evidence to Phase 5; update `status`.
- [x] Overlap provider plan: mark "frozen for legacy; only fixes to keep the
  legacy lane green", list the 6 known failures.
- [x] ngrams implementation plan: note interchange types moved to
  context-trace.
- [x] Update this plan's checkboxes and `status` as steps land.

**Step 5 done (2026-11-19)** in the docs commit following `088971a`:
reader plan revision 3 (prerequisite reference, Phases 0-2 marked done, type
deviation `Occurrence.child: Token`, Phase 5 provisional-retention rule and
monotonicity evidence with the `ngrams/src/tests/retention.rs` test names),
overlap provider plan frozen for legacy (extends the Step 1 freeze note),
ngrams implementation plan note on the interchange move (`859a785`),
`INDEX.md` statuses, context-trace/context-search README and FILE_INDEX
entries for the navigation modules. Still open: the editor-kernel build check
of 1.6.

---

## When algorithm work starts

After Gates 1, 2 and 4 (Step 3 is continuous). First algorithm work is reader
plan **Phase 3** (positioned merge + seam discovery in `context-insert`), then
**Phase 4** (fixed-retention reader; milestone `abcabca` → `[abca@0, abca@3]`
with no extra vertices). Phase 5 design must start from the 0.5 finding and
first add a proof or larger sweep plus measurements of affected hosts.

## Risks

| Risk | Mitigation |
|---|---|
| Move commits hide behavior changes | Move-only commits, `git diff -M`, baseline test list diff |
| Large import churn conflicts with parallel legacy fixes | Do Step 1 quickly, crate by crate; freeze legacy feature work meanwhile |
| `Token` display depends on legacy test graph | Name-resolver hook in 1.2 before moving `test_graph` |
| v3 manifest diverges from oracle | `fixture_validation` recomputes every v3 fixture from `parse_corpus` |
| Positioned scaffold over-designed before the reader needs it | Only types used by tests in 2.6/Step 4; promotion is pull-based |
| Hidden legacy dependency in new code | `--no-default-features` check (1.1) |
| Ignored legacy tests rot unnoticed | `legacy-tests` feature run required for any legacy-touching change (2.7, Gate 3) |
