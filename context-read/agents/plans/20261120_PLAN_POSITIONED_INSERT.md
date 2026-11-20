---
tags: `#plan` `#context-insert` `#context-trace` `#context-search` `#positioned-composition` `#insert` `#seam`
summary: Complete work package for positioned insertion (roadmap step 3; all planning for this step lives here). Explains the legacy row-based insertion (InitInterval, split, join, select/publish, overlap closure, reader use), what changes with positioned child lists (cover projection instead of split/join, one host update instead of rows on every path node, merge with seam discovery instead of complement closure), and the ordered implementation steps in context-trace and context-insert, validated on the oracle fixtures.
status: 📋 draft — decisions 1–6 proposed, awaiting confirmation
date: 2026-11-20
parent: 20261120_PLAN_POSITIONED_READER_OPEN_WORK.md (step 3)
depends_on: 20261120_PLAN_POSITIONED_SEARCH.md (done)
---

# Plan: Positioned Insertion

**Crates:** `context-trace` (graph mutation, publication record),
`context-insert` (cover projection, insertion from a `Response`, merge with
seam discovery). `context-search` gets one field (generation stamp).
**Roadmap:** step 3 of
[20261120_PLAN_POSITIONED_READER_OPEN_WORK](20261120_PLAN_POSITIONED_READER_OPEN_WORK.md).
Prerequisite: positioned search (step 2,
[20261120_PLAN_POSITIONED_SEARCH](20261120_PLAN_POSITIONED_SEARCH.md)) is
done. Consumer: the fixed-retention reader (step 4).

**Goal:** two operations on `PositionedGraphRef`, both content-free and
validated on every oracle fixture:

1. **`insert_match(Response)`** — turn the matched span of a search response
   into a token: the existing root for an `EntireRoot` match, otherwise a new
   vertex whose composition is projected from the root, with the root's
   composition updated. Successor of legacy `materialize`.
2. **`Cover::merge(left, right, r0)`** — combine the covers of two adjacent
   or overlapping observations into the cover of the combined string,
   including the occurrences crossing the seam. Successor of legacy
   `Overlap::close`.

## Binding rules

From the roadmap, as they apply here:

- **No content-keyed index, no content in algorithms.** Composite vertices
  are found by search only; `read_interval` and brute-force string
  computations are allowed in tests only.
- **Responses are reused.** Insertion consumes a `Response` and does not
  repeat the traversal that produced it.
- **No complements, wrappers, nested closure or content expansion.** A new
  vertex is created only for the span the caller asks for; its composition
  lists existing tokens only.
- **Oracle expectations.** Expected compositions and graphs are computed from
  fixture strings (through the name map), never from insertion output.
- **Legacy stays fenced.** New code lives at crate roots and passes
  `cargo check -p context-trace -p context-search -p context-insert -p
  context-read --no-default-features --features test-api`; run
  `--features legacy-tests` whenever legacy code or a shared manifest is
  touched.

## Evidence

A Python prototype on the fixture strings (session files,
`insert_prototype.py`) matched brute force on every fixture:

| Check | Cases | Failures |
|---|---|---|
| Cover projection of every proper span of every vertex (3.2) | 1,395 | 0 |
| Remove a single-host token, reinsert it at its host (3.3) | 41 | 0 |
| Merge of every overlapping or adjacent split of every vertex, with seam discovery (3.4) | 1,521 | 0 |

Seam discovery with the monotone stop rule needed 2,385 searches instead of
4,440 for scanning every candidate start.

A second check supports decision 1: on every fixture, every substring of a
vertex that is not itself a token has **exactly one minimal host** (830
substrings, 0 exceptions). A minimal host is a vertex containing the
substring at an offset where no child of that vertex contains it. Composite
tokens split into 41 that occur in exactly one host composition and 63 that
occur in more than one.

## Part A — How the legacy insertion works

Legacy code: [`context-insert/src/legacy`](../../../context-insert/src/legacy).
Note: `HIGH_LEVEL_GUIDE.md` says `InitInterval::end_bound` comes from
`checkpoint_position()`, but the code uses `cursor_position()`
(`interval/init.rs:20`).

### Module map

