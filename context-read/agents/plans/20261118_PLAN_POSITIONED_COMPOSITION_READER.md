---
tags: `#plan` `#context-read` `#context-insert` `#context-search` `#context-trace` `#positioned-composition` `#overlap` `#frequency` `#streaming`
summary: Migrate the context stack from contiguous child-pattern rows to positioned compositions. Retained vertices store maximal child occurrences with offsets; overlaps are explicit and gaps/wrappers are never vertices. Introduce positioned locations and navigation, replace row-based overlap closure with an occurrence merge plus seam discovery, accumulate input in owned positioned covers, and only then add streaming retention with retrospective host updates.
status: 🗄️ history; open work is tracked in 20261120_PLAN_POSITIONED_READER_OPEN_WORK.md
revision: 5
date: 2026-11-20
depends_on: ../../../ngrams/agents/plans/20261118_PLAN_POSITIONED_COMPOSITION_ORACLE.md
prerequisite: 20261119_PLAN_POSITIONED_MIGRATION_PREPARATION.md (Phase −1 legacy isolation, and Phases 0-2 below)
related: 20261116_PLAN_OVERLAP_PROVIDER_MIGRATION.md (row-based closure; superseded for the positioned backend once Phase 4 below lands)
---

# Plan: Positioned Composition Reader Migration

> **Open work now lives in [20261120_PLAN_POSITIONED_READER_OPEN_WORK.md](20261120_PLAN_POSITIONED_READER_OPEN_WORK.md).**
> This file is kept as design history and rationale.

**Date:** 2026-11-18
**Crates:** `context-trace` (model, locations, navigation), `context-search`
(boundary and occurrence search), `context-insert` (merge, seam discovery,
publication), `context-read` (expansion, accumulation, retention), plus
consumers in `context-api`, `context-cli`, `context-http`, `context-mcp` and
the context editor.
**Scope:** reader-side data model and algorithms. Oracle semantics and the v3
fixture format are owned by the ngrams plan.

## Revision 3 (2026-11-19)

[`20261119_PLAN_POSITIONED_MIGRATION_PREPARATION.md`](20261119_PLAN_POSITIONED_MIGRATION_PREPARATION.md)
is the prerequisite of this plan: its Step 1 ("Phase −1") fences the row model
off in `legacy` modules (`context_<crate>::legacy`, default-on `legacy`
feature, legacy tests opt-in via `legacy-tests`), and its Steps 2 and 4
implement Phases 0-2 below. State after the preparation plan:

- **Phase 0 ✅** (prep Step 2.4/2.5): v2/v3 fixture lanes with separate
  manifests, `v3/scenarios.json` authoritative; new v3 scenarios
  `repeat_abcabca`, `repeat_ababa`, `seam_bcd`, `late_retention__1/__2`,
  `duplicate_observation`, `substring_input`; the `subdivision`/`visualization`
  corpus is the existing `infix` scenario.
- **Phase 1 ✅** (prep Step 2.2/2.3/2.6): `context_trace::positioned`
  (`Composition`, `Occurrence`, `OccurrenceLocation`, `OccurrencePath`,
  `IntervalView`, `PositionedGraph<G>`, `PositionedGraphRef =
  Arc<RwLock<…>>`, `from_interchange`/`to_interchange`, `diff`); every v3
  fixture round-trips. **Type deviation:** `Occurrence { offset: AtomPosition,
  child: Token }` stores a `Token` (index + width) instead of a `VertexIndex`,
  so a composition knows its children's widths without graph lookups.
  `OccurrenceLocation`/`OccurrencePath` still address children by
  `VertexIndex`.
- **Phase 2 ✅** (prep Step 4): the three operations of §3 as separate APIs —
  `context_search::boundary::{boundary_occurrences, prefixes, postfixes}`,
  `PositionedGraph::occurrences_in` and `PositionedGraph::read_interval` —
  each tested on every v3 fixture against brute-force string matching.
- `context_read::read(&PositionedGraphRef, impl IntoIterator<Item = char>)
  -> Option<Token>` exists as an `unimplemented!` stub; the positioned reader
  lane (`context-read/tests/reader_behavior.rs`) is ignored until Phase 4.
- Phase 5 now records the within-input provisional-retention rule and the
  across-input monotonicity evidence (prep plan §0.5, Step 2.6).

## Revision 4 (2026-11-19): model requirements

