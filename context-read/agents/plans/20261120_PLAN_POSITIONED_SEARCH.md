---
tags: `#plan` `#context-search` `#context-trace` `#positioned-composition` `#search`
summary: Complete work package for positioned search (roadmap step 2; all planning for this step lives here). Explains the legacy row-based search, the local navigation changes required by child lists (`start_at`, `prefix_child`, offset locations), and the ordered implementation steps in context-trace and context-search, validated on v3 oracle fixtures.
status: 📋 ready (decisions 1–7 recorded 2026-11-20)
date: 2026-11-20
parent: 20261120_PLAN_POSITIONED_READER_OPEN_WORK.md (step 2)
---

# Plan: Positioned Search

**Crates:** `context-trace` (navigation, paths, trace states, trace cache),
`context-search` (search loop, compare, root cursor, result types).
**Roadmap:** step 2 of
[20261120_PLAN_POSITIONED_READER_OPEN_WORK](20261120_PLAN_POSITIONED_READER_OPEN_WORK.md);
this file is the complete work package for that step. Prerequisite: roadmap
step 1 (name map, debug labels, validation, navigation tests) is done.
Consumers of the result: positioned insertion (roadmap step 3) and the
reader (step 4).

**Goal:** `Find::find_ancestor` on `PositionedGraphRef` returns a reusable
`Response { cache: TraceCache, end: MatchResult }` with the legacy semantics,
validated on every v3 oracle fixture.

## Binding rules

From the roadmap, restated as they apply to this package:

- **No content-keyed index.** Only atom value → atom vertex. Composite
  vertices are found by search through parent links and compositions.
- **No content in algorithms.** `read_interval` is allowed in tests and
  diagnostics only; expanded content exists only under
  `#[cfg(debug_assertions)]`.
- **Legacy architecture.** Module layout and names mirror
  `context-search/src/legacy`; only sibling navigation, location addressing
  and the decisions below differ.
- **Reusable results.** `Response` is the only hand-off; insertion and the
  next adjacent search consume it without repeating its traversal.
- **Uniqueness is structural.** It follows from search completeness and is
  verified by counters and traversal in tests, never by lookups.
- **Oracle expectations.** Tests derive expectations only from v3 fixture
  strings via the name map, never from search output.
- **Legacy stays fenced.** New code lives at crate roots and passes the
  `--no-default-features --features test-api` check; run `--features
  legacy-tests` whenever legacy code is touched.

## Evidence

A Python prototype of this design (session files, `search_prototype.py`)
matched brute force on all v3 fixtures: 706/706 `start_at` checks,
17,979/17,979 queries, no ambiguous expectations, no equal-width ties at the
first match. It used the legacy fork on equal width and full recomputation of
the end path; decisions 1 and 2 are covered by the tests in 2.1 and 2.6.

## Part A — How the legacy search works

Legacy code: [`context-search/src/legacy`](../../../context-search/src/legacy),
trace states in [`context-trace/src/legacy/trace`](../../../context-trace/src/legacy/trace).

### Module map