| Module | Role |
|---|---|
| `insert/mod.rs`, `insert/context.rs` | `ToInsertCtx` (`insert`, `insert_init`, `insert_next_match`, `materialize`, scoped prefix/postfix helpers, deprecated `insert_or_get_complete`); `InsertCtx` does search → materialize |
| `insert/outcome.rs`, `insert/result.rs` | `InsertOutcome::{Created, Complete, NoExpansion}` (each with `IndexWithPath` and the `Response`), `Materialization { outcome, publication }`, `traced_prefix`; `InsertResult` (`Token` or `IndexWithPath` with the query path) |
| `insert/publication.rs` | `Publication`: created vertices, range replacements and added rows; `translate`/`translate_chain` map old child locations through the edits |
| `insert/scoped.rs`, `insert/direction.rs` | scoped complements for overlap closure; `Left`/`Right` composition order |
| `interval/init.rs`, `interval/mod.rs` | `InitInterval { root, cache, end_bound }` from a `Response`; `IntervalGraph` (split states, split cache, root, target range, required ranges, provenance) |
| `split/**` | positions to split per vertex and per row, seeded from the trace cache, propagated down to children |
| `interval/partition/**` | per-row partition info: borders, perfect borders, roles `Pre`/`Post`/`In`, deltas |
| `join/**` | frontier-driven bottom-up join: per-row partitions, range merging, candidate vertices, root pattern; `select` and `publish` |
| `overlap/mod.rs` | `Overlap::close`: closes an accepted overlap by building contiguous alternatives from complements |

### Algorithm

1. **Search first** (`InsertCtx::insert_impl`, `insert/context.rs:229`;
   `materialize`, `:309`). The query is searched. For `EntireRoot`, no
   insertion happens: `Complete` if the query is exhausted, `NoExpansion` if
   more query remains (the caller advances by the token width). Otherwise
   (`Prefix`/`Postfix`/`Range`) the matched range is inserted: `Created`.
2. **Interval** (`interval/init.rs:20`). `InitInterval { root, cache,
   end_bound = cursor_position() }`. `plan_init` rejects a zero bound and a
   root missing from the cache, then builds the `IntervalGraph`.
3. **Split: seed the root** (`split/context.rs:41`). The root's split
   positions are read from the trace cache (bottom-up entries give the match
   start, top-down entries the match end) and stored in a `SplitVertexCache`
   keyed by atom offset. `RootMode` is `Prefix`, `Postfix` (one target
   position) or `Infix` (two).
4. **Split: position → row** (`split/mod.rs:28–211`). For every row
   (`PatternId`) of a vertex, `TraceBack`/`TraceFront` walk the children's
   widths and map an atom offset to `(sub_index, inner_offset)`. No inner
   offset means a perfect split at a child border; an inner offset means the
   position lies inside child `sub_index`.
5. **Split: propagate down** (`split/run.rs`, `trace/states/*`). A FIFO
   `SplitStates` queue pushes every inner position into the child that
   contains it, recording the parent `PosKey` and the `SubLocation` used.
   Perfect positions become leaves. Each vertex's `SplitPositionCache`
   stores, per row, the `sub_index` and `inner_offset`
   (`cache/position.rs:36`).
6. **Split: augment** (`cache/vertex.rs:120–413`). The root keeps its target
   offsets, adds inner and wrapper offsets for its mode, and derives the
   target partition range and the *required* partition ranges (wrappers
   around imperfect borders). Inner nodes add splits between known
   positions. The split phase does not mutate the graph.
7. **Partitions** (`interval/partition/**`). For a range of cached positions,
   every row gets its borders (`Pre`: one right border, `Post`: one left
   border, `In`: two), split types and perfectness; `PatternSubDeltas`
   adjust sub-indices after replacements.
8. **Join: frontier** (`join/context/frontier.rs`). Starting from the split
   leaves, positions are visited in increasing token width. A node is merged
   once its child splits are available; the root is merged last in its
   `RootMode`.
9. **Join: per-row partitions** (`node/context.rs`, `pattern/borders.rs`,
   `merge/partition.rs`). Each node's rows are copied into a local overlay.
   For every row, a partition is built from its borders: prefix from the
   right split's left part, postfix from the left split's right part, infix
   from both. An existing exact token is reused; empty or one-child ranges
   resolve directly.
