---
tags: `#plan` `#context-trace` `#context-search` `#context-insert` `#context-read` `#refactoring` `#crate-layout` `#legacy`
summary: Re-home every item of the context stack in the crate that owns its responsibility (trace < search < insert < read) and extract the row-model code unchanged into four crates `context-*-legacy`. context-trace keeps only the shared model, store contracts, committed store, validation, interchange format and infrastructure; search gets paths, trace states, cache keys; insert gets `GraphEdit` with every mutation (incl. dissolve/stabilize and the `Stage` contract); read gets the overlay stage and commit. Legacy extraction validated by a spike (all four crates build, test totals equal the baseline).
status: 📋 planned (2026-11-26). Legacy extraction (Part L) validated by a throwaway spike; positioned relocation (Part P) analysed, not prototyped.
date: 2026-11-26
depends_on: 20261119_PLAN_POSITIONED_MIGRATION_PREPARATION.md (legacy modules), 20261124_PLAN_STAGING_GRAPH.md (store traits, seal), 20261125_PLAN_DISSOLUTION.md, 20261126_PLAN_CHEAP_STABILIZATION.md
---

# Plan: Crate ownership and legacy extraction

## 0. Overview

| Part | Task | Content | Behavior change |
|---|---|---|---|
| L | L1–L4 | Move `context-{trace,search,insert,read}/src/legacy` into new crates `context-{trace,search,insert,read}-legacy`, as is | none |
| L | L5 | Remove the `legacy`/`legacy-tests` features from the new crates; docs | none |
| P | P0 | Preparations in context-trace: `check` module, table construction, raw primitives behind `edit` | none |
| P | P1 | Paths, trace states, cache keys → context-search | none |
| P | P2 | Overlay stage, `StageHandle`, `Translation` → context-read | none |
| P | P3 | `GraphEdit`, operations, dissolve, stabilize, `Publication`, `Stage` → context-insert | none |
| P | P4 | Fixture lanes and manifest examples → insert / read | none |
| P | P5 | Search-only `Navigate` methods → context-search | none |
| V | V1–V3 | Layering guard test, ownership audit, docs and memories | none |

Order: L1 → L2 → L3 → L4 → L5 → P0 → P1 → P2 → P3 → P4 → P5 → V. Every task
is one commit that builds the workspace and keeps the test totals of §8.
Part L comes first: it removes all `cfg(feature = "legacy")` code from the
new crates, so Part P moves code without legacy noise.

## 1. Problem

Measured on `main` at `3ea3cb0` (non-test and test lines, `wc -l`):

| Crate | New code (lines) | `src/legacy` (lines) |
|---|---|---|
| context-trace | 18,971 (positioned 10,900, logging 4,400) | 18,617 |
| context-search | 4,584 | 12,443 |
| context-insert | 1,376 | 14,429 |
| context-read | 1,589 | 2,196 |

1. **context-trace owns algorithm code of the higher crates.** Of the
   positioned modules in context-trace only `model`, `store`, `committed`,
   `interchange` and `edit/check.rs` are used by more than one crate.
   - `positioned::path` and `positioned::trace` (≈2,100 lines) and the root
     `key` module are used by context-search only (context-insert reads
     `TraceCache`/`DirectedKey` only through the search `Response`).
   - `positioned::edit` (`GraphEdit`, operations, dissolve, stabilize,
     `Publication`; ≈1,070 lines) is used by context-insert and
     context-read only; the `edit` feature exists just to hide it.
   - `positioned::stage` (overlay, `StageHandle`, `Translation`; ≈1,385
     lines) is used by context-read only.
   - Their tests (≈3,000 lines in `positioned/tests/{trace,stage,edit.rs}`)
     live in context-trace as well.
   - The insert and read fixture lanes (`insert_cases`, `read_cases`,
     manifests) and the two manifest examples live in context-trace.
2. **Legacy code is mixed into the new crates.** Each crate has a
   `legacy` module behind a default-on feature, and the shared root of
   context-trace contains `cfg(feature = "legacy")` code that reaches into
   it: `GraphKind::Direction` changes its bound with the feature, `Token`
   has legacy-only inherent methods and types (`SubToken`), and trait impls
   for `legacy::VertexData`, `legacy::Hypergraph` live in the root
   (`graph/kind.rs`, `graph/vertex/{token,wide,has_vertex_index,has_vertex_key}.rs`,
   `logging/tracing_utils/graph_ref.rs`, `tests/test_case/environment.rs`).
   context-search promoted legacy cursor typestates into its root
   (`cursor/`) and legacy code accesses their `pub(crate)` fields.

## 2. Target state

### 2.1 Dependency graph

```
context-trace-macros
└── context-trace                      shared model, stores, validation, interchange, logging
    ├── context-search                 paths, trace states/cache/keys, cursors, search
    │   └── context-insert             GraphEdit + every mutation, Stage contract, split/join/insert
    │       └── context-read           overlay stage + commit, reader orchestration
    │           └── context-api        → tools (cli, http, mcp)
    └── context-trace-legacy           row model (frozen)
        ├── context-search-legacy
        │   └── context-insert-legacy
        │       └── context-read-legacy
        └── ngrams (oracle; also uses context-trace interchange)
```

Invariants (checked by the guard of §7.1):

- A crate depends only on crates to its left in
  trace < search < insert < read (plus `context-trace-macros`).
- No non-legacy crate depends on a `*-legacy` crate, in any dependency
  section. The only exceptions are `ngrams` (the oracle is written on the
  row model) and the legacy crates themselves.
- A `context-X-legacy` crate depends only on `context-trace` (shared ids,
  logging, macros) and on legacy crates below it. It never depends on
  context-search, context-insert or context-read.
- No source file of context-{trace,search,insert,read} mentions
  `legacy` outside comments; there are no `legacy`/`legacy-tests` features.

### 2.2 Crate contents

