---
tags: `#plan` `#context-trace` `#context-search` `#debugging` `#testing` `#performance`
summary: 1. **Generic Path Structures** - Made `SubPath`, `RolePath`, `RootedRolePath`, and `RootedRangePath` generic over node type `N`
status: 📋
---

# Plan: Position-Annotated Paths for Cache Tracing

**Created:** 2025-11-21  
**Status:** Infrastructure Complete - Ready for Implementation  
**Related Issue:** find_pattern1 test failure - cache entries at wrong positions

## Progress Summary

### ✅ Completed: Infrastructure Phase

1. **Generic Path Structures** - Made `SubPath`, `RolePath`, `RootedRolePath`, and `RootedRangePath` generic over node type `N`
   - `SubPath<N = ChildLocation>` - defaults to `ChildLocation` for backward compatibility
   - `RolePath<R, N = ChildLocation>` - role-based paths with generic nodes
   - `RootedRangePath<Root, EndNode = ChildLocation>` - end path can use different node type
   
2. **Position Annotation Wrapper** - Created `PositionAnnotated<N>` in `sub_path.rs`
   ```rust
   pub struct PositionAnnotated<N> {
       pub node: N,
       pub position: AtomPosition,
   }
   ```

3. **Type Aliases** - Created convenient aliases
   - `IndexRangePath` = `RootedRangePath<IndexRoot, ChildLocation>` (existing, unchanged)
   - `IndexRangePathWithPositions` = `RootedRangePath<IndexRoot, PositionAnnotated<ChildLocation>>` (new)

4. **Append Traits** - Extended path mutation with position tracking
   - `PathAppend` - existing trait for regular paths
   - `PathAppendWithPosition` - new trait for position-annotated paths
   
5. **Helper Methods** - Added position extraction
   - `SubPath<PositionAnnotated<_>>::entry_position()` - get first position
   - `RolePath<_, PositionAnnotated<_>>::entry_position()` - get first position  
   - `RootedRangePath<_, PositionAnnotated<_>>::end_entry_position()` - get end path entry position
   - `IndexRangePath::with_positions(AtomPosition)` - convert to position-annotated version

6. **Exports** - All new types and traits exported from context-trace:
   - `PositionAnnotated`
   - `PathAppendWithPosition`
   - `IndexRangePathWithPositions`

### Current State

- ✅ All code compiles
- ✅ context-trace tests pass (56 tests)
- ❌ **find_pattern1 still fails** (position 3 instead of 2) - infrastructure ready but not yet used
- 🔧 Ready to implement actual position tracking during traversal

**Next steps:**
1. Use `with_positions()` to convert paths to position-annotated versions
2. Track checkpoint position when appending to end paths during parent exploration
3. Extract entry position from annotated end path in `create_end_state()`

## Objective

Modify path structures to track checkpoint positions at each level during top-down traversal, enabling accurate cache tracing for deep hierarchies with partial matches.

## Problem Context

### Current Issue
- `root_pos` calculation is wrong because we try to calculate entry positions retroactively
- By the time we create `MatchResult`, checkpoint has been updated multiple times
- We've lost track of "what position were we at when entering each child"
- Tests expect cache entries at position 2, but we're recording position 1 (BU) or 3 (TD)

### Example from find_pattern1
```
Query: [a, b, y, x]
Graph: xab=[x,a,b], yz=[y,z], xabyz=[xab,yz]

Flow:
1. In xab, match "ab" → checkpoint at position 2
2. Go to parent xabyz (checkpoint still at 2)
3. Enter child yz to look for "y"
4. BEFORE matching "y", checkpoint is at position 2 ← Cache should use this!
5. Match "y" → checkpoint advances to position 3
6. Try "x", get mismatch
7. Create MatchResult with checkpoint=3 (WRONG - should be 2)

Expected: All cache entries at position 2
Actual: Position 1 (BU) or position 3 (TD)
```

### Root Cause
Retroactive calculation doesn't work:
- `checkpoint.atom_position - end_path_width` gives inconsistent results
- End path width calculation is complex and error-prone
- Fundamental mismatch: we need historical data, not calculated inference

## User's Proposed Solution

Track positions **during** traversal, not calculate afterward.

### Position-Annotated Path Structure

Instead of: `Vec<ChildLocation>`  
Use: `Vec<(ChildLocation, AtomPosition)>` or similar structure

Each entry records:
- **ChildLocation**: Where in the graph (parent token, pattern, sub_index)
- **AtomPosition**: What checkpoint position when we **entered** this child

### Example with Deeper Hierarchy

