---
tags: `#plan` `#testing` `#integration` `#context-api` `#context-read` `#context-insert` `#bug-fix` `#refactoring`
summary: Future work session plan for fixing the 9 failing integration tests across 3 root causes (RC-1 missing outer loop, RC-2 read-after-insert, RC-3 cursor advancement), structured as three prioritised fix rounds with acceptance criteria tied directly to the test suite.
status: 📋 ready
phase: 1-ready
parent: 20260314_PLAN_INTEGRATION_TESTS.md
related: 20260314_PLAN_INSERT_NEXT_MATCH.md, 20260314_PLAN_APPEND_TO_PATTERN_FIX.md, 20260314_PLAN_CONTEXT_READ_UX_IMPROVEMENT.md, 20260315_PLAN_NGRAMS_ORACLE_VALIDATION.md
priority: top — unblocks 20 ignored oracle/skill3 tests and 9 active failures
---

# Plan: Integration Test Remediation — RC-1 / RC-2 / RC-3 Fix Rounds

**Date:** 2026-03-15
**Scope:** Medium–Large (3 root causes, cross-crate: `context-api`, `context-insert`, `context-read`)
**Crates:** `context-api`, `context-insert`, `context-read`
**Test harness:** `tools/context-cli/tests/cli_integration.rs`

---

## Table of Contents

