---
tags: `#plan` `#context-read` `#aaa` `#decomposition` `#overlap` `#expansion` `#root-manager` `#segmentation`
summary: Implementation plan for fixing the `aaa` decomposition failure. The root cause is atom-anchor suppression in `ExpansionCtx` — the current code skips overlap probing when the anchor is an atom, which prevents the symmetric `[aa, a]` decomposition from ever being materialised. The fix must generalise to all repeated-pattern symmetry cases.
status: 📋 ready
date: 2026-03-17
related_interview: ../interviews/20260317_INTERVIEW_AAA_DECOMPOSITION_NEXT_STEP.md
related_analysis: ../analysis/20260317_ANALYSIS_AAA_SEGMENT_BOUNDARY_AND_OUTER_LOOP.md
related_analysis_2: ../analysis/20260315_ANALYSIS_AAA_DECOMPOSITION_NEXT_STEP.md
blocking: `repetition_aaa_both_decompositions` test in `src/tests/linear.rs`
---

# Plan: `aaa` Decomposition Fix

**Date:** 2026-03-17
**Scope:** fix the missing `[aa, a]` decomposition for `aaa` and generalise the fix to all repeated-pattern symmetry cases
**Primary crate:** `context-read`
**Secondary crate:** none — no changes outside `context-read` are expected

---

## Objective

After reading `aaa` from an empty graph, the graph must contain:

```
aa  => [[a, a]]
aaa => [[a, aa], [aa, a]]
```

Currently only `[a, aa]` is produced. The fix must also generalise naturally to:

- `aaaa`
- `abab`, `ababa`, `ababab`
- `abcabcabc`, `xyzxyzxyz`

without any case-specific logic.

---

## Background

### Semantic invariant

The root is always an eventually-consistent hypergraph node. After each commit it must satisfy:

> the root represents the composed string with a minimal child neighbourhood but full reachability of all existing tokens that represent any part of that string.

This means every valid binary adjacent decomposition of the current root — expressible in terms of already-known tokens — must be present as a child pattern of the root.

### Why segmentation is not the problem

Segmentation is a performance shortcut. For `aaa` on an empty graph:

1. `a` is unknown → inserted as atom, root = `a`, anchor = `a`
2. `aa` is now known → processed as a known segment by `BlockExpansionCtx`
3. The segment boundary must preserve `root = a`, `anchor = a` as left context

Segmentation itself is correct. The boundary must not erase semantic left context.

### Why both decompositions are expected

Once `aa` exists during the read, the width-3 token `aaa` has two valid adjacent binary decompositions:

- `[a, aa]` — left atom + right known compound
- `[aa, a]` — left known compound + right atom

Both arise naturally from the overlap bundling step applied when root `aa` is extended by the trailing `a`. No explicit alternate-branch buffering is needed — only correct overlap detection and root update.

---

## Root cause

### The atom-anchor suppression guards

`ExpansionCtx::next()` in `src/expansion/mod.rs` contains two guards that suppress overlap probing when the anchor is an atom:

**Guard 1 — multi-token remaining path:**

```rust
// Atom anchors have no true postfixes, so skip overlap detection.
let anchor_is_atom =
    self.anchor.map(|a| *a.width() == 1).unwrap_or(false);

// ...

if !anchor_is_atom {
    if let Some(anchor) = self.anchor {
        if let Some((postfix, t2, next_cursor)) =
            self.find_overlap(anchor, t1, self.cursor)
        { ... }
    }
}
```

**Guard 2 — single remaining token path:**

```rust
let anchor_is_atom =
    self.anchor.map(|a| *a.width() == 1).unwrap_or(true);
if !anchor_is_atom {
    if let Some(anchor) = self.anchor {
        if let Some((postfix, t2, next_cursor)) =
            self.find_overlap(anchor, token, self.cursor)
        { ... }
    }
}
```

Both guards state the same assumption: *atom anchors have no true postfixes, so overlap detection is unnecessary.*

### Why this assumption is wrong for `aaa`

These two statements are not equivalent:

1. an atom has no nontrivial postfix *tree* — **true**
2. an atom is irrelevant as left context for the next completion step — **false**

For `aaa`, after the unknown segment, the anchor is the atom `a`. The overlap step should detect:

- root/anchor = `a`
- incoming token `t1` = `aa`
- postfix of anchor = `a` itself (an atom *is* its own postfix)
- `insert_next_match([a, a])` = `aa`, width > postfix width `1` → overlap confirmed
- yields `BandState::WithOverlap { primary: [_, aa], overlap: [_, aa] }` with complement = `a`

