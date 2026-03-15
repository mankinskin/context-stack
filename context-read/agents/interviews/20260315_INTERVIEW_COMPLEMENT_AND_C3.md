---
tags: `#interview` `#context-read` `#complement` `#trace-cache` `#chain` `#c3`
summary: Completed design interview for the complement trace-cache implementation and Pass C3 (OverlapChain wiring). Answers here drive the execution plan for clearing the 10 failing tests and completing the expansion loop.
status: ✅ complete
date: 2026-03-15
related_design: 20260315_DESIGN_COMPLEMENT_PATH_BUILDING.md
related_plan: 20260315_PLAN_CONTEXT_READ_RESTRUCTURE.md
blocking: 10 failing context-read unit tests (all overlap-collapse paths)
---

# Interview: Complement Building & Pass C3 Wiring

**Date:** 2026-03-15  
**Scope:**
1. Settle the implementation strategy for `build_trace_cache_stub` in `complement.rs`
2. Define the execution plan for Pass C3 — wiring `OverlapChain::collapse` into `RootManager`
3. Resolve the remaining open questions about the `TraceCache` contract and `insert_init`

---

## Background

### Where we are

After Passes A, B, and C1/C2:

- `complement.rs` contains `build_trace_cache_stub`, which returns `TraceCache::new(root)` — an empty cache containing only the root vertex entry.
- `insert_init` wraps the cache in a `TraceCtx`, feeds it into `SplitTraceStatesCtx`, which immediately calls `completed_splits::<RootNode>` on the root token. That function calls `VertexSplitCtx::complete_splits`, which reads `bottom_up` and `top_down` entries from the `VertexCache`. Because the stub returns an empty `VertexCache` (no `bottom_up`, no `top_down` entries), `completed_splits` returns an empty offsets map, which triggers the guard in `SplitCacheCtx::init`:
  ```
  Err(ErrorReason::MissingCacheEntry(root.vertex_index()))
  ```
- All 10 failing tests reach this code path (overlap collapse → complement construction → `insert_init` → `MissingCacheEntry`).

### What changed during the interview

The initial framing of the problem was too flat. The real issue is not "extract the prefix of a token by width". The real issue is:

> Given a selected shared overlap token `P`, construct the structural complement on either side of `P` using the exact hierarchical path by which `P` is embedded in its parent token.

That means the overlap/collapse problem has **two symmetric structural complement cases**:

1. **Left complement of the old anchor `A`**
   - Build the token represented by all siblings to the **left** of the chosen postfix path inside `A`.

2. **Right complement of the overlap token `T2`**
   - Build the token represented by all siblings to the **right** of the shared overlap token inside `T2`.

Both are path-based recursive partition problems. They should not be implemented by flat width slicing or ad hoc `insert_pattern` heuristics.

---

## Core Corrected Model

Given:

- `A` = old anchor token
- `P` = selected postfix of `A`
- `T2` = overlap expansion token that contains `P`

We want to reuse the shared overlap token `P` and construct the two structural complements around it.

### Structural identity

```
A  = [left_of_P_in_A, P]            // structurally, along the selected postfix path
T2 = [P, right_of_P_in_T2]          // structurally, along the selected overlap path
```

The collapse phase then needs to bundle the two decompositions around the shared token `P` using the recursive split/join machinery.

### Important correction

The complement is **not** defined merely as "the first `width(A) - width(P)` atoms".  
It is:

> the token formed by recursively collecting all siblings to the relevant side of the selected path to `P`, preserving hierarchy and order.

So the path matters more than the width.

---

## Answers

---

### Q1 — Primary strategy: direct `insert_pattern` vs. `TraceCache` walk

**A:** Neither of the initial candidate framings is the right primary abstraction. We do not want a flat `insert_pattern` clean-split shortcut as the main strategy, and we do not want `context-read` to manually rediscover a path by width.

The correct strategy is:

1. Use the **structural path** to the selected shared overlap token `P`
2. Convert that path into a `TraceCache`
3. Use the recursive split/join machinery in `context-insert` to construct the needed partition token

There are two such partitions:
- left side of `P` in the old anchor
- right side of `P` in the overlap token `T2`

The key insight from the interview is that the actual postfix `P` of anchor `A` can be nested inside larger postfix structure in the representation of `A`. Therefore the path to `P` must drive the construction.

We want to be able to reuse the top-down path information to the split for a complement, but the meaningful reusable object is the **path**, not a flat width split. That path must be translated into the cache form expected by `insert_init`.

**Implications:**
- Reject the “Approach 3 primary, Approach 1 fallback” recommendation from the original design draft.
- The chosen direction is **path → TraceCache → recursive split/join**.
- `context-read` should provide the structural path witness.
- `context-insert` should own the transformation from path to partition token.

---

### Q2 — `ComplementBuilder` vs. inline complement extraction