The Phase 1 scaffold indexes vertices by expanded content
(`PositionedGraph::by_content: HashMap<Vec<Atom>, VertexIndex>`,
`PositionedGraph::find(&[Atom])`) and expands content inside `insert` and
`validate`. That contradicts three requirements, which now bind every phase:

- **R1 — No content-keyed index.** The graph must scale to massive numbers of
  vertices of arbitrary width, so no index structure stores expanded content.
  The only value-keyed index is atom value → atom vertex (bounded by the
  alphabet; the legacy `atom_keys` equivalent). A composite vertex is found
  only structurally: by **search** from atoms through parent links and
  compositions. A content lookup table also invites the wrong algorithm
  (searching indices by content instead of by structure), so it must not
  exist, not even as a convenience.
- **R2 — Expanded content is debug-only.** Expanded atom sequences may be
  *stored* only in debug builds (`#[cfg(debug_assertions)]`, e.g. a label per
  vertex for `Display` and the `TokenNameResolver`). Release builds store
  only structure. Computing content on demand by traversal
  (`read_interval`) stays allowed for export, tests and diagnostics, but no
  algorithm may depend on it.
- **R3 — Search states are reusable.** As in the legacy stack
  (`Response { cache: TraceCache, end }` → `InitInterval::from(Response)` →
  insertion, and the reader continuing from the last match), a search result
  carries the traced structure needed by (a) insertion and (b) the next,
  adjacent search. Neither insertion nor the next search may repeat the
  traversal that produced it.

Consequences:

- **Uniqueness of content** (one vertex per content) is no longer checked by
  lookup at insertion. It follows from search completeness: insertion is
  only reachable from a search outcome proving no retained vertex spans the
  range (a maximal cover loses no occurrence, and parent links reach every
  retained superstring, so a complete search cannot miss an existing vertex).
  Debug builds additionally assert it via the debug labels.
- **Overlap agreement** (overlapping children denote the same atoms) is
  likewise established by the search evidence that produced the occurrences,
  and checked by expansion only under `debug_assertions` / `test-api`.
- **Tests** address fixture vertices through a test-only name map built by
  `from_interchange` (fixture key → `Token`), never through the graph. Brute
  force expectations come from fixture strings, not from graph lookups.

Phases 2b and 2c below implement R1-R3 before any algorithm work (Phase 3).

## Revision 5 (2026-11-20): search keeps the legacy architecture

The rev-4 Phase 2c design (`SearchState` with a frontier, `continue_at`,
`PositionedTrace`, comparing content by descent) is withdrawn. The positioned
model only removes redundant links, so search keeps the legacy architecture:
climb parent links from the first query token, compare next siblings with the
query in a width-ordered queue, take the first (unique) match, extend it with
the root cursor, and raise on root exhaustion. The result is a trace with an
up path, a root span and a down path. Only sibling navigation changes
(`start_at`, `prefix_child`, offset-labelled locations). Legacy names are
kept (`ParentState`, `RootCursor`, `MatchResult`, `TraceCache`, `Response`).
R3 is met as in legacy: `Response` seeds insertion and the next adjacent
search. Phases 3 and 4 consume `Response`. A Python prototype confirmed the
design on all v3 fixtures (Phase 2c, "Evidence").

---

## 1. Target semantics

From the oracle plan:

- Vertices are roots, atoms and frequency-selected tokens only.
- A composite vertex stores `Composition = [(offset, child)]`: its maximal
  retained proper-child occurrences. Partial overlaps are kept; occurrences
  wholly contained in another child occurrence are removed; all positions are
  covered.
- Rows, gap views and overlap groups are optional owner-local layouts. They
  have no global identity and never influence compositions.

The reader must produce the same final semantic graph as the oracle, compared
through v3 fixtures. **No reader step may create a vertex for a gap, a wrapper,
a slice, an accumulation buffer or a successful lookup.**

## 2. Current dependencies on contiguous rows

| Area | Current assumption |
|---|---|
| `context-trace` `ChildPatterns = HashMap<PatternId, Pattern>` | A decomposition is a concatenating child sequence |
| `ChildLocation { parent, pattern_id, sub_index }` | A child is addressed by row and index |
| `postfix_iter` / prefix frames | Boundaries follow first/last row children |
| `context-search` | Matches advance through sibling indices in a row |
| `context-insert` split/join, `scope_complement`, `Overlap::close` | Overlaps are closed by constructing contiguous alternatives and interning complements |
| `context-read` `RootManager` | Accumulation appends to or replaces the tail of an owned row |
| `GraphData` v2 / fixtures | Expected structure is `string[][]` |