That overlap state, when collapsed by `bundle_overlap`, would produce `aaa` with both child patterns.

The suppression guards prevent this overlap probe from ever running, so `[aa, a]` is never created.

### How `[a, aa]` is currently produced

The sequential path (no overlap) still runs: the known segment processes `[a, a]` → yields `aa` → `commit_state Single(aa)` → wraps root `a` with `aa` → new root = `[a, aa]`. That is why one decomposition is present.

### Why the fix is in `ExpansionCtx`

Per the settled interview answers:

- `ExpansionCtx` runs on the current root via `find_overlap` before `commit_state` is called
- commit is applied only after overlaps have been searched
- `RootManager` must not contain special symmetry logic; it should remain generic

The fix therefore belongs in `ExpansionCtx::next()`.

---

## Non-goals

- No changes to `RootManager::commit_state` symmetry logic
- No changes to segmentation
- No buffering of multiple candidate states
- No changes outside `context-read`
- No removal of the immediate-commit model

---

## Success criteria

1. `repetition_aaa_both_decompositions` passes
2. all currently passing tests continue to pass
3. the fix does not introduce any atom-specific branching — it must follow from the general overlap detection path
4. the regression matrix (see Phase 1) is green or has documented expected failures with clear follow-up

---

## Execution plan

### Phase 0 — Confirm the hypothesis with a trace

Before changing any logic, add temporary `tracing` instrumentation to confirm the atom-anchor suppression is the actual cause.

#### 0.1 — Add a focused trace test

In `src/tests/linear.rs` (or a new `src/tests/aaa_trace.rs`), add a test that:

- reads `aaa`
- logs with `LOG_FILTER=trace`
- confirms via the log file in `target/test-logs/`:
  1. exact segment split (`unknown = [a]`, `known = [a, a]`)
  2. initial anchor passed into `BlockExpansionCtx` (expected: `a`)
  3. initial anchor inside `ExpansionCtx` (expected: `a`)
  4. whether `anchor_is_atom` is `true` at the multi-token guard
  5. whether `find_overlap` is called or skipped
  6. each `BandState` yielded
  7. each `commit_state` branch taken
  8. final graph patterns for `aa` and `aaa`

This test can use `assert_patterns!` as its final assertion and is expected to **fail** before the fix.

#### 0.2 — Record findings

Annotate the trace test or a comment block with the observed values from step 0.1 to confirm:

- `anchor_is_atom = true` at the guard → overlap skipped → `[aa, a]` never materialised

Only proceed to Phase 1 once this is confirmed.

---

### Phase 1 — Regression matrix

Before modifying any logic, ensure the full repeated-pattern test matrix exists.

#### 1.1 — Audit existing tests

Confirm which of the following cases already have explicit `assert_patterns!` assertions and which do not:

| Input    | Expected root patterns | Test exists? |
|----------|----------------------|--------------|
| `aa`     | `[[a, a]]`           | check        |
| `aaa`    | `[[a, aa], [aa, a]]` | ✅ `repetition_aaa_both_decompositions` |
| `aaaa`   | `[[aa, aa]]` + inner | check        |
| `abab`   | `[[ab, ab]]`         | check        |
| `ababa`  | `[[ab, aba], [abab, a]]` | check   |
| `ababab` | `[[ab, abab], [ababa, b]]` | check  |

#### 1.2 — Add missing tests

For each missing case, add a test in `src/tests/linear.rs` (simple repeated) or `src/tests/overlapping.rs` (overlap-heavy). These tests are expected to **fail** before the fix and serve as the validation target.

Mark any test that is known to fail before the fix with a comment:

```rust
// Expected to fail before atom-anchor suppression fix.
```

Do **not** use `#[ignore]` — the tests should be visible in the failure output.

---

### Phase 2 — Fix `ExpansionCtx` atom-anchor suppression

This is the core change.

#### 2.1 — Understand what `find_overlap` does with an atom anchor

Before removing the guard, trace through what happens when `find_overlap` is called with an atom anchor:

- `anchor.postfix_iter()` on an atom — does it yield the atom itself as its own postfix?
- if yes: postfix = `a`, postfix_width = 1, overlap_start = `t1_cursor + t1_width - 1`
- query = `atoms[overlap_start..]`
- `insert_next_match(query)` — does this find `aa` when `[a, a]` is known?
- result_width = 2 > postfix_width = 1 → overlap confirmed

If `postfix_iter` on an atom does **not** yield the atom itself, a small adapter may be needed. Verify this by reading `Token::postfix_iter` in `context-trace` before proceeding.