**A:** The existing `ComplementBuilder` shape is too narrow and too anchor-specific. The real feature is broader: path-based partition construction on either side of a shared overlap token.

So `ComplementBuilder` should not remain the long-term abstraction. The implementation should move toward helper APIs in `context-insert` that construct structural partitions from paths, and eventually toward a higher-level overlap bundling helper. `context-read` should remain an orchestrator.

For the immediate migration, `ComplementBuilder` can remain as a temporary wrapper while the new `context-insert` helper is introduced, but it should become a thin adapter and then disappear.

**Implications:**
- `ComplementBuilder` is transitional, not the final design.
- The final logic should not live as a bespoke struct in `context-read`.
- Move toward `context-insert` helpers first, then remove or flatten `ComplementBuilder`.

---

### Q3 — Where does complement extraction live after the fix?

**A:** The implementation belongs in `context-insert`, not in `context-read`.

Reasoning:
- The operation is fundamentally **path → trace cache → recursive split/join**.
- That is insertion/partition logic, not read-pipeline orchestration.
- `context-read` should stay focused on:
  - detecting overlap
  - computing/storing the relevant paths
  - calling the partition/bundling functionality

**Implications:**
- `context-read` should not gain more partition mechanics.
- The first implementation should add helper methods/functions in `context-insert`.
- `complement.rs` in `context-read` can either become a thin shim or be removed after the new helper is adopted.

---

### Q4 — Multi-level anchors: when does a flat clean split fail?

**A:** A flat clean-split framing is not reliable enough because the selected postfix `P` can be nested in multiple larger postfixes in the representation of `A`.

That means:
- the true structural complement may need to collect left siblings across several levels
- the relevant split is not just “a width boundary in the root token”
- the correct complement is determined by the **selected path to `P`**

So yes, the simple “does `intersection_start` align with a stored child-pattern boundary?” framing is insufficient for the actual algorithm we want.

**Implications:**
- Do not design the fix around root-level prefix extraction only.
- The partition helper must accept structural path input.
- The implementation must preserve the hierarchical embedding of `P`.

---

### Q5 — Fallback depth / recursion depth

**A:** The correct implementation should assume recursive structure and let the split/join machinery handle it. We should not bake a manual depth-bound assumption into the design.

The relevant point is not whether the recursion is logarithmic or linear in an abstract graph shape. The relevant point is:
- the path already tells us exactly which recursive descent/ascent must be represented
- `context-insert` should execute that recursive split/join correctly from the path-derived cache

So the design does not depend on a separate manual-walk depth analysis in `context-read`.

**Implications:**
- Avoid hand-written recursive path slicing in `context-read`
- Let `context-insert` own recursion complexity
- Path-to-cache conversion should be mechanical and local to `context-insert`

---

### Q6 — `build_prefix_complement` and `build_postfix_complement` symmetry

**A:** Yes — these are structurally symmetric cases and should be handled with the same conceptual mechanism.

The important clarification from the interview is:

- On the old anchor side, we need the **left siblings** of the selected postfix path.
- On the overlap token side (`T2`), we need the **right siblings** of the shared overlap token path.

Both cases can be handled by:
1. a trace cache constructed from the relevant path
2. a partition join of the relevant side

So although the current code is asymmetric (`build_prefix_complement` uses a stubbed `insert_init`, `build_postfix_complement` manually scans pattern tokens), the target design is symmetric.

**Implications:**
- Replace both the stubbed left-side helper and the ad hoc right-side helper
- Use a shared path-based partition model for both sides
- Initial implementation may expose two dedicated helpers, but they should share the same underlying machinery

---

### Q7 — Pass C3 scope: what exactly needs to change in `RootManager`?

**A:** C3 should come **after** the semantic collapse fix. The first goal is to make the current collapse path correct using the new partition/bundling machinery. Only after that should the chain abstraction be wired in.

The interview did not lock in the full future cursor-buffering semantics for multi-overlap chains, but it did establish the execution order:

1. Fix overlap collapse semantics first
2. Verify the current failing tests
3. Then wire `OverlapChain::collapse` into the orchestration path

So for planning purposes, C3 should be treated as **post-semantic-fix chain wiring**, not as part of the initial complement repair.

**Implications:**
- Keep C3 separate from the complement fix
- Use the current `BandState::collapse` path as the first integration point
- Only after semantic correctness is restored should `OverlapChain::collapse` replace it

---

### Q8 — `OverlapChain::collapse` output type

**A:** Not explicitly decided in the interview. The practical bias is toward returning a single `Token`, because successful partition/bundling helpers should return a single token as the complement/bundle result, and the collapse result is semantically a bundled token.

However, this was not a contentious design point and can remain an implementation convenience decision.

**Chosen working direction:** prefer `Token` for the new abstractions when possible; keep compatibility adapters if needed.

**Implications:**
- New partition helpers should return a single token on success
- `OverlapChain::collapse` may reasonably return `Token`
- Existing call sites can be adapted as needed