## 3. Core concepts to introduce

- **Occurrence location:** `(parent, offset, child)`. A path is a chain of
  occurrences; offsets add when descending.
- **Interval view:** `(owner, range)`. A read/navigation request, never a token.
- **Three distinct operations** (must not be conflated):
  1. **Boundary search:** retained occurrences starting/ending at a boundary,
     with the selected occurrence path preserved as evidence.
  2. **Occurrence enumeration in an interval:** descend through intersecting
     children, including children crossing the boundary; report whole
     retained occurrences inside the interval; deduplicate by
     `(token, owner-relative offset)`.
  3. **Content reading:** emit each atom position exactly once; may cut through
     a child without creating a token.

Key property: a maximal cover loses no occurrence of a retained token. Any
occurrence not directly listed is inside some maximal proper child; descending
reaches it in a strictly smaller vertex. Multiple paths can reach it, hence
deduplication.

## 4. Phases

### Phase 0 — Fixtures and contract

- Consume v3 oracle fixtures (`format_version: 3`) in a new reader test
  support module; keep v2 fixtures frozen and provisional.
- Add v3 scenarios needed below: `abcabca`, `ababa`, the
  `subdivision`/`visualization` corpus, a seam case where a retained token
  crosses both sides of a merge (e.g. `abc`/`cde` with retained `bcd`), and a
  late-retention case (a token that becomes retained after an earlier host
  was read).
- Never derive expectations from reader output.

**Gate:** v3 fixtures load and validate; existing v2 tests unchanged.

**Status:** ✅ done by the preparation plan, Step 2 (2026-11-19).

### Phase 1 — Positioned model in `context-trace`

- Add a positioned composition vertex form beside `ChildPatterns`, with
  reciprocal parent occurrence links `(parent, offset)`.
- Add `OccurrenceLocation`, occurrence paths and interval views.
- Add constructors from v3 `GraphData` and a validator equivalent to the
  oracle's rules (coverage, containment pruning, reciprocal links, no
  duplicate content).
- Keep the row model untouched; the backends coexist during migration.

**Gate:** v3 fixtures round-trip through the positioned graph.

**Status:** ✅ done by the preparation plan, Step 2 (2026-11-19). Deviation:
`Occurrence.child` is a `Token`, not a `VertexIndex` (see Revision 3).

### Phase 2 — Read-only navigation

- Implement the three operations from §3 over the positioned model, in
  `context-trace` (traversal) and `context-search` (boundary/prefix/postfix
  search returning occurrence paths).
- Tests on oracle fixtures only:
  - reconstruct every vertex's content;
  - boundary searches return the oracle-retained prefixes/postfixes;
  - enumeration in an interval cutting through a child finds the smaller
    retained occurrences inside it;
  - the same occurrence reached through two paths is reported once.

**Gate:** all navigation tests pass on every v3 fixture.

**Status:** ✅ done by the preparation plan, Step 4 (2026-11-19):
`context-search/src/boundary/` (boundary search with `OccurrencePath`
evidence), `context-trace/src/positioned/enumerate.rs`
(`occurrences_in`) and `read.rs` (`read_interval`). The tests compare every
vertex, every position and every interval of every v3 fixture with
brute-force string matching over the fixture's retained set, and pass
without the `legacy` feature.

### Phase 2b — Content-free model (R1, R2)

Corrects the Phase 1 scaffold; no new algorithms.

**Clean slate (2026-11-19):** every content-based implementation was removed
without replacement, so the correct model can be designed from scratch:
`by_content`, `find`, `content`, `compose_content`, the `ContentMismatch` /
`DuplicateContent` errors and the content checks in `insert`/`validate`.
What remains is only structural: `Composition`, parent links, an atom-value
index (`atoms: HashMap<Atom, VertexIndex>`, used by `insert_atom`),
`read_interval`, `occurrences_in`, boundary search, and `from_interchange` /
`to_interchange` (export names are read by traversal, never stored). Tests
that looked vertices up by content were deleted, not rewritten: the
`read_interval`, `occurrences_in` and boundary-search tests (8),
`content_reconstructs_from_compositions` and
`validate_rejects_duplicate_content`. The navigation APIs are untested until
the model is redefined. The reader lane no longer asserts per-input root
content; it still compares the final graph with the fixture. Workspace:
542 passed / 0 failed / 444 ignored. The remaining items below are to be
re-decided with the model guidance that follows; do not implement them
ahead of it.