| Module | Role |
|---|---|
| `search/mod.rs` | `Find::{find_ancestor, find_parent}`, `SearchState` (outer loop, keeps the best match), `finish_root_cursor`, `create_result_from_state` |
| `search/searchable.rs`, `state/start/core.rs` | turn a query (chars, tokens, pattern, cursor) into `StartCtx`; seed the first `ParentBatch` |
| `search/context.rs`, `policy.rs` | `AncestorSearchTraversal` / `ParentSearchTraversal`; `DirectedTraversalPolicy::{gen_parent_batch, next_batch}` (parent links → `ParentState`s; `ParentPolicy` never raises) |
| `match/mod.rs`, `match/iterator.rs` | `SearchQueue` (min-heap on root width), `SearchNode::{ParentCandidate, ChildCandidate}`, `NodeConsumer`, `SearchIterator::pop_and_process_one` |
| `compare/parent.rs` | `ParentCompareState::advance_state`: parent + query cursor → first `CompareState` on the next sibling |
| `compare/state/*` | `CompareState<Q, I>` (query cursor and graph cursor, each checkpointed), `compare_leaf_tokens`, prefix decomposition, `advance_query_cursor`, `advance_index_cursor` |
| `compare/iterator.rs` | `CompareIterator`: FIFO of decomposition states until the first `Match`/`Mismatch` |
| `match/root_cursor/*` | `RootCursor::advance_to_next_match`: lock-step advance inside one root |
| `cursor/*` | typestates `Matched`/`Candidate`/`Mismatched`, `PathCursor`, `Checkpointed<C, AtCheckpoint \| HasCandidate>` |
| `state/end`, `state/matched`, `state/response.rs` | `PathCoverage::{EntireRoot, Range, Prefix, Postfix}`, `MatchResult { path, cursor }`, `Response { cache, end, events }` |
| `context-trace/src/legacy/trace` | `ParentState` (`advance_state`, `path_raise`), `ChildState`/`RootChildState`, `TraceCache`, `Traceable` |

### Algorithm

The query is a pattern `q = [q0, q1, …]`; the query cursor holds a path into
it (descending into composite query tokens when needed) and an atom position.

1. **Seed** (`StartCtx::get_parent_batch`). Advance the query cursor past
   `q0` (if it cannot advance, the query is a single token → `SingleIndex`
   error). Every parent link of `q0`, i.e. every
   `(parent, pattern_id, sub_index)`, becomes a `ParentState` whose start
   path ends at `q0`. Each is paired with the query cursor
   (`ParentCompareState`) and pushed as `ParentCandidate`.
2. **Queue.** `SearchQueue` pops the candidate with the smallest root width
   first: the smallest context that could contain the match.
3. **Parent candidate** (`NodeConsumer::consume`).
   `ParentState::advance_state` looks up the next sibling
   `pattern[sub_index + 1]`.
   - Present → a `CompareState` (root = parent, start path ↑ to `q0`, end
     path at the sibling) compares the sibling with the next query token.
   - Absent (the start is the last child of its row) → `next_batch`: every
     parent of the root, each built by `path_raise` (push the old root's
     location onto the start path, update positions). Queued as new
     `ParentCandidate`s.
4. **Compare** (`compare_leaf_tokens`) of the graph leaf `g` with the query
   leaf `q`:
   - `g == q` → match;
   - both atoms, different → mismatch;
   - `|g| > |q|` → `GraphMajor`: decompose `g` into its prefix children (the
     head of every row, widest first), one `ChildCandidate` per prefix;
   - `|g| < |q|` → `QueryMajor`: decompose `q` the same way;
   - equal width, different → both decompositions.
5. **First match wins.** The first `ChildCandidate`/`ParentCandidate` that
   reaches a match clears the queue. Uniqueness argument: a sequence that is
   not a token is unique in the graph, and a repeated one is a narrower
   token that would have been popped first.
6. **Root cursor** (`finish_root_cursor` → `advance_to_next_match`). In the
   matched root, query and graph cursors advance in lock step:
   `advance_query_cursor` (query path `MovePath`), then
   `advance_index_cursor` (end path `MovePath`: pop until a parent row has a
   next index, `index + 1`, descend), then `CompareIterator` (step 4 without
   the queue). Each match updates both checkpoints. Ends:
   - query exhausted → `Conclusive(Exhausted)`: complete match;
   - mismatch → `Conclusive(Mismatch)`: maximal match in this root;
   - root exhausted → `Inconclusive`: yield the checkpoint as a
     `MatchResult`, push the root's parents (`get_parent_batch`) and continue
     at step 2. The next root match replaces the best match.
7. **Result.** `SearchState::search` keeps the last yielded `MatchResult`
   (no match at all → `EntireRoot` of `q0`). Paths are simplified
   (`simplify`), classified into `PathCoverage` (`from_range_path`: start at
   the root border? end at the root border?), traced into the `TraceCache`
   (`Traceable`: up edges along the start path, down edges along the end
   path, keyed by `DirectedKey { token, query position }`, labelled with the
   `SubLocation`), and returned as `Response { cache, end }`. `end.cursor`
   is the checkpointed query cursor the next adjacent search continues from.

