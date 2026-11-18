---
tags: `#plan` `#ngrams` `#oracle` `#composition` `#frequency` `#wrapper` `#gap` `#order-dependence`
summary: Replace the order-dependent wrapper/partition/gap-promotion pipeline of the n-gram oracle with a one-way pipeline. Only roots, atoms and frequency-selected tokens become vertices. Each vertex stores a positioned composition (maximal retained child occurrences with offsets, overlaps allowed). Wrappers and gaps become optional, owner-local navigation views that never create vertices or influence other compositions.
status: 📋 ready
revision: 1
date: 2026-11-18
blocking: `partitions_are_independent_of_root_hash_iteration_order` in `src/tests/hash_order.rs`; authoritative context-read fixtures
---

# Plan: Positioned Composition Oracle

**Date:** 2026-11-18
**Primary crate:** `ngrams`
**Consumers:** `context-read` fixture tests (`tests/support`, `reader_behavior`,
`fixture_validation`, `src/tests/cursor.rs`), `context-cli` oracle integration
tests, `context-api` compare commands.
**Scope:** oracle semantics, output model, validation and pipeline. Reader
algorithm migration is out of scope and needs a separate plan.

---

## 1. Problem

The current pipeline is:

```text
containment → frequency → wrapper → partition (+ ChildDedupPass gap promotion)
```

Partitioning writes decompositions while promoting gap tokens to global labels.
Already-written vertices are never rebuilt, so the output depends on root
processing order (`vis → [v, i, s]` vs `[vi, s]` in the regression).

Investigation established that this is not only a scheduling bug:

1. **Gap promotion has upward effects.** A promoted gap can partially overlap an
   existing tile in an unlabelled ancestor and turn it into a new wrapper, which
   requires new gaps (`abcabca`: `bca` → wrapper `bcabca` → gap `bc`).
2. **Promotions can make other gaps unnecessary.** In `abcabca`, promoting
   `bca` and wrapper `bcabca` eclipses an `abca` occurrence in the root and
   removes the need for gap `abc`. The symmetric schedule (`abc` → `abcabc` →
   `bc`) removes the need for `bca`.
3. **The wrapper/gap rules admit genuine choices.** Both schedules above reach
   fully closed, wrapper-closed label sets, neither containing the other. The
   non-frequency and non-wrapper constraints on initial gaps do not exclude this.
4. **Gap vertices break graph invariants.** A gap vertex such as `vi = [v, i]`
   next to `vis = [v, i, s]` represents the sequence `v, i` twice.

Ordering the promotion schedule cannot fix (3). The feedback edge from layout to
the vertex set must be removed.

## 2. Design decision

> Only roots, atoms and frequency-selected tokens are vertices. Wrappers and
> gaps are local views of intervals inside one owner vertex, not tokens.

| Layer | Contents | Authority |
|---|---|---|
| Semantic graph | Retained vertices, positioned child occurrences, reciprocal occurrence links | Authoritative; determines context relationships |
| Local layout (optional) | Rows, overlap groups, interval/gap views | Derived; can be rebuilt or omitted |

Dependency chain, with no backward edges:

```text
corpus → frequency labels → positioned compositions → optional local layout
```

Consequences:

- packing cannot change the vertex set;
- a gap view cannot change another composition;
- an overlap group cannot expose or eclipse children elsewhere;
- different layouts represent the same semantic graph.

Trade-off: omitted substrings (former wrappers/gaps) remain reconstructible
from compositions but are no longer directly addressable as vertices. Whether
frequency-only retention supports every reader query efficiently must be
verified separately (see §7).

## 3. Target model

### 3.1 Retained set

```text
S = roots ∪ atoms ∪ frequency-selected tokens
```

Frozen after the frequency pass. The exhaustive containment lattice remains
oracle working data only; unselected substrings never reach the output.

### 3.2 Positioned composition

For each non-atom `V ∈ S`:

```text
Composition(V) = sorted [(offset, child ∈ S), ...]
```

- children are retained proper substrings of `V`;
- an occurrence is kept iff its interval is not wholly contained in another
  retained proper-child occurrence of `V` (partial overlaps are kept);
- every atom position of `V` is covered (atoms are retained);
- repeated occurrences of one child are distinct by offset;
- sorted by `(offset, width)` for determinism.