- [x] Remove `PositionedGraph::by_content` and `PositionedGraph::find`;
  `insert_atom` uses an atom-value index.
- [x] `insert(composition)` no longer expands content. It checks only
  structure: `Composition` invariants, known children, recorded widths;
  parent links are written. (Content agreement and uniqueness checks were
  removed, not moved into debug checks.)
  (Phase 2c replaces the public `insert(Vec<Occurrence>)` by insertion from a
  `Response`; a raw structural insert stays `pub(crate)`/`test-api` for
  fixture loading.)
- [ ] Debug-only content: `#[cfg(debug_assertions)] label` on
  `PositionedVertex` (built from the children's labels at insertion); the
  graph registers itself as `TokenNameResolver` in debug builds. Release
  `Display` shows index and width only.
- [x] `content(index)` removed (export uses `read_interval` directly).
- [ ] `validate()`: structural rules and reciprocal links always; the
  one-vertex-per-content and overlap-agreement rules only under
  `debug_assertions` or `test-api`, computed by traversal into a temporary
  set that is dropped after validation.
- [ ] `from_interchange` returns the graph plus a test/interchange name map
  (`BTreeMap<String, Token>`, `test-api`); `to_interchange` names vertices by
  traversal. Rewrite the Phase 1/2 tests that call `find(&atoms)` to use the
  name map. Moved to Phase 2c.0 (prerequisite of the search tests).
- [ ] Guard: a test (or `clippy::disallowed_methods`/review rule) that the
  positioned graph has no content-keyed field; document R1/R2 in the
  context-trace README.

**Gate:** all Phase 1/2 tests pass through the name map; no positioned type
stores expanded content in release (`cargo test --release -p context-trace
--no-default-features --features test-api` passes and the release
`PositionedVertex` has no label field); `--no-default-features` check green.

### Phase 2c — Positioned search: legacy architecture, positional sibling navigation (R1, R3)

**Design decision (rev 5, 2026-11-20).** The positioned model only removes
redundant links: a vertex stores the children that are strictly necessary
(maximal occurrences with offsets), but every descendant stays reachable, and
parent links still reach every retained superstring. Therefore the legacy
search/trace architecture is **kept as is**; only sibling navigation and the
addressing of locations change. Do not redesign the algorithm.

#### Legacy algorithm (reference)

Code in `context-search/src/legacy` and `context-trace/src/legacy/trace`.

1. **Seed** (`DirectedTraversalPolicy::gen_parent_batch`, `policy.rs`): every
   parent link of the first query token, `parent × (pattern_id, sub_index)`,
   becomes a `ParentState { prev_pos, root_pos, path: IndexStartPath }` paired
   with the query cursor (`ParentCompareState`).
2. **Queue** (`match/mod.rs`, `SearchQueue`/`SearchNode`): a min-heap on the
   root width, so the smallest context is compared first.
3. **Next sibling** (`ParentState::advance_state`, `trace/state/parent.rs`):
   `pattern[sub_index + 1]`. If the row ends, `Err` → `next_batch` raises
   the state to the root's parents (`PathRaise::path_raise`, which extends the
   start path upward and advances `root_pos`).
4. **Compare** (`compare_leaf_tokens`, `compare/state/decomposition.rs`):
   - equal tokens → `FoundMatch`;
   - two different atoms → `Mismatch`;
   - otherwise the wider side is decomposed into its prefix children (the
     head of every row, widest first; `QueryMajor`/`GraphMajor`; both sides
     on equal width) → `ChildCandidate`s are queued.
5. **First match wins** (`SearchIterator::find_next_root_match`): the first
   `FoundMatch` clears the queue. A non-frequent token sequence is unique
   (otherwise it would be a token), so no other candidate can also match.
6. **Root cursor** (`match/root_cursor/advance.rs`): query cursor and graph
   end path advance together (`MovePath<Right, End>`: pop path nodes, next
   index in the row, else move the root index). It ends with:
   - `Conclusive(Exhausted)`: the query is consumed;
   - `Conclusive(Mismatch)`: maximal match after progress;
   - `Inconclusive`: the root is exhausted → parent batch of the root,
     back to step 2 with the best match kept.
7. **Result**: `MatchResult { path: PathCoverage (EntireRoot | Range |
   Prefix | Postfix over an IndexRangePath = start path ↑, root row span,
   end path ↓), cursor: checkpointed query cursor }`, traced into the
   `TraceCache` (`VertexIndex → { bottom_up, top_down }` keyed by
   `DirectedKey(token, Up/DownPosition)`, edges labelled with
   `SubLocation(pattern_id, sub_index)`). `Response { cache, end }` seeds
   insertion (`InitInterval::from(Response)`) and the next adjacent search.

