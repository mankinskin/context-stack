# Interview: Search Algorithm Event Emission Refactoring

**Date:** 2026-03-03  
**Scope:** `context-search` (search algorithm, events, visualization)  
**Status:** ANSWERED — ready for plan creation

---

## Objective

Critically review the search algorithm's event emission system and refactor it to:

1. Eliminate `PoppedNode` and duplicated code
2. Use `Token` directly instead of decomposed `usize` fields
3. Design a trait-based approach for emitting `GraphOpEvent` from algorithm types
4. Ensure events are faithful copies of actual algorithm state
5. Emit comprehensive events for both **search path** and **query path** sides
6. Handle parent/child/root candidate status transitions correctly
7. Cover the `(start_path, root, end_path)` path model completely

---

## Current State Analysis

### Code Duplication Issues Found

#### 1. `PoppedNode` is unnecessary

[iterator.rs](crates/context-stack/context-search/src/match/iterator.rs#L25-L30) defines `PoppedNode`:
```rust
pub(crate) struct PoppedNode {
    pub node: SearchNode,
    pub node_index: usize,
    pub is_parent: bool,
}
```

`node_index` and `is_parent` are trivially derivable from `SearchNode`:
- `node_index` = `node.root_parent().index.0`
- `is_parent` = `matches!(node, SearchNode::ParentCandidate(_))`

`SearchNode` should have methods for these directly.

#### 2. `CompareInfo` uses raw `usize` instead of `Token`

[mod.rs](crates/context-stack/context-search/src/match/mod.rs#L43-L52):
```rust
pub(crate) struct CompareInfo {
    pub node: usize,
    pub node_width: usize,
    pub cursor_pos: usize,
    pub outcome: CompareOutcome,
}
```

Similarly `PrefixChildInfo`:
```rust
pub(crate) struct PrefixChildInfo {
    pub child: usize,
    pub child_width: usize,
}
```

Both should use `Token` directly. `Token { index: VertexIndex, width: TokenWidth }` already carries both fields.

#### 3. Duplicated pop+process logic in `SearchIterator`

`pop_and_process_one()` (~50 lines) duplicates the exact logic of `pop_node()` + `process_node()` combined. The split `pop_node`/`process_node` API exists for `SearchState` to inject `VisitParent` events between pop and process, but `pop_and_process_one` and `find_next_root_match` don't need the split. This creates two parallel code paths.

#### 4. Bloated event emission in `SearchState`

[search/mod.rs](crates/context-stack/context-search/src/search/mod.rs#L260-L340) — `emit_compare_events` manually translates `CompareInfo` fields into `Transition` variants with duplicated `EdgeRef` construction. The same `EdgeRef { from, to, pattern_idx: 0, sub_index: 0 }` pattern appears 6+ times.

#### 5. `finish_root_cursor` duplicates child extraction

Lines ~540-610 and ~590-630 in `SearchState::finish_root_cursor` repeat the same pattern:
```rust
let child_token = child_state.path.role_rooted_leaf_token::<End, _>(trav);
let child_idx = child_token.index.0;
let child_width = child_token.width.0;
let child_sub_index = child_state.root_child_index();
```

This extraction should be a method on `ChildState` or `CompareState` returning a `Token`.

#### 6. `Transition` variants use `usize` instead of `Token`

All `Transition` variants (`VisitParent`, `VisitChild`, `ChildMatch`, etc.) use `node: usize` and `width: usize` separately. Since `Token` is `Serialize`, it could be used directly (or a simpler viz-specific struct). This would propagate type safety through the entire event pipeline.

---

## Algorithm Phases & Event Chain

The search algorithm has distinct phases. Here's the expected event chain:

### Phase 1: BFS Queue Processing (SearchState::next)

```
StartNode { node, width }           — once at start
ParentExplore { root, candidates }  — initial parent candidates

loop {
  // For each candidate popped from queue:
  VisitParent { from, to }          — if parent candidate (ascending)
  VisitChild { from, to }           — child comparison step
  ChildMatch { node, cursor_pos }   — if child token matches query
  ChildMismatch { node, ... }       — if child token mismatches
  CandidateMismatch { node }        — if overall candidate rejected
  CandidateMatch { root }           — if candidate becomes confirmed root

  ParentExplore { root, candidates } — if node expanded (prefix decomposition)
}
```

### Phase 2: RootCursor Advancement (finish_root_cursor)

```
loop {
  // advance_to_next_match:
  //   Step 1: advance query cursor
  //   Step 2: advance child cursor  
  //   Step 3: compare tokens
  
  VisitChild { from, to }           — each successful child advancement
  ChildMatch { node, cursor_pos }   — each matched child token
  ChildMismatch { node, ... }       — mismatch ends this root
  
  // If root boundary reached:
  ParentExplore { root, candidates } — need parent exploration
}
```

### Phase 3: Completion

```
Done { final_node, success }
```

### Missing Events (Gaps)

1. **Query cursor advancement** — no events emitted when query advances to next token
2. **Child cursor exhaustion** — no event when child path runs out (BothCursorsAdvanceResult::ChildExhausted)
3. **Query exhaustion** — no event when query fully consumed (QueryExhausted)
4. **Checkpoint updates** — no events when `update_checkpoint()` is called
5. **Root cursor internal comparison** — `CompareIterator` in `advance_to_next_match` does prefix decomposition internally but emits no events

---

## Proposed Refactoring Design

### A. Eliminate `PoppedNode` — add methods to `SearchNode`

```rust
impl SearchNode {
    fn root_token(&self) -> Token { self.root_parent() }
    fn is_parent(&self) -> bool { matches!(self, Self::ParentCandidate(_)) }
}
```

Remove `PoppedNode` entirely. `SearchIterator::pop_node` returns `Option<SearchNode>`.

### B. Replace `CompareInfo` with `Token`-based struct

```rust
pub(crate) struct CompareInfo {
    pub token: PathNode,               // replaces node + node_width
    pub query_token: PathNode,          // NEW: the query side being compared
    pub cursor_pos: usize,
    pub outcome: CompareOutcome,
}

pub(crate) struct PrefixChildInfo {
    pub token: PathNode,                // replaces child + child_width
}
```

### C. Trait for event emission from algorithm types

```rust
/// Trait for types that can describe themselves as a GraphOp transition.
trait IntoTransition {
    fn into_transition(&self, ctx: &EventContext) -> Transition;
}
```

Where `EventContext` carries the accumulated `VizPathGraph`, cursor positions, etc.
Implemented for `CompareInfo`, `RootAdvanceResult`, `BfsStepResult`, etc.

### D. Use `PathNode` in `Transition` variants

Replace split `node: usize` + `width: usize` fields with `PathNode { index, width }` across all Transition variants. `PathNode` is already `Serialize + TS` with plain `usize` fields. Frontend TS types will be updated accordingly.

### E. Two-phase root status: tentative → confirmed

`VisitParent` sets a *tentative* root (distinct styling from confirmed root). `CandidateMatch` promotes tentative → confirmed. `CandidateMismatch` clears tentative root. This requires:
- A new field or variant to distinguish tentative vs confirmed root in `VizPathGraph`
- CSS styling for tentative root (e.g. dimmed gold vs bright gold)

### F. Separate query path event stream

Query path events emitted on a separate `path_id` (e.g. `search/context-search/query-token-42`). Each query step gets its own `VizPathGraph` tracking the query pattern traversal. This gives the frontend two synchronized paths to render:
- **Search path:** bottom-up parent exploration + top-down child comparison in the graph
- **Query path:** linear traversal through the query pattern tokens

### G. Unified event emission from `RootCursor`

Two-phase approach:

Two-phase approach:
1. **Phase 1:** Inline cleanup — extract helper methods for child token extraction + event emission in `finish_root_cursor`
2. **Phase 2:** Move emission into `RootCursor` — `RootCursor` accepts an event collector, emits events at the point of state change. Infrastructure reusable for search paths.

Currently `advance_to_next_match` returns result types but emits no events. `SearchState::finish_root_cursor` then manually reconstructs what happened. Phase 2 fixes this by having `RootCursor` emit events directly.

---

## Answers Summary

| Q | Decision |
|---|----------|
| Q1 | **Remove `pop_node`/`process_node` split** — `SearchState` uses `pop_and_process_one` with pre/post hooks for event injection. Remove the separate pop+process API. |
| Q2 | **Reuse `PathNode { index, width }`** — already exists, plain usize fields, Serialize+TS. Use in Transition variants and CompareInfo. |
| Q3 | **Trait-based `IntoTransition`** — `impl IntoTransition for CompareInfo`, `RootAdvanceResult`, etc. Events derived from result types at call site. |
| Q4 | **Separate event stream** — query events on a different `path_id` with their own `VizPathGraph`. |
| Q5 | **Two-phase:** First inline cleanup with helpers, then move emission into `RootCursor`. Search paths can also benefit from this infrastructure. |
| Q6 | **Two-phase tentative/confirmed** — `VisitParent` sets tentative root (different styling). `CandidateMatch` confirms it. |
| Q7 | **Populate with real data** — use `ChildState::root_child_index()` and real pattern data. |
| Q8 | **Maximum scope** — everything including Transition variant changes and frontend updates. |

---

## Questions

### Q1: `SearchIterator` — remove `pop_and_process_one` entirely?

Currently three consumer patterns exist:
- `pop_node()` + `process_node()` — used by `SearchState::next()` (with VisitParent injection)
- `pop_and_process_one()` — used by `find_next_root_match()` (no event injection)
- `find_next_root_match()` — used by `Iterator for SearchIterator`

`pop_and_process_one` is the duplicated path. Options:
1. **Remove `pop_and_process_one`**, rewrite `find_next_root_match` to use `pop_node` + `process_node`
2. **Remove `pop_node`/`process_node`**, make `SearchState` work with `pop_and_process_one` + pre/post hooks
3. **Keep both** but extract shared logic into a private method

`Iterator for SearchIterator` (the non-event-emitting path) is only used for the simple case. Should we remove it entirely and always go through `SearchState`?

**Answer:** Option 2 — Remove `pop_node`/`process_node`. `SearchState` will use `pop_and_process_one` with pre/post hooks for event injection. The separate pop/process split is unnecessary complexity.

### Q2: Should `Transition` variants use `Token` instead of `usize`?

Current `Transition` variants use `node: usize` and `width: usize` separately. Since `Transition` is `#[derive(Serialize, TS)]` and consumed by the TypeScript frontend:

1. **Use `Token` directly** — cleaner Rust, but changes the JSON shape and TS types
2. **Use a `VizNode { index: usize, width: usize }` struct** — similar to `PathNode` in search_path.rs but for Transition
3. **Keep `usize` fields** — no frontend changes needed; conversion happens at emission site

Note: `PathNode { index: usize, width: usize }` already exists in [search_path.rs](crates/context-stack/context-trace/src/graph/search_path.rs#L53-L60) and serves exactly this purpose. We could reuse it.

**Answer:** Option 2 — Reuse `PathNode { index: usize, width: usize }`. Already Serialize+TS, plain usize fields. Use in both Transition variants and `CompareInfo`/`PrefixChildInfo`.

### Q3: Event emission architecture — where should events originate?

The fundamental tension: algorithm code (in `match/`) shouldn't know about visualization, but events need to capture algorithm-internal state changes.

Options:
1. **Callback/visitor pattern** — pass `&mut dyn SearchEventSink` into `SearchIterator` and `RootCursor` methods. Events emitted at point of state change. Clean but invasive.
2. **Rich result types** — algorithm returns enough info in results for `SearchState` to reconstruct events. Current approach, but `CompareInfo` is too thin.
3. **Event log on iterator** — `SearchIterator` accumulates an internal event log that `SearchState` drains. Less invasive than callbacks.
4. **Trait-based** — `impl IntoTransition for CompareInfo`, `impl IntoTransition for RootAdvanceResult`, etc. Events are derived from result types.

Which approach do you prefer? The trait-based approach (4) seems cleanest for not coupling algorithm code to viz, while ensuring complete information.

**Answer:** Option 4 — Trait-based `IntoTransition`. Implement `IntoTransition` for `CompareInfo`, `RootAdvanceResult`, etc. Events are derived from result types at the call site, keeping algorithm code decoupled from visualization.

### Q4: Query path events — what granularity?

The query path (pattern cursor) advances in lockstep with the search path. You mentioned emitting events for query advancement. What should these look like?

Options:
1. **Mirror the search events** — `QueryAdvance { from_pos, to_pos, token }` alongside each search step
2. **Embedded in existing events** — add query-side fields to `ChildMatch`/`ChildMismatch` (e.g. `query_token: Token`, `query_from_pos`, `query_to_pos`)
3. **Separate event stream** — query events on a different `path_id` with their own `VizPathGraph`
4. **Extend `QueryInfo`** — already included in every `GraphOpEvent`; just make it more detailed

Currently `QueryInfo` has `cursor_position`, `matched_positions`, and `active_token`. This gives some query tracking but doesn't show individual query token advancement steps.

**Answer:** Option 3 — Separate event stream. Query events on a different `path_id` with their own `VizPathGraph`. This gives the query path first-class treatment with independent visualization, highlighting, and edge/node styling distinct from the search path.

### Q5: `finish_root_cursor` event emission — inline or return-based?

`finish_root_cursor` currently does a loop calling `advance_to_next_match` and manually emits events after each result. This is where most of the bloat lives (~150 lines of event code mixed with algorithm logic).

Options:
1. **Keep inline** but clean up with helper methods/traits (least disruptive)
2. **Return events from `advance_to_next_match`** — each advancement returns a `Vec<SearchEvent>` alongside the result
3. **Move event emission into `RootCursor`** — `RootCursor` takes an event collector parameter
4. **Extract `RootCursorEventEmitter` wrapper** — separates concerns without modifying core types

**Answer:** Two-phase approach — first inline cleanup with helpers (option 1), then move emission into `RootCursor` (option 3). The infrastructure built for RootCursor event emission can also benefit search paths more broadly.

### Q6: Parent candidate status — how to handle VisitParent → CandidateMatch flow?

Currently:
- `VisitParent` is emitted BEFORE processing (sets root on `VizPathGraph`)
- `CandidateMatch` is emitted after `FoundMatch` (confirms root, mostly informational)
- `CandidateMismatch` clears root

This means rejected parents briefly become root and then get cleared. Is this correct behavior for visualization, or should `VisitParent` NOT set root (only `CandidateMatch` would)?

**Answer:** Two-phase tentative/confirmed — `VisitParent` sets a *tentative* root (with distinct styling, e.g. different color/opacity from confirmed root). `CandidateMatch` promotes it to confirmed root. `CandidateMismatch` clears the tentative root. This gives the visualization a clear progression: candidate → tentative root → confirmed root (or rejected).

### Q7: `EdgeRef` construction — should it use real graph data?

Currently all `EdgeRef` fields use `pattern_idx: 0, sub_index: 0` as placeholders, and the test helpers explicitly strip `EdgeRef` during comparison. Should we:

1. **Populate with real data** from `ChildState::root_child_index()` and pattern info
2. **Keep placeholder** and acknowledge EdgeRef is approximate
3. **Remove EdgeRef from Transition**, put it only in `VizPathGraph` operations

The search path visualization (`VizPathGraph`) already tracks edges separately via `PathTransition`. Having `EdgeRef` in both `Transition` and `VizPathGraph` is potentially redundant.

**Answer:** Option 1 — Populate with real data. Use `ChildState::root_child_index()` and real pattern data for `from`/`to`/`pattern_idx`/`sub_index`. This makes EdgeRef accurate and usable by the frontend for precise edge highlighting.

### Q8: Scope of this refactoring — what's in vs. out?

Proposed in-scope:
- Remove `PoppedNode`
- Add methods to `SearchNode` (`root_token()`, `is_parent()`)
- Refactor `CompareInfo` / `PrefixChildInfo` to use `Token`
- Remove `pop_and_process_one` duplication
- Clean up `finish_root_cursor` event emission
- Add query path event fields
- Define the canonical event chain

Proposed out-of-scope (follow-up):
- Changing `Transition` variants to use `Token`/`PathNode` (frontend impact)
- Implementing `IntoTransition` trait (depends on architecture decision)
- Changing `RootCursor` to accept event callbacks (invasive)
- Frontend changes for query path visualization

**Answer:** Maximum scope — everything is in scope, including:
- Changing `Transition` variants to use `PathNode`
- Implementing `IntoTransition` trait
- Moving event emission into `RootCursor`
- Two-phase tentative/confirmed root status
- Populating `EdgeRef` with real data
- Separate query path event stream
- Frontend updates

This is a large refactoring that should be broken into ordered execution steps in the plan.

---

## Canonical Event Chain (Proposed)

Here's the complete event sequence for a typical parent search:

```
// === Phase 1: BFS Queue — find root match ===

StartNode { node: start_token, width }
ParentExplore { current_root: start_token, candidates: [p1, p2] }

// Try parent p1:
VisitParent { from: start_token, to: p1 }
VisitChild { from: p1, to: c1 }              // compare p1's child against query
ChildMismatch { node: c1, cursor_pos, expected, actual }
CandidateMismatch { node: p1, queue_remaining, is_parent: true }

// Try parent p2:
VisitParent { from: start_token, to: p2 }
VisitChild { from: p2, to: c2 }              // compare p2's child against query
ChildMatch { node: c2, cursor_pos }
CandidateMatch { root: p2 }                  // p2 confirmed as root

// === Phase 2: RootCursor — extend match through children ===

// advance_to_next_match iteration 1:
VisitChild { from: p2, to: c3 }
ChildMatch { node: c3, cursor_pos }

// advance_to_next_match iteration 2:
VisitChild { from: p2, to: c4 }
ChildMismatch { node: c4, cursor_pos, expected, actual }
// → Mismatch after progress → maximum match found

// === OR: root boundary reached ===
// VisitChild { from: p2, to: c3 }
// ChildMatch { node: c3, cursor_pos }
// → ChildExhausted → need parent exploration
// ParentExplore { current_root: p2, candidates: [p3] }
// ... continue BFS with p3 ...

// === Phase 3: Done ===
Done { final_node: Some(p2), success: true }
```

### With Query Path Events (proposed additions in bold)

Each `VisitChild`/`ChildMatch`/`ChildMismatch` should carry **query-side** info:

```
VisitChild { from: p2, to: c2, query_pos: 0, query_token: q1 }
ChildMatch { node: c2, cursor_pos: 1, query_token: q1 }

VisitChild { from: p2, to: c3, query_pos: 1, query_token: q2 }
ChildMatch { node: c3, cursor_pos: 2, query_token: q2 }
```

This pairs each search-path step with the corresponding query-path position.

---

## Files Affected

| File | Changes |
|------|---------|
| `crates/context-stack/context-search/src/match/mod.rs` | `SearchNode` methods, `CompareInfo`→PathNode, `PrefixChildInfo`→PathNode |
| `crates/context-stack/context-search/src/match/iterator.rs` | Remove `PoppedNode`, `pop_node`, `process_node`; keep `pop_and_process_one` as primary API |
| `crates/context-stack/context-search/src/search/mod.rs` | Clean up `emit_compare_events`, `finish_root_cursor`, add `IntoTransition` impls, hooks around `pop_and_process_one` |
| `crates/context-stack/context-search/src/match/root_cursor/advance.rs` | Add event collector to `advance_to_next_match` (phase 2) |
| `crates/context-stack/context-trace/src/graph/visualization.rs` | Refactor Transition variants to use `PathNode`, add tentative root concept, query path Transitions |
| `crates/context-stack/context-trace/src/graph/search_path.rs` | Extend `VizPathGraph` for tentative root, update `apply_transition` |
| `crates/context-stack/context-search/src/tests/search/event_helpers.rs` | Update test helpers for new event shapes |
| `tools/log-viewer/frontend/src/` | Update TS types for PathNode in Transitions, tentative root styling, query path rendering |

---

## Next Steps

1. **Answer questions above** to resolve design decisions
2. **Create implementation plan** in `agents/plans/` with step-by-step execution order  
3. **Execute in order:** SearchNode methods → CompareInfo refactor → iterator cleanup → event emission cleanup → query path events → tests
