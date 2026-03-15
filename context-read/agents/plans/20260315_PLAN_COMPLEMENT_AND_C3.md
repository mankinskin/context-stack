---
tags: `#plan` `#context-read` `#context-insert` `#complement` `#trace-cache` `#overlap` `#c3`
summary: Execution plan for fixing structural overlap complements via path→TraceCache→split/join in `context-insert`, then wiring Pass C3 after semantic collapse is green.
status: 📋 ready
date: 2026-03-15
related_interview: 20260315_INTERVIEW_COMPLEMENT_AND_C3.md
related_design: 20260315_DESIGN_COMPLEMENT_PATH_BUILDING.md
related_restructure_plan: 20260315_PLAN_CONTEXT_READ_RESTRUCTURE.md
blocking: 10 failing context-read unit tests (all overlap-collapse paths)
---

# Plan: Complement Fix and C3 Follow-up

**Date:** 2026-03-15  
**Scope:** structural overlap complement construction, overlap bundling, and subsequent Pass C3 wiring  
**Primary crates:** `context-insert`, `context-read`  
**Goal:** fix the 10 failing overlap-collapse tests first, then wire `OverlapChain::collapse` cleanly afterward

---

## Objective

Restore semantic correctness of overlap collapse by replacing the current broken complement logic with a structural path-driven implementation.

The chosen architecture is:

1. `context-read` detects overlap and computes the relevant structural paths
2. `context-insert` converts those paths into `TraceCache`
3. `context-insert` uses recursive split/join to build structural partitions around the shared overlap token
4. `context-insert` exposes a durable overlap-bundling API
5. `context-read` delegates collapse to that API
6. only after tests are green, implement Pass C3 by routing collapse through `OverlapChain`

This plan explicitly separates:
- **semantic repair** of overlap collapse
- **orchestration refactor** for C3

---

## Background

### Current failure mode

The current collapse path fails because `context-read/src/complement.rs` uses:

- `build_trace_cache_stub(...)`
- which returns `TraceCache::new(root)`

That cache contains only a root entry and no structural path information below the root. When `insert_init` consumes it, `SplitTraceStatesCtx` cannot derive split offsets and `SplitCacheCtx::init` returns:

- `MissingCacheEntry(...)`

This blocks all 10 currently failing `context-read` tests.

### Corrected model

The design interview established that the complement problem is not a flat width-prefix problem.

The correct abstraction is:

> Given a selected shared overlap token `P`, construct the structural partition on either side of `P` from the exact hierarchical path by which `P` is embedded in its parent token.

There are two symmetric cases:

1. **Left partition of the old anchor `A` relative to `P`**
2. **Right partition of the overlap token `T2` relative to `P`**

Both should be handled through:

- structural path
- path → `TraceCache`
- recursive split/join in `context-insert`

---

## Non-Goals

This plan does **not** include:

- general public path→cache utilities
- exposing low-level partition helpers as stable public API immediately
- full overlap-chain buffering logic in the cursor loop
- unrelated `context-read` module reshuffling (already handled in the restructure plan)

Those are either internal implementation details or deferred to Pass C3 / later cleanup.

---

## Success Criteria

This plan is successful when:

1. The 10 currently failing `context-read` overlap-collapse tests are either fixed or meaningfully reduced with identified follow-up
2. `context-read` no longer contains the current broken complement stub path
3. collapse logic no longer uses asymmetric ad hoc left/right complement handling
4. `context-insert` owns the path-based partition/bundling implementation
5. `context-read` remains orchestration-focused
6. baseline tests are restored before Pass C3 wiring begins

---

## Execution Strategy

### Order of work

1. Add internal path-based partition helpers in `context-insert`
2. Add a durable higher-level overlap-bundling API in `context-insert`
3. Replace current complement/collapse logic in `context-read`
4. Run and stabilize `context-read` tests
5. Only then implement Pass C3 wiring

This order is mandatory. Do not mix semantic repair with chain-wiring refactors.

---

## Phase 1 — Add internal structural partition helpers in `context-insert`

**Goal:** teach `context-insert` how to construct left/right structural partitions from overlap paths.

### 1.1 Introduce an internal partition outcome enum

Add a small natural internal enum for partition results.

Suggested shape:

```text
PartitionOutcome::{Token, Empty}
```

This is primarily for graceful recovery and generic robustness.

### 1.2 Add internal helper for left partition from postfix path