### Row-specific parts

Only these touch rows: `ChildLocation`/`SubLocation` addressing,
`pattern_index_next`, `prefix_children` (one head per row), `MovePath` on
`(pattern_id, sub_index)`, `path_raise` widths from row slices,
`is_at_border`/`simplify` on row indices, and cache edges labelled by
`SubLocation`. Queue, typestates, checkpoints, compare outcomes, root-cursor
ends, best-match handling and the `Response` hand-off are model-agnostic.

## Part B — What changes with child lists

A composite stores `Composition = [(offset, child)]`: maximal retained child
occurrences, sorted, gap-free, no containment, overlaps allowed. Parent links
are `(parent, offset)`. There are no rows, so "next sibling" and "prefix
children" are replaced by position-based lookups.

### Model invariants used

- **Ends increase strictly** with offsets (sorted + no containment). So the
  last occurrence with `offset ≤ p` has the largest end among them and, by
  gap-freedom, covers `p` for `p < width`.
- **Occurrence completeness.** Every retained token occurring in `V`'s
  string at `x` is either a composition occurrence of `V` at `x` or lies
  inside the (maximal) occurrence covering it; inductively, it is reachable
  top-down from `V` at `x` and bottom-up through parent links. Checked
  against brute force by the 2.1 tests.

### New navigation primitives

- **`start_at(R, p)`**: the largest retained token starting at `p` inside
  `R`. Take the last occurrence `C` with `C.offset ≤ p` (binary search). If
  `C.offset == p` return `C.child`, else recurse into
  `start_at(C.child, p − C.offset)`. Returns the token and its
  `OccurrenceLocation` steps. Correctness: any token starting at `p` lies in
  an occurrence `O` with `O.offset ≤ p`; if `O.offset < p` its extent after
  `p` is smaller than `C`'s. Cost `O(depth · log children)`, no content.
- **`prefix_child(V)`**: the unique occurrence at offset 0. Every retained
  proper prefix of `V` lies on the chain `prefix_child(V)`,
  `prefix_child(prefix_child(V))`, … (occurrence completeness at `x = 0`).
  This replaces "head of every row": one chain instead of a fan-out.

### Mapping

| Legacy (rows) | Positioned |
|---|---|
| `ChildLocation(parent, pattern_id, sub_index)` | `OccurrenceLocation(parent, offset, child)` |
| seed `parent × (pattern_id, sub_index)` | seed `parent × offset` (`parent_occurrences`) |
| next sibling `pattern[i + 1]` | `start_at(root, span.end)` |
| root exhausted: no `i + 1` | `span.end == width(root)` |
| prefix children: head of every row | `prefix_child` chain |
| end path `MovePath` | reuse the old end path with the corrected pop rule, then descend by `start_at` (equals `start_at(root, span.end)`, see "End-path update") |
| `path_raise` with row widths | raise `R` via `(P, o)`: span `[s, e)` → `[o + s, o + e)`; extend the start path with `(P, o, R)` |
| `IndexRangePath` (root row, entry/exit indices) | `RangePath { root, span, start ↑, end ↓ }` in absolute offsets; start and end occurrences may overlap |
| `simplify`, `is_at_border` | border is a property of the occurrence: start `offset == 0`, end `offset + width == width(parent)`; canonical-path rule (2.4); coverage from `span` |
| cache keys `DirectedKey { token, query pos }`, edges labelled `SubLocation` | same keys (query positions), edges labelled by the child offset in its parent; the root offset `o` is stored once in the span |
| `ChildQueue`/`CompareIterator` fan-out over rows | same structure; each decomposition adds exactly one state (no fan-out) |
| equal width: fork `GraphMajor` + `QueryMajor` | one combined step `(pc(g), pc(q))`, third mode `BothMajor` |
| query cursor descends into composite query tokens | same, natively: `prefix_child` to split, `start_at` to advance inside the query token |