1. [Objective](#objective)
2. [Current Test Snapshot](#current-test-snapshot)
3. [Root Cause Summary](#root-cause-summary)
4. [Fix Round 1 — RC-1: `insert_sequence` Outer Loop](#fix-round-1--rc-1-insert_sequence-outer-loop)
5. [Fix Round 2 — RC-2: `read_sequence` After Insert](#fix-round-2--rc-2-read_sequence-after-insert)
6. [Fix Round 3 — RC-3: Repeated-Character Cursor Advancement](#fix-round-3--rc-3-repeated-character-cursor-advancement)
7. [Obs-Test Retirement](#obs-test-retirement)
8. [Execution Order](#execution-order)
9. [Acceptance Criteria](#acceptance-criteria)
10. [Risks & Mitigations](#risks--mitigations)
11. [Related Documents](#related-documents)
12. [Notes](#notes)

---

## Objective

The integration test suite (`tools/context-cli/tests/`) currently reports:

```
test result: FAILED. 44 passed; 9 failed; 22 ignored; 0 measured; 0 filtered out
```

This plan documents the three fix rounds needed to move those numbers to:

```
test result: ok. 55+ passed; 0 failed; 2 ignored; 0 measured; 0 filtered out
```

The two remaining ignored tests (`oracle_aa`, `skill3_exp_m_repeated_char_known_failing`) require
RC-3 (cursor advancement redesign), which is architecturally deeper and carries higher risk. They
are kept ignored after Rounds 1 and 2 until Round 3 is complete.

Each round is self-contained: it has a defined entry point in the codebase, a minimal change set,
and a concrete verification command. Rounds **must be executed in order** because RC-2 may resolve
as a side effect of RC-1, and RC-3 is independent of both.

---

## Current Test Snapshot

> Captured: 2026-03-15 — `cargo test -p context-cli --test cli_integration`

| Category | Total | ✅ Pass | ❌ Fail | ⏭️ Ignored |
|----------|-------|---------|---------|-----------|
| Common — graph_compare machinery | 9 | 9 | 0 | 0 |
| Cat 1 — Atom Management | 6 | 6 | 0 | 0 |
| Cat 2 — Basic Read | 8 | 4 | 4 | 0 |
| Cat 3 — Deduplication | 8 | 5 | 3 | 0 |
| Cat 4 — File Input | 4 | 3 | 1 | 0 |
| Cat 5 — REPL Integration | — | — | — | deferred |
| Cat 6 — Edge Cases | 6 | 5 | 1 | 0 |
| Skill3 exploration (green today) | 8 | 8 | 0 | 0 |
| Skill3 exploration (RC-1 blocked) | 9 | 0 | 0 | 9 |
| Skill3 exploration (RC-3 blocked) | 1 | 0 | 0 | 1 |
| Ngrams oracle (RC-1 blocked) | 11 | 0 | 0 | 11 |
| Ngrams oracle (RC-3 blocked) | 1 | 0 | 0 | 1 |
| Ngrams oracle — self-check | 4 | 4 | 0 | 0 |
| **Total** | **75** | **44** | **9** | **22** |

### Failing Tests

| Test | Root Cause |
|------|-----------|
| `integration::basic_read_tests::read_known_pattern` | RC-1 |
| `integration::basic_read_tests::read_produces_decomposition_tree` | RC-1 |
| `integration::basic_read_tests::read_text_output` | RC-1 |
| `integration::basic_read_tests::read_sequence_after_insert` | RC-2 |
| `integration::dedup_tests::dedup_insert_then_read` | RC-1 |
| `integration::dedup_tests::dedup_read_sequence_finds_existing` | RC-2 |
| `integration::dedup_tests::dedup_multiple_reads_same_result` | RC-2 |
| `integration::edge_case_tests::edge_repeated_single_char` | RC-3 |
| `integration::file_input_tests::file_read_basic` | RC-1 |

---

## Root Cause Summary

### RC-1 — `insert_sequence` Outer Loop Missing

**Location:** `crates/context-stack/context-api/src/workspace/manager.rs` — `WorkspaceManager::insert_sequence`

**Mechanism:** `insert_sequence` converts the input text to atom tokens and then calls
`insert_next_match([atom₀, atom₁, …, atomₙ])` **once**. On a fresh graph the first call
returns `InsertOutcome::NoExpansion` for `atom₀` (the first character) — meaning "matched
one atom, cursor can advance; the query extends further." Because `insert_sequence` does not
loop, it returns the first atom as the root token with `already_existed=true` and `width=1`
for **every** multi-character string.

**Impact:** 5 direct test failures + 20 ignored tests blocked (9 `skill3_exp_*`, 11 `oracle_*`).
Also manifests as `file_read_basic` returning "helo wrd" instead of "hello world" because the
`ReadFile` → `read_sequence` path shares the same missing accumulation loop.

**Fix:** Add an outer accumulation loop to `insert_sequence` that repeatedly calls
`insert_next_match` with the remaining atom slice, collects the returned segment tokens, and
finally wraps them into a compound root token. The loop is already implemented inside `ReadCtx`
(the read pipeline) — it needs to be extracted or replicated in the insert path.

---

### RC-2 — `ReadCtx::read_sequence` Returns `None` After Insert

**Location:** `crates/context-stack/context-read/src/context/mod.rs` (or `read.rs`) — `ReadCtx::read_sequence`

**Mechanism:** After `insert_sequence` writes atoms and a (broken, width=1) token into the graph,
a subsequent call to `ReadCtx::read_sequence` on the same text returns `None`. The API layer
converts this into `ApiError::InternalError("read_sequence returned None for text of length N")`.

The exact interaction is not fully traced, but the hypothesis is that `ReadCtx` builds its atom
index at construction time and the atom/pattern indices written by `insert_sequence` interfere
with the read traversal path (the graph has changed shape since `ReadCtx` was initialised).

**Impact:** 3 direct test failures. May resolve automatically after RC-1 because the graph state
after a *correct* insert will differ significantly from the current broken state.

**Fix:** Re-evaluate after RC-1. If still broken, add a re-initialisation step to `ReadCtx` (or
make atom lookup lazy) so that it sees the current graph state on each call.

---

### RC-3 — Repeated-Character Cursor Advancement

**Location:** `crates/context-stack/context-read/src/context/` — cursor advancement / `append_to_pattern`
interaction for all-same-character or repeated-pattern inputs.

**Mechanism:** When `read_sequence` (or the future looping `insert_sequence`) encounters a
repeated character (e.g. `"aaaa"`), the cursor advancement logic fails to advance past the first
match. The pattern accumulator gets stuck at position 0 and either:
- Returns width=1 (only the first atom), or
- Panics in the `append_to_pattern` / vertex-mutation path.

This is the pre-existing overlap/cursor bug documented in `20260218_PLAN_CONTEXT_READ_COMPLETION.md`
and partially addressed by `PLAN_APPEND_TO_PATTERN_FIX.md` (the `extend_root_pattern` /
`append_to_owned_pattern` split). The cursor re-use after overlap has not yet been corrected.

**Impact:** 1 direct test failure (`edge_repeated_single_char`) + 2 ignored tests (`oracle_aa`,
`skill3_exp_m_repeated_char_known_failing`). All oracle tests with repeated substrings will also
fail this case once RC-1 is unblocked (though they are currently blocked by RC-1, not RC-3).

**Fix:** Cursor advancement redesign in `context-read` — separate plan.
Blocked until RC-1 is resolved so the outer loop is in place to exercise the cursor logic.

---

## Fix Round 1 — RC-1: `insert_sequence` Outer Loop

### Goal

After this round:
- 5 failing tests turn green (RC-1 failures)
- 20 ignored tests unblock and turn green (`skill3_exp_*` + `oracle_*`)
- `file_read_basic` turns green
- Total score improves from **44/9/22** to **~64/3/2**

### Entry Point

```
crates/context-stack/context-api/src/workspace/manager.rs
```

Find the method `insert_sequence` (or the equivalent function that:
1. Converts text to atom tokens via `get_or_create_atoms`
2. Calls `insert_next_match` once
3. Returns an `InsertResult` / `CommandResult::InsertResult`

### Change Description

**Current (broken) logic (pseudocode):**

```rust
fn insert_sequence(&mut self, workspace: &str, text: &str) -> Result<CommandResult, ApiError> {
    let ws = self.get_open_workspace_mut(workspace)?;
    let atoms = ws.get_or_create_atoms(text);        // Vec<Token>
    let outcome = ws.insert_next_match(&atoms)?;     // called ONCE
    let token = outcome.into_token();
    Ok(CommandResult::InsertResult(InsertResult {
        token,
        already_existed: !outcome.is_created(),
    }))
}
```

**Desired (fixed) logic (pseudocode):**

```rust
fn insert_sequence(&mut self, workspace: &str, text: &str) -> Result<CommandResult, ApiError> {
    let ws = self.get_open_workspace_mut(workspace)?;
    let atoms: Vec<Token> = ws.get_or_create_atoms(text);

    // Outer accumulation loop — mirrors ReadCtx segment iteration
    let mut cursor = 0usize;
    let mut segments: Vec<Token> = Vec::new();

    while cursor < atoms.len() {
        let remaining = &atoms[cursor..];
        let outcome = ws.insert_next_match(remaining)?;
        let width = outcome.token().width;
        segments.push(outcome.into_token());
        cursor += width;
    }

    // If the full input is a single segment, return it directly.
    // Otherwise wrap the segments into a compound root token.
    let root_token = if segments.len() == 1 {
        segments.remove(0)
    } else {
        ws.insert_or_get_complete_from_parts(&segments)?
    };

    let already_existed = /* based on first outcome */ ...;
    Ok(CommandResult::InsertResult(InsertResult {
        token: root_token,
        already_existed,
    }))
}
```

> **Note:** The exact API names (`insert_next_match`, `into_token`, `insert_or_get_complete_from_parts`)
> may differ from the actual code. The key invariant is: **the loop advances `cursor` by
> `outcome.token().width` after each call and stops when `cursor == atoms.len()`.**

### Investigation Steps

Before writing the fix, answer these questions by reading the source:

1. **Where exactly is `insert_sequence` called in `WorkspaceManager`?**
   - `grep -r "insert_sequence\|InsertSequence" crates/context-stack/context-api/src/`
   - Note the actual function signature and which workspace method it delegates to.

2. **What does `insert_next_match` return today?**
   - Read `crates/context-stack/context-insert/src/insert/outcome.rs` for the `InsertOutcome` enum.
   - Confirm: `Created`, `Complete`, `NoExpansion` — what does each carry? What is `.width`?

3. **How does `ReadCtx` implement the outer loop?**
   - Read `crates/context-stack/context-read/src/context/` — find `SegmentIter` or the equivalent iterator.
   - The fix should replicate exactly this logic in `insert_sequence`.

4. **What wraps segments into a compound root?**
   - Read how `ReadCtx` commits a completed read — is there a `finalize`, `commit`, or
     `insert_root` step? This is the "wrap segments into compound root" step.
   - If no such function exists, it may need to be added to `context-insert`.

5. **How is `already_existed` determined for multi-segment results?**
   - Convention: `already_existed = true` iff ALL segments are `Complete` (nothing new was written).
   - If any segment is `Created`, the overall result is "new content added."

### Files to Read First

| File | Why |
|------|-----|
| `crates/context-stack/context-api/src/workspace/manager.rs` | Entry point — find `insert_sequence` |
| `crates/context-stack/context-api/src/commands/mod.rs` | `Command::InsertSequence` dispatch |
| `crates/context-stack/context-insert/src/insert/outcome.rs` | `InsertOutcome` enum — width, token |
| `crates/context-stack/context-insert/src/insert/mod.rs` | `insert_next_match` trait method |
| `crates/context-stack/context-read/src/context/mod.rs` | Outer loop reference implementation |
| `crates/context-stack/context-read/src/context/segment.rs` | `SegmentIter` — cursor advancement logic |

### Verification Commands

```sh
# After applying the fix:

# 1. Compile check
cargo check -p context-api 2>&1

# 2. Run RC-1 failing tests (should all pass)
cargo test -p context-cli --test cli_integration \
  read_known_pattern read_produces_decomposition_tree read_text_output \
  dedup_insert_then_read file_read_basic \
  -- --exact 2>&1

# 3. Run RC-1 ignored skill3 tests (should all pass when included)
cargo test -p context-cli --test cli_integration skill3_exp_ \
  -- --include-ignored 2>&1

# 4. Run RC-1 ignored oracle tests (should all pass when included)
cargo test -p context-cli --test cli_integration oracle_ \
  -- --include-ignored 2>&1

# 5. Full suite — confirm no regressions
cargo test -p context-cli --test cli_integration 2>&1 | tail -5

# 6. Run all context-api tests — confirm no regressions
cargo test -p context-api 2>&1 | tail -5
```

### Expected Outcome

```
test result: ok. 64 passed; 3 failed; 2 ignored; 0 measured; 0 filtered out
```

(3 remaining failures = RC-2; 2 ignored = RC-3)

---

## Fix Round 2 — RC-2: `read_sequence` After Insert

### Goal

After this round:
- 3 failing tests turn green (RC-2 failures)
- Total score improves from **~64/3/2** to **~67/0/2**

### Precondition

**Must complete Round 1 (RC-1) first.** RC-2 may resolve automatically after RC-1 because the
graph state written by a correct insert differs from the current broken state. Run the three RC-2
tests after Round 1 before investing effort in a separate Round 2 fix:

```sh
cargo test -p context-cli --test cli_integration \
  read_sequence_after_insert \
  dedup_read_sequence_finds_existing \
  dedup_multiple_reads_same_result \
  -- --exact 2>&1
```

If all three pass → Round 2 is already done. Skip to Round 3.

### Entry Point (if still failing after RC-1)

```
crates/context-stack/context-read/src/context/mod.rs   (or read.rs — wherever read_sequence lives)
```

### Investigation Steps

1. **Reproduce the failure in isolation:**

   ```sh
   cargo test -p context-cli --test cli_integration read_sequence_after_insert \
     -- --exact --nocapture 2>&1
   ```

   Note the exact error: `"read_sequence returned None for text of length N"`.

2. **Trace where `None` is returned:**
   - Find the call site in `context-api` that calls `ReadCtx::read_sequence`.
   - Add a `dbg!` or `eprintln!` before the call to print the graph state (vertex count,
     atom list) to confirm that prior insert has modified the graph.
   - Step into `ReadCtx::read_sequence` and find the first path that returns `None`.

3. **Identify the stale-state interaction:**
   - `ReadCtx` is likely created fresh for each `ReadSequence` command via something like
     `ReadCtx::new(&workspace.graph, chars)`.
   - The graph has been modified by `insert_sequence` since the last `ReadCtx` was created.
   - Hypothesis: `ReadCtx::new` caches atom indices or pattern IDs at construction time;
     these are now stale because new vertices have been added.
   - Alternative hypothesis: the outer loop (after RC-1 fix) advances the cursor but the
     `read_sequence` path has a different cursor logic that breaks on the new graph topology.

4. **Determine the minimal fix:**
   - If the issue is a stale atom cache: make atom lookup lazy (look up the atom index from
     the graph on demand rather than caching at construction time).
   - If the issue is a different cursor logic: align `read_sequence` with the new outer loop
     from Round 1 so both paths share the same cursor advancement contract.

### Files to Read First

| File | Why |
|------|-----|
| `crates/context-stack/context-api/src/workspace/manager.rs` | `read_sequence` dispatch → `ReadCtx` creation |
| `crates/context-stack/context-read/src/context/mod.rs` | `ReadCtx::new`, `read_sequence` return paths |
| `crates/context-stack/context-read/src/context/atom.rs` | Atom resolution / caching |
| `crates/context-stack/context-read/src/context/segment.rs` | Cursor state and advancement |

### Verification Commands

```sh
# After applying the fix (or confirming auto-resolution after RC-1):

cargo test -p context-cli --test cli_integration \
  read_sequence_after_insert \
  dedup_read_sequence_finds_existing \
  dedup_multiple_reads_same_result \
  -- --exact 2>&1

# Full suite — confirm no regressions
cargo test -p context-cli --test cli_integration 2>&1 | tail -5
cargo test -p context-read 2>&1 | tail -5
```

### Expected Outcome

```
test result: ok. 67 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out
```

---

## Fix Round 3 — RC-3: Repeated-Character Cursor Advancement

### Goal

After this round:
- 1 failing test turns green (`edge_repeated_single_char`)
- 2 ignored tests turn green (`oracle_aa`, `skill3_exp_m_repeated_char_known_failing`)
- Total score improves from **~67/0/2** to **~70/0/0**

### Precondition

**Must complete Rounds 1 and 2 first.** The outer loop from Round 1 must be in place before
RC-3 can be properly exercised — without the loop, all repeated-pattern inputs fail at the
first iteration for unrelated reasons.

### Background

The `append_to_pattern` split (`PLAN_APPEND_TO_PATTERN_FIX.md`, now complete) addressed the
vertex-corruption risk in `extend_root_pattern` vs. `append_to_owned_pattern`. However, the
cursor advancement for overlap/repeat sequences was not fixed in that plan — it was deferred.

When the outer loop processes `"aaaa"`, the sequence of calls is approximately:

```
insert_next_match([a, a, a, a])   → outcome: NoExpansion, width=1 (atom 'a')
insert_next_match([a, a, a])      → outcome: NoExpansion, width=1 (atom 'a') — cursor: +1
insert_next_match([a, a])         → outcome: NoExpansion, width=1 (atom 'a') — cursor: +1
insert_next_match([a])            → outcome: Complete,    width=1 (atom 'a') — cursor: +1
```

After 4 calls, `cursor = 4 = atoms.len()`. The segments are `[a, a, a, a]`. The compound root
should be `[a, a, a, a]` with width 4.

However, somewhere in this path, the cursor does not advance. The exact bug location is:
- Either in `insert_next_match` itself (returns width=0 or the wrong token), or
- In the overlap/repeat detection in `context-read`'s `SegmentIter` / cursor logic, or
- In the `extend_root_pattern` call that tries to build the compound token from 4 identical
  children, causing an infinite loop or width miscalculation.

### Investigation Steps

1. **Add instrumentation to the outer loop (after Round 1):**

   In the new outer loop in `insert_sequence`, add temporary `eprintln!` calls:

   ```rust
   while cursor < atoms.len() {
       let remaining = &atoms[cursor..];
       eprintln!("RC-3 debug: cursor={cursor}, remaining.len()={}", remaining.len());
       let outcome = ws.insert_next_match(remaining)?;
       eprintln!("RC-3 debug: outcome width={}, kind={:?}", outcome.token().width, outcome.kind());
       // ... advance cursor
   }
   ```

   Run `edge_repeated_single_char` with `--nocapture` and observe where the loop gets stuck.

2. **Check `insert_next_match` for width=0:**
   - If `outcome.token().width == 0`, the loop will loop forever. Add a guard:
     ```rust
     let width = outcome.token().width;
     assert!(width > 0, "insert_next_match returned zero-width token at cursor={cursor}");
     cursor += width;
     ```
   - A panic with this message immediately identifies whether width=0 is the issue.

3. **Check the compound root construction for identical children:**
   - After the loop: `segments = [a, a, a, a]` — four token references to the same atom vertex.
   - `insert_or_get_complete_from_parts([a, a, a, a])` must produce a width-4 compound token.
   - If the compound insertion mutates the atom vertex's width (the `append_to_pattern` bug),
     this is where the width corruption occurs.
   - Verify that `extend_root_pattern` (from `PLAN_APPEND_TO_PATTERN_FIX`) is being used, not
     the deprecated `append_to_pattern`.

4. **Consult `context-read` crate tests:**
   - Run `cargo test -p context-read -- --nocapture 2>&1 | grep -E "(FAILED|ok)"` and
     note which tests involve repeated characters.
   - The ~29/60 failing tests in `context-read` include the repeat/overlap cases.
     Fix must not regress these.

5. **Consult `PLAN_CONTEXT_READ_COMPLETION.md`** for the existing analysis of the cursor
   advancement and `append_to_pattern` issues — do not duplicate work already planned there.

### Files to Read First

| File | Why |
|------|-----|
| `crates/context-stack/context-read/src/context/segment.rs` | Cursor advancement, overlap detection |
| `crates/context-stack/context-read/src/context/root.rs` | `RootManager` — uses `append_to_owned_pattern` |
| `crates/context-stack/context-trace/src/graph/insert/parents.rs` | `extend_root_pattern` implementation |
| `crates/context-stack/context-insert/src/insert/context.rs` | `insert_next_match` — width returned |
| `agents/plans/20260218_PLAN_CONTEXT_READ_COMPLETION.md` | Existing analysis of repeat/overlap |

### Verification Commands

```sh
# After applying the fix:

# 1. RC-3 direct failure
cargo test -p context-cli --test cli_integration edge_repeated_single_char \
  -- --exact --nocapture 2>&1

# 2. RC-3 ignored tests (should now pass)
cargo test -p context-cli --test cli_integration \
  skill3_exp_m oracle_aa \
  -- --include-ignored --exact 2>&1

# 3. Full suite — confirm all 70 pass, 0 fail, 0 ignored
cargo test -p context-cli --test cli_integration \
  -- --include-ignored 2>&1 | tail -5

# 4. context-read unit tests — confirm no regressions
cargo test -p context-read 2>&1 | tail -10
```

### Expected Outcome

```
test result: ok. 70 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

---

## Obs-Test Retirement

The `skill3_obs*` tests in `skill3_exploration.rs` document the **current broken behaviour**
(RC-1 present) to prevent silent regressions. Once RC-1 is fixed, these observations become
false (they assert that `insert_sequence` returns `already_existed=true` and `width=1`).

After Round 1 is complete and green, retire each `obs` test:

| Test | Action |
|------|--------|
| `skill3_obs1_multi_char_insert_reports_already_existed_true_today` | Delete — now incorrect |
| `skill3_obs2_same_text_twice_same_atom_index_today` | Delete — now incorrect |
| `skill3_obs3_atoms_correctly_auto_created_and_deduped` | Keep — still correct |
| `skill3_obs4_cross_word_atom_sharing` | Keep — still correct |
| `skill3_obs5_graph_valid_under_rc1_conditions` | Rename to `skill3_obs5_graph_valid_after_inserts` and keep |

Also remove the `#[ignore = "RC-1: …"]` attributes from all `skill3_exp_*` tests once Round 1
is verified.

**Do this AFTER Round 1 verification passes**, not before — the `obs` tests serve as regression
guards during the Round 1 implementation work.

---

## Execution Order

```
Round 1 (RC-1)
  ├── Step 1: Read manager.rs + outcome.rs + segment.rs
  ├── Step 2: Implement outer loop in insert_sequence
  ├── Step 3: Verify: 5 failing → green; 20 ignored → green
  ├── Step 4: Run full suite — confirm 44→64 pass, 9→3 fail, 22→2 ignored
  └── Step 5: Retire obs1, obs2; remove #[ignore] from skill3_exp_*

Round 2 (RC-2) — only if still failing after Round 1
  ├── Step 1: Run 3 RC-2 tests — check if they self-resolve
  │   ├── [RESOLVED] → skip Round 2, proceed to Round 3
  │   └── [STILL FAILING] → continue
  ├── Step 2: Trace None path in ReadCtx::read_sequence
  ├── Step 3: Apply minimal fix (lazy atom lookup or cursor alignment)
  ├── Step 4: Verify: 3 failing → green
  └── Step 5: Run full suite — confirm 64→67 pass, 3→0 fail

Round 3 (RC-3) — requires Rounds 1 & 2 complete
  ├── Step 1: Read segment.rs + root.rs + outcome.rs
  ├── Step 2: Add width>0 guard + eprintln instrumentation
  ├── Step 3: Run edge_repeated_single_char with --nocapture
  ├── Step 4: Identify exact failure mode (width=0, infinite loop, corrupt width)
  ├── Step 5: Apply cursor advancement fix
  ├── Step 6: Verify: 1 failing + 2 ignored → green
  └── Step 7: Run full suite — confirm 0 failures, 0 ignored
```

---

## Acceptance Criteria

### Round 1 Complete

- [ ] `cargo test -p context-cli --test cli_integration read_known_pattern` → `ok`
- [ ] `cargo test -p context-cli --test cli_integration read_produces_decomposition_tree` → `ok`
- [ ] `cargo test -p context-cli --test cli_integration read_text_output` → `ok`
- [ ] `cargo test -p context-cli --test cli_integration dedup_insert_then_read` → `ok`
- [ ] `cargo test -p context-cli --test cli_integration file_read_basic` → `ok`
- [ ] `cargo test -p context-cli --test cli_integration skill3_exp_ -- --include-ignored` → all `ok`
- [ ] `cargo test -p context-cli --test cli_integration oracle_ -- --include-ignored` → all `ok` (excluding `oracle_aa`)
- [ ] `cargo test -p context-api` → no new failures
- [ ] `cargo test -p context-insert` → no new failures
- [ ] Full suite summary: **~64 passed, ≤3 failed, 2 ignored**

### Round 2 Complete

- [ ] `cargo test -p context-cli --test cli_integration read_sequence_after_insert` → `ok`
- [ ] `cargo test -p context-cli --test cli_integration dedup_read_sequence_finds_existing` → `ok`
- [ ] `cargo test -p context-cli --test cli_integration dedup_multiple_reads_same_result` → `ok`
- [ ] `cargo test -p context-read` → no new failures
- [ ] Full suite summary: **~67 passed, 0 failed, 2 ignored**

### Round 3 Complete

- [ ] `cargo test -p context-cli --test cli_integration edge_repeated_single_char` → `ok`
- [ ] `cargo test -p context-cli --test cli_integration oracle_aa -- --include-ignored` → `ok`
- [ ] `cargo test -p context-cli --test cli_integration skill3_exp_m -- --include-ignored` → `ok`
- [ ] `cargo test -p context-read` → no regressions (the ~29 pre-existing failures may remain — do not introduce new ones)
- [ ] Full suite (include-ignored) summary: **~70 passed, 0 failed, 0 ignored**

### Overall Done

- [ ] `FAILING_TESTS.md` updated: all 9 failures documented as resolved, table shows 0 failing
- [ ] `INDEX.md` entry for `PLAN_INTEGRATION_TESTS.md` updated: status `✅ complete`, progress note updated
- [ ] `PLAN_INTEGRATION_TESTS.md` frontmatter `status` updated to `✅ complete`
- [ ] This plan's frontmatter `status` updated to `✅ complete`

---

## Risks & Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|-----------|
| RC-2 does **not** resolve after RC-1 | Medium | Medium | Round 2 investigation steps are pre-planned; scope is narrow (3 tests, 1 call site) |
| Outer loop in RC-1 fix introduces infinite loop on repeated-char inputs | High | High | Add `assert!(width > 0)` guard immediately; this will panic instead of hanging |
| `insert_or_get_complete_from_parts` does not exist; compound root wrapping requires new API | Medium | High | Read the `ReadCtx` finalize path carefully before writing Round 1 — reuse existing APIs |
| RC-3 fix causes regressions in the ~29 already-failing `context-read` tests | Medium | Medium | Run `cargo test -p context-read` before and after Round 3; compare failure lists; only proceed if new failures = 0 |
| `skill3_obs*` retirement breaks CI before Round 1 lands | Low | Low | Retire obs tests in the same commit as the Round 1 fix, not before |
| RC-3 requires a large cursor redesign that touches RC-1 loop | Medium | High | If Round 3 requires redesigning the outer loop from Round 1, plan a combined Round 1+3 fix — do not patch twice |
| Oracle tests reveal correctness issues beyond RC-1 | Medium | Medium | Oracle tests are explicit about expected graph structure; failures after RC-1 fix give specific diff output. Treat each oracle failure as a new tracked bug, not a blocker for the remediation plan |

---

## Related Documents

| Document | Relationship |
|----------|-------------|
| [`20260314_PLAN_INTEGRATION_TESTS.md`](20260314_PLAN_INTEGRATION_TESTS.md) | Parent — this plan remediates the failures documented there |
| [`20260314_PLAN_CONTEXT_READ_UX_IMPROVEMENT.md`](20260314_PLAN_CONTEXT_READ_UX_IMPROVEMENT.md) | Grandparent — RC-1/RC-2/RC-3 are Phase 3 items in this plan |
| [`20260314_PLAN_INSERT_NEXT_MATCH.md`](20260314_PLAN_INSERT_NEXT_MATCH.md) | ✅ Complete — `InsertOutcome` enum now available; Round 1 builds on this |
| [`20260314_PLAN_APPEND_TO_PATTERN_FIX.md`](20260314_PLAN_APPEND_TO_PATTERN_FIX.md) | ✅ Complete — `extend_root_pattern` available; Round 3 uses this |
| [`20260315_PLAN_NGRAMS_ORACLE_VALIDATION.md`](20260315_PLAN_NGRAMS_ORACLE_VALIDATION.md) | Oracle tests are the long-form acceptance tests for Round 1 |
| [`20260218_PLAN_CONTEXT_READ_COMPLETION.md`](20260218_PLAN_CONTEXT_READ_COMPLETION.md) | Prior art for RC-3 — read before attempting Round 3 |
| `tools/context-cli/tests/FAILING_TESTS.md` | Live failure tracker — update after each round |

---

## Notes

### Session Start Checklist

Before beginning any fix round, run:

```sh
# Confirm baseline is unchanged
cargo test -p context-cli --test cli_integration 2>&1 | tail -3
# Expected: FAILED. 44 passed; 9 failed; 22 ignored

# Confirm crate-level tests have no new failures
cargo test -p context-api 2>&1 | tail -3
cargo test -p context-insert 2>&1 | tail -3
cargo test -p context-read 2>&1 | tail -3
```

If the baseline differs from the snapshot above, update this plan before proceeding.

### Deviations from Plan

<!-- Track changes made during execution -->
-

### Lessons Learned

<!-- Post-execution: what would you do differently? -->
-