10. **Join: merge ranges** (`merge/iter.rs`, `merge/range_map.rs`). Partition
    ranges of a node are merged from short to long; inner nodes merge all
    ranges, the root only single partitions and the required ranges. Each
    merged range proposes one decomposition per row. A range matching a row
    exactly proposes a *perfect replacement* of that row slice;
    `range_sub_merges` proposes every two-piece regrouping as another
    decomposition or an `AddPattern` on an existing token. Complements are
    interned at planning time, keyed by decomposition and by `(host, start,
    end)`.
11. **Join: root pattern** (`merge/node.rs:106`). Unless a perfect
    replacement already provides it, the root gets a `RootPattern`: `[target,
    post]`, `[pre, target]` or `[pre, target, post]`, using original children
    at aligned borders and merged edge tokens otherwise.
12. **Select and publish** (`join/select.rs:461`, `join/plan.rs:473`).
    `select` keeps candidates supported by evidence, inlines single-use
    helpers and deduplicates patterns. `publish` is the only mutation:
    candidates become vertices (`insert_pattern`, then
    `add_pattern_with_update` for further rows, `plan.rs:600`), perfect
    replacements apply `replace_in_pattern` (`:519`), and `AddPattern` edits
    add rows to existing vertices (`:531`). The returned `Publication` lets
    callers translate child locations through the edits.
13. **Overlap closure** (`overlap/mod.rs:66–257`, used by the reader). With
    an anchor scope `X`, its last-child path to a shared token `P`, and an
    accepted expansion `E` starting with `P`, `close` resolves the postfix
    complement `T` of `E` and, frame by frame, the prefix complement `L`,
    closing `C = [outer, T] = [L, closed]`. Complements with more than one
    sibling are inserted through the scoped split/join (`scoped.rs:417`);
    unions are found by lookup or inserted with both rows.

### How the legacy reader uses it

- `ExpansionCtx::match_forward` calls `insert_next_match` on the rest of the
  input and consumes the returned token's width (`expansion/mod.rs:102`).
- `expand_root` walks the committed root's postfixes largest-first, searches
  `[postfix, rest]`, rejects matches that do not extend past the postfix and
  `materialize`s the accepted `Response`; anchor frames are translated through
  the `Publication` (`expansion/mod.rs:136`).
- `ClosurePlan::apply` closes the overlap with `Overlap::close` and commits
  `ReplaceRoot`/`ReplaceTail` (`pipeline/root.rs:92`).

### Worked example (`tests/cases/insert/postfix.rs`)

Graph `ab = [a, b]`, `cd = [c, d]`, `abcd = [ab, cd]`, `ababcd = [ab, abcd]`.
Inserting the response for `b c d` (root `ababcd`, trace through `ab` to `b`)
creates `bcd = [b, cd]`, **adds the row `[a, bcd]` to `abcd`** (a node on the
split path, not the search root), and leaves `ababcd = [ab, abcd]`.

### Row-specific parts

Rows are involved everywhere except the outer API: `(pattern_id, sub_index,
inner_offset)` positions, per-row borders, perfectness and deltas, per-row
partitions and overlays, perfect replacements, sub-merge regroupings, the
root row, complements, row-addressed `Publication` translation, and closure
by contiguous alternatives. What remains model-agnostic: search-then-
materialize, the outcome variants, keeping the `Response`, and the idea of a
publication record that translates paths held by callers.

## Part B — What changes with positioned child lists

A vertex stores all of its maximal retained proper occurrences, overlaps
included. A new token therefore needs **one composition** and changes **only
its minimal hosts**. Nothing has to be closed or regrouped, because overlaps
are explicit.

### Facts used

- **F1 Occurrence completeness.** Every retained token inside a vertex `V`
  lies inside one of `V`'s composition occurrences, or is one.
- **F2 Minimal root.** Positioned search returns the narrowest root `R`
  containing the matched prefix, at offset `o` (validated in search 2.6).
- **F3 Unique minimal host.** A sequence that is not a token has exactly one
  minimal host `(R, o)` (checked on all fixtures, see Evidence). With F2,
  that host is the search root. Every other vertex containing the sequence
  contains it through `R`.