**If `postfix_iter` yields the atom itself:** the fix is simply removing (or relaxing) the two `anchor_is_atom` guards.

**If `postfix_iter` does not yield the atom itself:** the fix requires either:
- a special case that treats the atom anchor as its own postfix in `find_overlap`, or
- a pre-check before calling `find_overlap` when the anchor is an atom

Document which case applies before writing code.

#### 2.2 — Relax or remove the multi-token guard

Current code (Guard 1):

```rust
// Atom anchors have no true postfixes, so skip overlap detection.
let anchor_is_atom =
    self.anchor.map(|a| *a.width() == 1).unwrap_or(false);

// ...

if !anchor_is_atom {
    if let Some(anchor) = self.anchor {
        if let Some((postfix, t2, next_cursor)) =
            self.find_overlap(anchor, t1, self.cursor)
        { ... }
    }
}
```

The `anchor_is_atom` variable and the `if !anchor_is_atom` outer guard should be removed. The overlap probe should always run if an anchor exists.

If `find_overlap` handles the atom-postfix case correctly (see 2.1), no further change to `find_overlap` is needed.

Update the comment to reflect the new semantics:

```rust
// Always probe for an overlap between the current anchor and t1,
// even when the anchor is an atom. An atom is its own postfix and
// can form a valid left context for repeated-pattern completion.
if let Some(anchor) = self.anchor {
    if let Some((postfix, t2, next_cursor)) =
        self.find_overlap(anchor, t1, self.cursor)
    {
        // ...
    }
}
```

#### 2.3 — Relax or remove the single-token guard

Current code (Guard 2):

```rust
let anchor_is_atom =
    self.anchor.map(|a| *a.width() == 1).unwrap_or(true);
if !anchor_is_atom {
    if let Some(anchor) = self.anchor {
        if let Some((postfix, t2, next_cursor)) =
            self.find_overlap(anchor, token, self.cursor)
        { ... }
    }
}
```

Note the different `unwrap_or` default (`true` here vs `false` in Guard 1). This means that when there is no anchor, the single-token guard also suppresses. The semantics should mirror Guard 1 after the fix:

```rust
// Always probe for an overlap on the single trailing token.
if let Some(anchor) = self.anchor {
    if let Some((postfix, t2, next_cursor)) =
        self.find_overlap(anchor, token, self.cursor)
    {
        // ...
    }
}
```

Remove the `anchor_is_atom` variable entirely from both paths.

#### 2.4 — Verify `find_overlap` handles atom postfixes

Open `src/expansion/mod.rs` at `fn find_overlap` and verify:

1. `anchor.postfix_iter(self.graph.clone())` — does it yield anything for an atom?
2. if it yields nothing, add an early-exit atom-postfix path in `find_overlap`:

```rust
// If the anchor is an atom, treat it as its own postfix.
// postfix_width = 1, overlap_start = t1_cursor + t1_width - 1.
let is_atom_anchor = *anchor.width() == 1;
if is_atom_anchor {
    let overlap_start = t1_cursor + t1_width - 1;
    // ... same probe logic as the postfix loop body
}
```

This path is only needed if `postfix_iter` does not yield atoms themselves. Determine this empirically during Phase 0 or by reading `postfix_iter` in `context-trace`.

---

### Phase 3 — Verify and stabilise

#### 3.1 — Run the trace test

Run the Phase 0 trace test after the fix and confirm in the log:

- `anchor_is_atom` no longer suppresses the probe (or the guard no longer exists)
- `find_overlap` is called with the atom anchor
- a `WithOverlap` state is yielded
- `commit_state WithOverlap` is taken
- the graph contains `aaa => [[a, aa], [aa, a]]`

#### 3.2 — Run the `aaa` regression test

```bash
cargo test -p context-read repetition_aaa_both_decompositions -- --nocapture
```

Expected: green.

#### 3.3 — Run the full regression matrix

```bash
cargo test -p context-read -- --nocapture
```

Examine results for each case in the Phase 1 matrix.

For any new failures introduced by relaxing the guard:

- check whether `find_overlap` is now incorrectly matching cases it should not
- check whether the `anchor_is_atom` guard was compensating for a deeper issue in `find_overlap`
- document the failure and decide whether to adjust `find_overlap` or re-introduce a narrower guard

#### 3.4 — Run the full test suite

```bash
cargo test -p context-read
cargo test -p context-insert
cargo test -p context-trace
```

All previously passing tests must remain passing.

---

### Phase 4 — Clean up and document

#### 4.1 — Remove the trace test if temporary

