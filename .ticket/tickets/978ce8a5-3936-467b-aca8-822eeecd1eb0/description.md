---
tags: `#plan` `#context-read` `#context-insert` `#algorithm` `#expansion` `#overlap` `#refactoring`
summary: Redesign the ExpansionCtx inner loop so it drives insert_next_match in a cursor-advancing loop over known-atom segments, correctly handles the new/known classification boundary, and collects a tight set of decomposition patterns from all detected overlaps.
status: 🔄 in-progress
phase: 2-implementation
design: 20260315_DESIGN_ROOT_UPDATE_STEPS.md
parent: 20260314_PLAN_CONTEXT_READ_UX_IMPROVEMENT.md
related: 20260315_PLAN_INTEGRATION_TEST_REMEDIATION.md, 20260314_PLAN_INSERT_NEXT_MATCH.md, 20260314_PLAN_APPEND_TO_PATTERN_FIX.md
priority: top — this is the core algorithm fix that unblocks RC-1, RC-2, RC-3 and all 20 ignored integration tests
---

# Plan: Expansion Loop Redesign — `insert_next_match` Cursor Loop inside `ExpansionCtx`

**Date:** 2026-03-15
**Scope:** Medium (algorithm change in `context-read`; call-site ripple into `context-api`)
**Crates:** `context-read`, `context-api`
**Test baseline (original):** 44 pass / 9 fail / 22 ignored in `cli_integration`; `context-read` crate did not compile its test suite (247 errors — stale imports, not logic failures)
**Test baseline (current):** 53 pass / 0 fail / 22 ignored in `cli_integration`; `context-read` unit tests: **64 pass / 11 fail / 0 ignored**

---

## Table of Contents

