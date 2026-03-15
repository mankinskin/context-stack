---
tags: `#interview` `#context-read` `#api` `#restructuring` `#architecture`
summary: Design interview for context-read module restructuring, public API shape, dead code disposition, and overlap chain design.
status: ✅ complete
date: 2026-03-15
plan: 20260315_PLAN_CONTEXT_READ_RESTRUCTURE.md
---

# Interview: context-read Restructuring & Public API Design

**Date:** 2026-03-15
**Scope:** context-read crate — module layout, public API, dead code, chain design, infrastructure migration

---

## Q1 — What does a caller need from this crate?

**Q:** What is the *one thing* a caller outside this crate needs to do — read a string into a graph and get back a token? Or does a caller also need to stream segments, inspect intermediate results, or hook into the expansion loop? Is `ReadSequenceIter` a genuine caller need or is it only there for internal testing?

**A:** Provide text in any form and add it to a graph to get a root token, searchable in a context space. Fully connected context relationships of substrings and their semantic space. Transitions and weights can be counted/calculated to find meaningful tokens (future advancement required).

**Implications:**
- The primary return value is a `Token` — an addressable node in the graph with fully built context relationships.
- `ReadSequenceIter` is a genuine caller need: it is the composable form for repeated REPL calls, yielding one `SegmentResult` per step.
- `SegmentResult` stays `pub` so callers can inspect per-segment progress.
- Future work: transition weights and frequency counts will be attached to tokens/edges, but that is not part of this restructuring.

---

## Q2 — Should `ReadRequest` become the public entry point or be removed?

**Q:** `ReadRequest` and `ReadRequestBuilder` are entirely `pub(crate)`. Is the intent that `ReadRequest` eventually becomes the public entry point (replacing the `HasReadCtx` tuple trick), stays internal, or is removed entirely? What should calling this crate look like from the outside?

**A:** Find something composable for repeated calls in a REPL.

**Implications:**
- `ReadRequest` and `RequestInput` are collapsed into a blanket `IntoReadInput` trait so any text-like type converts naturally.
- The tuple trick `(&mut graph, "text".chars()).read_sequence()` stays as a `pub(crate)` test convenience via `HasReadCtx`.
- The public surface is: `pub fn read(graph: &HypergraphRef, input: impl IntoReadInput) -> Option<Token>` plus `pub struct ReadSequenceIter` for composable repeated calls.

---

## Q3 — What is the `context-read` binary for?

**Q:** The crate has a `main.rs` binary target. What is it for — a dev harness, a CLI tool, or vestigial?

**A:** A simple binary that calls the read function and produces a file representation of the parsed graph.

**Implications:**
- `context-cli` already provides this (`read-sequence`, `read-file`, `export-workspace` commands, workspace persistence, REPL).
- The `context-read` binary target is removed from `Cargo.toml`.
- `grammar.rs` (currently called by `main.rs` behind a feature flag) moves to `benches/grammar.rs` as the seed of a benchmark collection.

---

## Q4 — `segment.rs`: should IO/decoding split from classification?

**Q:** `segment.rs` owns atom classification, the lazy UTF-8 decoder, the eager path, `SegmentIter`, and `ToNewAtomIndices`. Does the UTF-8 streaming belong here or in a higher IO layer?

**A:** Gather all functions and restructure in a unified plan.

**Implications:**
- All input resolution (eager, lazy chars, `impl Read`, `NewAtomIndices`) lives in a single `input.rs` module via the `IntoReadInput` trait.
- `segment.rs` keeps segmentation (`SegmentIter`, `NextSegment`, `LazyAtomIter`, `Utf8CharIter`) — these are inherently tied to the atom classification boundary and belong together.
- `ToNewAtomIndices` moves into `input.rs` as an implementation detail of `IntoReadInput`.

---

## Q5 & Q6 — `context/mod.rs` and `RootManager` — same answer as Q4

**Q5:** Should `ReadCtx`, `ReadSequenceIter`, and `SegmentResult` stay together? Should `ReadCtx` stay hidden?

**Q6:** Should `RootManager` borrow the graph or own it?

**A:** Same as Q4 — gather and restructure in a unified plan.