Implement an internal helper that accepts the old-anchor postfix path and returns the structural partition to the left of the shared overlap token.

Target semantics:
- input: path from anchor root to selected postfix token `P`
- output: token for all siblings left of `P` along that path
- empty case: representable via `PartitionOutcome::Empty`

### 1.3 Add internal helper for right partition from overlap-side path

Implement the symmetric internal helper for the overlap token side.

Target semantics:
- input: path inside `T2` locating the shared overlap token `P`
- output: token for all siblings right of `P` along that path
- empty case: representable via `PartitionOutcome::Empty`

### 1.4 Keep path→cache conversion private

Do **not** expose raw path→`TraceCache` conversion publicly yet.

The implementation may use:
- dedicated private helper functions
- private module-local builder types
- side-specific conversion logic

but the public surface should remain small.

### 1.5 Validation targets

Add or adapt tests in `context-insert` covering:

- left partition of direct-child postfix
- left partition of nested postfix
- right partition of direct-child overlap leaf
- right partition of nested overlap leaf
- empty-side recovery behavior
- path conversion correctness on multi-level paths

---

## Phase 2 — Add durable overlap bundling API in `context-insert`

**Goal:** provide a higher-level abstraction so `context-read` does not assemble overlap bundles manually.

### 2.1 Expose a focused overlap bundling entry point

Add a durable API in `context-insert` for bundling overlap structure from the two path witnesses and the relevant tokens.

Suggested conceptual shape:

```text
bundle_overlap(...)
```

The exact signature can vary, but it should accept enough information to:
- identify the old-anchor side path
- identify the overlap-side path
- know the participating tokens (`t1`, `t2`, possibly the shared token path witness)
- return the final bundled token

### 2.2 Internal behavior of the bundling API

The bundling API should:

1. derive the left partition of the old anchor
2. derive the right partition of the overlap token
3. construct the needed decompositions around the shared overlap token
4. insert those decompositions into the graph
5. return the resulting bundled token

### 2.3 Keep the API durable but narrow

Avoid exposing:
- raw cache builders
- low-level split internals
- side enums unless truly needed immediately

Use the smallest abstraction that lets `context-read` delegate the work cleanly.

### 2.4 Validation targets

Add integration-style tests in `context-insert` for:
- simple overlap bundle
- nested-postfix overlap bundle
- repeated-token overlap bundle
- behavior when one side is empty
- idempotence / reuse of existing graph structure where expected

---

## Phase 3 — Replace collapse/complement logic in `context-read`

**Goal:** route the current broken collapse path through the new `context-insert` abstraction.

### 3.1 Replace `complement.rs` transitional logic

Remove the semantic role of:

- `build_trace_cache_stub`
- direct `insert_init` attempts based on empty cache

Depending on implementation convenience:

- keep `ComplementBuilder` temporarily as a thin adapter to `context-insert`
- or remove it entirely if the new API makes it unnecessary

### 3.2 Replace asymmetric complement logic in `expansion/chain/mod.rs`

Current state is asymmetric:
- left/prefix side uses broken stubbed `insert_init`
- right/postfix side manually scans pattern tokens

Replace both with delegation to the `context-insert` overlap bundling API.

### 3.3 Adjust overlap state metadata as needed

If the new bundling API requires more explicit path metadata, update overlap state construction in:

- `context-read/src/expansion/mod.rs`

Possible adjustments include:
- carrying more precise path witness data for the overlap-side token
- refining how `IndexStartPath` / `IndexEndPath` are built or stored
- avoiding recomputation where a path is already known structurally

### 3.4 Keep `context-read` orchestration-only

Do not re-implement partition assembly in `context-read`.
Do not add new manual recursive logic there.

`context-read` should:
- detect overlap
- package the path/token metadata
- call the new bundling API
- update root state

### 3.5 Validation targets

Run:

- `cargo test -p context-read`

Success target:
- eliminate or substantially reduce the 10 currently failing tests
- identify any residual failures as separate semantic issues, not cache-stub issues

---

## Phase 4 — Stabilize semantic overlap collapse

**Goal:** reach a clean semantic baseline before C3.

### 4.1 Required verification

Run:

- `cargo test -p context-insert`
- `cargo test -p context-read`

If failures remain:
- inspect whether they are due to path semantics
- inspect whether overlap-side path conversion is slightly wrong
- inspect whether the bundled decomposition shape differs from expected test assertions

### 4.2 Acceptable intermediate state