| Crate | Owns |
|---|---|
| context-trace | ids and atoms (`graph::{kind, vertex}`), `AtomPosition`, `Wide`, logging and test tracing, deterministic `HashMap`/`HashSet`, proc-macro re-exports; positioned `model`, `store` (read traits, `Navigate` core, sealed raw primitives), `committed` (`PositionedGraph`, serde, table construction), `check` (validation), `interchange` (format, conversion, diff, shared fixtures) |
| context-search | `trace::key` (former root `key`), `path` (former `positioned::path`), `trace` (former `positioned::trace`), `PathNavigate`, cursors, compare, match, search, `Response` |
| context-insert | `edit` (`GraphEdit`, operations, `Publication`/`VertexEdit`, dissolve `Candidates`/`Removal`/`Settled`, stabilize, `Stage`, `EditExt`), split, join, insert, insert fixture lane |
| context-read | `stage` (`StageHandle`, `Translation`, `CommitError`, overlay), reader (`read`, segments, expansion, forward), read fixture lane |
| context-*-legacy | the former `legacy` module of the same crate, unchanged apart from paths (§5) |

`context_trace::positioned` keeps its name in this plan; flattening it into
the crate root is a separate, purely cosmetic follow-up (no legacy namespace
remains to separate from).

## 3. Placement rules

### 3.1 Rules

- **O1 Layer order.** trace < search < insert < read. Dependencies point
  only downward. An item can never live above its lowest user or below
  the owner of anything it depends on.
- **O2 Pinned items stay.** An item stays where it is if an item that
  stays mentions it (signature, field, impl, body). Example: `Located` is
  returned by `Navigate::start_at`, which context-insert's `GraphEdit` also
  uses, so `Located` stays in context-trace.
