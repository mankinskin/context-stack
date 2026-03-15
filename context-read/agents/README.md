# context-read — Agent Workspace

Agent documentation for the `context-read` crate.

## Directory Structure

```
agents/
  README.md          ← this file
  plans/             ← execution plans for refactorings and features
  interviews/        ← design Q&A sessions with recorded answers and implications
  designs/           ← algorithm and data-structure design documents
  analysis/          ← algorithm analysis, state investigations, failure post-mortems
  guides/            ← how-to guides and reference material
  docs/              ← MCP-served crate documentation (module-level)
```

## What Goes Where

| Type | Directory | When to create |
|------|-----------|----------------|
| Multi-step execution plan | `plans/` | Before any change touching >3 files or involving unclear scope |
| Design Q&A | `interviews/` | When design decisions need to be recorded with their rationale |
| Algorithm design | `designs/` | Before implementing non-trivial algorithms or data structures |
| Post-mortem / investigation | `analysis/` | After a bug investigation or algorithm comparison |
| How-to / reference | `guides/` | When a pattern or workflow is worth preserving for future agents |

## Naming Convention

All files use the same convention as the root `agents/` directories:

```
YYYYMMDD_<TYPE>_<SUBJECT_IN_CAPS>.md
```

Examples:
- `20260315_PLAN_CONTEXT_READ_RESTRUCTURE.md`
- `20260315_INTERVIEW_CONTEXT_READ_RESTRUCTURE.md`
- `20260315_DESIGN_COMPLEMENT_PATH_BUILDING.md`

## Active Work

| Priority | Document | Status |
|----------|----------|--------|
| High | [plans/20260315_PLAN_CONTEXT_READ_RESTRUCTURE.md](plans/20260315_PLAN_CONTEXT_READ_RESTRUCTURE.md) | 📋 ready |
| Blocking | [designs/20260315_DESIGN_COMPLEMENT_PATH_BUILDING.md](designs/20260315_DESIGN_COMPLEMENT_PATH_BUILDING.md) | 📋 design-session-pending |

## Crate Summary

`context-read` is the pipeline that reads a sequence of characters (or any
`IntoReadInput`) into the hypergraph and returns a `Token` — an addressable
vertex with fully built context relationships to all substrings.

### Key types (current)

| Type | Location | Role |
|------|----------|------|
| `ReadCtx` | `pipeline/mod.rs` | Orchestrator — owns `RootManager` and `SegmentIter` |
| `RootManager` | `pipeline/root.rs` | Tracks the running root token and all primitive mutations |
| `ReadSequenceIter` | `pipeline/mod.rs` | Public iterator — yields one `SegmentResult` per segment |
| `SegmentResult` | `pipeline/mod.rs` | Per-segment outcome (Unknown / Known / Mixed) |
| `ExpansionCtx` | `expansion/mod.rs` | Cursor loop over a known-atom block; yields `BandState` |
| `BlockExpansionCtx` | `expansion/block.rs` | Drives `ExpansionCtx` + `RootManager` for one segment |
| `BandState` | `expansion/chain/mod.rs` | Single or WithOverlap expansion state; collapses to a bundled token |
| `ComplementBuilder` | `complement.rs` | Builds the complement token for an overlap (stub — see designs/) |

### Public entry points (target after restructuring)

```rust
// One-shot read
pub fn read(graph: &HypergraphRef, input: impl IntoReadInput) -> Option<Token>;

// Composable / streaming
pub struct ReadSequenceIter { ... }
pub enum SegmentResult { Unknown { .. }, Known { .. }, Mixed { .. } }
```

### Test baseline (2026-03-15)

- `context-read` unit tests: **70 pass / 10 fail / 0 ignored**
- The 10 failing tests all involve overlap collapse, blocked by the complement
  trace-cache stub (`build_trace_cache_stub` in `complement.rs`).

## Cross-References

| Document | Location |
|----------|----------|
| Expansion loop redesign plan | [../../agents/plans/20260315_PLAN_EXPANSION_LOOP_REDESIGN.md](../../agents/plans/20260315_PLAN_EXPANSION_LOOP_REDESIGN.md) |
| UX improvement parent plan | [../../agents/plans/20260314_PLAN_CONTEXT_READ_UX_IMPROVEMENT.md](../../agents/plans/20260314_PLAN_CONTEXT_READ_UX_IMPROVEMENT.md) |
| context-read completion plan | [../../agents/plans/20260218_PLAN_CONTEXT_READ_COMPLETION.md](../../agents/plans/20260218_PLAN_CONTEXT_READ_COMPLETION.md) |
| Block-iter overlap expansion guide | [../../agents/guides/20260207_BLOCK_ITER_OVERLAP_EXPANSION.md](../../agents/guides/20260207_BLOCK_ITER_OVERLAP_EXPANSION.md) |
| Band-chain overlap links guide | [../../agents/guides/20260211_BANDCHAIN_OVERLAP_LINKS_GUIDE.md](../../agents/guides/20260211_BANDCHAIN_OVERLAP_LINKS_GUIDE.md) |
| Root update design | [../../agents/designs/20260315_DESIGN_ROOT_UPDATE_STEPS.md](../../agents/designs/20260315_DESIGN_ROOT_UPDATE_STEPS.md) |