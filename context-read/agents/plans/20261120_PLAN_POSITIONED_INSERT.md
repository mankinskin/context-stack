---
tags: `#plan` `#context-insert` `#context-trace` `#context-search` `#positioned-composition` `#insert` `#split` `#join` `#seam`
summary: Complete work package for positioned insertion (roadmap step 3; all planning for this step lives here). Explains the legacy row-based insertion (InitInterval, split cache, join frontier, select/publish, overlap closure, reader use) and its positioned successor - a split graph of required intervals seeded from the search response's root span, start path and end path, joined bottom-up by a width frontier into intersection tokens and the new token - plus merge with seam discovery, ordered tasks in context-trace and context-insert, and a gate against oracle graphs of the extended corpus.
status: 🗄️ superseded by 20261122_PLAN_POSITIONED_INSERT.md (revision 3); Part A (legacy walkthrough) remains the reference
date: 2026-11-20 (revision 2: 2026-11-21)
parent: 20261120_PLAN_POSITIONED_READER_OPEN_WORK.md (step 3)
depends_on: 20261120_PLAN_POSITIONED_SEARCH.md (done)
---

# Plan: Positioned Insertion

> **Superseded (2026-11-22)** by revision 3,
> [20261122_PLAN_POSITIONED_INSERT](20261122_PLAN_POSITIONED_INSERT.md):
> split positions with roles instead of interval sets, explicit invariants,
> `GraphEdit` defined. Seam scan and cover merge (below) are dropped: the
> reader keeps the legacy root expansion,
> [20261122_PLAN_POSITIONED_READER](20261122_PLAN_POSITIONED_READER.md).
> Part A below (legacy insertion) is still accurate and is referenced from
> revision 3.

**Crates:** `context-trace` (graph edits, publication record, overlap-closure
validation, insert fixtures), `context-insert` (split graph, join, insertion
from a `Response`, cover projection and merge). `context-search` gets one
field (generation stamp).
**Roadmap:** step 3 of
[20261120_PLAN_POSITIONED_READER_OPEN_WORK](20261120_PLAN_POSITIONED_READER_OPEN_WORK.md).
Prerequisite: positioned search (step 2,
[20261120_PLAN_POSITIONED_SEARCH](20261120_PLAN_POSITIONED_SEARCH.md)) is
done. Consumer: the fixed-retention reader (step 4).

**Goal:** two operations on `PositionedGraphRef`, both content-free and
validated on every oracle fixture:

1. **`insert_match(Response)`** — turn the matched span of a search response
   into a token. For an `EntireRoot` match that is the existing root.
   Otherwise the split graph of the response is joined bottom-up: every node
   that a border of the span cuts through gains its intersection with the
   span as a token, and the root gains the new token. Successor of legacy
   `materialize` (split + join).
2. **`Cover::merge(left, right, r0)`** — combine the covers of two adjacent
   or overlapping observations into the cover of the combined string,
   including the occurrences crossing the seam. Successor of legacy
   `Overlap::close` in the reader (consumer: step 4).

**Revision 2 (2026-11-21).** Revision 1 claimed that inserting a token
changes only its host (the search root). That is wrong: the new token
partially overlaps every child of the root that crosses a border of the span,
and by overlap closure (I2) each such overlap must be a token, recursively
down to the path leaves. Revision 1 also validated with a synthetic
"remove a token, reinsert it" gate on graphs the oracle cannot produce, which
is where its "multi-host" scope came from. Both are replaced: insertion is a
split/join over the response (Part B), validated against the oracle graph of
the extended corpus (3.4).

## Binding rules

From the roadmap, as they apply here:

- **Insertion builds strictly on search.** The only input of `insert_match`
  is a `Response`. Its root is the narrowest host of the span (no child of the
  root contains it), so insertion never searches, never descends to find a
  host and accepts no other input form.