- **O3 Shared items stay in context-trace.** An item stays in
  context-trace iff it is pinned there (O2) or used by the non-test code
  of all three of search, insert and read (directly, or through a type of
  a lower crate's public API that they use). Shared test infrastructure
  used by the tests of several crates stays as well, behind `test-api`.
- **O4 Otherwise: lowest user.** Every other item moves to the lowest
  crate among its users. External consumers (context-api, ngrams,
  context-cli) import it from that crate; they do not pin it in
  context-trace.
- **O5 Responsibility check.** The result of O3/O4 must match the role of
  the crate: search algorithms and their state → search; anything that
  writes a graph → insert; reader lifecycle state and orchestration →
  read. If an item would land in a crate with a different role, split it
  (e.g. the overlay commit: the writes into the committed store use the
  existing sealed trace primitives, the stage and its commit loop are
  reader state, §6.3).
- **O6 Private access goes with the item.** An item that needs private
  fields of another item stays with it (e.g. `dissolve`/`stabilize` use the
  private helpers `removable`, `composite`, `recompose`, `unlink`,
  `touched` of `GraphEdit`, so they move together, to insert).
- **O7 Tests follow their subject.** A test lives in the crate that owns
  the code it tests. If it needs items of a higher crate (dissolve tests
  need the overlay stage), it lives in the lowest crate that has all of
  them (context-read).
- **O8 Legacy is extracted, not redistributed.** Row-model code moves as a
  whole into `context-X-legacy` for the crate X it lives in today. Items of
  a new crate used only by legacy code move into the legacy crate; items
  that the new code still needs but are moving up (e.g. `key`) are copied
  frozen into the legacy crate instead of making legacy depend on a higher
  new crate.
- **E1 Exception: the seal.** The raw write primitives (`raw::Write`,
  `VertexWrite`, `Remove`, `Stabilize`, `Seal`,
  `SEAL`) stay in context-trace: the committed store implements them and
  the seal must be defined by the crate that defines the stores. They are
  reachable only with the hidden feature `edit`, which only context-insert
  and context-read may enable (existing guard
  `only_insertion_and_the_reader_enable_the_edit_feature`).

### 3.2 How to find misplaced items

1. **Usage inventory.** For each consumer crate list every identifier it
   imports from a lower crate (non-legacy files, split into `src` and
   tests). The script used for this plan:

   ```python
   # uses.py <crate-dir> <lower-crate-ident>, e.g. uses.py context-insert context_trace
   import re, os, sys, collections
   crate, target = sys.argv[1], sys.argv[2]
   res = collections.defaultdict(set)
   for dp, _, fn in os.walk(crate):
       if '/target' in dp or '/legacy' in dp:
           continue
       for f in (f for f in fn if f.endswith('.rs') and 'legacy' not in f):
           s = open(os.path.join(dp, f)).read()
           kind = 'test' if ('/tests' in dp or '/benches' in dp) else 'src'
           for m in re.finditer(r'\buse\s+' + target + r'::(.*?);', s, re.S):
               for ident in re.findall(r'[A-Za-z_]\w*', re.sub(r'//[^\n]*', '', m.group(1))):
                   res[ident].add(kind)
           for m in re.finditer(target + r'::((?:[a-z_]\w*::)*)([A-Za-z_]\w*)', s):
               res[m.group(2)].add(kind)
   for k in sorted(res):
       print(k, ','.join(sorted(res[k])))
   ```

2. **Pin check.** For every candidate, grep the modules that stay
   (`positioned/{model,store,committed,interchange}`, `edit/check.rs`,
   `graph`, `position.rs`, `logging`) for the identifier. A hit pins it (O2).
3. **Coupling check.** List the `pub(crate)`/`pub(super)` items of the
   staying modules that the moving modules use; each becomes a visibility
   change (or a reason to split, O5/O6). For a moving module `m`:
   `rg -n 'pub\((crate|super)\)' <staying>` and match the names in `m`.
4. **Legacy reach-in.** `rg -n 'feature = "legacy"|legacy::' <crate>/src
   --glob '!**/legacy/**'` lists every root item that depends on legacy.
5. **Compile-driven confirmation.** After a move, `cargo check` lists
   every remaining crossing (privacy, orphan rule, missing imports).

## 4. Ownership mapping

Paths are relative to each crate's `src/`. "→ trace-legacy" etc. means the
item moves into that legacy crate (Part L); all other rows are Part P.

### 4.1 context-trace root (non-positioned)

| Item(s) | Today | New owner / path | Reason |
|---|---|---|---|
| `GraphKind`, `BaseGraphKind`, `AtomOf` | `graph/kind.rs` | stays; **`type Direction` removed** | O3; `Direction` is used only by legacy |
| `Direction`, `Left`, `Right`, `Both` | `direction/mod.rs` | → trace-legacy `direction/base.rs` | O8: legacy-only |
| legacy `GraphKind` (`Direction: PatternDirection`) | cfg in `graph/kind.rs` | → trace-legacy `graph/kind.rs`: `trait GraphKind: context_trace::GraphKind { type Direction: PatternDirection; }`, impl for `BaseGraphKind` | O8 |
| `VertexId`, `VertexIndex`, `CommittedIndex`, `StagedIndex`, `StageIndex`, atoms, `Token`, `TokenWidth`, `VertexKey`, `HasVertexIndex`, `ToToken`, `name_resolver` | `graph/vertex/*` | stay | O2/O3 |
| `Wide` | `graph/vertex/wide.rs` | stays; `impl Wide for [Token]` becomes unconditional (sum of widths) | orphan rule forbids it in a legacy crate |
| `WideMut`, `Wide for Pattern`, `Wide`/`WideMut for VertexData` | `graph/vertex/wide.rs`, `token.rs` | → trace-legacy `graph/vertex/wide.rs` | O8 (no new-code user) |
| `SubToken`, `HasToken`, `HasSubLocation` | cfg in `graph/vertex/token.rs` | → trace-legacy `graph/vertex/token.rs` | O8 |
| `Token::to_pattern_location`, `to_child_location` | cfg inherent methods | → trace-legacy trait `TokenLocation` (re-exported at the crate root) | inherent impls on a foreign type are impossible |
| `Token::down_key`, `up_key`, `get_width` (legacy cfg) | `token.rs` | deleted (no callers) | — |
| `HasVertexIndex for VertexData` | `has_vertex_index.rs` | → trace-legacy `graph/vertex/token.rs` | O8 |
| `HasVertexKey` (whole file) | `graph/vertex/has_vertex_key.rs` | → trace-legacy `graph/vertex/has_vertex_key.rs` | only legacy uses it |
| `AtomPosition` | `position.rs` | stays | O2 (`Occurrence`) |
| `DirectedKey`, `DirectedPosition`, `HasAtomPosition`, `UpKey`, `UpPosition`, `DownKey`, `DownPosition` | `key/` | → search `trace/key/`; frozen copy → trace-legacy `trace/cache/key/directed/` | O4 (users: positioned trace, insert via `Response`) |
| logging, `CompactFormat`, test tracing | `logging/` | stays; `tracing_utils::graph_ref` becomes `pub mod` | O3 |
| `AsGraphRef` impls for `Hypergraph`/`HypergraphRef` | cfg in `graph_ref.rs` | → trace-legacy `graph/test_graph.rs` | O8 |
| `TestEnv`, `TestCase`, `TestError` | `tests/test_case/` | → trace-legacy `tests/test_case/` (+ `pub use tests::test_case::TestEnv` at its root) | used by legacy and ngrams' legacy tests only |
| `HashMap`/`HashSet` aliases, `__tracing`, macro re-exports | `lib.rs` | stay | O3 |

### 4.2 context-trace `positioned`

| Item(s) | Today (`positioned/…`) | New owner / path | Reason |
|---|---|---|---|
| `Occurrence`, `Composition`, `CompositionError`, `OccurrenceLocation`, `VertexNode`, `PositionedVertex`, `PositionedError`, `maximal`, `IntervalView`, `PositionedLabels` | `model/` | stay | O2/O3 |
| `model::slice`, `Composition::{lifted, map_index}` | `model/composition.rs` (`pub(crate)`) | stay, become `pub` `#[doc(hidden)]` | used by the overlay (read) |
| `GraphRead`, `VertexRead`, `NodeRef`, `VertexState`, `Generation`, `CompositionRead`, `GraphWrite`, `HasGraph(Mut)`, `GraphAtom`, `IndexOf`, `KindOf`, `Located`, `string_repr` | `store/` | stay | O2/O3 |
| `Navigate` (`start_at`, `prefix_child`, `postfix_child`, `prefixes`, `postfixes`, `parent_occurrences`, `read_interval`) | `store/navigate.rs` | stays | used by insert/read/search |
| `Navigate::{end_at, advance_end_path}` (+ private helpers used only by them) | `store/navigate.rs` | → search `path/navigate.rs`, trait `PathNavigate` blanket-implemented for every `VertexRead` | O4: only `path/` calls them (P5) |
| `raw::{Write, VertexWrite, Remove, Stabilize, Seal, SEAL}` | `store/write.rs` (`pub(crate) mod raw`) | stay; `pub mod raw` + `pub const SEAL` under `#[cfg(feature = "edit")] #[doc(hidden)]` | E1 |
| `PositionedGraph`, `PositionedGraphRef`, `PositionedVertexRef`, serde | `committed/` | stay | O3 |
| `impl PositionedGraph { fn edit() }` | `edit/mod.rs` | removed; replaced by `context_insert::edit::EditExt::edit` | inherent impl on a foreign type is impossible |
| `validate_store`, `expect_vertex`, `expect_token`, `check_child`, `validate_vertex`, `check_overlap_closure`, `composed_label` | `edit/check.rs` | → context-trace `check.rs` (new module); `validate_store` public, the others `pub` `#[doc(hidden)]` | O2 (used by `committed/`), users in search and insert |
| `GraphEdit` + `operations.rs` (`atom`, `create`, `add_occurrences`, `extend`, `finish`, `graph`) | `edit/mod.rs`, `edit/operations.rs` | → insert `edit/{mod,operations}.rs` | O4/O5 (users: insert, read) |
| `Publication`, `VertexEdit` | `edit/publication.rs` | → insert `edit/publication.rs` | O4 |
| `Candidates`, `Removal`, `Settled`, `dissolve`, `discard`, `settle` | `edit/dissolve.rs` | → insert `edit/dissolve.rs` | O6 (private `GraphEdit` helpers) |
| `stabilize`, `stabilize_candidates` | `edit/stable.rs` | → insert `edit/stable.rs` | O6 |
| `Stage` trait | `stage/mod.rs` | → insert `edit/stage.rs` | O4: bound of dissolve/stabilize (insert) and implemented by the overlay (read) |
| `StageHandle`, `Translation`, `CommitError` | `stage/mod.rs` | → read `stage/mod.rs` | O4: read only |
| `OverlayHandle`, `OverlayView`, `OverlayVertex`, `OverlayVertexMut`, `StagingGraph`, `StagedVertex`, `Lifted`, `OverlayLabels` | `stage/overlay/` | → read `stage/overlay/` | O4: read only |
| `RolePath`, `StartPath`, `EndPath`, `QueryPath`, `RangePath`, `PathRole`, `Start`, `End`, `PathRoot`, `QueryRoot`, `HasRolePath`, `SpanCoverage` | `path/` | → search `path/` | O4: search only |
| `TraceCache`, `PositionCache`, `VertexCache`, `ParentState`, `ParentBatch`, `ChildState`, `RootChildState`, `ChildQueue`, `TraceCtx`, `TraceDirection`, `QueryAnchor`, `StateAdvance`, `IntoParentState`, `Traceable`, `TraceCommand`, `PrefixCommand`, `PostfixCommand`, `RangeCommand` | `trace/` | → search `trace/` | O4: search only (insert reads `TraceCache` from `Response`) |
| `PositionedGraphSnapshot`, `PositionedGraphData`, `PositionedCover`, `POSITIONED_FORMAT_VERSION`, `from_interchange(_named)`, `to_interchange`, `store_interchange` | `interchange/{mod,convert}.rs` | stay; `from_interchange` stops using `GraphEdit` (§6.1) | O3 (api, ngrams, all tests) |
| `diff`, `GraphDiff`, `CoverDiff` | `interchange/diff.rs` | stay (`test-api`) | used by insert, read, trace tests |
| `Scenario`, `fixtures_dir`, `manifest`, `fixture_snapshots`, `FixtureGraph`, `fixture_graphs` | `interchange/fixtures.rs` | stay (`test-api`) | shared test infrastructure (search, insert, read, api) |
| `insert_dir`, `insert_queries`, `insert_manifest`, `InsertCase`, `insert_cases` | `interchange/fixtures.rs` | → insert `fixtures.rs` (`#[cfg(any(test, feature = "test-api"))]`) | O4 (insert tests, read fixture validation) |
| `read_dir`, `read_manifest`, `ReadCase`, `read_cases` | `interchange/fixtures.rs` | → read `fixtures.rs` (`test-api`) | O4 |

### 4.3 Tests, examples, benches

| Today | New owner / path |
|---|---|
| context-trace `positioned/tests/{diff,fixtures,guard,interchange,navigation,serde}.rs` | stay |
| context-trace `positioned/tests/model.rs` | split: the `validate_*`/`occurrence_paths_*` checks stay (graphs built from tables, §6.1); the `create_*` rejection cases → insert `tests/edit.rs` |
| context-trace `positioned/tests/edit.rs` | → insert `tests/edit.rs` |
| context-trace `positioned/tests/stage/{overlay,dissolve,stable}.rs` | → read `tests/stage/` (O7: need the overlay) |
| context-trace `positioned/tests/trace/{mod,query,exhaustive}.rs` | → search `tests/trace/` |
| context-trace `examples/insert_manifest.rs` | → context-insert `examples/` (`required-features = ["test-api"]`) |
| context-trace `examples/read_manifest.rs` | → context-read `examples/` |
| context-read `tests/legacy_reader_behavior.rs`, `tests/support/legacy.rs`, legacy tests in `tests/fixture_validation.rs` (3 tests), `benches/grammar.rs`, `tests/fixtures/v2/` | → context-read-legacy `tests/`, `benches/`, `tests/fixtures/v2/` |
| legacy tests inside `src/legacy/tests` of each crate | move with the module (L1–L4) |

### 4.4 Feature flags after the refactor

| Crate | Features |
|---|---|
| context-trace | `edit` (raw primitives; insert/read only), `interchange`, `serde`, `test-api` |
| context-search | `test-api` |
| context-insert | `test-api` (insert fixture lane) |
| context-read | `test-api` (read fixture lane), `test-hashing`, `async`, `long-bench` |
| context-*-legacy | `test-api`, `legacy-tests` (keeps the legacy suites ignored by default) |

## 5. Part L — legacy extraction

Validated by a spike on a scratch worktree of `3ea3cb0` (patch kept in the
session workspace as `legacy-extraction-spike.patch`, not committed): all
four legacy crates and the whole workspace (except the winit crates) build
with `--all-targets`, and the test totals equal the baseline (§8).

### 5.1 Mechanical recipe (per crate X, in order trace, search, insert, read)

1. `mkdir context-X-legacy && git mv context-X/src/legacy context-X-legacy/src && git mv context-X-legacy/src/mod.rs context-X-legacy/src/lib.rs`
   (keeps history).
2. Rewrite references to the *non-legacy* root of the same crate first
   (they would otherwise collide with step 3):
   - trace: in `tests/mod.rs`, `graph/mod.rs`, `graph/vertex/mod.rs`,
     `direction/mod.rs`, `logging/mod.rs`, `path/mutators/move_path/key.rs`,
     `trace/cache/key/directed/mod.rs`:
     `crate::(key|tests|position|logging|graph|direction)` → `context_trace::…`;
   - search: `crate::cursor` → `crate::cursor::shared` (frozen copy, §5.2 S1).
3. Rewrite legacy-internal paths in all files:
   `$crate::legacy::` → `$crate::`, `crate::legacy::` → `crate::`,
   `crate::legacy` → `crate`.
4. Rewrite cross-crate legacy paths in all legacy crates, `ngrams`, and
   the moved read tests/benches:
   `context_X::legacy` → `context_X_legacy` (204 sites in search/insert/read
   legacy, 28 files in ngrams); doc comments included.
5. `lib.rs`: replace `pub use crate::*;` by `pub use context_trace::*;`
   (trace-legacy) or drop it (search/insert/read-legacy, which never used
   their own root except search's cursor). Put the crate attributes
   (`#![feature(type_changing_struct_update)]`, search also
   `#![feature(test)]` + `extern crate test;`,
   `#![deny(clippy::disallowed_methods)]`) *after* the `//!` header.
6. `Cargo.toml`: copy the origin crate's manifest (same **edition**:
   trace 2024, search 2021, insert 2024, read 2018; same dependencies),
   rename, then:
   - features: `test-api = [...]`, `legacy-tests = [<lower legacy crates>/legacy-tests]`;
   - dependencies: `context-trace` (macros, logging, ids) plus the legacy
     crates below (`features = ["test-api"]` where the old code used
     `test-api` items of the lower crate);
   - add the crate to the workspace `members` and
     `[workspace.metadata.context-stack] members`.
7. In the origin crate: remove `pub mod legacy;`, the `legacy`/
   `legacy-tests` features, `default = ["legacy"]`, and
   `#![cfg_attr(feature = "legacy", …)]`.

### 5.2 Non-mechanical changes (exact)

**trace (L1)** — everything below was needed in the spike:

- T1 `context-trace/src/graph/kind.rs`: delete both `type Direction`
  declarations, `type Direction = Right;` and the `PatternDirection`/`Right`
  imports. New `context-trace-legacy/src/graph/kind.rs`:
  ```rust
  pub use context_trace::graph::kind::{AtomOf, BaseGraphKind};
  use crate::direction::{Right, pattern::PatternDirection};
  pub trait GraphKind: context_trace::graph::kind::GraphKind {
      type Direction: PatternDirection;
  }
  impl GraphKind for BaseGraphKind {
      type Direction = Right;
  }
  ```
  and `pub mod kind;` next to `pub use context_trace::graph::*;` in
  `graph/mod.rs` (the local module shadows the glob). One qualified path
  needs the base trait: `trace/has_graph.rs`
  `<TravKind<G> as GraphKind>::Atom` → `<TravKind<G> as context_trace::GraphKind>::Atom`.
- T2 `git mv context-trace/src/direction/mod.rs context-trace-legacy/src/direction/base.rs`;
  legacy `direction/mod.rs`: `pub use crate::direction::*;` → `mod base; pub use base::*;`.
  Remove `pub mod direction;` and the `Direction, Left, Right` re-export from
  `context-trace/src/lib.rs`.
- T3 `graph/vertex/wide.rs`: remove the legacy impls and `WideMut`; keep
  `impl Wide for [Token] { fn width(&self) -> TokenWidth { self.iter().map(Wide::width).sum() } }`
  unconditionally. `token.rs`: remove `impl WideMut for Token`, `SubToken`,
  `HasToken`, `HasSubLocation`, `to_pattern_location`,
  `to_child_location`, `down_key`, `up_key` and the legacy imports.
  New shadow modules in the legacy crate (`graph/vertex/mod.rs` gets
  `pub mod has_vertex_key; pub mod token; pub mod wide;` after
  `pub use context_trace::graph::vertex::*;`):
  - `graph/vertex/wide.rs`: `pub use context_trace::graph::vertex::wide::*;`,
    `pub trait WideMut`, impls for `Token<I>`, `VertexData`, and `Wide` for
    `Pattern`, `VertexData`;
  - `graph/vertex/token.rs`: `pub use context_trace::graph::vertex::token::*;`,
    `SubToken`, `HasToken`, `HasSubLocation`, `pub trait TokenLocation`
    (`to_pattern_location`, `to_child_location`) implemented for `Token`,
    `impl HasVertexIndex for VertexData`;
  - `lib.rs`: `pub use self::graph::vertex::token::TokenLocation;` (callers
    in search-legacy import via globs).
- T4 `git mv context-trace/src/graph/vertex/has_vertex_key.rs context-trace-legacy/src/graph/vertex/`;
  drop the `cfg`, path `crate::graph::vertex::data::VertexData`. Remove
  `has_vertex_key` from `context-trace/src/graph/vertex/mod.rs` and
  `lib.rs`. Delete `impl HasVertexIndex for crate::legacy::VertexData`.
- T5 `logging/tracing_utils/mod.rs`: `mod graph_ref;` → `pub mod graph_ref;`;
  delete the four `AsGraphRef` impls from `graph_ref.rs` and add them to
  `context-trace-legacy/src/graph/test_graph.rs`.
- T6 orphan rule: `impl TokenNameResolver for Arc<Hypergraph<G>>` →
  local newtype `pub(crate) struct GraphResolver<G: GraphKind>(pub(crate) Arc<Hypergraph<G>>);`
  with the same impl (`self.0.…`), and
  `set_token_name_resolver(Box::new(GraphResolver(graph_arc)))` at both
  call sites.
- T7 `git mv context-trace/src/tests/test_case context-trace-legacy/src/tests/test_case`,
  delete `context-trace/src/tests/`, the `pub mod tests;` and
  `pub use tests::test_case::TestEnv;` of `context-trace/src/lib.rs`.
  Legacy `tests/mod.rs`: `pub use crate::tests::*;` → `pub mod test_case; pub use test_case::*;`;
  `environment.rs`: drop the `cfg`, `use crate::HypergraphRef;`.
  Legacy `lib.rs`: `#[cfg(any(test, feature = "test-api"))] pub use tests::test_case::TestEnv;`
  (search-legacy tests call `Env1::get()` through it).
- T8 `key`: replace the re-exports in
  `trace/cache/key/directed/mod.rs` (`pub use context_trace::key::…`,
  `mod down`/`mod up`) by frozen copies of `context-trace/src/key/{mod,up,down}.rs`
  (needed because P1 moves `key` to context-search).
- T9 docs: the two doctests in `logging/tracing_utils/{mod,test_tracing}.rs`
  that switch on `feature = "legacy"` (`no_run` vs `ignore`) — move the
  `Hypergraph` example into the legacy crate's docs and leave a
  model-independent `no_run` example (or `ignore`) in context-trace.
  Legacy doc comments: `context_trace::legacy::` → `context_trace_legacy::`.

**search (L2)**

- S1 frozen copy of the promoted row-independent cursor typestates:
  `cp context-search/src/cursor/{checkpointed,state_machine}.rs context-search-legacy/src/cursor/shared/`,
  and `shared/mod.rs` = `context-search/src/cursor/mod.rs` without
  `pub mod child; pub mod query;` and their re-exports. Add
  `pub mod shared;` to the legacy `cursor/mod.rs`. (Legacy code builds
  `PathCursor { path, atom_position, _state }` literally, so it needs the
  `pub(crate)` fields in its own crate.)
- S2 remove `#![cfg_attr(feature = "legacy", feature(type_changing_struct_update))]`
  from `context-search/src/lib.rs`; drop the comments in
  `context-search/src/cursor/mod.rs` that describe sharing with legacy.

**insert (L3)** — mechanical only (edition 2024: legacy uses let chains).

**read (L4)**

- R1 `src/tests/mod.rs`: `#[path = "../../../tests/support/mod.rs"]` →
  `#[path = "../../tests/support/mod.rs"]`.
- R2 move `tests/legacy_reader_behavior.rs`, `tests/support/legacy.rs`,
  `benches/grammar.rs` (and its `[[bench]]`), `tests/fixtures/v2/`; new
  `context-read-legacy/tests/support/mod.rs` = the shared part of
  `context-read/tests/support/mod.rs` with `pub mod legacy;` only. Remove
  `#[cfg(feature = "legacy")] pub mod legacy;` from context-read's support.
- R3 split `context-read/tests/fixture_validation.rs`: the three
  `#[cfg(feature = "legacy")]` tests (v2 lane) → `context-read-legacy/tests/fixture_validation.rs`.
- R4 manifest: context-read's dependency list (incl. `serde_json`) and
  dev-dependencies `ngrams`, `criterion`, `bincode`.

**ngrams** — `[dependencies.context-trace] features = ["legacy"]` → plain
`context-trace` + `context-trace-legacy`; rewrite paths (step 4).

**L5** — remove the remaining `legacy` mentions in manifests, READMEs,
`FILE_INDEX.md`, `HIGH_LEVEL_GUIDE.md` of the four crates and
`tools/cli/context-cli/README.md`, `ngrams/README.md`; each legacy crate
gets a short README ("frozen row model; run with
`cargo test -p context-X-legacy --features legacy-tests`").

## 6. Part P — positioned ownership

### 6.1 P0: preparations inside context-trace (no move yet)

- P0.1 `git mv positioned/edit/check.rs positioned/check.rs`; module
  `pub mod check` in `positioned/mod.rs`; make `expect_vertex`,
  `expect_token`, `check_child`, `validate_vertex`,
  `check_overlap_closure`, `composed_label` `pub` + `#[doc(hidden)]`;
  update `committed/{graph,serde}.rs`, `path/root.rs`,
  `trace/traceable/role.rs`, `edit/*` imports.
- P0.2 Table construction without `GraphEdit`: move `StoredGraph`,
  `StoredNode`, `widths` and `TryFrom<StoredGraph<A>> for PositionedGraph<G>`
  from `committed/serde.rs` into a new ungated `committed/table.rs`
  (serde derives/impls stay gated in `serde.rs`). Rewrite
  `interchange/convert.rs::build_from_interchange` to assign slots in the
  same order as today (atoms in `data.vertices` order, then composites
  sorted by `(char count, content)`), build a `StoredGraph` and call
  `PositionedGraph::try_from`. Check equal results: generation, labels
  and vertex indices of `fixture_graphs()` before/after (test in
  `positioned/tests/interchange.rs` comparing `to_interchange` and the
  `names` map; the generation after `GraphEdit::finish` must be
  reproduced in `StoredGraph::generation`).
- P0.3 `positioned/tests/model.rs`: graphs for the `validate_*` tests are
  built from tables (P0.2); `create_*` cases are marked for P3.
- P0.4 `store/write.rs`: under `#[cfg(feature = "edit")]` make
  `raw` `pub` + `#[doc(hidden)]` and `SEAL` `pub`; without the feature both
  stay `pub(crate)` (as today). Rewrite the module doc: the seal now proves
  "context-trace, context-insert or context-read" (guard E1).
- P0.5 No new primitive for the overlay commit. `StagingGraph::commit_into`
  (`stage/overlay/staging.rs`) writes `PositionedGraph` fields directly
  today, but every write maps onto the existing sealed primitives, so it is
  rewritten over them in place (still in context-trace, so P2 moves it
  unchanged):
  - translation: dense indices from `Write::next_index` (holes skipped);
  - new vertex: `Write::push(node mapped by translation, width)` (no child
    validation, forward indices are fine), then `VertexWrite::link_parent`
    per translated link and `set_label` (debug);
  - delta: `set_node` (composition), `unlink_parent` (removed; same
    semantics as `remove_link`), `link_parent` (added);
  - generation: `bump_generation` once per staged generation step.

Verification: `cargo test -p context-trace --features test-api,interchange,serde,edit`
equal to the baseline; `git diff` shows no behavior change outside
`convert.rs`/`staging.rs`.

### 6.2 P1: search owns paths, trace states and keys

- `git mv context-trace/src/key context-search/src/trace/key` (after
  `git mv context-trace/src/positioned/trace context-search/src/trace`),
  `git mv context-trace/src/positioned/path context-search/src/path`.
- Paths: `crate::positioned::{path,trace}` → `crate::{path,trace}`;
  `crate::key` → `crate::trace::key`; everything else `crate::…` →
  `context_trace::…` (`positioned::{store,model,check}`, `graph`, ids).
- Remove `pub mod key` and the key re-exports from `context-trace/src/lib.rs`,
  `pub mod path; pub mod trace;` and their re-exports from
  `positioned/mod.rs`; re-export the moved items from
  `context-search/src/lib.rs` (`pub mod path; pub mod trace;`).
- Imports in context-search: `context_trace::positioned::{…TraceCache…}` →
  `crate::trace::…`; in context-insert `split/mod.rs`:
  `DirectedKey`, `TraceCache` from `context_search::trace`.
- Tests: `git mv context-trace/src/positioned/tests/trace context-search/src/tests/trace`.

Verification: search 20 + insert 13 tests pass with the same totals;
`rg 'positioned::(path|trace)' context-trace` → no match.

### 6.3 P2: read owns the stage

- `git mv context-trace/src/positioned/stage context-read/src/stage`
  except the `Stage` trait, which stays in context-trace until P3
  (`positioned/store/stage.rs` for one commit).
- `OverlayView`/`OverlayVertex`/`OverlayVertexMut` implement
  `context_trace::positioned::store::raw::{Write, VertexWrite, Remove, Stabilize}`
  (feature `edit`, already enabled by context-read).
- Replace every direct access to `PositionedGraph` internals in the
  overlay by public API:
  - `base.vertices.len()` → `vertex_count()`; `&base.vertices[i]` →
    `vertex_data(i)` / `PositionedVertex::{node,width,parents,label}`;
  - `base.atoms.get(v)` → `GraphRead::atom(v)`;
  - `base.generation` → `GraphRead::generation().base`;
  - commit → already over `raw::{Write, VertexWrite}` with `SEAL` (P0.5);
  - `StageIndex::lift(i)` → `StageIndex::from(i)` (the `From<CommittedIndex>` impl exists);
    `Composition::lifted`, `map_index`, `model::slice` → `pub` `#[doc(hidden)]` (§4.2).
- `OverlayHandle::atom` keeps `base.edit()` (still in context-trace).
- Tests: `git mv context-trace/src/positioned/tests/stage context-read/src/tests/stage`;
  benches `staging*.rs` import from `context_read::stage`.
- `OverlayLabels` registration moves along; `name_resolver` stays generic.

Verification: read 129 tests and the staging benches compile and pass;
`rg 'overlay|Overlay' context-trace/src` → only docs.

### 6.4 P3: insert owns every mutation

- `git mv context-trace/src/positioned/edit context-insert/src/edit`
  (`mod.rs`, `operations.rs`, `publication.rs`, `dissolve.rs`, `stable.rs`)
  and the `Stage` trait → `context-insert/src/edit/stage.rs`.
- Delete `impl PositionedGraph { fn edit }`; add
  ```rust
  pub trait EditExt: GraphWrite { fn edit(&mut self) -> GraphEdit<'_, Self> { GraphEdit::new(self) } }
  impl<S: GraphWrite + ?Sized> EditExt for S {}
  ```
  so the call sites (`insert/mod.rs`, `context-read/src/root.rs`,
  `stage/overlay/handle.rs`, benches, tests: 79 sites) only change
  imports. `GraphEdit::new` becomes plain `pub`; the `cfg(feature = "edit")`
  split of `new`/`edit` disappears.
- Paths inside the moved code: `crate::positioned::{store,model,check,committed}` →
  `context_trace::positioned::…`, `raw::SEAL` from
  `context_trace::positioned::store::raw`.
- context-read: `GraphEdit`, `Publication`, `Candidates`, `Settled`,
  `Stage` from `context_insert::edit`; the overlay implements
  `context_insert::edit::Stage`.
- context-trace keeps feature `edit` (raw primitives only). The guard
  `only_insertion_and_the_reader_enable_the_edit_feature` stays as is.
- Tests: `positioned/tests/edit.rs` and the `create_*` cases of
  `model.rs` → `context-insert/src/tests/edit.rs`.

Verification: `rg 'GraphEdit|\.edit\(\)' context-trace/src` → no match
outside docs; all totals unchanged.

### 6.5 P4: fixture lanes and manifest examples

- Split `context-trace/src/positioned/interchange/fixtures.rs`: keep the
  shared part (§4.2); move the insert lane to
  `context-insert/src/fixtures.rs` and the read lane to
  `context-read/src/fixtures.rs`, both `#[cfg(any(test, feature = "test-api"))] pub mod fixtures;`.
  Paths stay anchored on context-read's fixture directory through
  `context_trace::positioned::fixtures::fixtures_dir()`.
- `git mv context-trace/examples/insert_manifest.rs context-insert/examples/`,
  `git mv context-trace/examples/read_manifest.rs context-read/examples/`;
  move the `[[example]]` entries; update the commands in the example docs
  and `context-read/README.md`.
- Update imports in `context-insert/src/tests/*`,
  `context-read/tests/{fixture_validation,reader_behavior}.rs`,
  `context-read/tests/support/*`.

### 6.6 P5: search-only navigation

- Move `Navigate::end_at` and `Navigate::advance_end_path` (plus private
  helpers only they use) into `context-search/src/path/navigate.rs` as
  `pub trait PathNavigate<'a>: Navigate<'a>` with a blanket impl; import it
  in `path/role.rs`, `path/role_path.rs`. Keep `Located` in context-trace
  (returned by `start_at`, O2).
- If a helper is shared with the remaining `Navigate` methods, keep it in
  context-trace as `pub` `#[doc(hidden)]` instead of duplicating it.

## 7. Guards and audits (V)

### 7.1 V1: layering guard

Extend `context-trace/src/positioned/tests/guard.rs` (it already parses
`cargo metadata`) with:

```rust
#[test]
fn crates_depend_only_on_lower_layers() {
    const ORDER: [&str; 4] = ["context-trace", "context-search", "context-insert", "context-read"];
    // for every package p in ORDER, every dependency d (normal, dev, build)
    // that is in ORDER must have a lower index than p;
    // no package outside {*-legacy, ngrams} depends on a "*-legacy" package;
    // a "*-legacy" package depends only on context-trace(-macros) and lower legacy crates.
}
```

### 7.2 V2: ownership audit (repeatable)

Run the script of §3.2 for search, insert, read, api, ngrams against
`context_trace`; every identifier exported by `context_trace` must be
pinned (O2) or appear in the lists of search, insert **and** read, or be
`test-api` infrastructure. Record the result in this plan (§9).

### 7.3 V3: docs and memories

- READMEs, `FILE_INDEX.md`, `HIGH_LEVEL_GUIDE.md` of the four crates,
  the repository `README.md` (crate list), plan `INDEX.md`.
- After merge, update the repository memories that describe
  `<crate>::legacy` and `cargo test -p <crate> --features legacy-tests`
  (new: `cargo test -p context-X-legacy --features legacy-tests`).

## 8. Verification

Commands (Linux codespace: exclude the winit crates):

```sh
cargo check --workspace --all-targets --exclude context-editor-kernel --exclude sandbox-app
cargo test -p context-trace --features test-api,interchange,serde,edit
cargo test -p context-search --features test-api
cargo test -p context-insert --features test-api
cargo test -p context-read  --features test-api
cargo test -p context-trace-legacy  --features legacy-tests,test-api
cargo test -p context-search-legacy --features legacy-tests,test-api
cargo test -p context-insert-legacy --features legacy-tests,test-api
cargo test -p context-read-legacy   --features legacy-tests,test-api
cargo test -p ngrams
cargo test -p context-api
```

Baseline (`3ea3cb0`, origin crates with `--features legacy-tests,test-api`;
trace also `interchange,serde,edit`) and spike after Part L
(passed / ignored, unit + integration + doc tests):

| Crate | Baseline | After Part L (new + legacy) | Note |
|---|---|---|---|
| trace | 205 / 39 | 95 + 108 = 203 / 11 + 30 = 41 | 2 root doctests flip `no_run` → `ignore` (T9 restores them) |
| search | 77 / 7 | 20 + 57 = 77 / 7 | equal |
| insert | 67 / 1 | 13 + 54 = 67 / 1 | equal |
| read | 212 / 10 | 129 + 80 = 209 / 10 | 3 v2 tests in `fixture_validation.rs` compiled out until R3 moves them |
| ngrams | 50 / 3 | 50 / 3 | equal |
| context-api | 115 passed, 3 failed | 118 / 0 | the 3 failures came from the concurrent baseline run, not the refactor; recheck on a quiet machine |

Acceptance criteria:

1. Every row of the table matches after each task (after R3 and T9 the
   read and trace totals equal the baseline exactly). Part P moves tests
   between crates; compare the sum over all crates, which must stay
   205 + 77 + 67 + 212 + 50 + 118 (+ the doctests restored by T9).
2. `rg -n 'legacy' context-{trace,search,insert,read}/src context-{trace,search,insert,read}/Cargo.toml`
   → comments only (no `cfg`, no paths, no features).
3. `rg -n 'context_\w+_legacy' context-{trace,search,insert,read,api}` → no match.
4. Guards: `crates_depend_only_on_lower_layers`,
   `only_insertion_and_the_reader_enable_the_edit_feature`,
   `positioned_graph_has_no_content_keyed_field` pass.
5. `context-trace/src/positioned` contains only `model`, `store`,
   `committed`, `check`, `interchange`, `tests`; context-trace no longer
   has `key/`, `direction/`, `tests/`, `examples/`.
6. Benches build (`cargo bench -p context-read --no-run`,
   `cargo bench -p context-read-legacy --no-run`).
7. The V2 audit finds no context-trace export that fails O2/O3.

## 9. End result

- **context-trace** (≈11k lines, down from ≈37.6k): ids, atoms, widths,
  logging/test tracing; the positioned model, store contracts (read traits,
  `Navigate`, sealed raw writes), committed store with
  serde and table construction, validation, interchange format and shared
  fixtures. Features: `edit`, `interchange`, `serde`, `test-api`.
- **context-search** (≈7.7k): search plus the paths, trace states/cache
  and keys it runs on.
- **context-insert** (≈2.9k): all mutation — `GraphEdit` with create,
  extend, add occurrences, dissolve, discard, settle, stabilize — and
  split/join/insert, plus the insert fixture lane.
- **context-read** (≈3k + tests): reader orchestration and the overlay
  stage it owns, plus the read fixture lane.
- **context-{trace,search,insert,read}-legacy**: the row model, unchanged
  except paths, buildable and testable on its own; used only by `ngrams`
  and its own tests.
- The dependency graph of §2.1 is enforced by a test; no `cfg(feature =
  "legacy")` remains in the new crates.

## 10. Risks and decisions

| ID | Topic | Decision |
|---|---|---|
| D1 | Legacy crates depend on the new context-trace | Yes, for ids, `Token`, atoms, logging and macros (sharing them avoids duplicating 4.4k lines of logging and keeps `Token` identical for ngrams). Everything legacy-specific moves out of context-trace (T1–T8). |
| D2 | Items moving up that legacy still needs (`key`, cursor typestates) | Frozen copies in the legacy crates (T8, S1), not a dependency on a higher new crate. |
| D3 | Overlay stage placement | read (O4). Cost: the overlay commit is rewritten over the existing sealed primitives (P0.5, no new primitive) and a few `#[doc(hidden)]` model helpers become `pub`. Fallback if P2 turns out larger than expected: keep the overlay in context-trace and record an exception (E2) — the rest of the plan is independent of it. |
| D4 | Dissolve/stabilize placement | insert, with `GraphEdit` (O6), although only the reader calls them; the `Stage` trait goes with them. |
| D5 | `from_interchange` without `GraphEdit` | Table construction + `validate` (as serde already does). Risk: vertex indices or generation differ from today; P0.2 adds an equality test before anything moves. |
| D6 | Seal exposure | The raw primitives become nameable with feature `edit` (insert, read only, guarded). Mutation stays limited to the insert and read algorithms (repository rule); the overlay's raw writes, including its commit, are documented as reader-internal. |
| D7 | `positioned` namespace | Kept for this plan; flattening is a separate follow-up. |