#### What changes in the positioned model

| Legacy (rows) | Positioned |
|---|---|
| `ChildLocation(parent, pattern_id, sub_index)` | `OccurrenceLocation(parent, offset, child)`; no `pattern_id` |
| parent links `parent → {(pattern_id, sub_index)}`; seed batch `parent × pattern index` | `parents: parent → {offset}`; seed batch `parent × offset` |
| next sibling `pattern[i + 1]` | `start_at(root, p)` with `p = entry offset + width` of the matched part (below) |
| `prefix_children()`: head of every row, several branches | `prefix_child()`: the single occurrence at offset 0; decomposition is one deterministic chain |
| end path `MovePath`: pop, `index + 1`, else move root index | pop steps whose occurrence ends at or before `p`, then descend by `start_at`; root exhausted ⇔ `p == width(root)` |
| `path_raise`: `root_pos += width(pattern[entry+1..])`, push `ChildLocation` | raising root `R` through link `(P, o)`: span `[s, e)` in `R` becomes `[o + s, o + e)` in `P`; push `OccurrenceLocation(P, o, R)` onto the start path |
| `IndexRangePath`: start/end paths of `ChildLocation`s, root row with entry/exit index | `RangePath { root, start: Vec<OccurrenceLocation> (↑), end: Vec<OccurrenceLocation> (↓) }`; root span = absolute offsets, entry and exit occurrences may overlap |
| `TraceCache` edges labelled `SubLocation` | same shape and keys (`DirectedKey`), edges labelled by occurrence offset |
| `PathCoverage` from row indices | same four variants, classified by `span.start == 0` and `span.end == width(root)` |

Unchanged:
- seeding from the first query token;
- the width-ordered queue and clearing it on the first match;
- the three root-cursor outcomes and raising on `Inconclusive`;
- the checkpoint/candidate query cursor;
- the trace shape (up path, root span, down path);
- `Response` as the reusable result (R3).

**`start_at(R, p)`: the new sibling primitive.** It returns the largest
retained token that starts at relative position `p` inside `R`, together with
the descent steps. By the `Composition` invariants (sorted, no gaps, no
containment, hence strictly increasing ends):

1. Binary search (`partition_point`) for the last occurrence `C` with
   `C.offset ≤ p`. `C` contains `p`, and among the occurrences containing `p`
   it ends furthest.
2. If `C.offset == p`, return `C.child`. Every other retained token starting
   at `p` lies inside `C`, at offset 0, so it is a prefix and is reachable
   through the `prefix_child` chain.
3. Otherwise recurse into `start_at(C.child, p − C.offset)` and record the
   step. Overlaps are handled without special cases: the next sibling may
   lie inside a cover that started before `p`.

Cost: O(depth · log(children)), without any content access (R1).

**Evidence** (Python prototype on all 58 v3 fixtures; kept in the session
files as `search_prototype.py`, not committed):
- `start_at` returned the largest retained token occurring at `p` in all
  706 `(vertex, position)` checks;
- the algorithm above (climb from the first atom, width-ordered queue, first
  match clears the queue, root cursor with `start_at`, raise on
  exhaustion) matched the brute-force expectation for all 17,979 queries;
- no query had an ambiguous expectation, and no query had two candidates
  of equal width matching at the first comparison.

#### Implementation steps

New code lives at the crate roots (not in `legacy`), passes the
`--no-default-features --features test-api` check, mirrors the legacy module
layout and names (so the correspondence stays obvious), and does not port
the visualization events (later, if needed).

- [ ] **2c.0 Prerequisite (from Phase 2b).** `from_interchange` returns a
  test-only name map `BTreeMap<String, Token>` (`test-api`). Search tests
  translate brute-force string expectations into tokens with it; never
  through the graph (R1).
- [ ] **2c.1 Navigation primitives** (`context-trace`, `positioned`):
  - `Composition::covering(p) -> &Occurrence` (binary search);
  - `PositionedGraph::start_at(root, p) -> (Token, Vec<OccurrenceLocation>)`;
  - `PositionedGraph::prefix_child(token) -> Option<Occurrence>`;
  - `PositionedGraph::parent_occurrences(token) -> impl Iterator<Item =
    (Token, AtomPosition)>`, in a deterministic order (width, index, offset).

  Tests: `start_at` on every vertex and position of every v3 fixture equals
  the brute-force largest fixture string occurring at `p` in the vertex
  string; steps form a valid `OccurrencePath` (`contains_path`) whose
  offsets sum to `p`.