- **F4 Monotone reach.** For a string `C`, let `m(x)` be the end of the
  longest prefix of `C[x..]` occurring in the graph. Then `m` is
  non-decreasing in `x`, because `C[x+1..m(x)]` occurs wherever
  `C[x..m(x)]` occurs.

### Cover projection replaces split + join

The composition of a new token `N` for span `[s, e)` of `R` is

```
cover(V, [s, e)) = [V@0]                                    if [s, e) = [0, |V|)
                 = maximal( ⋃ over children c@a of V meeting [s, e):
                            cover(c, [s, e) ∩ [a, a+|c|) − a) shifted to N )
```

`maximal` sorts by start and keeps an occurrence only if it ends after every
earlier one; this removes duplicates and contained occurrences and keeps
partial overlaps. It is the oracle's normalization. Correctness follows from
F1: every token inside the span lies in some child of `V`, so recursion finds
it. Children entirely inside the span return themselves at once; only
children crossing `s` or `e` recurse. This is the positioned form of
"propagate split positions down" (legacy steps 3–6). It needs no rows,
partitions, perfectness, wrappers or complements, because the projected
pieces are existing tokens.

**Reuse.** The response already holds one descent per border: the start path
ends at the largest token at `o` (the first occurrence of `N`), and the end
path at the largest token ending at `o + k` (the last occurrence of `N`).
Projection takes those from the paths. It descends only into the *other*
children crossing a border, which the trace does not contain (caveat 3 in the
search plan's cache note), using `covering`/`occurrences`.

### One host update replaces rows on path nodes

Legacy adds a row to every node on the split path (`abcd` in the example).
Positioned: `N` is maximal exactly in its minimal hosts, and by F3 that is
the search root `R` alone. So:

- `R`'s new composition is `maximal(R.composition ∪ {N@o})`, minus the
  children inside `[o, o+|N|)`. `N@o` is never contained in a child of `R`
  (that child would be a narrower root, against F2).
- Removed children lose the parent link `(R, a)` and are children of `N` at
  `a − o`: they were maximal in `R`, so they are maximal in `N`.
- Ancestors of `R` and all other vertices are unchanged.

In the example, positioned search for `bcd` returns `abcd`, the narrowest
root. Insertion creates `bcd = [b@0, cd@1]` and updates
`abcd = [ab@0, bcd@1]`; the old `cd@2` lies inside `bcd` and is dropped.

**Tokens with several hosts** are out of scope. A token occurring in more
than one host composition (63 fixture tokens) is frequent; promoting it
retrospectively is roadmap step 5 ("reconcile hosts on promotion"). Within
one graph that keeps F3, `insert_match` never meets such a token. The step 3
gate is restricted accordingly (decision 1).

### Merge with seam discovery replaces overlap closure

The combined string is `C`: the left side has width `wl`, and the right side
starts at `r0 ≤ wl` (overlap `wl − r0`; `r0 = wl` means adjacent). The cover
of `C` is the set of maximal occurrences among:

1. **Align/project:** the left cover, and the right cover shifted by `r0`. A
   side that is a token contributes itself (`[side@0]`); otherwise it
   contributes its cover (from projection).
2. **Seam:** for each start `x` from `r0 − 1` down to `0`, search the token
   sequence of `C` from `x`. The query is built by navigating the covers
   with `start_at`, never by reading content. With `m(x) = x + k`, stop at
   the first `x` with `m(x) ≤ wl`; by F4 no smaller `x` can reach past the
   left side. Otherwise `S(x)` is the largest token at `x` inside the match:
   the root for `EntireRoot`, else the start path's start token (validated
   in search 2.6). Keep `S(x)` if it crosses the seam (`x + |S(x)| > wl`).
3. **Prune:** `maximal` over everything.

This is complete: every token in `C` lies inside the left side, inside the
right side, or crosses the seam. Inside a side, F1 places it inside a side
occurrence. A crossing token starting at `x` is no larger than `S(x)`, which
then also crosses.

**Outcomes.** If the search from `x = 0` matches all of `C`, the combined
string already occurs:

- `EntireRoot` → `Existing(token)`;
- otherwise `Occurs { response, cover }`, and the caller inserts it with
  `insert_match`, which updates the unique host.

Only if `C` does not occur is the result `New(Cover)`, published without
hosts.