- **No content-keyed index, no content in algorithms.** Composite vertices
  are found by search or by positional navigation (`covering`, `ending_at`,
  `start_at`, `end_at`) only; `read_interval` and brute-force string
  computations are allowed in tests only.
- **Responses are reused.** The split graph is seeded from the response's
  root, span, start path and end path; navigation is needed only for
  intervals the paths do not reach (counted in tests).
- **No complements, wrappers or content expansion.** New vertices are the
  matched span and the intersections of the span with the vertices its
  borders cut through (the intervals of the split graph). Each composition
  lists existing or just-created tokens only. No token is created twice.
- **Oracle expectations.** Expected graphs come from the `ngrams` oracle run
  on the extended corpus, stored as fixtures; never from insertion output.
  Oracle corpora list only the minimal set of larger strings; the oracle
  derives the repeated children.
- **Legacy stays fenced.** New code lives at crate roots and passes
  `cargo check -p context-trace -p context-search -p context-insert -p
  context-read --no-default-features --features test-api`; run
  `--features legacy-tests` whenever legacy code or a shared manifest is
  touched.

## Evidence

Python prototypes on the fixture graphs in
[`context-insert/agents/prototypes`](../../../context-insert/agents/prototypes/README.md)
(execution manual and expected outputs there; content used only to name
vertices and to compare). Insert cases: for every fixture (66) and every
substring `N` of a vertex string that is not itself a vertex, the response
is the unique minimal host occurrence, and the expected graph is
`ngrams(corpus + [N])`.

| Check | Cases | Result |
|---|---|---|
| Every non-token substring has exactly one minimal host occurrence (F3) | 1,247 | 0 exceptions |
| Overlap of every two overlapping occurrences in a composition is a vertex (I2) | 123 pairs | 0 exceptions |
| Split graph + bottom-up join (`split_join.py`) equals the oracle graph of `corpus + [N]` | 1,247 | 0 failures, 2,119 vertices created, 0 duplicates |
| Response start/end paths are exactly the primary split chains | 1,247 | all |
| Split nodes off the response paths (reached via an intersection interval) | 1,247 | 2 cases |
| Intervals per split node | 2,910 nodes | at most 3 |
| Changed vertices are the root or split nodes; new vertices are intervals | 1,247 | all |
| Crossing children skipped by the join lie inside another piece | 344 | all |
| A vertex lies on both response paths | 1,247 | 225 cases |
| Cover projection of every proper span of every vertex (3.5, merge) | 2,646 | 0 failures |
| Merge with seam scan equals the vertex's composition (3.5) | 2,850 | 0 failures; 5,288 searches vs 9,486 for scanning every start |

Scripts: F3 and I2 rows from `invariants.py`, insertion rows from
`split_join.py` (vertex on both paths: `two_chains.py`), projection and merge
rows from `cover_merge.py`.

Rejected alternatives. `two_chains.py` closes the start chain and then the
end chain independently. It also reproduces the oracle, but a vertex on both
paths (225 cases) is then composed twice and its larger tokens are built
before the smaller intersection inside them exists; the end path is stale
after the start chain in 79 cases and has to be re-navigated or translated.
`naive_chains.py` walks both response paths as they were before the
insertion and fails on 60 periodic cases. The split graph avoids both
(decision D4).

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

1. **Search first** (`InsertCtx::insert_impl`, `insert/context.rs:230`;
   `materialize`, `:294`). The query is searched. For `EntireRoot`, no
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
11. **Join: root pattern** (`RootPattern` in `join/plan.rs` and
    `join/context/node/merge/iter.rs`). Unless a perfect
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
13. **Overlap closure** (`Overlap::close`, `overlap/mod.rs:105`, used by the
    reader). With
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
materialize, the outcome variants, keeping the `Response`, the split cache
keyed by vertex, the bottom-up join frontier, and a publication record that
translates paths held by callers.

## Part B — Positioned insertion: split graph and bottom-up join

