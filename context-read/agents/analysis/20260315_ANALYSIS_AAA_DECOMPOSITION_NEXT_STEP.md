---
tags: `#analysis` `#context-read` `#aaa` `#decomposition` `#root-manager` `#expansion`
summary: Small overview of the current `aaa` failure, what has already been ruled out, and the recommended next debugging step.
status: 📋 active
date: 2026-03-15
related_plan: ../plans/20260315_PLAN_COMPLEMENT_AND_C3.md
related_design: ../designs/20260315_DESIGN_COMPLEMENT_PATH_BUILDING.md
---

# Analysis: `aaa` Decomposition — Recommended Next Step

## Current failure

The smallest remaining semantic failure is the `aaa` case.

### Expected
For input `aaa`, `context-read` should produce both decompositions of the width-3 token:

- `[a, aa]`
- `[aa, a]`

### Actual
The current result only contains:

- `[a, aa]`

The decomposition:

- `[aa, a]`

is missing.

This shows that the system is able to create the repeated token `aa`, but does not retain or synthesize the alternate sequential decomposition when reading the third `a`.

---

## What has already been established

### 1. The old complement stub is no longer the primary blocker
The overlap collapse path has been redirected through the new `context-insert` overlap bundling API.

That means the original empty-cache complement stub is no longer the only explanation for the remaining failures.

### 2. The remaining failures are now semantic
The failing cases are now better interpreted as decomposition-shape problems rather than raw cache-construction failures.

### 3. Root semantic latching was tested
A semantic-root latch was added to `RootManager` to stop claimed semantic tokens from being treated as appendable flat containers.

That was a reasonable hypothesis, but it did **not** fix the `aaa` case.

So the missing `[aa, a]` decomposition is **not explained solely** by the root being treated as extendable too long.

---

## Interpretation

The most likely issue is now upstream of the final root update.

The problem is probably one of these:

1. `ExpansionCtx` does not yield the right state sequence for the third `a`
2. the immediate commit model commits the first valid construction and never materializes the alternate one
3. `BandState::Single` / its commit path is too weak to create symmetric known-token decompositions in repeated cases
4. the current cursor loop only supports one forward construction path per step and does not preserve a second valid sequential decomposition

The `aaa` failure strongly suggests the system currently prefers:

- build `aa`
- then extend to `aaa` as `[a, aa]`

but never also captures:

- `[aa, a]`

as a sibling decomposition of the same width-3 token.

---

## Why this matters

This is the smallest example showing that the remaining failures are not just about overlap complements.

It indicates that even after fixing the overlap bundling boundary, the read pipeline still lacks a mechanism to preserve or generate alternate valid decompositions in repeated-known-token cases.

That is likely relevant not only for:

- `aaa`

but also for larger remaining failures such as:

- `abcabcabc`
- `xyzxyzxyz`
- `aabbaabb`
- infix-related failing cases

---

## Recommendation

## Recommended next debugging step

Focus directly on the `aaa` execution path.

### Specifically inspect:

1. **`ExpansionCtx::next()`**
   - What states are yielded for the three positions in `aaa`?
   - What token is produced at the third step?
   - Is that third step being treated as a `Single` state when a second semantic decomposition is also available?

2. **`BlockExpansionCtx::process()`**
   - Confirm the exact order of state commits
   - Verify whether the immediate-commit model prevents the alternate decomposition from being created

3. **`RootManager::commit_state()`**
   - Confirm what happens when the third `a` arrives after `aa` has already been created
   - Determine whether the commit path only wraps/extends one chosen state instead of preserving both valid decompositions

---

## Working hypothesis

The most likely root cause is now:

> the system commits one valid sequential construction path for `aaa`, but does not preserve the alternate valid known-token decomposition `[aa, a]`.

This suggests the next fix will likely belong in:

- state generation in `ExpansionCtx`, or
- how `Single` commits are interpreted in the presence of an already-known compound token, or
- the immediate-commit orchestration model

rather than purely in root mutability flags.

---

## Suggested execution order

1. Trace `aaa` through `ExpansionCtx`
2. Record the exact yielded `BandState`s
3. Record how each is committed by `RootManager`
4. Decide whether the fix belongs in:
   - `ExpansionCtx`
   - `BandState::Single` handling
   - or the commit/orchestration layer

Only after that should broader repeated-pattern failures be revisited.

---

## Bottom line

The `aaa` case is now the best debugging anchor.

It is:
- minimal
- deterministic
- independent enough from the larger overlap cases
- and already shows the exact missing semantic output: `[aa, a]`

### Recommendation
Use `aaa` as the next focused investigation target before continuing with the larger overlap and infix failures.