This is the existing maximal child cover, evaluated against the fixed
frequency-only label set. It can be computed with the downward cover search, or
incrementally bottom-up with the exposed-span recurrence:

- a retained vertex exposes its whole span;
- an unretained vertex exposes its own composition;
- `cover(V)` = merge of the exposed spans of its prefix `V[0..n-1]` and suffix
  `V[1..n]` (suffix offsets shifted by 1), removing duplicate and wholly
  contained spans.

### 3.3 Local layout (optional)

Derived from one composition only:

- **Row:** non-overlapping subsequence of the composition's occurrences.
- **Gap view:** `(owner, range)` between row children; traversed by restricting
  existing child occurrences, never by creating a token.
- **Overlap group:** `(owner, range, members)` over partially overlapping
  occurrences.

Layout objects have no global identity, cannot be children of other vertices,
and are never consulted when computing compositions.

### 3.4 Examples

`abcabca` (retained: atoms, `abca`, root):

```text
abca:    a@0 b@1 c@2 a@3
abcabca: abca@0 abca@3
```

No `abc`, `bca`, `bc`, `abcabc` or `bcabca` vertices. A row layout of the root
is `[abca@0][gap 4..7]` / `[gap 0..3][abca@3]`; each gap view resolves through a
slice of an existing `abca` occurrence.

`ababa` (retained: atoms, `aba`, root):

```text
aba:   a@0 b@1 a@2
ababa: aba@0 aba@2
```

## 4. Output format (version 3)

Version 2 (`string[][]` concatenating decompositions) cannot represent positioned
overlaps without gap tokens, so it is retired, not adapted.

```json
{
  "format_version": 3,
  "corpus": ["abcabca"],
  "vertices": {
    "a": [], "b": [], "c": [],
    "abca": [[0, "a"], [1, "b"], [2, "c"], [3, "a"]],
    "abcabca": [[0, "abca"], [3, "abca"]]
  }
}
```

Layout is not serialized. If a debugging layout is wanted, emit it from a
separate command/field that the validator ignores semantically.

Validator rules (`GraphData::validate` replacement):

1. one vertex per content string; atoms have empty compositions, composites
   non-empty;
2. every child is a vertex and strictly narrower than its owner;
3. `owner[offset..offset+|child|] == child` for every occurrence;
4. union of child intervals covers `0..|owner|`;
5. no duplicate `(offset, child)`;
6. no occurrence wholly contained in another occurrence of the same owner;
7. sorted order;
8. every corpus text is a vertex.

Oracle-only semantic checks (not derivable from the snapshot alone):

9. vertex set equals `S`;
10. each composition equals the maximal cover of retained occurrences
    (no retained occurrence omitted).

## 5. Phases

### Phase 0 — Fix the semantic contract in tests

- Add regression corpora with expected v3 outputs derived by hand from the
  rules above: `abcabca`, `ababa`, `subdivision`/`visualization`, and one
  multi-text corpus.
- Add a property test: the output is invariant under input permutation (where
  root semantics are order-independent) and under root/queue iteration order.
- Keep the existing hash-order regression until Phase 5 replaces it.

**Gate:** tests compile and fail only for the expected reasons.

### Phase 1 — Harden frequency selection as the sole label source

The new design relies on frequency labels being order-independent. Audit and
fix, with tests:

- root detection only on first insertion (an input already seen as a substring
  may not become a root);
- byte vs character widths in containment;
- frequency labels identical under input permutation and root iteration order;
- `steps_total > 0` assertion for width-one-only corpora.

**Gate:** label-set permutation property passes on the regression corpora and a
generated corpus sweep.

### Phase 2 — Compute positioned compositions

- New module, e.g. `src/graph/composition/`:
  `Composition { entries: Vec<(usize, NGramId)> }` and
  `fn compositions(ctx: &LabellingCtx) -> BTreeMap<VertexKey, Composition>`.
- Labels are read-only (`&LabellingCtx`); propagate cancellation instead of
  discarding traversal results.
- Prefer the bottom-up exposed-span recurrence (one pass, cached); keep the
  downward `ChildCoverPass` as a test reference implementation and compare both.
- Use containment parents/children, not output links.

**Gate:** both implementations agree on all regression corpora and the sweep;
oracle-only checks 9–10 pass.

### Phase 3 — Output model and validator

- Add v3 `GraphData` (content → `Vec<(usize, String)>`) and `GraphSnapshot`
  validation (§4).