A vertex stores all of its maximal retained proper occurrences, overlaps
included. Insertion keeps the legacy architecture — split positions pushed
down from the root, then a bottom-up join — but the split cache holds
**intervals per vertex** instead of positions per row, and the join creates
**one occurrence per interval** instead of rows, partitions and complements.

### Facts and invariants

- **F1 Occurrence completeness.** Every retained token inside a vertex `V`
  lies inside one of `V`'s composition occurrences, or is one.
- **F2 Narrowest root.** Positioned search returns the narrowest root `R`
  containing the matched span `[o, e)` (validated in search 2.6). No child of
  `R` contains the span.
- **F3 Unique minimal host.** In an oracle-consistent graph, a sequence that
  is not a token has exactly one minimal host occurrence: a substring with
  two would occur more often than its smallest retained superstrings and be
  retained. With F2, that host is the response's `(R, o)`.
- **I2 Overlap closure.** If two occurrences in a composition overlap, their
  overlap is a token. Checked by `validate()` from 3.1 on.
- **F4 Monotone reach** (merge only). For a string `C`, let `m(x)` be the end
  of the longest prefix of `C[x..]` occurring in the graph; `m` is
  non-decreasing in `x`.

### Why more than the root changes

`N = R[o, e)` is added to `R` at `o`. A child `c@a` of `R` that crosses `o`
(starts before `o`, ends inside the span) partially overlaps `N@o`; by I2 the
overlap `R[o, a + |c|)`, a postfix of `c`, must be a token, and it must occur
maximally in `c`. Inside `c` the same argument applies to `c`'s child crossing
that position, down to the token that starts exactly at `o` — the leaf of the
response's start path. The end border works the same way with prefixes along
the end path. When one vertex is reached at both borders (225 of 1,247 cases),
its new postfix and prefix overlap, and their overlap is a token too.

### Split graph (successor of `InitInterval`, `IntervalGraph` and the split cache)

`SplitGraph`: the root plus, per vertex, the set of required intervals
(ranges relative to that vertex). One node per vertex, however often and on
whichever path it is reached — like legacy `SplitVertexCache`.

1. **Seed from the response.**
   - The root gets the span `[o, e)`.
   - Every step of the start path except the last, `(parent, a, c)` with `c`
     at absolute offset `s`, gives `c` the postfix interval `[o − s, |c|)`.
   - Every step of the end path except the last gives its child the prefix
     interval `[0, e − s)`.
   - The path leaves need no interval: the border lies on their border.
2. **Top-down frontier** (decreasing width; a node is processed once every
   wider node is done, so its interval set is final):
   1. *Close* the node's intervals: add the intersection of every two
      partially overlapping intervals (I2 inside the node).
   2. *Push* each interval `[x, y)` into the children:
      - if a child `c@a` contains it, push `[x − a, y − a)` into `c`, unless
        that is all of `c`;
      - otherwise push the postfix `[x − a, |c|)` into `covering(x)` if it
        starts before `x`, and the prefix `[0, y − b)` into `ending_at(y)` if
        it ends after `y`.

   For the root's span these pushes are exactly the steps of the response's
   start and end paths (all 1,247 cases), so they are taken from the paths.
   Only intersection intervals of vertices on both paths can need navigation
   (`covering`, `ending_at`): off-path nodes occurred in 2 cases.
3. The split phase is read-only. Widths strictly decrease along pushes, so it
   terminates; no node had more than 3 intervals.

### Join (successor of the join frontier, partitions, merge and publish)

Bottom-up frontier: nodes in increasing width (children before parents);
inside a node, intervals in increasing width (an intersection before the
postfix and prefix containing it; the root span last). Interval `[x, y)` of
`v` resolves to a token:

1. **Inside a child.** If an original child `c@a` contains it: `c` itself
   if equal, else the token already resolved for `(c, [x − a, y − a))`.