```
Query: [a, b, y, x, w]
Graph: xabyxz with nested structure

Traversal:
1. Match "ab" → checkpoint=2
2. Explore parent, enter yxz → record ("yxz", 2)
3. Match "y" → checkpoint=3, append ("y", 2) to path
4. Match "x" → checkpoint=4, pop ("y", 2), append ("x", 3)
5. Try "w", mismatch

Result: path = [("yxz", 2), ("x", 3)]
Use position 2 for yxz cache entry, position 3 for x cache entry
```

### Key Insight: Different Handling for Start vs End Paths

**Start paths** (bottom-up):
- Can trace cache immediately as we traverse
- No need to defer or store positions

**End paths** (top-down):
- Must defer cache tracing until we find the end
- Need to store positions as we go
- On mismatch or query exhaustion, use stored positions

## Analysis

### Current Architecture

**ChildState** (`context-trace/src/trace/child/state.rs`):
```rust
pub struct ChildState {
    pub current_pos: AtomPosition,  // Current position in graph child
    pub path: IndexRangePath,        // Path through graph
}
```

**IndexRangePath** (`context-trace/src/path/structs/rooted/index_range.rs`):
```rust
pub struct IndexRangePath {
    start: RolePath<Start>,
    end: RolePath<End>,
    root: Token,
}
```

**RolePath<R>** (`context-trace/src/path/structs/role_path.rs`):
```rust
pub struct RolePath<R: PathRole> {
    sub_path: SubPath,
    _ty: PhantomData<R>,
}
```

**SubPath** (`context-trace/src/path/structs/sub_path.rs`):
```rust
pub struct SubPath {
    root_entry: usize,
    path: Vec<ChildLocation>,  // ← Currently just locations
}
```

**ChildLocation** (`context-trace/src/graph/vertex/location/child.rs`):
```rust
pub struct ChildLocation {
    pub parent: Token,
    pub pattern_id: PatternId,
    pub sub_index: usize,
}
```

### Design Options

#### Option 1: Parallel Position Vector
Add separate vector to ChildState:
```rust
pub struct ChildState {
    pub current_pos: AtomPosition,
    pub path: IndexRangePath,
    pub path_positions: Vec<AtomPosition>,  // NEW: positions when entering each child
}
```

**Pros:**
- Minimal changes to existing path structures
- Easy to add/remove positions
- Clear separation of concerns

**Cons:**
- Two parallel structures to keep in sync
- Need to ensure both vectors have same length
- Duplication of traversal logic

#### Option 2: Generic SubPath/RolePath
Make SubPath generic over node type:
```rust
pub struct SubPath<N = ChildLocation> {
    root_entry: usize,
    path: Vec<N>,
}

// Use with position-annotated nodes
type AnnotatedSubPath = SubPath<(ChildLocation, AtomPosition)>;
```

**Pros:**
- Single structure maintains consistency
- Generic design supports future extensions
- Natural fit with Rust type system

**Cons:**
- Requires updating many trait implementations
- May complicate existing code that assumes ChildLocation
- Needs careful handling of PhantomData and type parameters

#### Option 3: Optional Position Tracking
Add optional position field, only used when needed:
```rust
pub struct SubPath {
    root_entry: usize,
    path: Vec<ChildLocation>,
    positions: Option<Vec<AtomPosition>>,  // NEW: optional position tracking
}
```

**Pros:**
- Zero-cost when not needed
- Backward compatible
- Easy to add to existing structures

**Cons:**
- Adds complexity with Option handling
- Still need parallel vector synchronization
- Less type-safe than generic approach

#### Option 4: Wrapper Structure
Create new structure that wraps existing paths:
```rust
pub struct PositionAnnotatedPath {
    path: IndexRangePath,
    end_positions: Vec<AtomPosition>,  // Positions for end path only
}
```

**Pros:**
- No changes to existing structures
- Clear that this is for special use case
- Easier to reason about scope

**Cons:**
- Additional indirection
- May need to duplicate path manipulation methods
- Unclear how to integrate with existing traits

### Recommended Approach: Option 1 (Parallel Vector) + Careful Scoping

**Rationale:**
1. **Minimal disruption** - doesn't require changing core path structures
2. **Clear semantics** - positions are explicitly for end paths during top-down search
3. **Easy to validate** - can compare vector lengths as invariant
4. **Scoped usage** - only used in specific contexts (parent exploration with mismatches)

**Implementation:**
- Add `end_path_positions: Vec<AtomPosition>` to `ChildState`
- Update when appending to end path during top-down traversal
- Use in `create_end_state()` to get correct `root_pos`

## Execution Steps

### Step 1: Add Position Tracking to ChildState
**File:** `crates/context-stack/context-trace/src/trace/child/state.rs`