---

### Q9 — Test strategy for C3: fix complement first, then wire C3, or together?

**A:** Fix semantic collapse first, then chain wiring second.

This was explicitly agreed:
1. Add path-based partition helpers to `context-insert`
2. Replace the current complement logic and right-side ad hoc logic in `context-read`
3. Verify the 10 failing tests
4. Then implement C3 / `OverlapChain::collapse` wiring

**Implications:**
- The next implementation plan should be split into:
  - semantic overlap collapse fix
  - later chain wiring
- This keeps the failing-test diagnosis clean
- It avoids mixing semantic repair with orchestration refactoring

---

### Q10 — Naming: `collapse` vs. `bundle` vs. `resolve`

**A:** Not explicitly finalized during the interview.

However, the stronger abstraction on the `context-insert` side is clearly **bundling overlap structure from paths**, while `collapse` remains acceptable for the existing `BandState` / `OverlapChain` API in `context-read`.

**Chosen working direction:** keep `collapse` in `context-read` for continuity; use a more explicit name such as `bundle_overlap` for the new `context-insert` abstraction.

**Implications:**
- Preserve local API continuity in `context-read`
- Give the new `context-insert` feature a clearer name aligned with its role

---

### Q11 — General-case ownership of path/partition helpers

**A:** This belongs in `context-insert`.

More specifically:
- start with **specific helper APIs first**
- generalize to a side enum later if useful
- keep path→`TraceCache` conversion **private/internal** to `context-insert` at first
- keep the public `context-insert` API simple

This was explicitly agreed.

**Implications:**
- Do not expose low-level path→cache builders yet
- Keep side-specific helpers private if possible
- Prefer a higher-level durable API such as overlap bundling in `context-insert`

---

## Concrete API direction settled by the interview

### Step 1 — Start specific
Begin with dedicated partition helpers in `context-insert`:

- left complement from anchor/postfix path
- right complement from overlap path

The helpers should:
- assume non-empty complement in the read use case
- return a **single token** on success
- support graceful recovery if an empty complement is encountered via a small natural outcome enum

### Step 2 — Then generalize
Once proven:
- unify the implementation internally
- possibly introduce a side enum later

### Step 3 — Public API durability
The preferred durable direction is to go beyond side helpers and provide a higher-level `context-insert` overlap-bundling abstraction, so `context-read` can stay orchestration-only.

---

## Outcome enum direction

The chosen style is a **natural** small enum, e.g.:

```
PartitionOutcome::{Token, Empty}
```

This is primarily for graceful recovery and generic robustness. In the read-pipeline overlap case, success returning a token is the expected path.

---

## High-level implementation direction

### Chosen architecture

1. `context-read` detects overlap and provides the relevant structural paths:
   - old-anchor postfix path
   - overlap-side path inside `T2`

2. `context-insert`:
   - converts those paths to the internal trace/cache representation
   - uses recursive split/join to construct the structural partitions
   - bundles the overlap result at a higher level

3. `context-read` calls the high-level overlap-bundling API and remains focused on orchestration.

### Why this is better

- avoids duplicating partition logic in `context-read`
- keeps path→cache logic close to `insert_init`
- improves code durability by centralizing recursive partition mechanics in the crate that already owns split/join

---

## Summary Table

| # | Question | Answer | Drives |
|---|----------|--------|--------|
| Q1 | Strategy | Path → TraceCache → recursive split/join | Core fix direction |
| Q2 | `ComplementBuilder` | Transitional only; move logic into `context-insert` | Refactor target |
| Q3 | Ownership | `context-insert` | Crate boundary |
| Q4 | Flat clean-split validity | Insufficient; path structure matters | Reject width-only design |
| Q5 | Recursion/depth | Let `context-insert` own it | Avoid manual path logic in `context-read` |
| Q6 | Left/right symmetry | Yes, symmetric partition cases | Shared implementation model |
| Q7 | C3 scope | Semantic fix first, chain wiring second | Execution order |
| Q8 | Collapse return type | Prefer token-oriented abstractions | API bias |
| Q9 | Test strategy | Fix collapse semantics before C3 | Plan phases |
| Q10 | Naming | Keep `collapse` locally; use `bundle_overlap` in `context-insert` | Naming strategy |
| Q11 | API exposure | Start with simple `context-insert` API; keep path→cache private | Public surface design |

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

## Next Steps

1. Update `20260315_DESIGN_COMPLEMENT_PATH_BUILDING.md` with the corrected structural model and chosen direction.
2. Create `20260315_PLAN_COMPLEMENT_AND_C3.md` in `plans/` with the concrete execution phases:
   - phase 1: add path-based private helpers in `context-insert`
   - phase 2: expose durable overlap-bundling API from `context-insert`
   - phase 3: replace collapse/complement logic in `context-read`
   - phase 4: verify the failing tests
   - phase 5: implement C3 wiring afterward
3. Begin implementation from the plan.