2. **New token.** Otherwise collect pieces:
   - each original child `c@a` meeting `[x, y)`: if inside, `c` at `a − x`;
     if crossing, the token resolved for `c`'s intersection with the
     interval;
   - `v`'s own already resolved smaller intervals inside `[x, y)`.

   The composition is `maximal(pieces)` (sorted by start, keep an occurrence
   only if it ends after all earlier ones). A crossing child whose
   intersection is not an interval of that child lies inside another piece
   and is skipped (344 cases, asserted). If this insertion already created a
   token with the same composition, that token is reused (same composition ⇒
   same content); otherwise a vertex is created.
3. **Update the node.** After its intervals, `v`'s composition becomes
   `maximal(original ∪ own interval tokens)`: each new token is added, the
   children it contains move under it, and the edit is recorded in the
   `Publication`.

The root's span resolves to `N`. No recursive cover projection is needed:
by frontier order, every crossing child's intersection token already exists,
and `maximal` removes contained pieces when occurrences overlap.

### Worked example: vertex on both paths

Corpus `acabcabcab, bcabcb` (oracle graph, children derived by the oracle):

```
bc = b@0, c@1            cab = c@0, a@1, b@2       bcab = bc@0, cab@1
cabc = cab@0, bc@2       bcabc = bcab@0, cabc@1    cabcab = cabc@0, bcab@2
acabcabcab = a@0, cabcab@1, bcabc@3, cabcab@4
```

Query `abcabca`: not a token, and no proper prefix or postfix of it (2 atoms
or more) is a token. Response: root `acabcabcab`, span `[2, 9)`, start path
`root → cabcab@1 → cabc@0 → cab@0 → a@1`, end path
`root → cabcab@4 → bcab@2 → cab@1 → a@1`.

Split graph (`cabcab` and `cab` are reached from both paths):

| Node | Intervals | From |
|---|---|---|
| `acabcabcab` | `[2,9)` | span |
| `cabcab` | `[1,6)`, `[0,5)`, `[1,5)` | start path, end path, intersection |
| `cabc` | `[1,4)` | start path |
| `bcab` | `[0,3)` | end path |
| `cab` | `[1,3)`, `[0,2)`, `[1,2)` | start path, end path, intersection (atom `a`) |

Join (increasing width):

| Node | Interval → token | Node after |
|---|---|---|
| `cab` | `[1,2)` → `a`; `[0,2)` → `ca = c@0, a@1`; `[1,3)` → `ab = a@0, b@1` | `ca@0, ab@1` |
| `cabc` | `[1,4)` → `abc = ab@0, bc@1` | `cab@0, abc@1` |
| `bcab` | `[0,3)` → `bca = bc@0, ca@1` | `bca@0, cab@1` |
| `cabcab` | `[1,5)` → `abca = abc@0, bca@1`; `[0,5)` → `cabca = cabc@0, abca@1`; `[1,6)` → `abcab = abca@0, bcab@1` | `cabca@0, abcab@1` |
| root | `[2,9)` → `abcabca = abcab@0, bcabc@1, cabca@2` | `a@0, cabcab@1, abcabca@2, cabcab@4` |

The intersection `abca` is built before the postfix and prefix that contain
it, so `cabca` and `abcab` are composed once, from consolidated pieces. The
result equals the oracle graph of the corpus extended by `abcabca`.

Smaller case: `ababab` (`abab = ab@0, ab@2`, root `abab@0, abab@2`) with
query `baba` at `[1,5)`. `abab` gets `[1,4)`, `[0,3)` and their intersection
`[1,3)`; the join creates `ba = b@0, a@1`, `aba = ab@0, ba@1`,
`bab = ba@0, ab@1`, `abab → aba@0, bab@1`, `baba = bab@0, aba@1`.

### Publication and path translation

The `Publication` lists the created tokens and, per edited vertex, the added
and removed occurrences. A held path step `(v, a, c)` whose occurrence was
removed becomes `(v, x, T), (T, a − x, c)`, where `T@x` is the added
occurrence of `v` containing `c@a`; if `c` is not a direct child of `T`, the
translation repeats inside `T`. This serves callers holding paths (the
reader); insertion itself never translates.