The legacy reader walked only postfixes that are tokens. That misses seam
tokens whose left part is not retained, which retention allows (`abcab`
retains `ab`, `abcabc` does not).

### Mapping

| Legacy | Positioned |
|---|---|
| `materialize(Response)` → `Complete` / `NoExpansion` / `Created` | `insert_match(Response)`, same three outcomes |
| `InitInterval`, `IntervalGraph`, split caches, partitions, `RootMode` | `Cover::of_match(Response)`: projection seeded by the start and end paths |
| join: per-row partitions, range merge, sub-merges, complements, `select` | none; the projected occurrences are existing tokens |
| root row `[pre, target, post]`; rows added on path nodes | one host update: `N@o` into `R`, contained children moved under `N` |
| `publish`: `insert_pattern`, `replace_in_pattern`, `add_pattern_with_update` | `PositionedGraph::publish(cover, host)`: one new vertex, at most one host edit |
| `Publication::translate` over row locations | `Publication::translate`: step `(R, a, c)` with `c` inside `N` → `(R, o, N), (N, a − o, c)` |
| `Overlap::close`, `scope_complement`, unions with two rows | `Cover::merge` with seam discovery |
| postfix walk `[postfix, rest]` in `expand_root` | seam scan from `r0 − 1` down with the F4 stop (step 4 uses the merge) |

## Part C — Implementation steps

Tasks are numbered 3.x after the roadmap step and run in order. New code
lives at crate roots. Module names follow the operations; no legacy modules
are mirrored, because none of the split/join structure survives.

### 3.0 Crate setup

- [ ] `context-insert/Cargo.toml`: depend on `context-trace` and
  `context-search` **without** `test-api`. Add `test-api =
  ["context-trace/test-api", "context-search/test-api"]` and enable it in
  `[dev-dependencies]`. Today the dependency always enables `test-api`,
  which exposes the raw `PositionedGraph::insert` to all of context-insert.
  Check that `--features legacy` still builds and that the legacy tests do
  not need `test-api` (forward it from `legacy-tests` if they do).
- [ ] Root modules `cover/`, `insert/`, `merge/` and an `InsertError` enum
  (`Search(SearchError)`, `StaleResponse`, `Composition(PositionedError)`).
- [ ] Run `cargo test -p context-insert --features legacy-tests`.

### 3.1 Graph mutation and publication record (`context-trace`, `positioned/publish.rs`)

- [ ] `pub struct HostEdit { host: VertexIndex, offset: AtomPosition,
  removed: Vec<Occurrence> }` and `pub struct Publication { token: Token,
  host: Option<HostEdit> }`.
- [ ] `PositionedGraph::publish(&mut self, cover: Vec<Occurrence>, host:
  Option<(VertexIndex, AtomPosition)>) -> Result<Publication,
  PositionedError>`. It inserts the vertex through `insert_composition`. With
  a host, it checks that `N@o` is not contained in a host child, builds the
  host's new `Composition` (old occurrences not inside `[o, o+|N|)` plus
  `N@o`), updates the parent links (dropped children lose `(host, a)`; `N`
  gets `(host, o)`), and in debug builds recomputes the host's label. It
  checks structure only; content agreement is checked by `validate()` in
  tests.
- [ ] `PositionedGraph::generation()`: a counter bumped by every mutation.
- [ ] `Publication::translate(&OccurrencePath) -> OccurrencePath` and the
  same for `StartPath`/`EndPath`/`RangePath`: a step `(host, a, c)` with `c`
  in `removed` becomes `(host, o, N), (N, a − o, c)`; other steps stay.
- [ ] Test support (`fixtures`, `test-api`): `FixtureGraph::without(key) ->
  FixtureGraph`, the fixture with `key` removed. Every composition is
  recomputed from the fixture strings as the maximal occurrences of the
  remaining keys (string computation, tests only), then loaded through the
  interchange as usual.
- **Tests:**
  - for each of the 41 single-host tokens `T`, `without(T)` then `publish`
    of `T`'s fixture composition with its fixture host and offset gives a
    graph equal (`diff`) to the fixture;
  - `validate()` passes after every publish;
  - `translate` maps every path in the reduced graph that runs through a
    removed child to a path with `contains_path` and the same absolute span.

