---
tags: `#design` `#context-read` `#complement` `#trace-cache` `#algorithm`
summary: Completed design for structural complement construction around a shared overlap token. The chosen direction is path → TraceCache → recursive split/join in `context-insert`, with `context-read` remaining orchestration-only.
status: ✅ complete
date: 2026-03-15
plan: 20260315_PLAN_COMPLEMENT_AND_C3.md
related: 20260315_PLAN_CONTEXT_READ_RESTRUCTURE.md
blocking: 10 failing context-read unit tests (all overlap-collapse paths)
---

# Design: Complement Path Building

**Date:** 2026-03-15  
**Scope:** overlap complement construction for `context-read` collapse paths  
**Unblocks:** all 10 currently failing `context-read` unit tests  
**Related future work:** Pass C3 (`OverlapChain::collapse` wiring) after semantic collapse is fixed

---

## Executive Summary

The original framing of the complement problem was too flat.

The complement in overlap collapse is **not** simply “the prefix of the anchor by width”, and it should **not** be built by ad hoc token slicing or root-level `insert_pattern` heuristics.

The correct abstraction is:

> Given a selected shared overlap token `P`, construct the structural partition on either side of `P` from the exact hierarchical path by which `P` is embedded in its parent token.

The chosen implementation direction is:

1. `context-read` computes and carries the relevant structural paths
2. `context-insert` converts those paths into `TraceCache`
3. `context-insert` uses its recursive split/join machinery to build:
   - the left-side partition in the old anchor
   - the right-side partition in the overlap token
4. `context-insert` exposes a higher-level overlap bundling operation
5. `context-read` calls that abstraction and stays focused on orchestration

This replaces the current broken `TraceCache::new(root)` stub strategy and the current asymmetric ad hoc complement logic.

---

## Problem Statement

When the expansion loop detects an overlap between the current anchor token and an incoming token, it builds a `BandState::WithOverlap` and later collapses it into a bundled token.

Today, that collapse path is blocked by incomplete complement construction:

- `context-read/src/complement.rs` uses `build_trace_cache_stub`
- that stub returns `TraceCache::new(root)`
- `insert_init` then fails because the cache lacks the required structural path entries below the root

This leads to `MissingCacheEntry(...)` during overlap collapse and blocks all 10 currently failing tests.

However, the deeper issue is not merely “the cache is empty.” The more important issue is that the current design treats complement extraction as a flat width-split problem, while the real graph structure is hierarchical.

---

## Correct Structural Model

Let:

- `A` = old anchor token
- `P` = selected shared overlap token
- `T2` = overlap expansion token that also contains `P`

The overlap collapse needs two structural partitions:

1. **Left partition of `A` relative to `P`**
   - all siblings to the **left** of the selected path to `P` inside `A`

2. **Right partition of `T2` relative to `P`**
   - all siblings to the **right** of the selected path to `P` inside `T2`

These are not mere width ranges. They are path-defined recursive partitions.

### Structural identity

Conceptually:

```text
A  = [left_of_P_in_A, P]      // along the selected postfix path
T2 = [P, right_of_P_in_T2]    // along the selected overlap path
```

The collapse operation needs to bundle decompositions around the shared token `P` using these two structural sides.

---

## Why Width-Only Thinking Fails

A selected postfix token `P` may be nested inside multiple larger postfix structures in the representation of `A`.

Example shape:

```text
A
└─ pattern: [X, Y]
           where Y
           └─ pattern: [Z, P]
```

In this case, the left complement of `P` within `A` is not just “the first `width(A) - width(P)` atoms” in a root-level sense. It is structurally:

```text
[X, Z]
```

That token must be built by respecting the hierarchy:

- collect left siblings of `P` inside `Y`
- then collect left siblings of `Y` inside `A`
- then recursively join those partitions

So the correct complement is:

> the token formed by recursively collecting all siblings on the relevant side of the selected path to `P`, preserving order and hierarchy.

That is the invariant the implementation must preserve.

---

## Current Broken Behavior

The current stub in `complement.rs` is:

```text
TraceCache::new(root)
```

This gives `insert_init` only a root entry and no actual path information.

`insert_init` expects a cache whose entries describe how the split is embedded structurally in the graph. Without those entries:

- `SplitTraceStatesCtx` cannot derive split positions
- `completed_splits::<RootNode>` returns no offsets
- `SplitCacheCtx::init` aborts with `MissingCacheEntry(...)`

This is a symptom of the larger issue: the structural path to the selected overlap token is not being represented in the cache.

---

## Chosen Direction

### Core decision

The design session concluded that the correct implementation is:

> path → `TraceCache` → recursive split/join

not:

- flat `insert_pattern(prefix)` as the primary strategy
- manual width-based prefix slicing in `context-read`
- ad hoc asymmetrical complement logic

### Ownership decision

This logic belongs in **`context-insert`**, not `context-read`.

Reason:

- the operation is fundamentally insertion/partition logic
- `context-insert` already owns recursive split/join
- `context-read` should remain orchestration-only

So:

- `context-read` detects overlap and provides paths
- `context-insert` performs structural partition construction and bundling

---

## Symmetry of the Two Complement Cases

The current code treats the two sides asymmetrically:

- one side uses stubbed `insert_init`
- the other side manually scans tokens in a band pattern

That asymmetry is accidental, not desirable.

The design session established that both sides are the same kind of problem:

### Left side
Build the token represented by all siblings to the **left** of the selected postfix path in the old anchor.

### Right side
Build the token represented by all siblings to the **right** of the selected overlap path in `T2`.

Both should be handled by:

1. deriving a `TraceCache` from a structural path
2. asking `context-insert` to recursively split/join the corresponding side

---

## API Direction

### Phase 1: start with specific internal helpers

Begin with private/internal helper APIs in `context-insert` for the two concrete cases:

- left complement from anchor/postfix path
- right complement from overlap-side path

These helpers should:

- return a **single `Token`** on success
- support graceful recovery for empty-side cases
- keep the public API small and durable

A small natural outcome enum is appropriate for the internal layer, e.g.:

```text
PartitionOutcome::{Token, Empty}
```

In the read-pipeline overlap case, a non-empty result is the expected path.

### Phase 2: durable public abstraction

After the side-specific helpers work, move to a higher-level public abstraction in `context-insert`, e.g. a `bundle_overlap(...)`-style operation.

That lets `context-read` remain focused on:

- finding the overlap
- assembling the metadata
- delegating overlap bundling

### Phase 3: generalization later

If useful after the initial implementation is proven, the side-specific helpers can be generalized internally to a shared helper with a side enum.

This generalization is explicitly **not** the first step.

---

## Path Requirements

The path is the central semantic input.

Two distinct path witnesses are needed:

1. **Old anchor side**
   - path from anchor root to selected postfix token `P`

2. **Overlap side**
   - path inside `T2` locating the same shared token `P`

The current `IndexEndPath` / `IndexStartPath` structures are close to what is needed, but the overlap-side path may require adjustment during path-to-cache conversion so that the resulting partition corresponds to the complement of the leaf token inside `T2`.

So the path-to-cache step must be treated as a semantic conversion, not just a dumb wrapper.

---

## Path-to-TraceCache Conversion

The path-to-cache conversion should remain **private/internal** to `context-insert` at first.

This was an explicit design choice to keep the public API simple.

### Why private first

- the internal representation may need refinement while the semantics are proven
- exposing path→cache builders too early would freeze the wrong abstraction
- callers should ask for structural partitions or bundled overlap results, not raw cache construction

### What the conversion must guarantee

Given a structural path to the shared overlap token `P`, the resulting `TraceCache` must contain enough information for `insert_init` to reconstruct the correct recursive partition on the requested side.

The exact low-level cache representation remains an implementation detail of `context-insert`.

---

## Role of `ComplementBuilder`

`ComplementBuilder` in `context-read` is now considered a **transitional wrapper**, not a final abstraction.

It may temporarily survive while the new `context-insert` helper is introduced, but the desired end state is:

- `context-read` does not own path-based partition mechanics
- `context-insert` exposes the durable feature
- `ComplementBuilder` becomes unnecessary or a very thin call adapter

---

## Recommended Implementation Architecture

### `context-read`
Responsible for:

- detecting overlap
- identifying the shared overlap token
- computing/storing the relevant structural paths
- calling the `context-insert` overlap-bundling API

