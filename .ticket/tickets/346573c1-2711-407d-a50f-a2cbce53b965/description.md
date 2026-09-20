---
tags: `#context-search` `#context-insert` `#TraceCache` `#InitInterval` `#panic` `#critical`
summary: Search Response returns root_token that isn't present in its TraceCache, causing insert panics
---

# TraceCache Missing Root Token Entry Bug

## Symptoms

Panic in `context-insert` with:
```
thread 'tests::ngrams_validation::validate_triple_repeat' panicked at 
crates/context-stack/context-insert/src/interval/partition/info/range/splits.rs:63:14:
called `Option::unwrap()` on a `None` value
```

This occurs when:
- context-read searches for a known pattern (e.g., `[T0, T1, T0, T1]` for "abab")
- The search finds that this pattern exists inside a larger token (e.g., T2 = "ab")
- The returned `Response` has:
  - `root_token()` = T2 (index 2)
  - `cache.entries` = {0} (only vertex 0)
- context-insert's `insert_init` tries to use this `InitInterval`
- The split algorithm expects the cache to contain the root token's entry
- `.unwrap()` panics on empty iterator

## Reproduction

```rust
#[test]
fn validate_triple_repeat() {
    // Input: "ababab" - pattern "ab" repeats 3 times
    assert_graphs_equivalent("ababab");
}
```

Trace output:
```
new_atom_indices = [New(0), New(1), Known(0), Known(1), Known(0), Known(1)]
read_block: unknown = [T0, T1], known = [T0, T1, T0, T1]
insert_init: root=T2 (index=2), end_bound=AtomPosition(2)
insert_init: cache entries: [0]   <-- ROOT T2 NOT IN CACHE!
```

## Root Cause

The bug is in how `context-search` builds the `Response` and how `InitInterval` is constructed from it:

1. **Search traverses leaf tokens**: During search, the cache records vertices that are *traversed* (visited during the search). For pattern `[T0, T1, T0, T1]`, only T0 might be visited.

2. **Root is inferred, not traversed**: The `root_token()` method returns the *parent* that contains the matched path. This parent (T2) was never explicitly traversed - it was inferred from the graph structure.

3. **Cache doesn't include inferred root**: The TraceCache only contains entries for vertices that were explicitly visited with `add_state()`. The inferred parent T2 is not added.

4. **Insert expects root in cache**: `IntervalGraph::from(InitInterval)` creates `SplitTraceStatesCtx` which iterates over cache entries to build split positions. When it tries to process the root T2, the cache lookup fails.

## Location

**Bug origin** (where the mismatch is created):
- File: `context-insert/src/interval/init.rs`
- Function: `impl From<Response> for InitInterval`
- Lines 18-26
```rust
impl From<Response> for InitInterval {
    fn from(state: Response) -> Self {
        let root = state.root_token();      // Returns T2
        let end_bound = state.checkpoint_position();
        Self {
            cache: state.cache,              // Contains only {0}, not T2!
            root,
            end_bound,
        }
    }
}
```

**Panic location** (where it manifests):
- File: `context-insert/src/interval/partition/info/range/splits.rs`
- Line: 63
- Code: `.nth(self.start).unwrap()` on empty iterator

## Fix

### Option A: Ensure cache contains root before returning Response

In `context-search`, when building the final `Response`, ensure the `root_token` is added to the cache:

```rust
// In context-search where Response is built
impl Response {
    pub fn new(/* ... */, cache: TraceCache) -> Self {
        let root = /* compute root_token */;
        // Ensure root is in cache
        if !cache.entries.contains_key(&root.vertex_index()) {
            cache.insert(root.vertex_index(), VertexCache::start(root));
        }
        // ... rest of construction
    }
}
```

**Pros**: Fixes at the source, ensures cache invariant
**Cons**: Need to understand how to properly construct VertexCache entry for an inferred root

### Option B: Validate in InitInterval::from and return error

```rust
impl TryFrom<Response> for InitInterval {
    type Error = ErrorReason;
    
    fn try_from(state: Response) -> Result<Self, Self::Error> {
        let root = state.root_token();
        if !state.cache.entries.contains_key(&root.vertex_index()) {
            return Err(ErrorReason::CacheMissingRoot);
        }
        // ... rest
    }
}
```

**Pros**: Explicit error instead of panic
**Cons**: Doesn't fix the underlying issue, just makes it a graceful error

### Option C: Fix in context-read to avoid producing this state

In `ExpansionCtx::new`, when `insert_or_get_complete` returns a result that leads to cache/root mismatch, handle it differently:

```rust
// Check if result is valid before using
if let Ok(result) = trav.insert_or_get_complete(cursor.clone()) {
    // Validate cache contains the root
    // If not, fall back to alternative path
}
```

**Pros**: Prevents the invalid state from propagating
**Cons**: May hide deeper issues in search algorithm

## Verification

After fix, these tests should pass:
- [ ] `validate_triple_repeat` ("ababab")
- [ ] `validate_simple_repeat` ("abab")
- [ ] `validate_complex_short` ("abcabc")
- [ ] And other ngrams validation tests with repeating patterns

## Related Issues

- This explains 8+ failing context-read tests
- Related to empty pattern issue (already fixed separately)
- May affect any pattern with repeated subsequences