```rust
pub struct ChildState {
    pub current_pos: AtomPosition,
    pub path: IndexRangePath,
    pub end_path_positions: Vec<AtomPosition>,  // NEW: checkpoint when entering each end path child
}
```

**Changes needed:**
- Update constructor (if any)
- Update `PathAppend` implementation to also track positions
- Add helper methods to access position for a given path level

### Step 2: Track Positions During Traversal
**File:** `crates/context-stack/context-search/src/match/root_cursor.rs`

**Where to record positions:**
- In `advance_child()` - when entering a child during top-down search
- Before `mark_match()` updates checkpoint
- Only for end path (top-down), not start path (bottom-up)

**Pseudocode:**
```rust
// When entering child during top-down search
if traversing_end_path {
    child_state.end_path_positions.push(checkpoint.atom_position);
}
child_state.path.end_path.push(child_location);
```

### Step 3: Use Stored Positions in create_end_state
**File:** `crates/context-stack/context-search/src/match/root_cursor.rs` (lines 567-625)

```rust
fn create_end_state(...) -> MatchResult {
    // Get root_pos from stored positions
    let root_pos = if let Some(&entry_pos) = path.end_path_positions.first() {
        entry_pos  // Position when entering first end path child
    } else {
        checkpoint.atom_position  // Fallback if no end path
    };
    
    // Rest of function...
}
```

### Step 4: Handle Position Updates
**File:** `crates/context-stack/context-search/src/compare/state.rs`

When `mark_match()` updates checkpoint, we might need to track:
- Whether we're matching in end path vs start path
- Update position annotations when popping/replacing path segments

### Step 5: Update Tests
**File:** `crates/context-stack/context-search/src/tests/search/mod.rs`

Run `find_pattern1` and verify:
- Cache entries all at position 2 (as expected)
- Both bottom-up and top-down give correct positions

## Validation Steps

1. **Run find_pattern1 test**
   ```bash
   LOG_STDOUT=1 LOG_FILTER=trace cargo test -p context-search find_pattern1 -- --nocapture
   ```
   - Verify all cache entries at position 2
   - Check trace logs show correct position tracking

2. **Run all context-search tests**
   ```bash
   cargo test -p context-search
   ```
   - Ensure all 9 failing tests now pass
   - No regressions in passing tests

3. **Test with deeper hierarchies**
   - Create test with query [a,b,y,x,w] as per user's example
   - Verify positions tracked correctly at each level
   - Check cache entries use right positions

4. **Verify both QueryExhausted and Mismatch cases**
   - Mismatch: partial match then mismatch
   - QueryExhausted: query ends mid-traversal
   - Both should use stored entry positions

## Risks and Mitigations

### Risk 1: Position Vector Out of Sync
**Mitigation:** 
- Add debug assertions checking `end_path_positions.len()` matches end path length
- Clear positions when clearing/resetting paths

### Risk 2: Performance Impact
**Mitigation:**
- Only track positions for end path (not start path)
- Vec operations are cheap (push/pop)
- Consider making positions Optional if not always needed

### Risk 3: Complex State Management
**Mitigation:**
- Document when positions are recorded/used
- Add tracing logs for position tracking
- Keep position update logic close to path update logic

### Risk 4: Missing Edge Cases
**Mitigation:**
- Test with various graph structures (flat, nested, deep)
- Test with different query lengths
- Test both complete and partial matches

## Open Questions

1. **When exactly to record positions?**
   - Before or after entering child?
   - Answer: BEFORE matching in the child, so before `mark_match()` is called

2. **What about position when popping path segments?**
   - Do we need to track position history for each level?
   - Answer: User's example shows replacing at same level, keeping positions aligned

3. **How to handle start path positions?**
   - User says start paths can trace immediately - do we need positions?
   - Answer: Probably not, since we're tracing as we go

4. **Should positions be Option<Vec> or always exist?**
   - Trade-off between memory and simplicity
   - Answer: Start with always present, optimize later if needed

## Next Steps After Completion

1. Update `CHEAT_SHEET.md` with position tracking patterns
2. Update `agents/guides/` with cache tracing guide
3. Archive this plan in `agents/implemented/` with summary
4. Update `agents/bug-reports/INDEX.md` to mark find_pattern1 issue resolved

## References

- **Test:** `crates/context-stack/context-search/src/tests/search/mod.rs` - find_pattern1 (lines 108-177)
- **ChildState:** `crates/context-stack/context-trace/src/trace/child/state.rs` (lines 75-105)
- **RootCursor:** `crates/context-stack/context-search/src/match/root_cursor.rs` (lines 567-625)
- **CompareState:** `crates/context-stack/context-search/src/compare/state.rs` (lines 140-165)
- **User's explanation:** Conversation about entering yz at checkpoint 2, matching y → checkpoint 3