### Compare with one prefix chain

`compare_leaf_tokens(g, q)` keeps the legacy outcomes (match, atom
mismatch, decompose). Decomposition produces exactly one state, with
`pc = prefix_child`:

- `|g| > |q|` → `GraphMajor`: `(pc(g), q)`;
- `|g| < |q|` → `QueryMajor`: `(g, pc(q))`;
- `|g| == |q|`, `g ≠ q`, width > 1 → `BothMajor`: `(pc(g), pc(q))`.

**Decision 1 (b).** Legacy forks on equal width because a token may have
several rows. With one prefix chain the fork re-joins after one step
(`(pc(g), pc(q))` twice) and `k` equal-width steps would produce `2^k`
identical states. The combined step is correct: the largest common retained
prefix `t` of `g` and `q` is narrower than both (they differ), so it lies on
both chains below `g` and `q`. Always descending the wider side (both on
equal width) reaches `t` on both sides, otherwise width 1 is reached and the
atoms mismatch. This is a change of local navigation only.

### End-path update

**Decision 2 (b), in this phase (not deferred).** Occurrences may overlap, so
legacy-style "pop finished steps" can keep a step whose child still covers
the next position `p` while a later sibling starting at or before `p`
reaches further:

```
R = X@0 (width 5), Y@2 (width 5); end path through X; next p = 3
naive pop:      X covers 3 → start_at(X, 1) → ends at 5 at most
start_at(R, 3): last occurrence with offset ≤ 3 is Y → can reach 7
```

**Corrected pop rule.** Walk the old end path from the root. Step
`(P, o, C)` (absolute offset `a` of `C`) is kept iff `C` covers `p`
(`a ≤ p < a + |C|`), it is the last occurrence of `P` with offset `≤ p`
(the next occurrence of `P` starts after `p`, or there is none), and
`a < p`. At the first step that fails, truncate there and continue with
`start_at(P, p − offset(P))`; if `a == p` the step's child is the next
token and the path ends there. If every step is kept, descend from the last
child with `start_at`. The result is by construction the descent chosen by
`start_at(root, p)`, with only the changed tail recomputed. A test asserts
equality with `start_at(root, p)` on every fixture position (2.1).

### Composite queries

**Decision 6 (b).** Queries are token sequences, as in legacy (context-insert
overlap search, context-api, continuation cursors pass multi-token
patterns). Reading a composite query token's atoms would be content access,
so the query side navigates natively, symmetric to the graph side:
`prefix_child` splits a query token in `QueryMajor`/`BothMajor`, and the
query cursor advances inside a token with the same end-path update
(`start_at` plus the corrected pop rule). Overlaps inside query tokens are
handled identically.

### Worked example (`repeat_abcabca`)

Graph: `abca = a@0 b@1 c@2 a@3`, `abcabca = abca@0 abca@3`. Query `abcab`.

1. Seed: parents of `a` = `abca@0`, `abca@3` (width 4).
2. Pop `(abca, 0)`: `start_at(abca, 1) = b` = `q1` → match, queue cleared
   (drops `abca@3`).
3. Root cursor in `abca`: `c@2`, `a@3` match; `span.end = 4 = width` →
   inconclusive. Raise to `abcabca@0` (span `[0, 4)`) and `abcabca@3` (span
   `[3, 7)`).
4. Pop `(abcabca, 0)`: `start_at(abcabca, 4)` → `abca@3` covers 4 →
   `start_at(abca, 1) = b`; end path `[(abcabca, 3, abca), (abca, 1, b)]`;
   `b` matches; query exhausted.