### `context-insert`
Responsible for:

- converting paths into `TraceCache`
- recursively constructing left/right structural partitions
- bundling overlap decompositions into the final token

This keeps the layering clean and improves code durability.

---

## Test and Rollout Strategy

The design session explicitly chose a two-stage rollout:

### Stage 1 — semantic collapse fix
1. Add path-based private helpers to `context-insert`
2. Add a durable higher-level overlap bundling API
3. Replace the current complement and right-side collapse logic in `context-read`
4. verify the 10 currently failing tests

### Stage 2 — Pass C3
Only after semantic correctness is restored:

1. wire `OverlapChain::collapse`
2. refactor `RootManager` / cursor-loop orchestration
3. keep semantic behavior unchanged while changing the chain abstraction

This keeps the debugging surface small and avoids mixing semantic repair with orchestration refactoring.

---

## Consequences for Existing Code

### `context-read/src/complement.rs`
- no longer the long-term home of the real algorithm
- should become a thin adapter or be removed after migration

### `context-read/src/expansion/chain/mod.rs`
- both complement sides should stop using bespoke asymmetric logic
- collapse should delegate to `context-insert`’s overlap-bundling abstraction

### `context-insert`
Will gain:

- private path-based partition helpers
- private path→cache conversion
- a durable overlap bundling API

---

## Rejected Directions

The design session rejected the following as primary strategies:

### 1. Root-level direct `insert_pattern` clean-split approach
Rejected because it is too flat and does not faithfully model nested postfix structure.

### 2. Manual recursive complement construction in `context-read`
Rejected because it duplicates split/join semantics outside the crate that already owns them.

### 3. Exposing raw path→`TraceCache` builders immediately
Rejected because the public API should stay simple until the semantics are fully proven.

---

## Key Invariants

The implementation must preserve the following:

1. **Selected overlap token may be nested**
   - the shared token `P` can be embedded several levels deep

2. **Complement is structural, not just width-based**
   - it is determined by siblings along a selected path

3. **Left and right complement cases are symmetric**
   - they differ only in which side of the path is requested

4. **`context-read` stays orchestration-focused**
   - partition mechanics belong in `context-insert`

5. **Path→cache conversion is internal**
   - callers should not manipulate caches directly

---

## Session Notes

- **Date of session:** 2026-03-15
- **Participants:** user + assistant
- **Chosen approach:** structural path-driven partition construction in `context-insert`
- **Key invariants discovered:**
  - the selected overlap token `P` may be nested inside larger postfix structure
  - complements are structural sibling partitions around `P`, not merely width slices
  - both left and right complement cases can be handled through path-derived cache + recursive split/join
- **Implementation sketch:**
  1. add private path-based partition helpers to `context-insert`
  2. expose a simple higher-level overlap bundling API from `context-insert`
  3. switch current `context-read` collapse/complement logic to that API
  4. verify the 10 failing tests
  5. then wire `OverlapChain::collapse` / C3

---

## Files Likely to Change

| File | Change |
|------|--------|
| `crates/context-insert/src/...` | Add private path-based partition helpers and internal path→cache conversion |
| `crates/context-insert/src/...` | Add durable overlap bundling API |
| `crates/context-read/src/complement.rs` | Replace transitional logic with `context-insert` call or remove |
| `crates/context-read/src/expansion/chain/mod.rs` | Replace current asymmetric collapse complement logic |
| `crates/context-read/src/expansion/mod.rs` | Adjust overlap state construction to pass the needed path metadata |
| `crates/context-read/src/pipeline/root.rs` | Later C3 wiring only, after semantic fix |

---

## Final Design Decision

The final design decision is:

> Implement structural overlap complement handling in `context-insert` by converting overlap paths into trace caches and using recursive split/join to construct the required left/right partitions and bundle the overlap result. `context-read` remains responsible only for overlap detection and orchestration.

---

## Next Steps

1. Create and execute `20260315_PLAN_COMPLEMENT_AND_C3.md`
2. Implement the semantic overlap collapse fix in `context-insert` and `context-read`
3. Run `context-read` tests and verify the 10 blocked failures are addressed
4. After semantic correctness is restored, implement Pass C3 wiring