It is acceptable if:
- the original 10 `MissingCacheEntry` failures disappear
- a smaller number of deeper semantic mismatches remain

If so, fix those before starting C3.

### 4.3 Not acceptable before C3

Do **not** start C3 if:
- `context-read` still fails due to complement construction
- left/right side logic is still asymmetric in substance
- the path→cache logic is still clearly unstable

---

## Phase 5 — Implement Pass C3 after semantic fix

**Goal:** wire `OverlapChain::collapse` after collapse semantics are known-good.

### 5.1 Replace direct `BandState::collapse` path

Update `RootManager` / related orchestration so that the chain abstraction becomes the collapse entry point once semantic correctness is already established.

### 5.2 Keep semantics unchanged during wiring

Pass C3 is a refactor of orchestration shape, not a semantic redesign of complement logic.

The semantic source of truth remains the overlap bundling implementation added in `context-insert`.

### 5.3 Defer bigger cursor-loop redesign if necessary

If buffering multiple overlaps into a growing chain requires larger iterator restructuring, treat that as a contained follow-up within C3, but do not reopen the semantic partition design.

### 5.4 Validation targets

Run:

- `cargo test -p context-read`
- relevant `context-api` checks if public surface changed indirectly

Success target:
- keep the now-green semantic baseline green
- no new overlap regressions

---

## Files Likely Affected

### `context-insert`

Likely areas:
- internal path-based partition helpers
- internal path→cache conversion
- overlap bundling API
- tests covering recursive partition extraction and overlap bundle behavior

### `context-read`

Likely areas:
- `src/complement.rs`
- `src/expansion/mod.rs`
- `src/expansion/chain/mod.rs`
- later: `src/pipeline/root.rs` for C3 wiring

---

## Risks

### Risk 1 — Overlap-side path semantics are slightly wrong
`IndexStartPath` may be close but not exactly the right witness for the right-side partition of `T2`.

**Mitigation:**  
Treat overlap-side path conversion as a semantic step, not a mechanical one. Add targeted tests for nested overlap-side cases.

### Risk 2 — Public API in `context-insert` grows too quickly
Exposing low-level path→cache helpers too early may freeze the wrong abstraction.

**Mitigation:**  
Keep low-level helpers private initially. Expose only the durable bundling API.

### Risk 3 — Semantic and C3 work get entangled
If chain wiring starts before the semantic fix is stable, debugging becomes much harder.

**Mitigation:**  
Enforce the phase boundary: semantic green first, C3 second.

### Risk 4 — Hidden asymmetry remains
One side may still accidentally use a simpler path than the other.

**Mitigation:**  
Require both left and right sides to go through the same conceptual path→partition pipeline, even if implemented by two specific helpers first.

---

## Validation Checklist

### After Phase 1
- [ ] `context-insert` has internal left/right partition helpers
- [ ] nested path cases are tested
- [ ] empty-side recovery is representable

### After Phase 2
- [ ] `context-insert` exposes a durable overlap bundling API
- [ ] bundling tests cover direct and nested cases

### After Phase 3
- [ ] `context-read` no longer relies on `TraceCache::new(root)` for overlap complements
- [ ] `context-read` no longer uses asymmetrical ad hoc right-side collapse logic
- [ ] collapse delegates to `context-insert`

### After Phase 4
- [ ] `cargo test -p context-read` is green or meaningfully improved with clearly identified residual issues
- [ ] original `MissingCacheEntry` overlap failures are gone

### After Phase 5
- [ ] `OverlapChain::collapse` is wired into orchestration
- [ ] semantic behavior remains stable
- [ ] no new regressions in overlap tests

---

## Recommended Commit Structure

1. **Commit 1:** internal path-based partition helpers in `context-insert`
2. **Commit 2:** durable overlap bundling API in `context-insert`
3. **Commit 3:** switch `context-read` collapse/complement path to bundling API
4. **Commit 4:** semantic test fixes / stabilization
5. **Commit 5:** Pass C3 wiring

This preserves bisectability and keeps semantic vs. orchestration changes separate.

---

## Final Decision

The chosen plan is:

> Fix overlap collapse semantically first by implementing structural path-driven partition and bundling in `context-insert`, then migrate `context-read` to use that abstraction, verify the blocked tests, and only afterward wire Pass C3 through `OverlapChain`.

---

## Next Steps

1. Update indices and workspace docs to reference this plan
2. Execute Phase 1 in `context-insert`
3. Continue sequentially through the phases above