5. Result: root `abcabca`, span `[0, 5)`, `Prefix`. Start path
   `[(abcabca, 0, abca)]`: the climbed chain is `a → abca@0 → abcabca`, and
   it stops at `abca`, the largest climbed token starting at the match
   start. End path `[(abcabca, 3, abca), (abca, 1, b)]` stops at `b`, the
   first token on it ending at the match end 5. (`abcabca@3` alone would
   end at once: `start_at(…, 7)` is the root end, and `abcabca` has no
   parents.)

## Part C — Implementation steps

Tasks are numbered 2.x after the roadmap step and run in order; 2.1 can
start in parallel with 2.0. New code lives at crate roots (not under `legacy`); module and type names
mirror legacy so that a side-by-side diff shows only navigation changes.

### 2.0 Promotions out of `legacy` (row-independent types only)

**Decision 7.3.** Promote only types without row-dependent behaviour; move
them to the crate root and re-export them from `legacy` so legacy keeps
compiling unchanged. Anything with row-specific behaviour is written new at
the root, not promoted.

- [x] `context-search::cursor`: `CursorState`, `Matched`, `Candidate`,
  `Mismatched`, `Checkpointed<C, AtCheckpoint | HasCandidate>` and the
  traits `MarkMatchState`, `CursorStateMachine` (generic over the path
  type). Their row-specific impls stay in `legacy::cursor`.
- [x] `context-trace` root: `DirectedKey`, `UpKey`, `DownKey`,
  `UpPosition`, `DownPosition` (token + atom position only).
- [x] **Decision 7.1.** A small error enum for positioned search at the
  crate root, with the legacy meanings: `SingleIndex` for a one-token query
  and `UnknownAtom` for an atom without a vertex; an empty query is rejected
  as legacy rejects it (check the legacy reason when porting `Searchable`).
  No parent match is a result, not an error: an `EntireRoot` response for
  `q0`, as in legacy. Do not reuse `legacy::ErrorReason`.

Run `cargo test -p context-trace -p context-search --features legacy-tests`
after each promotion.

*Done (2026-11-20), deviations:* `PathCursor<P, S>` (generic over the path)
was promoted too, with `HasCheckpoint`, `CandidateState` and
`AtCheckpoint`/`HasCandidate`; its row-specific impls (`GraphRoot`,
`MoveKey`, `MovePath`, `Display` via the legacy `CompactFormat` impl,
`PatternCursor` aliases) and `ChildCursor` stay in `legacy::cursor`. The keys
live in `context_trace::key` (re-exported at the root and from
`legacy::trace::cache::key::directed`); `DirectedPosition` and
`HasAtomPosition` moved with them (field type / shared accessor); the
`MoveKey` impls stay in legacy. The error enum is
`context_search::SearchError { EmptyPatterns, SingleIndex(Token),
UnknownAtom }`: legacy rejects an empty query with `EmptyPatterns` (empty
`Chars` give an empty pattern); `SingleIndex` carries only the token until the
positioned query path exists (2.2).

### 2.1 Navigation primitives (`context-trace`, `positioned`)

- [x] `Composition::covering(p) -> Option<&Occurrence>` (binary search,
  last `offset ≤ p`) and `Composition::prefix() -> &Occurrence`.
- [x] `PositionedGraph::start_at(root, p) -> Option<(Token,
  Vec<OccurrenceLocation>)>`; `None` iff `p ≥ width(root)`. Iterative.
- [x] `PositionedGraph::prefix_child(v) -> Option<Occurrence>`.
- [x] `PositionedGraph::parent_occurrences(v) -> impl Iterator<Item =
  (Token, AtomPosition)>` in deterministic order (parent width, index,
  offset).
- [x] `PositionedGraph::atom_token(&atom) -> Option<Token>`: read-only
  lookup in the existing atom index (today only `insert_atom` touches it).
- [x] `PositionedGraph::advance_end_path(root, steps, p)`: the corrected pop
  rule from "End-path update" (Decision 2 b, built in this phase). Truncates the old steps at the
  first step that is not `start_at`'s choice for `p` and descends from there
  with `start_at`. Shared by the graph-side and the query-side cursor.
