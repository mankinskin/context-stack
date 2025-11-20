# EndState Refactoring Plan

## Problem

The current `EndState` type uses an `EndReason` enum to distinguish between different terminal states:
- `EndReason::QueryEnd` - Query pattern fully matched
- `EndReason::Mismatch` - Comparison failed

This loses type safety because:
1. Both cases return the same type, requiring runtime checks
2. Can't distinguish "no match" from "partial match" at the type level
3. Easy to accidentally treat non-matches as matches

## Proposed Solution

Replace `EndState` with distinct types that encode the match state:

### New Type Hierarchy

```rust
/// A matched state - query matched at least partially in this root
pub enum MatchedEndState {
    /// Query pattern fully exhausted - complete match
    Complete(CompleteMatchState),
    /// Query continues but root exhausted - partial match
    Partial(PartialMatchState),
}

/// Query pattern fully matched in this root
pub struct CompleteMatchState {
    pub path: PathEnum,
    pub cursor: PatternCursor,  // checkpoint at query end
}

/// Query partially matched (some tokens matched before mismatch/end)
pub struct PartialMatchState {
    pub path: PathEnum,
    pub cursor: PatternCursor,  // checkpoint shows how far we got
}

/// No match in this candidate root (immediate mismatch at position 0)
pub struct NoMatchState {
    pub path: PathEnum,
    pub cursor: PatternCursor,  // checkpoint at position 0
}
```

### Type-Safe Returns

```rust
// RootCursor::find_end returns one of:
Result<MatchedEndState, RootCursor<Candidate, Matched>>

// SearchIterator returns:
Option<MatchedEndState>  // Only returns actual matches

// SearchState accumulates:
enum LastMatch {
    Query(PatternRangePath),
    Located(MatchedEndState),  // Only valid matches
}
```

### Benefits

1. **Type Safety**: Can't accidentally use a non-match as a match
2. **Clear Intent**: `MatchedEndState` explicitly means "this matched"
3. **No Runtime Checks**: Type system enforces proper handling
4. **Better API**: `match matched_state { Complete(_) => ..., Partial(_) => ... }`

## Migration Strategy

### Phase 1: Create New Types
- [ ] Define `MatchedEndState`, `CompleteMatchState`, `PartialMatchState` in `context-search/src/state/matched/`
- [ ] Keep old `EndState` for compatibility

### Phase 2: Update RootCursor
- [ ] Change `RootCursor::find_end()` to return `Result<MatchedEndState, ...>`
- [ ] Update internal logic to create appropriate matched state types
- [ ] Remove `NoMatchState` returns (convert to `None` or continue iteration)

### Phase 3: Update SearchIterator
- [ ] Change `SearchIterator::next()` to return `Option<MatchedEndState>`
- [ ] Filter out non-matches internally

### Phase 4: Update SearchState
- [ ] Change `SearchState::next()` to work with `MatchedEndState`
- [ ] Update `LastMatch` enum to use `MatchedEndState`
- [ ] Remove checkpoint position checks (handled by types)

### Phase 5: Update Response
- [ ] Change `Response::end` to `MatchedEndState`
- [ ] Update `is_complete()` to match on `Complete` variant

### Phase 6: Update Tests
- [ ] Update all test assertions to use new types
- [ ] Remove runtime EndReason checks

### Phase 7: Cleanup
- [ ] Remove old `EndState` and `EndReason` types
- [ ] Update documentation

## Files to Update

### Core Types (Phase 1-2)
- `context-search/src/state/matched/` (new module)
- `context-search/src/state/end/mod.rs` (deprecate)
- `context-search/src/match/root_cursor.rs`

### Search Logic (Phase 3-4)
- `context-search/src/match/iterator.rs`
- `context-search/src/search/mod.rs`
- `context-search/src/state/start.rs`

### API Surface (Phase 5)
- `context-search/src/state/result.rs`
- `context-search/src/search/final_state.rs`

### Tests (Phase 6)
- `context-search/src/tests/search/*.rs`
- `context-search/src/tests/traversal.rs`

## Example Usage After Refactoring

```rust
// Before (runtime check required)
match end.reason {
    EndReason::QueryEnd => { /* handle complete */ },
    EndReason::Mismatch => {
        if end.cursor.atom_position > 0 {
            /* handle partial */
        } else {
            /* handle no match */
        }
    }
}

// After (type-safe)
match matched_end {
    MatchedEndState::Complete(state) => { /* handle complete */ },
    MatchedEndState::Partial(state) => { /* handle partial */ },
}
// NoMatchState never reaches this code - filtered out by iterator
```

## Testing Strategy

1. Add new types alongside old ones
2. Gradually migrate code file-by-file
3. Keep tests passing at each step
4. Remove old types only when fully migrated

## Open Questions

1. Should `PathEnum` be split into separate types for Complete/Partial cases?
2. Do we need to preserve `NoMatchState` for error reporting?
3. Should `MatchedEndState` include match quality metrics (width, position)?
