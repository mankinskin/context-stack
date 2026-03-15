---
tags: `#plan` `#context-read` `#architecture` `#restructuring` `#api` `#refactoring`
summary: Restructure context-read module layout, establish a clean public API, migrate bands traversal to context-trace, delete dead code, consolidate overlap chain types, and seed the benchmark collection.
status: 📋 ready
date: 2026-03-15
interview: 20260315_INTERVIEW_CONTEXT_READ_RESTRUCTURE.md
parent: 20260314_PLAN_CONTEXT_READ_UX_IMPROVEMENT.md
related: 20260315_PLAN_EXPANSION_LOOP_REDESIGN.md, 20260218_PLAN_CONTEXT_READ_COMPLETION.md
priority: high — structural precondition for complement fix and OverlapChain implementation
---

# Plan: context-read Restructuring

**Date:** 2026-03-15
**Scope:** Medium-large (module moves, one infrastructure migration, no logic change in Pass A/B; logic consolidation in Pass C)
**Crates affected:** `context-read` (primary), `context-trace` (receives `bands/` machinery)
**Test baseline:** 70 pass / 10 fail / 0 ignored in `context-read` unit tests

---

## Table of Contents

1. [Objective](#objective)
2. [Background: Q&A Summary](#background-qa-summary)
3. [Target Layout](#target-layout)
4. [Pass A — Migrate and Delete](#pass-a--migrate-and-delete)
5. [Pass B — Rename and Reorganise](#pass-b--rename-and-reorganise)
6. [Pass C — Chain Consolidation](#pass-c--chain-consolidation)
7. [Complement Design Stub](#complement-design-stub)
8. [Files Affected](#files-affected)
9. [Risks](#risks)
10. [Validation Criteria](#validation-criteria)

---

## Objective

Produce a `context-read` crate with:

- A single clear public entry point (`pub fn read` + `pub struct ReadSequenceIter`).
- A module tree whose directory names reflect what each layer does (`pipeline/` not `context/`).
- No dead code files (`stack.rs`, `cursor.rs`, `chain/op.rs` deleted).
- Graph traversal utilities (`bands/`) migrated to `context-trace` where they belong.
- Overlap chain types consolidated so `BandExpansion`/`BandCap`/`ChainOp` no longer duplicate `OverlapLink`.
- A `benches/` directory seeded with the grammar worst-case builder.
- A named stub for the complement trace-cache fix so its incompleteness is visible.

---

## Background: Q&A Summary

The following decisions were reached in the design interview
(`agents/interviews/20260315_INTERVIEW_CONTEXT_READ_RESTRUCTURE.md`).

| # | Question | Decision |
|---|----------|----------|
| 1 | What does a caller need? | `Token` from any text input; `ReadSequenceIter` for REPL composability; future: transition weights on tokens/edges |
| 2 | `ReadRequest` — public or remove? | Collapse into `IntoReadInput` trait; keep `HasReadCtx` tuple as `pub(crate)` test convenience |
| 3 | `context-read` binary? | Remove it; `context-cli` already covers the use case; `grammar.rs` → `benches/` |
| 4 | `segment.rs` split? | Unify input resolution in `input.rs`; keep segmentation in `segment.rs` |
| 5/6 | `context/` layout and `RootManager`? | Rename `context/` → `pipeline/`; merge `has_read_context.rs` into `input.rs` |
| 7 | `stack.rs` / `cursor.rs`? | Delete both; design `OverlapChain` as the correct chain structure in Pass C |
| 8 | Dead types in `chain/link.rs`? | Merge `BandExpansion`/`BandCap`/`ChainOp` into `OverlapLink`; delete `StartBound`/`EndBound` traits; delete `chain/op.rs` |
| 9 | `grammar.rs`? | Move to `benches/grammar.rs`; add `criterion` bench target |
| 10 | `bands/` — move to `context-trace`? | Yes; delete `bands/` from `context-read`; add `traversal.rs` to `context-trace` |
| 11 | `complement.rs` stub? | Rename stub function; design session required before implementation |

---

## Target Layout

```
context-read/src/
  lib.rs                ← pub fn read(graph, input) + pub use {ReadSequenceIter, SegmentResult}
  input.rs              ← IntoReadInput trait + impls for &str / String / impl Read / chars / NewAtomIndices
                          (absorbs request.rs, RequestInput, ToNewAtomIndices, HasReadCtx)
  segment.rs            ← SegmentIter, NextSegment, LazyAtomIter, Utf8CharIter, ErasedSegmentIter
  pipeline/
    mod.rs              ← ReadCtx, ReadSequenceIter, SegmentResult  (was context/mod.rs)
    root.rs             ← RootManager  (was context/root.rs)
  expansion/
    mod.rs              ← ExpansionCtx iterator
    block.rs            ← BlockExpansionCtx
    link.rs             ← ExpansionLink
    chain/
      mod.rs            ← BandState + collapse()
      band.rs           ← Band struct
      link.rs           ← OverlapLink  (dead BandExpansion/BandCap/ChainOp removed)
  complement.rs         ← ComplementBuilder (stub renamed, design session pending)

context-read/benches/
  grammar.rs            ← moved from tests/grammar.rs; criterion benchmark

context-trace/src/graph/vertex/
  traversal.rs          ← HasTokenRoleIters, BandExpandingIterator, PostfixIterator,
                          PrefixIterator, PostfixExpandingPolicy, PrefixExpandingPolicy
                          (moved from context-read/src/bands/)
```

**Deleted from `context-read`:**
- `src/bands/` (entire directory)
- `src/expansion/stack.rs`
- `src/expansion/cursor.rs`
- `src/expansion/chain/op.rs`
- `src/context/has_read_context.rs`
- `src/request.rs`
- `src/main.rs` + `[[bin]]` target in `Cargo.toml`

---

## Pass A — Migrate and Delete

**Goal:** Zero logic changes. Move infrastructure that belongs elsewhere; delete files with no production use. Tests must remain green (70/10 baseline unchanged).

### A1 — Migrate `bands/` to `context-trace`

- [ ] Create `context-trace/src/graph/vertex/traversal.rs`.
- [ ] Copy `BandIterator`, `BandExpandingIterator`, `HasTokenRoleIters`, `PostfixIterator`, `PrefixIterator`, `PostfixExpandingPolicy`, `PrefixExpandingPolicy`, `BandExpandingPolicy` into it.
- [ ] Add `pub mod traversal;` to `context-trace/src/graph/vertex/mod.rs`.
- [ ] Re-export `HasTokenRoleIters`, `PostfixIterator`, `PrefixIterator` from `context-trace/src/lib.rs`.
- [ ] In `context-read`: replace all `use crate::bands::HasTokenRoleIters` with `use context_trace::HasTokenRoleIters`.
- [ ] Delete `context-read/src/bands/mod.rs` and `bands/policy.rs`.
- [ ] Verify: `cargo check -p context-trace && cargo check -p context-read` — zero errors.
- [ ] Search workspace for any other postfix/prefix traversal patterns that can now use the migrated types.

### A2 — Delete dead code files

- [ ] Delete `src/expansion/stack.rs` and remove its `pub(crate) mod stack;` declaration from `expansion/mod.rs`.
- [ ] Delete `src/expansion/cursor.rs`. Audit `tests/cursor.rs` — re-anchor any tests that used `CursorCtx` to use `ExpansionCtx` directly; delete tests with no valid replacement.
- [ ] Delete `src/expansion/chain/op.rs` and remove its `mod op;` declaration.
- [ ] Verify: `cargo test -p context-read` — same pass/fail counts as baseline.

### A3 — Move `grammar.rs` to `benches/`

- [ ] Create `context-read/benches/grammar.rs` with the content of `tests/grammar.rs`, wrapped in a `criterion` bench harness.
- [ ] Add `[dev-dependencies] criterion = "0.5"` and `[[bench]] name = "grammar" harness = false` to `context-read/Cargo.toml`.
- [ ] Remove `pub(crate) mod grammar;` from `tests/mod.rs`.
- [ ] Remove the `[[bin]]` target and `main.rs` from `Cargo.toml`.
- [ ] Delete `src/main.rs`.
- [ ] Verify: `cargo bench -p context-read --bench grammar` compiles (does not need to produce meaningful numbers yet).

### A4 — Rename complement stub

- [ ] In `complement.rs`, rename `build_complement_trace_cache` → `build_trace_cache_stub`.
- [ ] Add a single `// DESIGN SESSION REQUIRED: see agents/designs/20260315_DESIGN_COMPLEMENT_PATH_BUILDING.md` comment on the stub function.
- [ ] Verify: `cargo test -p context-read` — baseline unchanged.

---

## Pass B — Rename and Reorganise

**Goal:** Rename `context/` → `pipeline/`; introduce `input.rs`; expose clean `pub fn read`. No logic changes. Tests must remain green.

### B1 — Rename `context/` → `pipeline/`

- [ ] Create `src/pipeline/` directory.
- [ ] Move `src/context/mod.rs` → `src/pipeline/mod.rs`; update all internal imports.
- [ ] Move `src/context/root.rs` → `src/pipeline/root.rs`; update all internal imports.
- [ ] Update `src/lib.rs`: `pub(crate) mod pipeline;` replaces `pub mod context;`.
- [ ] Update all `use crate::context::` references throughout the crate.
- [ ] Verify: `cargo test -p context-read` — baseline unchanged.

### B2 — Introduce `input.rs`

- [ ] Create `src/input.rs`.
- [ ] Define `pub trait IntoReadInput` with a single method `fn into_read_input(self, graph: &HypergraphRef) -> NewAtomIndices`.
- [ ] Move `impl ToNewAtomIndices for Chars<'_>` and the `RequestInput` → `NewAtomIndices` conversion from `segment.rs` / `request.rs` into impls of `IntoReadInput` for `&str`, `String`, `impl Iterator<Item=char>`, `impl Read + 'static`, `NewAtomIndices`.
- [ ] Move the `HasReadCtx` tuple impls from `context/has_read_context.rs` into `input.rs` as `pub(crate)`.
- [ ] Delete `src/request.rs` and `src/context/has_read_context.rs`.
- [ ] Update `src/lib.rs`: add `pub(crate) mod input;`; remove `pub(crate) mod request;`.
- [ ] Verify: `cargo test -p context-read` — baseline unchanged.

### B3 — Expose `pub fn read`

- [ ] In `src/lib.rs`, add:
  ```rust
  pub fn read(graph: &HypergraphRef, input: impl IntoReadInput) -> Option<Token> {
      pipeline::ReadCtx::new(graph.clone(), input).read_sequence()
  }
  ```
- [ ] Ensure `ReadSequenceIter` and `SegmentResult` remain `pub` re-exports.
- [ ] Update `context-read/README.md` and `agents/docs/README.md` to reflect the new entry point.
- [ ] Verify: `cargo test -p context-read` — baseline unchanged; `cargo check -p context-api` — zero errors.

---

## Pass C — Chain Consolidation

**Goal:** Merge the dead `BandExpansion`/`BandCap`/`ChainOp` types into `OverlapLink`; define `OverlapChain` as the growth path for `BandState`. This is a logic-adjacent change; tests must remain green and the 10 failing tests must not regress further.

### C1 — Clean up `chain/link.rs`

- [ ] Remove `BandExpansion`, `BandCap`, `ChainOp` from `chain/link.rs`.
- [ ] Remove `StartBound` and `EndBound` traits (redundant with `Band` fields).
- [ ] Add a `BandCapLink` variant to `OverlapLink` representing the terminal element of a chain (the complement-only node at the right end of a fully resolved chain). Document its invariant: a `BandCapLink` has no `search_path` because the chain is terminated.
- [ ] Verify: `cargo test -p context-read` — baseline unchanged.

### C2 — Define `OverlapChain`

- [ ] In `chain/mod.rs`, alongside `BandState`, define:
  ```rust
  pub(crate) struct OverlapChain {
      head: Token,
      links: Vec<OverlapLink>,
      tail: Token,
  }
  ```
- [ ] Add `OverlapChain::push(link: OverlapLink)` and `OverlapChain::cap(link: BandCapLink)` methods (stubs that panic — full implementation is Pass C3).
- [ ] Add `BandState::into_chain(self) -> OverlapChain` conversion that lifts the current `WithOverlap` state into the new type.
- [ ] Verify: `cargo test -p context-read` — baseline unchanged (stubs not called in production paths yet).

### C3 — Wire `OverlapChain` into `ExpansionCtx` (future, not this pass)

This step requires the complement fix (Q11 / design session). It is listed here for completeness and will be its own plan once the complement design session is complete.

- [ ] Replace the `BandState::WithOverlap` → `commit_state` path in `RootManager` with `OverlapChain::collapse`.
- [ ] Ensure the cursor loop in `ExpansionCtx` can accumulate multiple overlaps into a single `OverlapChain` before committing.

---

## Complement Design Stub

The complement trace-cache fix (`Q11`) is **not** part of this plan. It is tracked separately:

- Design session document: `agents/designs/20260315_DESIGN_COMPLEMENT_PATH_BUILDING.md` (to be created)
- Blocking tests: all 10 currently failing `context-read` unit tests involve overlap collapse, which calls `build_trace_cache_stub`.
- Pre-condition: the downward path from the anchor vertex to the intersection point must be walked and each `ChildLocation` collected into `TraceCache` before `insert_init` is called.

---

## Files Affected

### `context-read`

| File | Change |
|------|--------|
| `src/lib.rs` | Add `pub fn read`; update module declarations |
| `src/main.rs` | **Delete** |
| `src/request.rs` | **Delete** (absorbed into `input.rs`) |
| `src/input.rs` | **Create** — `IntoReadInput` trait + impls |
| `src/segment.rs` | Remove `ToNewAtomIndices` impls for `RequestInput` / `Chars` (moved to `input.rs`) |
| `src/context/mod.rs` | **Move** → `src/pipeline/mod.rs` |
| `src/context/root.rs` | **Move** → `src/pipeline/root.rs` |
| `src/context/has_read_context.rs` | **Delete** (absorbed into `input.rs`) |
| `src/bands/mod.rs` | **Delete** |
| `src/bands/policy.rs` | **Delete** |
| `src/expansion/mod.rs` | Update `use crate::bands::` → `use context_trace::` |
| `src/expansion/stack.rs` | **Delete** |
| `src/expansion/cursor.rs` | **Delete** |
| `src/expansion/block.rs` | No change |
| `src/expansion/link.rs` | No change |
| `src/expansion/chain/mod.rs` | Add `OverlapChain` struct (Pass C) |
| `src/expansion/chain/band.rs` | No change |
| `src/expansion/chain/link.rs` | Remove dead types; add `BandCapLink` variant |
| `src/expansion/chain/op.rs` | **Delete** |
| `src/complement.rs` | Rename stub function |
| `src/tests/mod.rs` | Remove `grammar` module |
| `src/tests/cursor.rs` | Audit; re-anchor or delete tests |
| `benches/grammar.rs` | **Create** — moved from `tests/grammar.rs` |
| `Cargo.toml` | Remove `[[bin]]`; add `criterion` dev-dep + `[[bench]]` |

### `context-trace`

| File | Change |
|------|--------|
| `src/graph/vertex/traversal.rs` | **Create** — migrated from `context-read/src/bands/` |
| `src/graph/vertex/mod.rs` | Add `pub mod traversal;` |
| `src/lib.rs` | Re-export `HasTokenRoleIters`, `PostfixIterator`, `PrefixIterator` |

---

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| `BandExpandingIterator` has lifetime params that complicate the `context-trace` re-export | Medium | Low | Introduce a `pub type` alias in `lib.rs` to hide the generic noise |
| Removing `HasReadCtx` tuple impls from `has_read_context.rs` breaks test call sites | Low | Low | `HasReadCtx` impls move to `input.rs`, not deleted |
| `cursor.rs` unit tests cannot be cleanly re-anchored to `ExpansionCtx` | Low | Low | Tests that require `CursorCtx` internals are deleted; coverage is provided by `linear.rs` and `overlapping.rs` |
| Pass C `OverlapChain` stubs accidentally enter a production path | Low | Medium | Stubs use `unreachable!()` with a clear message; `BandState::WithOverlap` path is unchanged until Pass C3 |
| `grammar.rs` bench depends on crate internals gated by `test-api` feature | Medium | Low | Add `features = ["test-api"]` to the `[[bench]]` target or make the bench public-API-only |

---

## Validation Criteria

Each pass must satisfy its own gate before the next pass begins.

**Pass A gate:**
- [ ] `cargo test -p context-read` — 70 pass / 10 fail (baseline preserved, no new failures)
- [ ] `cargo check -p context-trace` — zero errors
- [ ] `cargo bench -p context-read --bench grammar` — compiles

**Pass B gate:**
- [ ] `cargo test -p context-read` — 70 pass / 10 fail
- [ ] `cargo check -p context-api` — zero errors (public API is now `pub fn read`)
- [ ] `cargo test -p context-api` — no regressions

**Pass C gate:**
- [ ] `cargo test -p context-read` — 70 pass / 10 fail (no regression; chain stubs not exercised)
- [ ] `OverlapChain` type compiles with `push` and `cap` stubs
- [ ] `BandState::into_chain` converts correctly for the `"aabb"` test case (assert in a unit test)

---

## Notes

### Execution order within passes

Passes A and B are strictly non-breaking and can be executed in any order within each pass. Pass C depends on Pass A (dead type removal) being complete but does not depend on Pass B.

### Relationship to expansion loop redesign

The `OverlapChain` defined in Pass C2 is the structural foundation that Pass C3 (and the complement design session) will build on. The expansion loop redesign plan (`20260315_PLAN_EXPANSION_LOOP_REDESIGN.md`) remains the authoritative document for the algorithmic correctness work; this plan only establishes the data-type scaffolding.

### `context-read` README

The `README.md` and `agents/docs/README.md` currently describe the old `context/` structure and refer to `ReadContext` as the main type. Both must be updated in Pass B3 to describe `pub fn read` and `ReadSequenceIter` as the entry points.