- **Tests** (`positioned/tests/navigation.rs`): on every vertex and position
  of every v3 fixture, `start_at` equals the longest fixture key starting at
  that position of the owner string (brute force through the name map); the
  steps are a valid `OccurrencePath` (`contains_path`) whose offsets sum to
  `p`; the `prefix_child` chain equals all fixture keys that are prefixes,
  longest first. `advance_end_path(root, start_at(root, p₀).steps, p)`
  equals `start_at(root, p)` for every vertex and every pair `p₀ < p`
  (including overlap fixtures such as `seam_bcd`, `complex_overlap`).
- [x] Tests done (2026-11-20, `navigate.rs`, `tests/navigation.rs`), all v3
  fixtures pass, occurrence completeness holds everywhere. *Deviations:*
  `start_at` also returns `None` for an atom root (no proper descendant);
  roots are `impl HasVertexIndex`, positions `impl Into<AtomPosition>`;
  `advance_end_path` takes the old steps by value and returns the same
  `Option<(Token, Vec<OccurrenceLocation>)>` as `start_at`;
  `Composition::covering(p)` is `None` for `p ≥ width`. Extra tests:
  `covering`, `parent_occurrences` (= maximal key occurrences by brute force),
  `atom_token`; the `advance_end_path` test also asserts that a naive
  legacy-style pop differs on `seam_bcd` and `complex_overlap`.

### 2.2 Paths and trace states (`context-trace`, `positioned::{path, trace}`)

- [ ] `positioned/path.rs`: `StartPath { root, steps }` (stored bottom-up,
  raised in O(1); exposed top-down as `OccurrencePath`), `EndPath`,
  `RangePath { root: Token, span: Range<AtomPosition>, start, end }`,
  `QueryPath { pattern: Vec<Token>, position, end }` (Decision 6 b: descends
  into composite query tokens natively with `prefix_child` and
  `advance_end_path`, never by reading content).
- [ ] `positioned/trace/state/parent.rs`: `ParentState { path: StartPath,
  span, root_pos, prev_pos }` with `advance_state` (`start_at(root,
  span.end)` → `RootChildState`, `Err(self)` at the root end) and
  `path_raise(P, o)` (canonical start-path rule, 2.4).
- [ ] `positioned/trace/child/state.rs`: `ChildState { entry_pos, exit_pos,
  start_pos, path: RangePath }`, `RootChildState`, `advance_state` (end path
  via `advance_end_path(root, end, span.end)`), `parent_state()`.
- [ ] `positioned/trace/cache/` (**Decision 4 a**): `TraceCache` with the
  legacy shape (`VertexCache { bottom_up, top_down }`, `PositionCache { top,
  bottom }`), keyed by the promoted `DirectedKey` with **query positions**
  as in legacy. Edges are labelled by the child's offset in its parent, so
  insertion reads offsets off the edge without a row lookup. The root
  offset `o` is stored once in `RangePath::span`; root offset = query
  position + `o` within one `Response`. Keys stay valid across raises and
  are comparable across chained searches. `Traceable` for start and end
  paths.
- [ ] `PathCoverage` classification from `span`: `EntireRoot` iff
  `span == 0..width`, `Prefix` iff `span.start == 0`, `Postfix` iff
  `span.end == width`, else `Range`.
- **Tests:** raise arithmetic and coverage on the worked example and on
  `repeat_ababa` (`aba → ababa`); trace up edges agree with parent links.

### 2.3 Search (`context-search` root modules)

Mirror legacy files one to one; drop visualization events.

- [ ] `policy.rs`, `search/context.rs`: `SearchKind`,
  `DirectedTraversalPolicy::{gen_parent_batch, next_batch}` over
  `parent_occurrences`; `AncestorPolicy` and `AncestorSearchTraversal`
  only. **Decision 7.2:** no `find_parent`, `ParentPolicy` or
  `ParentSearchTraversal` (legacy uses them only in tests).