- [ ] **2c.2 Trace states** (`context-trace`, `positioned::trace`):
  - `ParentState { root: Token, start: AtomPosition (entry), end:
    AtomPosition, start_path }` with `advance_state` (sibling via
    `start_at(root, end)`, `Err` when `end == width(root)`) and `path_raise`;
  - `ChildState`/`RootChildState` over `RangePath`;
  - `TraceCache` (same structure as legacy, offset-labelled edges);
  - `PathCoverage` classification.
- [ ] **2c.3 Search** (`context-search`, root modules `search`, `match`,
  `compare`, `cursor`, `state`):
  - `SearchQueue` (min-heap on root width), `SearchNode::{ParentCandidate,
    ChildCandidate}`, `NodeConsumer`;
  - `CompareState::compare_leaf_tokens` with the four cases, decomposing by
    `prefix_child`;
  - `RootCursor::advance_to_next_match` with the three outcomes, advancing
    the end path by `start_at`;
  - `MatchResult`, `Response { cache, end }`, and
    `Find::find_ancestor(query)` for `PositionedGraphRef`.

  Queries are token sequences. The reader passes atoms (resolved through the
  atom index). Composite query tokens follow the `QueryMajor` decomposition.
  `find_parent` is ported only when a consumer needs it.
- [ ] **2c.4 Canonical paths.** Define the positioned analogue of the
  legacy `simplify` and of the `path_raise` rule that omits a start-path
  step at offset 0 when the sub path is empty. Candidate rule: a path stops
  at the largest token whose boundary coincides with the span boundary. Fix
  the rule by tests before Phase 3 consumes paths.
- [ ] **2c.5 Reuse (R3).** `Response` is the only hand-off:
  - Phase 3 builds its insertion init from `Response` (successor of
    `InitInterval::from(Response)`) and reads occurrences from `cache`/`end`;
  - the reader continues the next adjacent search at the query cursor's
    checkpoint of `end`.

  `PositionedGraph::insert` stays `pub(crate)`/`test-api` (fixture
  loading). Public composite insertion goes through a `Response`.

#### Validation against oracle fixtures

All in `context-search` tests, loading fixtures via
`context_trace::positioned::fixtures::fixture_graphs()` plus the 2c.0 name map.
Expectations are computed from the fixture **strings** (corpus and vertex
keys), never from graph lookups or reader output.

- **Brute-force expectation** for a query `q`:
  - `k` = length of the longest prefix of `q` occurring in a corpus text;
  - `root` = the minimal-width fixture vertex string containing `q[..k]`,
    and `o` its offset there.

  The test asserts that `(root, o)` is unique. If it is not, the oracle
  contradicts the uniqueness assumption, and that must fail loudly.
- **Query set per fixture:**
  - every substring of every corpus text (complete matches, all coverage
    kinds);
  - every such substring extended by every alphabet atom (mismatch and
    raise paths);
  - seeded random strings over the alphabet (300 per fixture).
- **Assertions per query:**
  - `end.root == name_map[root]` and the span is `[o, o + k)`;
  - `PathCoverage` is `EntireRoot` iff `o == 0 && k == |root|`, `Prefix` iff
    `o == 0`, `Postfix` iff `o + k == |root|`, `Range` otherwise;
  - the end reason is query exhausted iff `k == |q|`, mismatch otherwise;
  - the start path ends at the atom `q[0]` at absolute offset `o`, the end
    path at `q[k − 1]` at `o + k − 1`, and every step satisfies
    `contains_path`;
  - test-side diagnostic: `read_interval(root, o..o + k) == q[..k]`
    (allowed in tests, R2);
  - the trace cache has an entry for every vertex on both paths, and its up
    edges agree with the parent links.
- **Uniqueness probe:** a `test-api` counter records how often a second
  equal-width candidate would also match at the first comparison. It must
  be 0 on every fixture (0 in the prototype).
- **Named cases** (mirroring the legacy search cases on oracle-consistent
  graphs):
  - `repeat_abcabca`: `abcab` matches through `abca@0`, raises to
    `abcabca` and descends into the overlapping `abca@3` →
    `Prefix(abcabca, 0..5)`;
  - `repeat_ababa`: `abab` raises `aba → ababa`;
  - `seam_bcd`: `bcd` and `abcd` (a span crossing the seam);
  - `repeat_ababab` and `repeating_known` (`xyyxy`), the legacy
    `cases/ababab.rs` and `cases/xyyxy.rs` corpora;
  - `late_retention__2`.