**Implications:**
- `context/mod.rs` → `pipeline/mod.rs` (orchestrator, contains `ReadCtx` + `ReadSequenceIter` + `SegmentResult`).
- `context/root.rs` → `pipeline/root.rs` (unchanged except module path).
- `context/has_read_context.rs` → merged into `input.rs`.
- `ReadCtx` stays `pub(crate)`; `ReadSequenceIter` and `SegmentResult` are `pub`.
- `RootManager` continues to own `HypergraphRef` (shared-ref clone, not exclusive ownership) — no change needed.

---

## Q7 — `stack.rs` and `cursor.rs`: planned infrastructure or dead code?

**Q:** `OverlapStack`, `StackBand`, `CursorCtx` appear entirely unused in production paths. Are these planned future infrastructure or should they be deleted?

**A:** We want to use a chain structure to ensure complete tight packing of new root vertices. We can store transitions between overlaps as links in a chain where each overlap can be tightly organised for maximum coverage and reuse.

**Implications:**
- `stack.rs` is deleted as-is — the struct shapes (`OverlapStack`, `StackBand`) pre-date the current `BandState`/`OverlapLink` design and are incompatible.
- The chain concept is the correct direction: `BandState` evolves into an `OverlapChain` — a linked list of `OverlapLink`s with a head token and a tail token, replacing the current flat two-state enum.
- `cursor.rs` (`CursorCtx`) is deleted; cursor unit tests are re-anchored to `ExpansionCtx` directly.

---

## Q8 — Dead types in `chain/link.rs`: merge with existing overlap logic?

**Q:** `BandExpansion`, `BandCap`, `ChainOp`, `StartBound`, `EndBound` are unused in production. The overlap is high with existing types. Can these be merged?

**A:** The overlap is high. We can merge these with existing overlap logic for a self-documenting algorithm.

**Implications:**
- `BandExpansion` ≈ `OverlapLink` + `IndexWithPath` outcome — same concept described twice; merge into `OverlapLink` variants.
- `BandCap` = the terminal element of an `OverlapChain` — becomes a variant of the chain, not a standalone struct.
- `ChainOp` = replaced by `OverlapChain::push` / `OverlapChain::cap` methods.
- `StartBound` / `EndBound` traits are redundant with `Band::start_bound` / `end_bound` fields — deleted.
- `chain/op.rs` is deleted (currently empty).

---

## Q9 — `grammar.rs`: benchmark or delete?

**Q:** `grammar.rs` in `tests/` is a worst-case grammar builder with no `#[test]` attributes, called only from `main.rs` behind a feature flag. Benchmark fixture or delete?

**A:** We can start a benchmark collection with this.

**Implications:**
- `tests/grammar.rs` moves to `benches/grammar.rs`.
- A `[[bench]]` target is added to `context-read/Cargo.toml` using `criterion`.
- The `[[bin]]` target and `test-api` feature usage in `main.rs` are removed.

---

## Q10 — `bands/` traversal machinery: move to `context-trace`?

**Q:** `HasTokenRoleIters`, `PostfixIterator`, `PrefixIterator`, and their policies operate purely on graph structure. Should they move to `context-trace`?

**A:** Yes — move it to `context-trace` and ensure any duplicated infrastructure is replaced.

**Implications:**
- `bands/mod.rs` and `bands/policy.rs` are deleted from `context-read`.
- `HasTokenRoleIters`, `BandExpandingIterator`, `PostfixIterator`, `PrefixIterator`, `PostfixExpandingPolicy`, `PrefixExpandingPolicy` move to `context-trace/src/graph/vertex/traversal.rs`.
- `context-trace`'s `lib.rs` re-exports them from the `pub` surface.
- All `use crate::bands::HasTokenRoleIters` references in `context-read` become `use context_trace::HasTokenRoleIters`.
- Any duplicate postfix/prefix traversal patterns elsewhere in the workspace are identified and replaced.

---

## Q11 — `complement.rs`: stub or fixable now?

**Q:** `build_complement_trace_cache` returns `TraceCache::new(root)` unconditionally. This is on the critical path for the 10 remaining failing tests. Can it be fixed now?

**A:** This has to be fixed by building the complement from the downward path into the anchor. We may need a proper design session for the complement building of paths if you need help after your next research round.

**Implications:**
- No code change to `complement.rs` in this restructuring pass.
- `build_complement_trace_cache` is renamed to `build_trace_cache_stub` so the name signals incompleteness.
- A separate design session is required before implementing: the complement must be built by following the downward path from the anchor vertex to the intersection point, collecting each child location into the `TraceCache`.
- This is tracked as a blocking item in the restructuring plan with a dedicated `designs/` entry.