- [ ] `state/start.rs`, `search/searchable.rs`: `StartCtx`; `Searchable`
  for `Chars` (atoms through `atom_token`, unknown atom → `UnknownAtom`),
  `&[Token]`, `Vec<Token>` (composite tokens allowed, Decision 6 b), and the
  positioned query cursor (continuation).
- [ ] `match/mod.rs`, `match/iterator.rs`: `SearchQueue` (**Decision 5 a:**
  min-heap on width only, as legacy; seeds in a defined order from
  `parent_occurrences`), `SearchNode`, `NodeConsumer`,
  `SearchIterator::pop_and_process_one` (first match clears the queue).
  A `test-api` hook selects an explicit ascending or descending
  `(index, offset)` tie order for the tie-order test (2.6).
- [ ] `compare/parent.rs`, `compare/state/*`, `compare/iterator.rs`:
  `ParentCompareState::advance_state`, `CompareState<Q, I>`,
  `PathPairMode::{GraphMajor, QueryMajor, BothMajor}`,
  `compare_leaf_tokens` with `prefix_child` and one state per
  decomposition (Decision 1 b), `advance_query_cursor`,
  `advance_index_cursor`, `CompareIterator`.
- [ ] `match/root_cursor/*`: `RootCursor::advance_to_next_match` with the
  three ends; `get_parent_batch` raises the matched span.
- [ ] `state/matched.rs`, `state/response.rs`, `search/mod.rs`:
  `MatchResult { path: PathCoverage, cursor: CheckpointedCursor }`,
  `Response { cache, end }` with the accessors consumers use today
  (`root_token`, `query_exhausted`, `is_entire_root`, `query_cursor`,
  `checkpoint_position`, `entire_root_start_path`), `SearchState`,
  `Find::find_ancestor` for `PositionedGraphRef` (one read guard per search).
  Positioned responses carry no visualization events (no `events`,
  `transitions()`).
- [ ] `test-api` counters: equal-width candidates that also match at the
  first comparison (uniqueness), positions compared per search (reuse).

### 2.4 Canonical paths

**Decision 3 (a), canonical paths only.** Responses carry the canonical
paths, as legacy returns them; no full paths down to atoms. Insertion only
needs paths down to where a match boundary falls inside a token.

- [ ] Start path from the **climbed chain** (the tokens raised through
  during the search), not rebuilt top-down: every climbed token below the
  root was matched to its end, so it lies inside the match. On raise of `R`
  via `(P, o)`: if the match starts at `R`'s start, the new start path is
  `[(P, o, R)]`; otherwise prepend `(P, o, R)`. Positioned form of the
  legacy rule that skips an offset-0 step with an empty sub path; "at the
  border" is `offset == 0` of the occurrence.
- [ ] End path: stop at the first step whose child ends at `span.end`
  (positioned `simplify`; border is `offset + width == width(parent)`).
- [ ] Invariant: each path stops at the largest token on it whose boundary
  coincides with the span boundary.

### 2.5 Reuse

- [ ] `Response` is the only hand-off; `Searchable` for the `end` cursor
  continues at its checkpoint without re-reading earlier positions.
- [ ] `PositionedGraph::insert` stays `pub(crate)`/`test-api`; public
  composite insertion goes through a `Response` (step 3).

### 2.6 Validation on v3 fixtures (`context-search/src/tests/`)

Graphs from `context_trace::positioned::fixtures::v3_graphs()`, vertices
addressed only through the name map.

- **Expectation for `q`:** `k` = longest prefix of `q` occurring in a corpus
  text; `root` = minimal-width fixture key containing `q[..k]`, at offset
  `o`. Assert `(root, o)` is unique; a failure is an oracle/model issue.
- **Queries:** every substring of every corpus text; each substring
  extended by every alphabet atom; 300 seeded random strings.
- **Per query:** `end.root == names[root]`, span `[o, o + k)`; coverage as
  in 2.2; end reason (exhausted iff `k == |q|`); both paths pass
  `contains_path`; diagnostic `read_interval(root, o..o + k) == q[..k]`;
  cache entries for every vertex on both paths; up edges agree with parent links.
