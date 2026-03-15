# context-read — Crate Documentation

Reference documentation for the modules and types in `context-read`.
Generated and maintained by agents; consumed by the MCP docs server.

> **Looking for plans, designs, or interview records?**
> Those live one level up in the agent workspace:
> - [`../plans/`](../plans/INDEX.md) — execution plans
> - [`../interviews/`](../interviews/INDEX.md) — design Q&A sessions
> - [`../designs/`](../designs/INDEX.md) — algorithm design documents
> - [`../analysis/`](../analysis/INDEX.md) — investigations and post-mortems
> - [`../guides/`](../guides/INDEX.md) — how-to guides
> - [`../README.md`](../README.md) — agent workspace overview

---

## Modules

| Module | Description |
|--------|-------------|
| `pipeline/` | Orchestrator layer — `ReadCtx`, `ReadSequenceIter`, `SegmentResult`, `RootManager` |
| `expansion/` | Cursor loop, overlap detection, band states, complement building |
| `expansion/chain/` | `BandState`, `Band`, `OverlapLink`, `OverlapChain` (in progress) |
| `complement.rs` | `ComplementBuilder` — builds the prefix complement token for an overlap (stub — see designs/) |
| `segment.rs` | `SegmentIter`, `NextSegment`, `LazyAtomIter`, `Utf8CharIter` |
| `input.rs` | `IntoReadInput` trait — unifies all text input sources (target layout; currently `request.rs`) |

> **Note:** The module layout above reflects the *target* structure after
> `20260315_PLAN_CONTEXT_READ_RESTRUCTURE.md` Pass B is complete.
> The current on-disk layout uses `context/` instead of `pipeline/` and
> `request.rs` instead of `input.rs`.

---

## Public Entry Points

### After restructuring (target)

```rust
// One-shot: read any text input into the graph, get back the root token.
pub fn read(graph: &HypergraphRef, input: impl IntoReadInput) -> Option<Token>;

// Composable: yield one SegmentResult per segment — suitable for REPL loops.
pub struct ReadSequenceIter { ... }
pub enum SegmentResult {
    Unknown { atoms: Pattern, root: Option<Token> },
    Known   { pattern: Pattern, root: Option<Token> },
    Mixed   { unknown_atoms: Pattern, known_pattern: Pattern, root: Option<Token> },
}
```

### Current (pre-restructuring)

```rust
// context-api surface (WorkspaceManager methods)
pub fn read_sequence(&mut self, ws: &str, text: &str) -> Result<PatternReadResult, ReadError>;
pub fn read_file(&mut self, ws: &str, path: &str) -> Result<PatternReadResult, ReadError>;

// crate-internal helpers used by context-api
pub(crate) struct ReadCtx { ... }       // ReadCtx::new(graph, input).read_sequence()
pub struct ReadSequenceIter { ... }     // yields SegmentResult
pub enum SegmentResult { ... }
```

---

## Key Concepts

### Segmentation

The input atom stream is partitioned into alternating *unknown* runs (characters
seen for the first time — inserted into the graph immediately) and *known* runs
(characters whose atom vertex already exists). Each `NextSegment` carries both a
`known: Pattern` and an `unknown: Pattern`; unknown atoms are appended to the
root before the known atoms are expanded.

### Expansion Loop

For each known-atom segment, `BlockExpansionCtx` drives `ExpansionCtx` over the
atom slice. `ExpansionCtx` calls `insert_next_match` to find the largest compound
token starting at the cursor, then checks whether any postfix of the current
anchor overlaps with the incoming token. If an overlap is found it yields
`BandState::WithOverlap`; otherwise it yields `BandState::Single`. Each state is
committed to `RootManager` before the cursor advances.

### Root Manager

`RootManager` owns the `HypergraphRef` and the running `root` token. It tracks
`anchor` (last committed expansion result, used as left-side context for the next
overlap search) and `flat_root` (whether the root is a mutable unknown-atom
container or a semantic compound that must not be mutated in-place).

### Complement Building

When an overlap is committed via `BandState::WithOverlap`, the `collapse()`
method builds the complement token — the prefix of the anchor that lies to the
left of the overlap region. This is currently a stub: see
[`../designs/20260315_DESIGN_COMPLEMENT_PATH_BUILDING.md`](../designs/20260315_DESIGN_COMPLEMENT_PATH_BUILDING.md).

### OverlapChain (planned)

A linked list of `OverlapLink`s with a head token and a tail token. Replaces
the flat `BandState::WithOverlap` two-state enum so that successive overlaps
within a single segment can be accumulated and collapsed in a single pass. Defined
as a stub in Pass C of the restructuring plan.

---

## Test Baseline (2026-03-15)

| Suite | Pass | Fail | Ignored |
|-------|------|------|---------|
| `context-read` unit tests | 70 | 10 | 0 |

The 10 failing tests all involve overlap collapse paths, blocked by
`build_trace_cache_stub` in `complement.rs`. See the complement design document.