### 3.2 Cover projection (`context-insert`, `cover/`)

- [ ] `pub struct Cover { width: TokenWidth, occurrences: Vec<Occurrence> }`,
  always normalized (`maximal`), with an unchecked builder for tests.
- [ ] `Cover::of_span(graph, root, span)`: the recursion from Part B,
  iterative or memoized per `(vertex, sub-span)`.
- [ ] `Cover::of_match(graph, &Response)`:
  - `EntireRoot` → `[root@0]`;
  - otherwise the first occurrence comes from the start path's start token,
    the last from the end path's leaf, and the rest from projecting the
    children crossing a border that are not on the traced paths.
- [ ] `Cover::start_at(graph, p)`: the largest token at `p` inside the cover
  (last occurrence with `offset ≤ p`, then `PositionedGraph::start_at`).
  `Cover::tokens_from(graph, x)`: the greedy gap-free token sequence of the
  cover from `x`, used as a search query.
- [ ] `test-api` counter: steps of the traced paths descended again (must be
  0).
- **Tests:**
  - `of_span` on every proper span (at least 2 atoms) of every vertex of
    every fixture equals the brute-force maximal occurrences of the
    substring among the fixture keys; a span that is itself a key gives
    `[key@0]`;
  - `of_match` on every response of the 2.6 atom queries equals
    `of_span(root, span)`, with the counter at 0;
  - `tokens_from` covers exactly `[x, width)` without gaps, and every token
    starts where `Cover::start_at` says.

### 3.3 Insertion from a response (`context-insert`, `insert/`)

- [ ] `context-search`: `Response` records the graph generation under its
  read guard (`Response::generation()`).
- [ ] `pub enum InsertOutcome { Complete { token, response }, NoExpansion
  { token, response }, Created { token, response, publication } }` with the
  legacy accessors (`token`, `response`, `is_created`, …).
- [ ] `insert_match(&PositionedGraphRef, Response) -> Result<InsertOutcome,
  InsertError>`. Under one write guard: generation check (`StaleResponse`);
  `EntireRoot` → `Complete`/`NoExpansion`; otherwise
  `publish(Cover::of_match(..), Some((root, span.start)))` → `Created`.
- [ ] `insert(&PositionedGraphRef, impl Searchable) -> Result<Token,
  InsertError>` and `insert_next_match(..) -> Result<InsertOutcome, ..>`:
  search, then `insert_match`. As in legacy, only the matched part is
  inserted; the caller continues from the response's checkpoint.
- **Tests:**
  - **Reinsert gate:** for each of the 41 single-host tokens, search `T`'s
    string on `without(T)` (atom query): the root equals the fixture host and
    the span equals the fixture offset (expectations from strings);
    `insert_match` then gives a graph equal to the fixture, and `validate()`
    passes.
  - **Scope boundary:** for each of the 63 multi-host tokens, assert by
    brute force that `T`'s string has two or more minimal hosts in
    `without(T)`. This documents the step 5 case and is not inserted.
  - `Complete`/`NoExpansion` for queries that are a token or start with one,
    with the graph unchanged.
  - `StaleResponse` after an unrelated `publish`.
  - The response's paths, translated through the publication, are valid in
    the new graph and cover the same span.
  - Named single-host cases from oracle fixtures (no hand-built graphs):
    `direct_tail` (`abcd` in `xabcd`), `multiple_overlaps` (`cdeab` in
    `abcdeabcde`), `flat_complement` (`vis` in `xvisu`).

### 3.4 Merge with seam discovery (`context-insert`, `merge/`)

- [ ] `pub enum MergeOutcome { Existing(Token), Occurs { response, cover },
  New(Cover) }` (the computed cover is returned whenever `C` is not a
  token) and `Cover::merge(graph, left: &Cover, right: &Cover, r0)`:
  1. align (`r0 ≤ left.width`; in debug/test builds, check by traversal that
     the overlap agrees);
  2. project (left ∪ right shifted by `r0`);
  3. seam scan from `r0 − 1` down with the F4 stop, query
     `tokens_from(x)` over the combined covers, `S(x)` from the response, the
     `x = 0` full-match outcomes;
  4. prune with `maximal`.
- [ ] `insert_cover(&PositionedGraphRef, Cover) -> Result<Publication, ..>`:
  `publish(cover, None)`, for a `New` result only.