- **Canonical paths** (Decision 3; replaces the earlier "start path ends at
  the atom `q[0]`" expectation, which contradicted the canonical rule):
  - the start path's last token `S` starts at `o` and ends at or before
    `o + k`; following `prefix_child` from `S` reaches the atom at `o`;
  - the end path's last token `E` ends at `o + k` and starts at or after
    `o`; following last occurrences from `E` reaches the atom at
    `o + k − 1`;
  - trimming is maximal: no intermediate token on the start path starts at
    `o` and ends within the match, and no intermediate token on the end path
    ends at `o + k` and starts within the match.
- **Uniqueness counter** is 0 on every fixture.
- **Tie-order independence** (Decision 5): every query run with ascending
  and with descending `(index, offset)` tie order gives identical
  `Response`s, paths and trace caches included.
- **Composite queries** (Decision 6): query sequences built from fixture
  tokens via the name map (each corpus text split into covering fixture
  tokens, plus mixed atom/token sequences) give the same match root, span,
  coverage and paths as the equivalent atom query.
- **Named cases** (mirroring `legacy/tests/search/ancestor.rs`):
  `repeat_abcabca`, `repeat_ababa`, `seam_bcd` (`bcd`, `abcd`),
  `repeat_ababab`, `repeating_known` (`xyyxy`), `late_retention__2`;
  errors `SingleIndex` and `UnknownAtom`; no parent match → `EntireRoot` of
  `q0`.
- **Continuation:** chained searches from each `end` checkpoint over every
  corpus text equal fresh searches at the same positions.

## Gate

```sh
cargo check -p context-trace -p context-search -p context-insert -p context-read --no-default-features --features test-api
cargo test -p context-trace -p context-search --no-default-features --features test-api
cargo test --release -p context-trace -p context-search --no-default-features --features test-api
cargo test -p context-trace -p context-search --features legacy-tests   # after promotions
```

All 2.6 checks pass on every v3 fixture; no algorithm calls `read_interval`;
no content-keyed structure in release builds.

## Decisions (2026-11-20)

| # | Topic | Decision |
|---|---|---|
| 1 | Equal-width compare | (b) one combined step `(pc(g), pc(q))`, mode `BothMajor`, no fork |
| 2 | End-path update | (b) reuse the old end path with the corrected pop rule, built in this phase; tested equal to `start_at(root, p)` |
| 3 | Canonical paths | (a) climbed chain with border trimming as in legacy; canonical paths only, no full atom paths; 2.6 spec fixed |
| 4 | Trace-cache positions | (a) query positions as keys, edges labelled by child offset, `o` stored once in the span |
| 5 | Queue ties | (a) width only as legacy, plus the tie-order independence test |
| 6 | Composite queries | (b) native support in this phase, with tests |
| 7.1 | Errors | legacy meanings: `SingleIndex`, `UnknownAtom`; no match → `EntireRoot` of `q0` |
| 7.2 | `find_parent` | dropped together with `ParentPolicy` (test-only in legacy) |
| 7.3 | Promotions | only row-independent types; row-specific behaviour written new |

Visualization events are not ported.

## Risks

| Risk | Mitigation |
|---|---|
| Occurrence completeness fails for some fixture | 2.1 brute-force tests fail first, before search exists |
| Drift from the legacy architecture | One-to-one module mirror; only `start_at`, `prefix_child`, offsets and the combined equal-width step differ |
| Promotions break legacy | Re-exports; `legacy-tests` after each promotion |
| Corrected pop rule wrong on overlaps | Equality test with `start_at` on all position pairs, including overlap fixtures (2.1) |
| Canonical trimming wrong with overlaps | Maximal-trimming checks on all fixtures (2.6), incl. `seam_bcd`, `complex_overlap` |
| Step 3 needs data the trace cache lacks | Offset-labelled edges plus `o` in the span; review the cache shape against `context-insert` before 2.3 ends |