- Decide the in-memory result type: `context_trace::Hypergraph` stores
  concatenating child patterns and cannot hold positioned overlaps. Either
  - (a) `ParseResult` exposes the composition table and v3 `GraphData` directly
    (recommended for the oracle), or
  - (b) extend `context-trace` with a positioned-composition vertex form
    (belongs to the reader migration, not this plan).
- Update the CLI to emit v3; reject v2 requests explicitly.

**Gate:** CLI tests (`tests/cli.rs`) updated and passing; snapshots round-trip.

### Phase 4 — Switch the pipeline

```rust
ctx.label_freq()?;
let compositions = ctx.compose()?; // read-only
```

- Remove `label_wrap` and `label_part` from `parse_corpus`.
- Delete or quarantine: `labelling/wrapper.rs`, `partitions/` (including
  `PartitionBuilder`, `PartitionContainer`), `utils/dedup` (`ChildDedupPass`).
- Update trace events (`src/graph/trace.rs`) to record composition construction
  instead of partition choices.

**Gate:** full `cargo test --release -p ngrams` passes; Phase 0 tests pass.

### Phase 5 — Optional local layout

- Library function `layout(owner, &Composition) -> Rows` with deterministic
  packing, gap views and overlap groups, for debugging and future reader
  compatibility.
- Tests: layout never introduces vertices; every row is a non-overlapping,
  position-covering traversal when gap views are expanded; every gap view
  resolves via restricted existing occurrences.
- Replace the hash-order regression with the permutation/iteration-order
  property tests from Phase 0.

**Gate:** layout tests pass; README "known correctness limitation" removed.

### Phase 6 — Downstream coordination

- `context-read`: existing v2 fixtures are already marked provisional. Generate
  v3 fixtures alongside; reader behavior tests cannot compare against v3 until
  the reader stores or can project positioned compositions. Tracked in
  [the reader migration plan](../../../context-read/agents/plans/20261118_PLAN_POSITIONED_COMPOSITION_READER.md);
  do not regenerate v2 fixtures from the new oracle.
- `context-cli` oracle integration tests and `context-api` compare commands:
  migrate to v3 `GraphData` or gate them until the reader migration lands.
- Update `ngrams/README.md` (format, pipeline, limitations) and
  `context-read/README.md` (fixture format).

**Gate:** workspace builds
(`cargo build --workspace --all-targets --exclude context-editor-kernel --exclude sandbox-app`
in the Linux codespace) and all non-gated tests pass.

## 6. Test strategy

- Hand-derived expectations for small corpora (Phase 0); never accept oracle
  output as its own expectation without review.
- Properties: permutation/iteration-order invariance, validator rules 1–10,
  agreement of the two composition algorithms, layout introduces no vertices.
- Corpus sweep: all binary strings of lengths 2–8 plus generated 1–3 text
  corpora over alphabets of size 2–4; run in release mode, gated behind an
  ignored or feature-flagged test if slow.

## 7. Risks and open questions

| Risk / question | Mitigation |
|---|---|
| Frequency labels themselves are order-dependent | Phase 1 gate before relying on them |
| Frequency-only retention loses context relationships the reader needs | Reconstructibility holds by construction; query sufficiency and cost must be evaluated in the reader plan |
| Contained relationships reachable via several paths are counted twice during traversal | Deduplicate by `(vertex, owner-relative offset)` in traversal APIs |
| `Hypergraph` cannot store positioned overlaps | Phase 3 option (a) for the oracle; reader-side representation in a separate plan |
| Reader fixtures and compare tooling depend on v2 | Phase 6 keeps v2 fixtures frozen and provisional; v3 added alongside |
| Composition computation cost on large corpora | Bottom-up recurrence with cached exposed spans; one pass over containment |

## 8. Superseded approaches (do not revisit without new evidence)

- **Separate gap-labelling pass / rounds until fixpoint:** promotions change
  other covers and create wrappers; result is schedule-dependent.
- **Promote all eligible gaps and keep them:** adds unnecessary tokens and
  still requires choosing a schedule semantics.
- **Typed label strengths (gap labels excluded from covers):** keeps gap
  vertices, which still duplicate sequences (`vi`/`vis`).
- **Freezing rows bottom-up by width:** wider vertices can promote narrower
  gaps after narrower rows were visited.
- **Global subset optimization of promotions:** non-local, poor fit for the
  streaming reader.