### Merge with seam discovery (successor of overlap closure in the reader)

The combined string is `C`: the left side has width `wl`, and the right side
starts at `r0 ≤ wl` (overlap `wl − r0`; `r0 = wl` means adjacent). The cover
of `C` is the set of maximal occurrences among:

1. **Align/project:** the left cover, and the right cover shifted by `r0`. A
   side that is a token contributes `[side@0]`; otherwise its cover from the
   read-only cover projection
   `cover(V, [s, e)) = maximal(⋃ children c@a meeting [s, e): cover(c, [s, e) ∩ [a, a+|c|) − a) shifted)`,
   with `cover(V, [0, |V|)) = [V@0]` (F1 makes it complete).
2. **Seam:** for each start `x` from `r0 − 1` down to `0`, search the token
   sequence of `C` from `x` (built with `Cover::tokens_from`, never from
   content). With `m(x) = x + k`, stop at the first `x` with `m(x) ≤ wl`
   (F4). Otherwise `S(x)` is the largest token at `x` inside the match (the
   root for `EntireRoot`, else the start path's leaf); keep it if it crosses
   the seam (`x + |S(x)| > wl`).
3. **Prune:** `maximal` over everything.

Outcomes: `EntireRoot` from `x = 0` → `Existing(token)`; another full match
from `x = 0` → `Occurs { response, cover }` (the caller inserts it with
`insert_match`); otherwise `New(Cover)`, published without hosts. Seam
matches that are not tokens are returned with their responses; whether to
insert them is the caller's retention decision (P6).

### Mapping

| Legacy | Positioned |
|---|---|
| `materialize(Response)` → `Complete` / `NoExpansion` / `Created` | `insert_match(Response)`, same three outcomes |
| `InitInterval { root, cache, end_bound }` | `SplitGraph::from_response`: root, span, start path, end path |
| `SplitVertexCache` / `SplitPositionCache`: positions per vertex and row | `SplitNode`: intervals per vertex (postfix, prefix, intersection, root span) |
| split propagation queue (`TraceBack`/`TraceFront`, `SplitStates`) | top-down width frontier; pushes along the response paths, `covering`/`ending_at` elsewhere |
| augmentation: inner/wrapper offsets, required ranges | interval closure: intersections of overlapping intervals (I2) |
| join frontier (increasing width) | join frontier (increasing width; inside a node, increasing interval width) |
| per-row partitions, range merge, sub-merges, complements, `select` | interval resolution: inside a child → child's token; else `maximal` of child pieces and smaller own intervals |
| root pattern `[pre, target, post]`; rows added on path nodes | one occurrence per interval added to its node; contained children move under it |
| `publish`: `insert_pattern`, `replace_in_pattern`, `add_pattern_with_update` | `GraphEdit`: `create`, `recompose`, `finish` → `Publication` |
| `Publication::translate` over row locations | `Publication::translate` over occurrence steps |
| `Overlap::close`, `scope_complement`, unions with two rows | `Cover::merge` with seam discovery (reader) |
| postfix walk `[postfix, rest]` in `expand_root` | seam scan from `r0 − 1` down with the F4 stop |

## Part C — Implementation steps

Tasks are numbered 3.x after the roadmap step and run in order. New code
lives at crate roots; module names follow the legacy split/join architecture
(`split/`, `join/`, `insert/`), plus `cover/` and `merge/` for the reader.

### 3.0 Crate setup

- [ ] `context-insert/Cargo.toml`: depend on `context-trace` and
  `context-search` **without** `test-api`. Add `test-api =
  ["context-trace/test-api", "context-search/test-api"]` and enable it in
  `[dev-dependencies]`. Today the dependency always enables `test-api`,
  which exposes the raw `PositionedGraph::insert` to all of context-insert.
  Check that `--features legacy` still builds and that the legacy tests do
  not need `test-api` (forward it from `legacy-tests` if they do).
- [ ] Root modules `split/`, `join/`, `insert/`, `cover/`, `merge/` and an
  `InsertError` enum (`Search(SearchError)`, `StaleResponse`,
  `Composition(PositionedError)`).
- [ ] Run `cargo test -p context-insert --features legacy-tests`.

### 3.1 Graph edits, publication, validation, insert fixtures (`context-trace`)

- [ ] `PositionedGraph::generation()`: a counter bumped once per finished
  edit (atom insertion changes no composition and does not bump it).
- [ ] `positioned/edit.rs`: `PositionedGraph::edit(&mut self) ->
  GraphEdit<'_>` with
  - `create(Vec<Occurrence>) -> Result<Token, PositionedError>` (through
    `insert_composition`);
  - `recompose(VertexIndex, Vec<Occurrence>) -> Result<(), PositionedError>`:
    a complete composition of the same width; removed children lose their
    `(vertex, offset)` link, added ones gain it; the debug label is unchanged
    (same content);
  - `finish() -> Publication`: bumps the generation; in debug/`test-api`
    builds validates the touched vertices (structure and I2).
- [ ] `Publication { created: Vec<Token>, edits: Vec<VertexEdit> }`,
  `VertexEdit { vertex, added: Vec<Occurrence>, removed: Vec<Occurrence> }`;
  `translate` for `OccurrencePath`, `StartPath`, `EndPath`, `RangePath`
  (rule in Part B).
- [ ] `validate()`: overlap closure I2 — for adjacent overlapping occurrences
  `L@l`, `R@r`, the largest token at `r − l` in `L` ends at `|L|`
  (structural, via `start_at`).
- [ ] Fixtures (`test-api`):
  - add the named scenarios to `context-read/tests/fixtures/v3/scenarios.json`
    with minimal corpora: `insert_shared_vertex` (`acabcabcab`, `bcabcb`;
    query `abcabca`), `insert_periodic` (`ababababa`; query `bababab`, 5
    end-side levels), `insert_deep_borders` (`baaabababba`, `abaaabbaabba`,
    `aabbaabbaabbaa`; query `bbaabbaabb`, 4 start-side and 5 end-side
    levels, no shared vertex);
  - insert cases: for every fixture and every substring of a vertex string
    that is not a vertex, a scenario `<fixture>__<n>` with inputs
    `corpus + [query]` in `fixtures/v3/insert/scenarios.json` (written by an
    ignored regeneration test from the fixture strings), graphs produced with
    `ngrams --file … --out …` (storage: P4);
  - `insert_cases() -> Vec<InsertCase { base: FixtureGraph, query: String,
    expected: FixtureGraph }>`.
- **Tests:** `recompose` keeps `validate()` green and rejects a different
  width; `translate` maps every path through a removed occurrence to a path
  with `contains_path` and the same absolute span; I2 holds on every fixture
  and fails on a test-api graph that violates it; `insert_cases` loads and
  every query is absent from its base and present in its expected graph.

### 3.2 Split graph (`context-insert`, `split/`)

- [ ] `Interval` (`Range<AtomPosition>` relative to its vertex), `SplitNode {
  intervals: BTreeSet<Interval> }`, `SplitGraph { root, span, nodes:
  BTreeMap<VertexIndex, SplitNode> }` (keyed by `VertexIndex`; `Token`'s
  `Ord` compares width only).
- [ ] `SplitGraph::from_response(&PositionedGraph, &Response)`: seed from
  root, span, start path and end path; top-down frontier keyed by
  `(TokenWidth, VertexIndex)`, closing and pushing intervals (Part B).
  `EntireRoot` responses have no split graph.
- [ ] `test-api` counter: pushes that needed navigation (not a response path
  step).
- **Tests** (every insert case; expectations from strings):
  - every interval's string (test read) is a vertex of the base graph or a
    new vertex of the expected graph, and every new vertex of the expected
    graph is some interval's string;
  - the pushes of the root span are the response path steps (navigation
    counter counts intersection intervals only);
  - widths strictly decrease along pushes; the graph is unchanged.

### 3.3 Join (`context-insert`, `join/`)

- [ ] Bottom-up frontier over the split graph (increasing `(width, index)`;
  inside a node, increasing interval width); interval resolution, pieces,
  `maximal`, reuse by composition within the insertion, node update — all as
  in Part B, through one `GraphEdit`.
- [ ] Resolved tokens in a map `(VertexIndex, Interval) → Token`; the root
  span's token is `N`.
- **Tests:** through 3.4; plus a unit test that a skipped crossing child is
  always inside another piece (debug assertion exercised on all insert
  cases).

### 3.4 Insertion from a response (`context-insert`, `insert/`)

- [ ] `context-search`: `Response` records the graph generation under its
  read guard (`Response::generation()`).
- [ ] `pub enum InsertOutcome { Complete { token, response }, NoExpansion
  { token, response }, Created { token, response, publication } }` with the
  legacy accessors (`token`, `response`, `is_created`, …).
- [ ] `insert_match(&PositionedGraphRef, Response) -> Result<InsertOutcome,
  InsertError>` under one write guard: generation check (`StaleResponse`);
  `EntireRoot` → `Complete`/`NoExpansion`; otherwise split (3.2), join
  (3.3), `finish` → `Created`.
- [ ] `insert(&PositionedGraphRef, impl Searchable) -> Result<Token,
  InsertError>` and `insert_next_match(..) -> Result<InsertOutcome, ..>`:
  search, then `insert_match`. As in legacy, only the matched part is
  inserted; the caller continues from the response's checkpoint.
- **Tests:**
  - **Insert gate:** for every insert case, search the query (atom query) on
    the base graph; the root and span equal the brute-force minimal host
    (F3); `insert_match` gives a graph with an empty `diff` against the
    expected graph; `validate()` (with I2 and `DuplicateContent`) passes; no
    search runs inside `insert_match`.
  - Named cases `insert_shared_vertex`, `insert_periodic`,
    `insert_deep_borders` with their tables from Part B.
  - `Complete`/`NoExpansion` for queries that are a token or start with one,
    with the graph unchanged.
  - `StaleResponse` after an unrelated edit.
  - The response's paths, translated through the publication, are valid in
    the new graph and cover the same span.

### 3.5 Cover projection and merge (`context-insert`, `cover/`, `merge/`)

- [ ] `Cover { width, occurrences }` (always `maximal`), `Cover::of_span`
  (read-only projection, memoized per `(vertex, sub-span)`), `Cover::of_match`
  (start/end path leaves as first and last occurrence, projection between),
  `Cover::start_at`, `Cover::tokens_from`.
- [ ] `MergeOutcome { Existing(Token), Occurs { response, cover },
  New(Cover) }`, `Cover::merge(graph, left, right, r0)` (Part B),
  `insert_cover` for `New` (`GraphEdit::create` without host edits).
- [ ] `test-api` counters: seam searches per merge; no start searched twice.
- **Tests:** `of_span` on every proper span of every vertex equals brute
  force; `of_match` equals `of_span` on all 2.6 responses; merge gate as in
  revision 1 (every `0 < r0 ≤ wl < |V|` of every composite vertex: merged
  cover equals the composition; `New` for parentless `V`, `Occurs`
  otherwise, `Existing` on the full graph), re-run after 3.4 with the
  `Occurs` responses inserted by `insert_match`; named case `seam_bcd`.

### 3.6 Documentation and gate

- [ ] `context-insert/README.md`: positioned API section (`insert_match`,
  split graph, join, outcomes, publication, `Cover`, `merge`). Mark
  `HIGH_LEVEL_GUIDE.md` as legacy-only and fix its `end_bound` statement.
- [ ] Roadmap step 3 ticked, INDEX status updated, deviations recorded here.

## Gate

```sh
cargo check -p context-trace -p context-search -p context-insert -p context-read --no-default-features --features test-api
cargo test -p context-trace -p context-search -p context-insert --no-default-features --features test-api
cargo test --release -p context-trace -p context-search -p context-insert --no-default-features --features test-api
cargo test -p context-trace -p context-search -p context-insert --features legacy-tests
cargo build --workspace --all-targets --exclude context-editor-kernel --exclude sandbox-app
```

All 3.1–3.5 tests pass on every fixture and insert case. No algorithm calls
`read_interval` or builds content-keyed structures. context-insert without
`test-api` cannot reach `PositionedGraph::insert`.

## Decisions

Confirmed (2026-11-21):

| # | Topic | Decision |
|---|---|---|
| D1 | Input | `insert_match` takes a `Response` only. Its root is the narrowest host (F2), so insertion never searches, never descends to a host and supports no other input. |
| D2 | Intersection tokens | Mandatory: every overlap of two occurrences is a token (I2), checked by `validate()`. |
| D3 | Duplicates | No token is created twice: within one insertion a composition equal to an already created one reuses that token; `validate()` rejects duplicate content. |
| D4 | Execution order | Build the split graph from the response (one node per vertex, both borders merged), then join bottom-up by width. Rejected: closing the start and end chains independently (larger tokens before smaller ones are consolidated, stale second path), translating the second path, a plan-then-apply overlay with provisional tokens. |
| D5 | Hosts | No multi-host case: in an oracle-consistent graph a non-token substring has one minimal host (F3). Tokens created by one insertion may land in several split nodes (`ba` in `aba` and `bab`); the join handles that. Revision 1's step-5 scope and reinsert gate are dropped. |

Proposed:

| # | Topic | Proposal |
|---|---|---|
| P1 | Mutation primitive | `GraphEdit` (`create`, `recompose`, `finish`) is public in context-trace and checks structure; context-insert is its only caller. Raw `insert` stays `test-api`; context-insert stops enabling `test-api` unconditionally. |
| P2 | Stale responses | Generation stamp in `Response` (part of its equality), checked by `insert_match` (`StaleResponse`); bumped by `finish`, not by atom insertion. |
| P3 | API shape | `insert_match` / `insert` / `insert_next_match` with legacy outcome semantics, plus `Cover::merge`. No `InitInterval`. The caller decides what to insert; no retention logic here. |
| P4 | Insert expectations | One oracle graph per insert case under `fixtures/v3/insert/` (about 1,250 small files from the existing CLI). Alternative: a delta format (new vertices and changed compositions per case), which needs an `ngrams` change. |
| P5 | Failure atomicity | Edits are applied node by node under the write guard (parents need their children's new tokens). Errors after the first edit can only come from violated invariants: return the error and debug-assert; no rollback. Alternative: snapshot the touched compositions for rollback. |
| P6 | Seam merge | Right-to-left scan with the F4 stop; outcomes `Existing` / `Occurs` / `New`. Seam matches that are not tokens are returned with their responses; inserting them is the caller's retention decision. |

## Risks

| Risk | Mitigation |
|---|---|
| F3 or I2 fails for some oracle output or after a step-4 insertion | Both checked on every fixture and insert case; I2 in `validate()`; the insert gate asserts root = brute-force minimal host; formal argument against the frequency rule (`ngrams/src/graph/labelling/frequency.rs`) still open |
| Intersection intervals off the response paths are rarely exercised (2 insert cases) | Navigation counter; those cases are listed by the 3.2 test and stay in the gate |
| Partial edits after an internal error | P5; the gate requires no error on any case |
| Paths held by the reader go stale after an insertion | `Publication::translate` with validity tests; generation check |
| Fixture volume of the insert cases | P4; cases are generated, not hand-maintained |
| Cargo feature change breaks legacy builds | 3.0 runs the legacy suite before anything else |
| Merge was validated against rev-1 assumptions | 3.5 re-runs the merge gate after 3.4 with `insert_match` for `Occurs` |