- **Continuation (R3):** for every corpus text, chained adjacent searches
  starting at each `end` checkpoint give the same sequence of matches as
  fresh searches at those positions.

**Gate:** all of the above pass on every v3 fixture; release build without
any content-keyed structure; `--no-default-features` check green. Only then
does Phase 3 start.

### Phase 3 — Positioned merge and seam discovery in `context-insert`

Replace row closure for the positioned backend:

1. Align the accepted match with the committed suffix using its offset.
2. Project retained occurrences of both sides into the new owner's
   coordinates (whole occurrence if the side is a retained proper child,
   otherwise its cover).
3. **Seam discovery:** find retained occurrences crossing the seam and not
   wholly inside either side.
4. Deduplicate and prune by containment; keep partial overlaps.

No complement interning, no wrapper creation, no nested closure. Preserve the
accepted evidence (anchor occurrence, expansion path) as in the overlap
provider plan; only its consumer changes.

Inputs are `Response`s (Phase 2c), not tokens: the occurrences to project
and the seam candidates come from their trace caches and end paths, so the merge
does not repeat the searches that found the two sides. Seam discovery may
traverse only beyond what the traces already cover. No content expansion
(R1/R2).

**Gate:** merging the oracle covers of overlapping observations reproduces the
oracle composition of the combined vertex, including the seam case.

### Phase 4 — Fixed-retention reader prototype

- Behind a backend switch, run `context_read::read` with the positioned model.
- The retained identity set for each test corpus is supplied (from the
  fixture), but not their offsets: the reader must discover occurrences,
  accumulate, merge and publish covers itself.
- `RootManager` keeps its ownership and commit boundary, but the in-progress
  root becomes an owned positioned cover with a committed width. Append
  extends the width and reconciles affected occurrences; an accepted overlap
  extends the same coordinate space. The real root vertex is published or
  reused only at the end of the input.
- The reader drives one chain of searches per input, as in legacy: each
  next search starts at the query checkpoint of the previous `Response`,
  and the committed root keeps the `Response` of its committed suffix (its
  trace cache and end path), so overlap expansion at the commit boundary
  reuses it. Re-searching a position already covered by a `Response` is a
  bug; a `test-api` counter is asserted in the reader tests.
- First milestone: read `abcabca` into one owned cover and publish
  `abcabca = abca@0, abca@3` without allocating `abc`, `bca`, `bc` or any
  wrapper vertex.

**Gate:** complete-graph equality with v3 fixtures for all scenarios under
supplied retention.

### Phase 5 — Streaming retention and retrospective updates

**Retention is append-only.** The reader induces a grammar for all input
observed so far. New input may add vertices but never removes a published
token or changes the atom sequence it denotes. Compositions and reciprocal
links may change as new tokens are discovered. Every historical input must
remain exactly reconstructible after those updates.

1. **Keep lossless input observations.** Preserve each input's full atom
   sequence, either explicitly or through references to composite tokens that
   expand to that sequence. Preserve input boundaries and multiplicity: two
   observations of the same text can share a root vertex but are still two
   observations. Occurrence evidence refers to input identity and atom offset,
   not to graph edges or row references.
2. **Discover previously untokenized spans.** New input can supply additional
   occurrences in different contexts, causing a span to qualify as a token.
   Discovery must reach inside existing composites and across their boundaries
   within an input; matching only existing vertices is not enough. Keep
   candidate evidence separate from vertex storage until the retention
   criterion is met. Candidate evidence obeys R1 too: candidates are keyed
   by structure (e.g. the pair of adjacent tokens/occurrences that spans
   them, plus input offsets), never by expanded content.
3. **Reconcile current and historical hosts.** On promotion, find the token's
   occurrences, including in earlier inputs and their retained hosts. For
   example, if `vi` qualifies later, revisit published `vis`. Insert each
   occurrence where visible; an eclipsed occurrence leaves that host's direct
   cover unchanged. A change to a retained child's internal cover does not
   change the whole span it exposes to its parents, but other occurrences of
   the promoted token can still affect those parents directly.