If the Phase 0 trace test was added as a temporary diagnostic (not a proper regression test), remove it or convert it into a clean regression test with proper assertions.

#### 4.2 — Update the analysis document

Update `agents/analysis/20260317_ANALYSIS_AAA_SEGMENT_BOUNDARY_AND_OUTER_LOOP.md`:

- mark the atom-anchor suppression hypothesis as confirmed or revised
- record which guard(s) were changed
- note whether `find_overlap` needed an atom-postfix special path

#### 4.3 — Update this plan

Mark completed phases. If any phase deviated, record the deviation and reason.

#### 4.4 — Add a summary to `agents/implemented/`

After the fix is confirmed green, create:

```
agents/implemented/20260317_IMPLEMENTED_AAA_DECOMPOSITION_FIX.md
```

with:

- root cause summary
- fix location and change description
- test evidence
- generalisability note

Update `agents/implemented/INDEX.md`.

---

## Risks and mitigations

### Risk 1 — `find_overlap` produces false positives for atom anchors

**Likelihood:** medium

**Description:** With the atom-anchor guard removed, `find_overlap` may find spurious overlaps for atom anchors in cases where the current tests pass because that guard was also accidentally preventing incorrect behaviour.

**Mitigation:** run the full test suite after Phase 2. If new failures appear, inspect `find_overlap` for the false-positive path and add a narrower predicate inside `find_overlap` rather than restoring the broad `anchor_is_atom` guard.

### Risk 2 — `postfix_iter` does not yield atoms

**Likelihood:** medium — atom postfix iteration may be a no-op by design in `context-trace`.

**Description:** If `postfix_iter` on an atom yields nothing, removing the guard alone will have no effect. The overlap probe will run but immediately return `None` with no postfixes to iterate.

**Mitigation:** Phase 2.4 handles this explicitly. If `postfix_iter` does not yield atoms, add the early-exit atom-postfix path in `find_overlap` before running the regression tests.

### Risk 3 — The fix passes `aaa` but breaks a larger case

**Likelihood:** low, but non-zero

**Description:** `ababab` and `abcabcabc` involve multi-step overlap chains. Relaxing the atom-anchor guard for single-atom repeated patterns might interact unexpectedly with multi-step overlap detection.

**Mitigation:** the Phase 1 regression matrix covers `ababab`. If this case regresses, investigate whether the commit model needs to be re-examined after confirming the anchor semantics are correct.

### Risk 4 — `build_overlap_state` does not handle atom anchors correctly

**Likelihood:** medium

**Description:** `build_overlap_state` builds `root_postfix` by iterating `anchor.postfix_iter` and traversing until it finds `postfix`. For an atom anchor where postfix = anchor = `a`, this iteration may produce an incorrect or empty path.

**Mitigation:** If `build_overlap_state` is reached with an atom anchor during Phase 3 testing and produces incorrect structure, inspect the `root_postfix` construction. The path from an atom to itself is trivially the identity; an `IndexEndPath` with a single root-level `ChildLocation` pointing at the atom should suffice.

---

## Execution order summary

```
Phase 0 — trace and confirm hypothesis
  └─ 0.1  add trace test
  └─ 0.2  record findings, confirm atom-anchor suppression

Phase 1 — regression matrix
  └─ 1.1  audit existing tests
  └─ 1.2  add missing matrix tests

Phase 2 — fix ExpansionCtx
  └─ 2.1  verify postfix_iter behaviour on atoms
  └─ 2.2  remove/relax multi-token guard
  └─ 2.3  remove/relax single-token guard
  └─ 2.4  patch find_overlap for atom postfix if needed

Phase 3 — verify and stabilise
  └─ 3.1  run trace test, inspect log
  └─ 3.2  run aaa regression test
  └─ 3.3  run full matrix
  └─ 3.4  run full suite

Phase 4 — clean up
  └─ 4.1  finalise trace test
  └─ 4.2  update analysis doc
  └─ 4.3  update this plan
  └─ 4.4  write implemented summary
```

---

## Key files

| File | Role |
|------|------|
| `src/expansion/mod.rs` | contains `ExpansionCtx::next()`, `find_overlap`, `build_overlap_state` — primary change location |
| `src/expansion/block.rs` | drives the expansion loop — read-only during this fix |
| `src/pipeline/root.rs` | `RootManager::commit_state` — read-only during this fix |
| `src/tests/linear.rs` | contains `repetition_aaa_both_decompositions` — primary regression target |
| `src/tests/overlapping.rs` | contains larger overlap regression tests — must remain green |
| `src/tests/cursor.rs` | contains `cursor_repeated_atoms_*` tests — must remain green |