1. [Objective](#objective)
2. [Context](#context)
3. [Current Architecture (as-is)](#current-architecture-as-is)
4. [Root Cause Analysis](#root-cause-analysis)
5. [Proposed Architecture (to-be)](#proposed-architecture-to-be)
6. [Interview Batches](#interview-batches)
7. [Files Affected](#files-affected)
8. [Risks](#risks)
9. [Related Documents](#related-documents)
10. [Notes](#notes)

---

## Objective

Fix the read/insert pipeline so that:

1. `ReadCtx` + `ExpansionCtx` correctly process a known-atom block by driving
   `insert_next_match` in a cursor-advancing loop, finding overlaps via postfix
   descent, and collecting a complete set of tight decomposition patterns.
2. `insert_sequence` in `context-api` reuses the same mechanism so that multi-
   character insertions produce a correct compound root token (not just the first
   atom).
3. The new/known classification is preserved as the correct boundary control —
   new atoms are concatenated directly without overlap search; known atoms go
   through the expansion loop.

---

## Context

### Architecture layers involved

```
ReadCtx (context-read)
  └─ SegmentIter          — splits input into (unknown-run, known-run) pairs
       └─ RootManager     — accumulates the growing root token
            ├─ append_pattern    — handles unknown atoms (direct concatenation)
            └─ BlockExpansionCtx — handles known atoms (overlap search)
                 └─ ExpansionCtx — drives postfix iteration
                      └─ ExpandCtx  — per-postfix: calls insert() to test expansion
```

### What the algorithm is supposed to do

The read algorithm processes input left-to-right. At each position it asks:
*"what is the largest token already in the graph that matches the input starting
here?"* If found via search, it records that token and advances the cursor by its
width. If the next call from the new cursor position also finds an existing token
whose **prefix overlaps** with the **postfix** of the previously found token, an
overlap is detected and both decompositions are recorded into the graph.

The new/known classification is the efficiency gate: a **new** atom (one that did
not exist before this read began) cannot be part of any existing compound token
and therefore cannot be part of any overlap. Processing it is just concatenation.
A **known** atom (pre-existing) may already be a child of compound tokens, so
overlap search is warranted.

### Key types

| Type | Location | Role |
|------|----------|------|
| `NewAtomIndex` | `context-trace` | `New(idx)` or `Known(idx)` — produced by lazy/eager atom resolution |
| `SegmentIter<I>` | `context-read/segment.rs` | Partitions `NewAtomIndex` stream into `NextSegment { unknown, known }` |
| `NextSegment` | `context-read/segment.rs` | One `(unknown: Pattern, known: Pattern)` pair |
| `RootManager` | `context-read/context/root.rs` | Owns the accumulating root `Option<Token>`; provides `append_pattern`, `append_token`, `commit_state` |
| `BlockExpansionCtx` | `context-read/expansion/block.rs` | Wraps `ExpansionCtx`; calls `process()` then `finish()` |
| `ExpansionCtx` | `context-read/expansion/mod.rs` | Owns `CursorCtx` + `BandState`; `Iterator` yielding `BandState` |
| `ExpandCtx<'_>` | `context-read/expansion/chain/expand.rs` | Per-step: iterates postfixes of anchor token, calls `insert()` per postfix |
| `BandState` | `context-read/expansion/chain/mod.rs` | `Single { band }` or `WithOverlap { primary, overlap, link }` |
| `Band` | `context-read/expansion/chain/band.rs` | Pattern + start/end bounds |
| `InsertOutcome` | `context-insert/insert/outcome.rs` | `Created`, `Complete`, `NoExpansion` — each carries `IndexWithPath + Response` |
| `insert_next_match` | `context-insert/insert/mod.rs` | Single-step: find or create largest-match token at query start |

---

## Current Architecture (as-is)

### Segment loop in ReadCtx

```
ReadCtx::read_sequence()
  for each NextSegment { unknown, known } from SegmentIter:
    root.append_pattern(unknown)   ← direct concat, no search
    BlockExpansionCtx::new(root, known).process()
      while ExpansionCtx::next() -> Some(state):
        root.commit_state(state)
```

### ExpansionCtx initialisation

On construction `ExpansionCtx::new` checks if a `band` (root's last token) is
provided. If not, it calls `insert_next_match` once to find the first token from
the cursor and builds a `BandState::Single` from it. The `cursor` is the known
pattern as a `PatternRangePath`.

### ExpandCtx iteration (per ExpansionCtx::next step)

`ExpandCtx::try_new` takes the anchor token (last token in `BandState`'s primary
band) and calls `anchor.postfix_iter()`. If the anchor has no parents in the graph
(e.g. it is a fresh atom with no compound ancestors), `postfix_iter()` returns
nothing and `ExpandCtx::try_new` returns `None`.

For each postfix `P` of the anchor (largest first):
- Calls `insert(&cursor)` — tests whether the cursor can expand from this postfix
- If `Ok(expansion)` → `ChainOp::Expansion` (overlap found)
- If `Err(_)` → `ChainOp::Cap` (no expansion from this postfix)

`ExpansionCtx::next` picks the first `Expansion` or a qualifying `Cap` and calls
`apply_op`, producing a new `BandState`.

### The problem

`ExpandCtx` calls `insert(&cursor)` with the **same static cursor for every
postfix**. The cursor is not advanced between postfix probes. After finding an
overlap and committing via `commit_state`, the cursor is also not advanced past
the consumed atoms. `BlockExpansionCtx::process` loops over `ExpansionCtx::next`
but `ExpansionCtx` has no mechanism to advance the cursor through the known-atom
block: it handles exactly one overlap event and then exhausts.

Consequence: on the first call to a fresh graph for `"aaaa"`, atom `a` has no
parents, `postfix_iter()` is empty, `ExpandCtx::try_new` returns `None`,
`ExpansionCtx::next` returns `None` immediately, and the loop exits with the root
still holding only the first appended atom (width=1 from `append_pattern`).

For `insert_sequence("hello")` in `context-api`, the call sequence does not go
through `ReadCtx` at all — it calls `insert_next_match` once with the full atom
slice, gets `NoExpansion { token=h, width=1 }`, and returns `h` as the root.

---

## Root Cause Analysis

There are two distinct missing loops:

### RC-1 — `insert_sequence` has no outer loop

`WorkspaceManager::insert_sequence` calls `insert_next_match([a₀…aₙ])` once.
On a fresh graph `insert_next_match` returns `NoExpansion { token=a₀, width=1 }`.
The function returns `a₀` as the root. Fix: add a cursor-advancing `while` loop
that calls `insert_next_match` with the remaining slice and collects segment tokens
until `cursor == atoms.len()`, then wraps the segments into a compound root.

### RC-2/RC-3 — `ExpansionCtx` has no inner cursor loop

Within a known-atom block, `ExpansionCtx` does not call `insert_next_match` in a
loop over the atoms. After one overlap event (or zero if no postfixes exist), the
expansion stops. The cursor inside `CursorCtx` is a `PatternRangePath` and is
never advanced in the current code after an overlap is committed.

Fix: `ExpansionCtx` must drive a cursor loop:
1. Call `insert_next_match(remaining_known_atoms)` from current cursor position.
2. If `Created` or `Complete` → record the token, advance cursor by `token.width`.
3. If `NoExpansion { token, width }` → the token matches at the cursor but the
   block is longer. Descend postfixes of `token` to check for overlap with the
   next part of the block. If overlap found, record both decompositions and advance
   cursor by `token.width - postfix.width` (up to the overlap start). If no
   overlap, advance cursor by `token.width`.
4. Repeat until cursor reaches the end of the known-atom block.

---

## Proposed Architecture (to-be)

```
ReadCtx::read_sequence()
  for each NextSegment { unknown, known } from SegmentIter:
    root.append_pattern(unknown)          ← unchanged: direct concat
    if !known.is_empty():
      // IMPORTANT: commit each state before computing the next —
      // step N+1 may depend on graph state produced by committing step N.
      let mut ctx = ExpansionCtx::new(graph, known, root.last_token());
      while let Some(state) = ctx.next() {
        root.commit_state(state);
      }
```

### ExpansionCtx (to-be)

```
struct ExpansionCtx {
    graph:   HypergraphRef,
    atoms:   Pattern,          // the full known-atom block for this segment
    cursor:  usize,            // atom-index position within `atoms`
    anchor:  Option<Token>,    // last committed token (for postfix overlap search)
    pending: Option<BandState> // buffered state awaiting commit
}

impl Iterator for ExpansionCtx {
    type Item = BandState;

    fn next(&mut self) -> Option<BandState> {
        if self.cursor >= self.atoms.len() { return None; }

        let remaining = &self.atoms[self.cursor..];

        // Single atom: direct advance, no insert_next_match (would error)
        if remaining.len() == 1 {
            let token = remaining[0];
            self.cursor += 1;
            self.anchor = Some(token);
            return Some(BandState::new(token));
        }

        let outcome = insert_next_match(graph, remaining)?;

        match outcome {
            Complete { token } | Created { token } => {
                self.cursor += token.width;
                self.anchor = Some(token);
                Some(BandState::new(token))
            }

            NoExpansion { token } => {
                // token is the best match starting at cursor (width = token.width)
                // Check postfixes of token for overlap with remaining[token.width..]
                // Complement tokens (prefix/suffix) are resolved inside find_overlap
                // via recursive insert_next_match before collapse() is ever called.
                match self.find_overlap(token, &remaining[token.width..]) {
                    Some((postfix, expansion, next_cursor)) => {
                        // Cursor advances into T2's overlap region, NOT past T1.
                        // next_cursor = T2.start + (T2.width - largest_postfix_of_T2.width)
                        self.cursor = next_cursor;
                        // Anchor = expansion (T2), NOT token (T1).
                        // T2 is the next largest leftmost match; its postfixes are
                        // the candidates for the next overlap check. (Batch 5, Q23)
                        self.anchor = Some(expansion);
                        Some(BandState::with_overlap(token, postfix, expansion))
                    }
                    None => {
                        self.cursor += token.width;
                        self.anchor = Some(token);
                        Some(BandState::new(token))
                    }
                }
            }
        }
    }
}
```

`find_overlap(anchor, next_remaining)`:
- Iterates postfixes of `anchor`, largest first.
- For each postfix `P`, calls `insert_next_match(next_remaining)`.
  - **Overlap condition:** result.width > P.width (the postfix was genuinely
    expanded by the following atoms into a larger token).
  - If result.width == P.width the postfix was found verbatim — not an overlap;
    skip and try the next smaller postfix.
- On first qualifying match: resolve complement tokens (`prefix_complement`,
  `suffix_complement`) via recursive `insert_next_match` on their atom sub-slices.
  Complements may be multi-token patterns requiring a new compound token.
  All complement tokens must exist in the graph **before** `collapse()` is called.
- Returns `Some((postfix, expansion_token, next_cursor))` on first match,
  `None` if all postfixes exhausted without a qualifying expansion.

### insert_sequence outer loop (context-api, to-be)

```rust
let mut cursor = 0;
let mut segments: Vec<Token> = Vec::new();

while cursor < atoms.len() {
    let remaining = &atoms[cursor..];
    if remaining.len() == 1 {
        segments.push(remaining[0]);
        cursor += 1;
        continue;
    }
    let outcome = insert_next_match(&graph_ref, remaining.to_vec())?;
    let width = outcome.token().width.0;
    assert!(width > 0, "insert_next_match returned zero-width token");
    segments.push(outcome.token());
    cursor += width;
}

let root = if segments.len() == 1 {
    segments[0]
} else {
    graph_ref.insert_pattern(segments)
};
```

---

## Interview Batches

Interview questions are split into separate files for focused iteration. Each
batch is answered independently; answers feed back into the plan via **Plan
Impact** sections in each batch file.

| Batch | File | Topic | Status |
|-------|------|-------|--------|
| 1 | [20260315_INTERVIEW_BATCH_1.md](20260315_INTERVIEW_BATCH_1.md) | Classification Boundary | ✅ answered |
| 2 | [20260315_INTERVIEW_BATCH_2.md](20260315_INTERVIEW_BATCH_2.md) | ExpansionCtx Loop Contract | ✅ answered |
| 3 | [20260315_INTERVIEW_BATCH_3.md](20260315_INTERVIEW_BATCH_3.md) | Overlap Collection and Decomposition Output | ✅ answered |
| 4 | [20260315_INTERVIEW_BATCH_4.md](20260315_INTERVIEW_BATCH_4.md) | Cursor Advancement and NoExpansion Handling | ✅ answered |
| 5 | [20260315_INTERVIEW_BATCH_5.md](20260315_INTERVIEW_BATCH_5.md) | RootManager and Commit Contract | ✅ answered |
| 6 | [20260315_INTERVIEW_BATCH_6.md](20260315_INTERVIEW_BATCH_6.md) | insert_sequence Outer Loop in context-api | ✅ answered |
| 7 | [20260315_INTERVIEW_BATCH_7.md](20260315_INTERVIEW_BATCH_7.md) | Performance and Streaming | ✅ answered |

---

## Files Affected

> Preliminary — final list after interview answers are incorporated.

| File | Change | RC |
|------|--------|----|
| `crates/context-read/src/expansion/mod.rs` | Redesign `ExpansionCtx` iterator — add `atoms`, `cursor`, `anchor` fields; implement `insert_next_match` loop with postfix overlap search | RC-2/RC-3 |
| `crates/context-read/src/expansion/block.rs` | Simplify `BlockExpansionCtx` — may reduce to thin wrapper or dissolve | RC-2/RC-3 |
| `crates/context-read/src/expansion/chain/mod.rs` | `BandState::new` / `BandState::with_overlap` constructors; `collapse` may need adjustment | RC-2/RC-3 |
| `crates/context-read/src/expansion/chain/expand.rs` | `ExpandCtx` may be repurposed as the per-step postfix probe helper (no longer an `Iterator` driving the outer loop) | RC-2/RC-3 |
| `crates/context-read/src/context/root.rs` | `append_collapsed` overlap logic — may be removed if overlaps are fully committed via `BandState::WithOverlap` | RC-2/RC-3 |
| `crates/context-read/src/context/mod.rs` | `ReadCtx::read_segment` — simplify to delegate to new `ExpansionCtx` | RC-2/RC-3 |
| `crates/context-api/src/commands/insert.rs` | `insert_sequence` — add outer cursor-advancing loop | RC-1 |
| `crates/context-read/src/tests/cursor.rs` | Fix compilation (stale imports); update `TODO` tests to assert correct behaviour | all |
| `crates/context-read/src/tests/linear.rs` | Should pass without changes once RC-1 fixed | RC-1 |
| `crates/context-read/src/tests/overlapping.rs` | Should pass after RC-2/RC-3 fixed | RC-2/RC-3 |
| `tools/context-cli/tests/FAILING_TESTS.md` | Update after each fix round | all |

---

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|-----------|
| Zero-width token from `insert_next_match` causes infinite loop | Medium | Critical | Add `assert!(width > 0)` guard immediately inside the loop |
| `BandState::collapse` produces wrong patterns when called once-per-token | Medium | High | Add unit tests for `collapse` with single-token `Single` and two-token `WithOverlap` before touching the loop |
| `append_collapsed` overlap logic in `RootManager` fires redundantly alongside `ExpansionCtx` overlap, double-committing | Medium | High | Answer Q21 (Batch 5) before touching `RootManager`; disable the `append_collapsed` overlap path if `ExpansionCtx` takes over |
| RC-2 (`read_sequence` returns `None` after insert) survives even after RC-1 fix | Medium | Medium | Re-run RC-2 tests immediately after RC-1 fix; if still failing, trace `ReadCtx::new` construction after prior `insert_sequence` call |
| Fixing `ExpansionCtx` loop breaks the 44 currently-passing integration tests | Low | High | Run full suite after every sub-step; do not merge until 44 still pass |
| `context-read` test compilation fix introduces unexpected test failures | Low | Medium | Fix compilation first in a separate commit; confirm 0 new regressions before algorithm work begins |

---

## Related Documents

| Document | Relationship |
|----------|-------------|
| [`20260314_PLAN_CONTEXT_READ_UX_IMPROVEMENT.md`](20260314_PLAN_CONTEXT_READ_UX_IMPROVEMENT.md) | Grandparent plan — RC-1/RC-2/RC-3 are Phase 3 items |
| [`20260315_PLAN_INTEGRATION_TEST_REMEDIATION.md`](20260315_PLAN_INTEGRATION_TEST_REMEDIATION.md) | Fix round structure — Round 1 (RC-1), Round 2 (RC-2), Round 3 (RC-3) |
| [`20260314_PLAN_INSERT_NEXT_MATCH.md`](20260314_PLAN_INSERT_NEXT_MATCH.md) | ✅ Complete — `InsertOutcome` enum available; `insert_next_match` API stable |
| [`20260314_PLAN_APPEND_TO_PATTERN_FIX.md`](20260314_PLAN_APPEND_TO_PATTERN_FIX.md) | ✅ Complete — `extend_root_pattern` / `append_to_owned_pattern` available |
| [`20260315_PLAN_NGRAMS_ORACLE_VALIDATION.md`](20260315_PLAN_NGRAMS_ORACLE_VALIDATION.md) | Oracle tests are the long-form acceptance criteria for the fixed algorithm |
| `tools/context-cli/tests/FAILING_TESTS.md` | Live failure tracker |

---

## Notes

### Deviations from Plan

- **T2 merged into T1:** `BandState::collapse()` review was conducted as part of the T1 root update design session rather than as a separate step. The existing `collapse()` implementation was retained with minor adjustments (complement construction logic moved into `build_prefix_complement` / `build_postfix_complement` helpers in `chain/mod.rs`).
- **T3 split into T3a/T3b:** The `RootManager` rewrite was large enough to merit two sub-steps: (T3a) adding `anchor`/`flat_root` fields + deleting `append_collapsed` overlap logic, (T3b) implementing `replace_last_child` (Op-4a/4b). Both are complete.
- **T5 wiring confirmed (PI-16):** `insert_sequence` now delegates entirely to `ReadCtx::read_sequence` via a one-line replacement. The `< 2` guard was removed (PI-18). The `already_existed` signal is derived from comparing `pre_vertex_count` vs `post_vertex_count` (PI-19/PI-20).
- **RC-1 fully resolved:** The `cli_integration` suite now shows **53 pass / 0 fail / 22 ignored** (up from 44/9/22). All nine previously-failing tests now pass; the 22 ignored tests remain (oracle and skill3_exp overlap tests, RC-2/RC-3 scope).
- **RC-2 partially resolved:** Several tests that were RC-2 failures (`read_sequence_after_insert`, `dedup_*`) now pass as a side-effect of the correct `ReadCtx::read_sequence` delegation. See updated `cli_integration` baseline above.
- **RC-3 in progress (T4):** The `ExpansionCtx` cursor loop and `find_overlap` are implemented in `expansion/mod.rs`. The `context-read` unit test suite now shows **64 pass / 11 fail / 0 ignored**. The 11 remaining failures are all overlap/repetition tests requiring correct `BandState::WithOverlap` collapse output.

### Plan Impacts from Interview

#### From Root Update Design Session (OQ-1 through OQ-5)

- **PI-30** — **`replace_last_child(bundled)` is a new required primitive on
  `RootManager`.** Op-4 dispatches internally on `can_extend`: when `can_extend`
  is true (Op-4a) it mutates the root's single pattern `Vec` in-place — no new
  graph token created; when `!can_extend` (Op-4b) it rebuilds the first child
  pattern with `B` as the last element and calls `graph.insert_pattern` to create
  a new root token. Callers never check `can_extend` before calling
  `replace_last_child` — the dispatch is encapsulated inside the primitive.
- **PI-31** — **Zero-width prefix complement case is structurally impossible.**
  Atoms have no true postfixes; the atom fast-path in `ExpansionCtx::next` (PI-14)
  prevents `find_overlap` from ever being called with an atom anchor. Therefore
  `BandState::collapse()` never receives a zero-width complement slot and does not
  need to handle that case.
- **PI-32** — **`anchor: Option<Token>` is a new field on `RootManager`.**
  Updated by `commit_state` at the end of every call:
  `BandState::Single` → `anchor = Some(token)`;
  `BandState::WithOverlap` → `anchor = Some(T2)` (expansion result, never `B`).
  Read by `ExpansionCtx::next` at the start of each step via `root.anchor()`.
  The existing `last_child_token()` method is a different concept and must not be
  used as the anchor.
- **PI-33** — **Reachability invariant is self-maintaining across Case D/E
  replacements.** The old last-child token `T1` is not deleted from the graph.
  Bundled token `B` wraps `T1` in its own child patterns. The path
  `root → B → T1 → ...` is always valid. No back-reference surgery is needed on
  the old token; the graph's reference-by-index model is self-consistent.
- **PI-34** — **`commit_state` dispatch table** (all cases now fully resolved):
  - `BandState::Single`, `root == None` → Case A: `set_root(token)`
  - `BandState::Single`, `root.is_some() && can_extend` → Case B: `extend_root(token)`
  - `BandState::Single`, `root.is_some() && !can_extend` → Case F: `wrap_root(token)`
  - `BandState::WithOverlap`, `root == None || root == T1` → Case C: `set_root(B)`
  - `BandState::WithOverlap`, `root.is_some() && root != T1 && can_extend` → Case D: `replace_last_child(B)` Op-4a
  - `BandState::WithOverlap`, `root.is_some() && root != T1 && !can_extend` → Case E: `replace_last_child(B)` Op-4b
  After every branch: update `anchor` as per PI-32.

#### From Batch 7 (Performance and Streaming)

- **PI-26** — **`ExpansionCtx` uses a lazy-buffered source, not a pre-materialised
  slice.** Replace `atoms: Pattern` with `buffer: Vec<Token>` (atoms materialised
  so far) + `source: impl Iterator<Item = NewAtomIndex>` (lazy remainder). Add a
  private `ensure_materialised(n)` helper. This preserves stream plumbing for the
  lazy path without requiring an eager pre-pass.
- **PI-27** — **No width-comparison guard in `find_overlap`.** Postfixes are
  structurally bounded by the query (they are subtokens of a previously expanded
  token). The only valid early exit is `remaining.is_empty()`. No
  `postfix.width > remaining.len()` check is needed.
- **PI-28** — **No atom pre-pass; interleaved writes are correct and optimal.**
  New atoms are inert for overlap detection (no parents yet). Separating them into
  a pre-pass would require full eager materialisation, conflicting with the lazy
  streaming model. Interleaving is intentional.
- **PI-29** — **Definitive implementation task order** (see below).

#### From Batch 6 (insert_sequence Outer Loop in context-api)

- **PI-16** — **RC-1 fix is subsumed by RC-2/RC-3.** `insert_sequence` delegates
  entirely to `ReadCtx::read_sequence`. `ReadCtx::read_sequence` is the single
  canonical entry point for consuming a sequence and creating all necessary tokens.
  The change to `context-api/src/commands/insert.rs` is a one-line replacement of
  the `insert_next_match` call with a `ReadCtx::read_sequence` call.
- **PI-17** — **No segment token list / wrapping step.** Segments are committed
  into the root incrementally as `BandState` values are yielded. There is no
  final `insert_pattern(segments)` call. The root is fully built when the loop
  terminates.
- **PI-18** — **Remove `< 2` input guard from `insert_sequence`.** A single-token
  input returns that token directly with `already_existed = true` and no graph
  writes. The loop handles this naturally without a pre-check.
- **PI-19** — **`already_existed` and `has_expanded` are per-outcome signals, not
  aggregate.** `already_existed` = `outcome != Created`. `has_expanded` =
  `outcome != NoExpansion`. These are orthogonal; do not conflate them into a
  single sequence-level flag.
- **PI-20** — **Root wrapping always produces a new token.** The "already existed
  root wrap" scenario is impossible: `insert_next_match` always finds the largest
  existing token (which therefore has no parents matching the broader query), so
  any composed root is always new. The workspace is always marked dirty when a
  multi-segment sequence is produced.

#### From Batch 5 (RootManager and Commit Contract)

- **PI-21** — **Delete `append_collapsed` overlap logic entirely.** The two
  overlap-detection branches in `append_collapsed` are architecturally invalid.
  After deletion, `append_collapsed` is a pure structural append: (1) `root==None`
  → create fresh root; (2) `can_extend` → extend in-place; (3) `!can_extend` →
  wrap. No overlap awareness. Add a `// REMOVED: overlap detection — handled by
  ExpansionCtx` comment at the deletion site.
- **PI-22** — **Fix anchor update: set `self.anchor = Some(expansion)` not
  `Some(token)` in the `WithOverlap` arm.** The anchor after an overlap commit is
  `T2` (the expansion result), not `T1` (the token that triggered the postfix
  descent). `T2` is the largest leftmost match found and has the shortest path to
  the next overlap candidate. The pseudo-code has been corrected above.
- **PI-23** — **`can_extend` is volatile; check it per-commit, never cache it.**
  An overlap commit adds a second pattern to the root, invalidating `can_extend`.
  Subsequent commits in the same loop must use the wrap path. Re-evaluate
  `can_extend` at every `commit_state` call.
- **PI-24** — **Root update design session is a hard prerequisite.** The exact
  root mutation sequence for every combination of root state × `BandState` variant
  has been captured in
  [`20260315_DESIGN_ROOT_UPDATE_STEPS.md`](../designs/20260315_DESIGN_ROOT_UPDATE_STEPS.md).
  Implementation of RC-2/RC-3 must not begin without reviewing that document and
  resolving its five open questions (OQ-1 through OQ-5).
- **PI-25** — **`root=None` + first `WithOverlap` commit behaviour is TBD.** The
  Q25 case (root starts `None`, first committed state is a bundled overlap token)
  must be explicitly validated in the root update design session. The existing
  `None` branch in `append_collapsed` may not handle this correctly.

#### From Batch 4 (Cursor Advancement and NoExpansion Handling)

- **PI-12** — **Unify RC-1 and RC-2/RC-3 into a single loop.** `insert_sequence`
  may be a duplicated read endpoint. A single shared loop mechanism drives
  `insert_next_match` + `ExpansionCtx` postfix descent. `insert_sequence` in
  `context-api` becomes a thin wrapper; `ReadCtx::read_segment` calls the same
  shared helper. The RC-1 and RC-2/RC-3 fixes are one change, not two independent
  ones. Update Files Affected table accordingly.
- **PI-13** — **`Complete` treated same as `NoExpansion` for cursor advancement.**
  `Complete { token }` does not guarantee full query consumption. Both `Complete`
  and `NoExpansion` advance the cursor by `token.width` and then check for overlap.
  The distinction matters only for `already_existed` bookkeeping (Batch 6).
- **PI-14** — **Add atom fast-path inside `ExpansionCtx::next`.** Atoms have no
  true postfixes — skip `find_overlap` entirely when the current token is an atom.
  The guard `token.is_atom()` is stronger and clearer than `remaining.len() == 1`.
- **PI-15** — **Assert strict cursor advance every step.** Add
  `debug_assert!(self.cursor > cursor_before)` inside `ExpansionCtx::next` to
  catch any regression where a step fails to advance the cursor.

#### From Batch 3 (Overlap Collection and Decomposition Output)

- **PI-8** — **Overlap detection predicate corrected.** The original plan stated
  "result.width == P.width is the overlap." This is wrong. The correct condition
  is `result.width > P.width` — the postfix must have been genuinely expanded by
  the following atoms. A verbatim match (result == postfix, no expansion) is not
  an overlap; skip to the next smaller postfix.
- **PI-9** — Complement tokens (`prefix_complement`, `suffix_complement`,
  `prefix_of_T1`, `suffix_of_T2`) are **not guaranteed to exist** and may be
  multi-token patterns requiring a new compound token. They must be resolved via
  recursive `insert_next_match` **inside `find_overlap`**, before `collapse()` is
  called. Invariant: by the time `BandState::collapse()` runs, all child tokens in
  both bands must already exist as valid graph indices.
- **PI-10** — **Cursor advance is dynamic, not `+= token.width`.** After an
  overlap, the cursor advances to the start of `T2`'s largest postfix (inside
  `T2`'s range), keeping that postfix in view for the next iteration's overlap
  check. The `find_overlap` return value now carries the computed `next_cursor`
  position. The `cursor += token.width` formula in the `NoExpansion` branch is
  replaced with `cursor = next_cursor` from the overlap result.
- **PI-11** — `BandState::collapse()` review is a **prerequisite sub-task** before
  RC-2/RC-3 implementation begins. Review whether the existing logic handles
  one-token-per-yield bands with externally-resolved padding. Rewrite only if it
  cannot accommodate the new band shape.

#### From Batch 2 (ExpansionCtx Loop Contract)

- **PI-5** — `.collect()` anti-pattern removed from proposed pseudo-code. Each
  `BandState` must be committed to the root **before** `ctx.next()` is called
  again. Step N+1 may depend on graph state produced by committing step N
  (e.g. a `Created` token from step N must be visible to step N+1's postfix
  search). The interleaved `while let Some(state) = ctx.next() { root.commit_state(state); }`
  loop is mandatory.
- **PI-6** — Padding token resolution is part of `BandState::collapse()` / band
  construction. Every overlap-derived pattern must be padded at both ends so all
  patterns on a bundled token span the same atom range. Padding tokens are
  resolved via `insert_next_match` on the relevant atom sub-slice (same mechanism
  as PI-4 complement tokens — the two concepts converge).
- **PI-7** — Single-element guard (`remaining.len() == 1`) applies to any token
  type (atom or compound). No special-casing needed. A single token cannot expand
  further; append it directly as `BandState::new(token)`.

#### From Batch 1 (Classification Boundary)

- **PI-1** — No cross-boundary overlap guard needed. A `New` atom cannot be part
  of any pre-existing compound token, so the unknown→known boundary is always a
  clean cut. Add a safety comment at that boundary in the implementation.
- **PI-2** — Confirm the `read_sequence` segment loop is unbounded (handles
  arbitrary many `NextSegment` values). Add `"abac"` as a required test case.
- **PI-3** — `SegmentIter` may be redundant in the lazy path (per-atom dispatch
  could replace it). Defer to a post-fix-round cleanup plan; no action during
  this redesign.
- **PI-4** — Tight-packing rule (Q3) means complement tokens in overlap
  decompositions must be resolved via `insert_next_match` (best existing match),
  not constructed from raw atom slices. This feeds into Batch 3 (Q12/Q13).

### Interview Progress

| Batch | Status | Answer file |
|-------|--------|-------------|
| 1 — Classification Boundary | ✅ answered | [BATCH_1](20260315_INTERVIEW_BATCH_1.md) |
| 2 — ExpansionCtx Loop Contract | ✅ answered | [BATCH_2](20260315_INTERVIEW_BATCH_2.md) |
| 3 — Overlap Collection and Decomposition Output | ✅ answered | [BATCH_3](20260315_INTERVIEW_BATCH_3.md) |
| 4 — Cursor Advancement and NoExpansion Handling | ✅ answered | [BATCH_4](20260315_INTERVIEW_BATCH_4.md) |
| 5 — RootManager and Commit Contract | ✅ answered | [BATCH_5](20260315_INTERVIEW_BATCH_5.md) |
| 6 — insert_sequence Outer Loop in context-api | ✅ answered | [BATCH_6](20260315_INTERVIEW_BATCH_6.md) |
| 7 — Performance and Streaming | ✅ answered | [BATCH_7](20260315_INTERVIEW_BATCH_7.md) |

### Implementation Task Order

Derived from all seven interview batches. This is the definitive sequence for
the implementation phase:

| # | Task | Key PIs | Prerequisite | Status |
|---|------|---------|--------------|--------|
| T0 | Fix `context-read` test compilation (247 stale import errors). No logic changes. Record baseline pass/fail counts. | PI-29 | — | ✅ **Done** — 0 errors, 0 warnings. **Baseline: 45 pass / 30 fail / 0 ignored.** Fixes: added `context_trace::graph::vertex::parent::PatternIndex` + `HasReadCtx` to `read/mod.rs`; added `context_search::{assert_indices, Find}` to `linear.rs` and `overlapping.rs`; added `context_insert::ToInsertCtx` + `context_search::ErrorState` to `cursor.rs`. |
| T1 | Review and resolve OQ-1 through OQ-5. | PI-24, PI-25, PI-30–34 | T0 | ✅ **Done** — all five open questions resolved. See [`DESIGN_ROOT_UPDATE_STEPS.md`](../designs/20260315_DESIGN_ROOT_UPDATE_STEPS.md). |
| T2 | Review `BandState::collapse()`. Confirm it handles one-token-per-yield bands with externally-resolved complements. Zero-width complement case does not occur (PI-31). Rewrite only if needed. | PI-11, PI-31 | T1 | ✅ **Done** — merged into T1. Existing `collapse()` retained; complement construction factored into `build_prefix_complement` / `build_postfix_complement` in `chain/mod.rs`. |
| T3 | Add `anchor: Option<Token>` + `flat_root: bool` fields to `RootManager` (PI-32). Add `anchor()` accessor. Delete `append_collapsed` overlap logic (PI-21). Add `replace_last_child(bundled)` primitive (PI-30). Implement full `commit_state` dispatch table (PI-34). Confirm no regressions against T0 baseline. | PI-21, PI-30, PI-32, PI-34 | T2 | ✅ **Done** — `root.rs` fully rewritten: `anchor`, `flat_root`, `set_root`, `extend_root`, `wrap_root`, `replace_last_child` (Op-4a/4b), `commit_state` (Cases A/B/C/D/E/F) all implemented. `append_collapsed` removed. |
| T4 | Implement `ExpansionCtx` cursor loop: lazy-buffered source (PI-26), atom fast-path (PI-14), correct overlap predicate `result.width > postfix.width` (PI-8), dynamic cursor advance from `find_overlap` (PI-10), anchor refresh via `BlockExpansionCtx::process` after each commit (PI-32/OQ-5), complement resolution inside `find_overlap` before `collapse()` (PI-9), `debug_assert!(cursor advanced)` (PI-15), no width guard in `find_overlap` (PI-27). | many | T3 | 🔄 **In progress** — core loop + `find_overlap` + `build_overlap_state` implemented in `expansion/mod.rs`; `BlockExpansionCtx::process` refreshes anchor after each commit. `context-read` unit tests: **64 pass / 11 fail / 0 ignored**. Remaining 11 failures analysed in [`20260315_ANALYSIS_T4_FAILURE_REVIEW.md`](20260315_ANALYSIS_T4_FAILURE_REVIEW.md) — four groups: (A) 8 tests missing second overlap pattern in bundled token; (B) 1 test has spurious intermediate token `aab`; (C) 1 ngrams oracle test inapplicable to non-repeated input; (D) 1 test has premature token creation in second read. Resolution steps R0–R4 defined in analysis doc. |
| T5 | Wire `insert_sequence` → `ReadCtx::read_sequence` (PI-16). Remove `< 2` guard (PI-18). One-line change in `commands/insert.rs`. | PI-16, PI-18 | T4 | ✅ **Done** — `insert_sequence` delegates fully to `ReadCtx::read_sequence`. `< 2` guard removed. `already_existed` derived from `pre_vertex_count` vs `post_vertex_count`. `cli_integration`: **53 pass / 0 fail / 22 ignored**. |
| T6 | Retire `obs1`/`obs2` tests. Run full suite. Record new pass/fail counts against T0 baseline. | PI-29 | T5 | ⏳ **Pending** — blocked on T4 completion (11 overlap failures in `context-read`). |
| T7 | Schedule test review session for any still-failing `skill3_exp_*` tests. | PI-29 | T6 | ⏳ **Pending** |

#### Current Failing Tests (T4 scope)

All 11 remaining `context-read` unit test failures require correct overlap collapse output.
They are the acceptance criteria for completing T4:

| Test | Category |
|------|----------|
| `tests::linear::repetition_aabbaabb` | Non-overlapping repetition (multi-level compound reuse) |
| `tests::overlapping::repetition_abcabcabc` | 3× repeat overlap |
| `tests::overlapping::repetition_xyzxyzxyz` | 3× repeat overlap |
| `tests::overlapping::complex_abcabababcaba` | Complex multi-overlap |
| `tests::read::read_infix1` | Infix overlap (`subdivision` / `visualization`) |
| `tests::read::read_infix2` | Infix overlap (`subvisu` / `visub`) |
| `tests::read::read_multiple_overlaps1` | 5-sequence progressive overlap build |
| `tests::read::sync_read_text2` | Second read rewrites existing root overlap |
| `tests::ngrams_validation::validate_three_repeated` | ngrams oracle — 3× repeat |
| `tests::ngrams_validation::validate_triple_repeat` | ngrams oracle — triple |
| `tests::ngrams_validation::validate_mixed_pattern` | ngrams oracle — mixed |

### Lessons Learned

- **`flat_root` flag was not in the original plan but proved essential.** The `commit_state` dispatch table assumed a clean separation between the unknown-atom append path and the known-atom `BandState` commit path. In practice, the root built by `append_pattern`/`append_token` (an in-place-extendable flat container) must not be wrapped via `insert_pattern` when the first known-atom `BandState::Single` arrives — doing so creates redundant intermediate compound tokens. Introducing `flat_root: bool` to track whether the current root is still a mutable flat container solved this cleanly without changing the public API.
- **`BlockExpansionCtx::process` must refresh the anchor explicitly.** `ExpansionCtx` holds its own `anchor` field, but `commit_state` updates `anchor` on `RootManager`. Without an explicit `self.ctx.anchor = self.root.anchor()` call after each `commit_state`, the cursor loop sees stale anchor values and misses overlap opportunities on the second and subsequent steps. This is OQ-5 in concrete form.
- **T4/T5 order inversion was safe.** `insert_sequence` was wired to `ReadCtx::read_sequence` (T5) before T4 was fully complete. This was safe because the RC-1 cli_integration failures were entirely driven by the missing delegation — the overlap logic failures in T4 only surface in `context-read` unit tests, not in the cli_integration suite at its current test coverage level. The inversion unblocked RC-1 pass/fail reporting early.