**Within-input provisional retention.** Retention is *not* monotone while an
input is still being read. Over a growing prefix of one input the batch oracle
gains and loses frequency-derived tokens (prep plan §0.5: 278 violations in the
probe; `abcab` retains `ab`, `abcabc` does not; `ababa` → `ab` lost at
`ababab`). Rule: frequency-derived tokens discovered inside an input stay
provisional (candidate evidence, not published vertices) until the input
completes; only completed-input boundaries are publication points. Atoms and
the input's own root are not affected.

**Across-input monotonicity evidence (empirical, not a proof).** Across
completed inputs no retained token was lost in any probe:

- prep plan §0.5 probe: 51 manifest scenarios plus 57 600 checks over
  3-text sequences of words of width 1-4 over 2-3 letters;
- permanent tests in `ngrams/src/tests/retention.rs`:
  `retention_is_monotone_across_inputs_of_reader_scenarios` (every v3 reader
  scenario, default), `retention_is_monotone_across_inputs_bounded_sweep`
  (196 two-text corpora over `{a,b}`, width ≤ 3, default),
  `retention_is_monotone_across_inputs_sweep` (`#[ignore]`, release:
  55 521 transitions = 54 000 from 27 000 three-text corpora over `{a,b}`,
  width ≤ 4, plus 1 521 two-text corpora over `{a,b,c}`, width ≤ 3), and the
  counter-example
  `retention_is_not_monotone_within_an_input` (`abcab` → `abcabc` loses `ab`).

Phase 5 design starts from these two facts; the proof (or a larger sweep plus
measurements of affected hosts) below is still required before cutover.

**Still open:** which index or cached evidence is needed beyond lossless input
content, and what bounds apply to retrospective discovery and affected hosts.
Local cover updates do not imply a bound on the number of affected hosts.
Prove that the batch oracle's retained set is append-only for the same
observation model, or resolve any divergence with the oracle plan before
cutover. Lossless reconstruction alone does not establish that equivalence.

**Gate:** after each completed input, every previously published token still
exists with unchanged content, and every observed input reconstructs exactly.
Compare complete graphs with independent v3 fixtures for successive corpora
without supplied retention, including late retention, duplicate observations,
and input permutations where the oracle is order-independent.

### Phase 6 — Switch default and retire row machinery

- Make the positioned backend the default for `context_read::read`.
- Migrate `context-api` compare, `context-cli` oracle tests, HTTP/MCP
  serialization and the context editor kernel to the positioned model or to an
  explicitly derived row layout.
- Optional: provide `layout(owner) -> rows` for consumers that still need
  brickwork; it must not create vertices.
- Remove row-based closure (`Overlap::close`, complement interning in
  `scope_complement`, row tail replacement) once no consumer depends on it.
  Mark the overlap provider plan superseded.
- Regenerate fixtures only from the oracle; remove v2 fixtures.

**Gate:** workspace build
(`cargo build --workspace --all-targets --exclude context-editor-kernel --exclude sandbox-app`
in the Linux codespace) and all tests pass; READMEs updated.

## 5. Risks

| Risk | Mitigation |
|---|---|
| Oracle not yet order-independent or v3 not available | Phases 0-3 of the ngrams plan are prerequisites |
| Greedy content reading used where structural search is required | Keep the three operations as separate APIs with separate tests |
| Seam occurrences missed by merging side covers | Explicit seam discovery step and dedicated fixture |
| Batch frequency disagrees with append-only retention | Phase 4 isolates frequency by supplying retention; Phase 5 requires resolving any divergence before cutover |
| Retrospective updates are unbounded | Measure affected hosts; answer the Phase 5 open questions before committing to a data structure |
| Large blast radius of the `context-trace` model change | Coexisting backends until Phase 6; no in-place rewrite of `ChildPatterns` |
| Content-keyed shortcuts creep back (lookup tables, expanding content to compare) | R1/R2 in Revision 4; Phase 2b guard test; review rule: no algorithm calls `content`/`read_interval` |
| Repeated traversals while reading many adjacent patterns | R3: `Response` seeds insertion and the next adjacent search (as legacy); continuation test in Phase 2c, counter asserted in Phase 4 |
| Uniqueness assumption (a non-frequent sequence has one context) fails for some oracle output | Phase 2c asserts unique brute-force expectations and a zero tie counter on every fixture; a failure is an oracle/model issue, not something to work around in search |
| Positioned search drifts from the legacy architecture | Rev 5: mirror legacy modules and names; only `start_at`/`prefix_child`/offset locations differ |

## 6. Non-goals

- Changing oracle semantics (owned by the ngrams plan).
- Storing gaps, wrappers or local layouts as vertices.
- Persistence format optimizations beyond what v3 requires.