- [ ] `test-api` counters: seam searches per merge, and no start position
  searched twice.
- **Tests (merge gate):**
  - For every composite vertex `V` of every fixture and every `0 < r0 ≤ wl <
    |V|` (1,521 cases):
    - build the sides on `without(V)` from search responses of the side
      strings (`Cover::of_match`), cross-checked with brute force;
    - the merged cover equals `V`'s fixture composition;
    - the outcome is `New` for parentless `V` (then `insert_cover` gives a
      graph equal to the fixture) and `Occurs` otherwise;
    - for single-host `V`, `insert_match` of the `Occurs` response
      reproduces the fixture.
  - On the full fixture graph the same merges return `Existing(V)`.
  - Named case `seam_bcd`: `abc` + `cde` at `r0 = 2` gives `[abc@0, bcd@1,
    cde@2]`, with `bcd` found by the seam scan.
  - Seam search counts stay within the stop rule (no search beyond the first
    failing `x`).

### 3.5 Documentation and gate

- [ ] `context-insert/README.md`: positioned API section (`insert_match`,
  `Cover`, `merge`, outcomes, publication). Mark `HIGH_LEVEL_GUIDE.md` as
  legacy-only and fix its `end_bound` statement.
- [ ] Roadmap step 3 ticked, INDEX status updated, deviations recorded here.

## Gate

```sh
cargo check -p context-trace -p context-search -p context-insert -p context-read --no-default-features --features test-api
cargo test -p context-trace -p context-search -p context-insert --no-default-features --features test-api
cargo test --release -p context-trace -p context-search -p context-insert --no-default-features --features test-api
cargo test -p context-trace -p context-search -p context-insert --features legacy-tests
cargo build --workspace --all-targets --exclude context-editor-kernel --exclude sandbox-app
```

All 3.1–3.4 tests pass on every fixture. No algorithm calls `read_interval`
or builds content-keyed structures. context-insert without `test-api`
cannot reach `PositionedGraph::insert`.

## Decisions (proposed 2026-11-20, to be confirmed)

| # | Topic | Proposal |
|---|---|---|
| 1 | Host scope | Single host by F3 (the search root). Multi-host promotion belongs to step 5. The roadmap's "reinsert removed token" gate covers the 41 single-host tokens; the 63 multi-host ones are asserted to violate F3 after removal. |
| 2 | Mutation primitive | `PositionedGraph::publish(cover, host)` is public in context-trace and checks structure only. The public entry points take a `Response` or a merged `Cover` (context-insert). Raw `insert` stays `test-api`, and context-insert stops enabling `test-api` unconditionally. |
| 3 | Seam discovery | Right-to-left scan with the F4 stop (complete, backed by the prototype) instead of the legacy postfix walk (incomplete) or trace-only candidates (not complete with overlaps). |
| 4 | Stale responses | A generation stamp in `Response`, checked by `insert_match` (`StaleResponse`). |
| 5 | API shape | `insert_match` / `insert` / `insert_next_match` with legacy outcome semantics, plus `Cover::merge`. No `InitInterval`/`IntervalGraph`. The caller decides what to insert; no retention logic here. |
| 6 | Merge when `C` already occurs | `Existing` / `Occurs { response, cover }` instead of a new vertex; the caller uses `insert_match` for `Occurs`, so the unique host is updated. |

## Risks

| Risk | Mitigation |
|---|---|
| F3 fails for some oracle output or after a step-4 insertion | Fixture-wide check (Evidence); 3.3 asserts root = brute-force host; `publish` rejects an `N@o` contained in a host child |
| Projection misses children crossing a border that are off the traced paths | `of_span` exhaustive brute-force test; `of_match` equals `of_span` on all 2.6 responses |
| Seam scan cost on long inputs | F4 stop; counters; measure on reader inputs in step 4 before optimizing (e.g. reusing the right side's response for `x` inside its climb) |
| Paths held by the reader go stale after a host edit | `Publication::translate` with validity tests; generation check |
| Cargo feature change breaks legacy builds | 3.0 runs the legacy suite before anything else |
| Step 4 needs more than one host per insertion | Out of scope by decision 1; step 5 owns retrospective hosts |
